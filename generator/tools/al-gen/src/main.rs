//! AL Tree-Sitter Generator - Dynamic from Cursor/VSCode AL extension
//!
//! Core constraint:
//! - **No hardcoded AL keyword strings** anywhere in Rust or in grammar templates.
//!   We extract keywords from `syntaxes/alsyntax.tmlanguage` and generate:
//!   - `src/keywords.c`  (keyword lookup tables)
//!   - `src/scanner.c`   (external scanner; includes keywords.c)
//!   - `grammar.js`      (tree-sitter grammar; uses keyword categories, not strings)
//!   - `src/parser.c`    (via `tree-sitter generate`)
//!   - Optional repo test run (via `tree-sitter parse`)

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO_TEST_CONFIG: &str = "tests/test_repos.toml";
const REPO_TEST_WORKDIR: &str = "tests/.repos";
const FIXTURES_INVALID_DIR: &str = "tests/fixtures/invalid";
const FIXTURES_VALID_DIR: &str = "tests/fixtures/valid";

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  AL Tree-Sitter Generator - Dynamic from Extension          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("🔍 Finding AL extension...");
    let extension_path = find_al_extension()?;
    println!("✅ Found: {}", extension_path.display());

    let syntax_file = find_syntax_file(&extension_path)?;
    println!("📄 Using syntax: {}\n", syntax_file.display());

    // We are running from 'generator/' dir (cargo runs from manifest dir), output to repo root
    let root_offset = "../";

    println!("📖 Extracting keywords from TextMate grammar...");
    let keywords = extract_keywords(&syntax_file)?;
    let scope_name = extract_scope_name(&syntax_file).unwrap_or_else(|_| "source.al".to_string());
    print_keyword_stats(&keywords);

    println!("\n🔧 Generating C scanner files...");
    let external_tokens = build_external_tokens(&keywords);
    
    let src_dir = format!("{}src", root_offset);
    fs::create_dir_all(&src_dir)?;
    
    generate_keywords_c(&keywords, &src_dir)?;
    generate_scanner_c(&keywords, &external_tokens, &src_dir)?;
    println!("✅ Generated src/keywords.c and src/scanner.c");

    println!("\n📝 Generating grammar.js...");
    generate_grammar_js(&keywords, &external_tokens, root_offset)?;
    println!("✅ Generated grammar.js");

    println!("🎨 Generating highlight queries...");
    
    // Create queries dir in root
    let queries_dir = format!("{}queries", root_offset);
    fs::create_dir_all(&queries_dir)?;
    
    let highlights_path = format!("{}/highlights.scm", queries_dir);
    
    generate_highlights(&keywords, &highlights_path)?;
    println!("✅ Generated {}", highlights_path);

    println!("\n🌳 Running tree-sitter generate...");
    run_tree_sitter_generate(root_offset)?;

    // Build a parser library once; used for fast `tree-sitter parse` runs.
    // Build a parser library once; used for fast `tree-sitter parse` runs.
    println!("\n🔨 Building parser library...");
    let lib_path = run_tree_sitter_build(root_offset)?;
    println!("✅ Built: {}", lib_path.display());

    // Quick always-on fixture validation (guards against false positives/over-tolerance).
    run_fixture_tests(&lib_path, &scope_name, root_offset)?;

    // Optional: real-world validation against Microsoft repos.
    // Opt-in only: `cargo run --release -- --test`
    let args: Vec<String> = std::env::args().collect();
    let do_tests = args.iter().any(|a| a == "--test" || a == "--tests");

    if do_tests {
        let repo_test_config_path = Path::new(root_offset).join(REPO_TEST_CONFIG);
        if repo_test_config_path.exists() {
            println!("\n🧪 Testing against real repositories...");
            run_repo_tests(&lib_path, &scope_name, root_offset)?;
        } else {
            println!("\nℹ️  {} not found; skipping repo tests.", repo_test_config_path.display());
        }
    } else {
        println!("\nℹ️  Skipping repo tests. Run with `--test` to enable.");
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  ✅ DONE!                                                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

#[derive(Debug, Default)]
struct Keywords {
    control: BTreeSet<String>,
    operator_words: BTreeSet<String>,
    objects: BTreeSet<String>,
    types: BTreeSet<String>,
    metadata: BTreeSet<String>,
    properties: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct ExternalTokenSpec {
    grammar: String,
    c_enum: String,
}

fn token_spec(grammar: impl Into<String>, c_enum: impl Into<String>) -> ExternalTokenSpec {
    ExternalTokenSpec {
        grammar: grammar.into(),
        c_enum: c_enum.into(),
    }
}

fn kw_token_spec(prefix: &str, kw: &str, c_prefix: &str) -> ExternalTokenSpec {
    // Keywords extracted from the TextMate grammar are identifier-like (ASCII, _, digits).
    // Keep names stable and predictable for use in grammar.js rules.
    token_spec(
        format!("{prefix}_{kw}"),
        format!("{c_prefix}_{}", kw.to_ascii_uppercase()),
    )
}

// IMPORTANT: order must match between:
// - grammar.js `externals: $ => [...]`
// - src/scanner.c TokenType enum indices
fn build_external_tokens(keywords: &Keywords) -> Vec<ExternalTokenSpec> {
    let mut out = Vec::new();

    // Control keywords as distinct tokens come FIRST (highest priority)
    for kw in &keywords.control {
        out.push(kw_token_spec("kw", kw, "KW"));
    }

    // Operator words as distinct tokens
    for kw in &keywords.operator_words {
        out.push(kw_token_spec("op", kw, "OP"));
    }

    // Category tokens (small, stable set) fallback
    out.push(token_spec("keyword", "KEYWORD"));
    out.push(token_spec("control_keyword", "CONTROL_KEYWORD"));
    out.push(token_spec("operator_word", "OPERATOR_WORD"));
    out.push(token_spec("object_keyword", "OBJECT_KEYWORD"));
    out.push(token_spec("type_keyword", "TYPE_KEYWORD"));
    out.push(token_spec("metadata_keyword", "METADATA_KEYWORD"));
    out.push(token_spec("property_keyword", "PROPERTY_KEYWORD"));

    // Preprocessor / directives
    out.push(token_spec("directive", "DIRECTIVE"));
    // Inactive preprocessor regions (extra token that consumes until next directive boundary).
    out.push(token_spec("inactive_code", "INACTIVE_CODE"));

    out
}

fn render_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        let needle = format!("{{{{{}}}}}", key);
        let needle_spaced = format!("{{{{ {} }}}}", key);
        out = out.replace(&needle, value);
        out = out.replace(&needle_spaced, value);
    }
    out
}

fn gen_scanner_token_enum_fragment(tokens: &[ExternalTokenSpec]) -> String {
    let mut out = String::new();
    for t in tokens {
        out.push_str("  ");
        out.push_str(&t.c_enum);
        out.push_str(",\n");
    }
    out
}

fn gen_grammar_externals_fragment(tokens: &[ExternalTokenSpec]) -> String {
    let mut out = String::new();
    for t in tokens {
        out.push_str("    $.");
        out.push_str(&t.grammar);
        out.push_str(",\n");
    }
    out
}

fn gen_scanner_valid_symbols_fastpath_fragment(tokens: &[ExternalTokenSpec]) -> String {
    let mut out = String::new();
    out.push_str("  if (");
    for (i, t) in tokens.iter().enumerate() {
        if i > 0 {
            out.push_str(" &&\n      ");
        }
        out.push_str("!valid_symbols[");
        out.push_str(&t.c_enum);
        out.push(']');
    }
    out.push_str(") {\n    return false;\n  }\n");
    out
}

fn gen_scanner_directive_handling_fragment() -> String {
    // Preprocessor handling lives in templates/scanner.c.template; this just wires it into scan().
    r#"  if (scanner_scan_directive(scanner, lexer, valid_symbols)) {
    return true;
  }
  if (scanner_scan_inactive_code(scanner, lexer, valid_symbols)) {
    return true;
  }
"#
    .to_string()
}

fn gen_scanner_keyword_dispatch_fragment() -> String {
    // Dispatch order is important for overlap cases; keep most-specific first.
    // These are *categories* backed by generated keyword tables (src/keywords.c).
    r#"  // Specific control-keyword tokens (kw_*) come first when the grammar asks for them.
  TokenType specific;
  if (al_lookup_control_kw_token(word, &specific) && valid_symbols[specific]) {
    lexer->result_symbol = specific;
    return true;
  }

  // Specific operator-word tokens (op_*) come next when the grammar asks for them.
  if (al_lookup_operator_word_token(word, &specific) && valid_symbols[specific]) {
    lexer->result_symbol = specific;
    return true;
  }

  if (valid_symbols[CONTROL_KEYWORD] && is_al_control_keyword(word)) {
    lexer->result_symbol = CONTROL_KEYWORD;
    return true;
  }
  if (valid_symbols[OPERATOR_WORD] && is_al_operator_word_keyword(word)) {
    lexer->result_symbol = OPERATOR_WORD;
    return true;
  }
  if (valid_symbols[OBJECT_KEYWORD] && is_al_object_keyword(word)) {
    lexer->result_symbol = OBJECT_KEYWORD;
    return true;
  }
  if (valid_symbols[TYPE_KEYWORD] && is_al_type_keyword(word)) {
    lexer->result_symbol = TYPE_KEYWORD;
    return true;
  }
  if (valid_symbols[METADATA_KEYWORD] && is_al_metadata_keyword(word)) {
    lexer->result_symbol = METADATA_KEYWORD;
    return true;
  }
  if (valid_symbols[PROPERTY_KEYWORD] && is_al_property_keyword(word)) {
    lexer->result_symbol = PROPERTY_KEYWORD;
    return true;
  }
  if (valid_symbols[KEYWORD] && is_al_keyword(word)) {
    lexer->result_symbol = KEYWORD;
    return true;
  }
"#
    .to_string()
}

fn find_al_extension() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("Cannot find home directory")?;

    let search_paths = vec![
        PathBuf::from(&home).join(".cursor/extensions"),
        PathBuf::from(&home).join(".vscode/extensions"),
    ];

    for search_path in search_paths {
        if !search_path.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&search_path) {
            let mut al_extensions: Vec<_> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("ms-dynamics-smb.al-"))
                        .unwrap_or(false)
                })
                .collect();

            al_extensions
                .sort_by(|a, b| al_extension_version_key(a).cmp(&al_extension_version_key(b)));
            if let Some(latest) = al_extensions.last() {
                return Ok(latest.clone());
            }
        }
    }

    anyhow::bail!("AL extension not found")
}

fn al_extension_version_key(path: &Path) -> Vec<u64> {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let prefix = "ms-dynamics-smb.al-";
    let ver = name.strip_prefix(prefix).unwrap_or("");
    // Parse dotted numeric versions; non-numeric components are ignored.
    ver.split('.')
        .filter_map(|p| p.parse::<u64>().ok())
        .collect()
}

fn find_syntax_file(extension_path: &Path) -> Result<PathBuf> {
    let direct = extension_path.join("syntaxes/alsyntax.tmlanguage");
    if direct.exists() {
        return Ok(direct);
    }

    let syntaxes_dir = extension_path.join("syntaxes");
    if !syntaxes_dir.is_dir() {
        anyhow::bail!(
            "No syntaxes directory in extension: {}",
            extension_path.display()
        );
    }

    let mut candidates = Vec::new();
    for entry in fs::read_dir(&syntaxes_dir).context("Failed to read syntaxes directory")? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("tmlanguage") {
            candidates.push(path);
        }
    }

    candidates.sort();
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No .tmlanguage files found in {}", syntaxes_dir.display()))
}

fn extract_keywords(syntax_file: &Path) -> Result<Keywords> {
    let xml = read_encoding_aware(syntax_file)?;
    let mut out = Keywords::default();

    // Find all keyword lists like (?i:(k1|k2|...))
    // Note: many keywords in AL TM grammar are just VAR or BEGIN without the group, but
    // usually they are in a choice group for performance.
    let kw_list_re = Regex::new(r#"(?i)\(\?i:\((.*?)\)\)"#)?;
    let name_re = Regex::new(r#"(?s)<key>name</key>\s*<string>([^<]+)</string>"#)?;

    // We also search for individual matches for safety
    let single_kw_re = Regex::new(r#"(?i)<key>match</key>\s*<string>\\b\(\?i:([a-zA-Z0-9_]+)\)\\b</string>"#)?;

    for mat in kw_list_re.find_iter(&xml) {
        let kw_list_str = &xml[mat.start()..mat.end()];
        
        // Find the nearest scope name by searching backwards and forwards around this match
        let start_search = mat.start().saturating_sub(1000);
        let end_search = (mat.end() + 1000).min(xml.len());
        let search_window = &xml[start_search..end_search];
        
        let mut scope_name = "keyword.control"; // fallback
        // Find nearest name in window (closest to the match)
        let mut best_dist = usize::MAX;
        for name_caps in name_re.captures_iter(search_window) {
            let name_mat = name_caps.get(0).unwrap();
            let dist = if name_mat.start() + start_search < mat.start() {
                mat.start() - (name_mat.end() + start_search)
            } else {
                (name_mat.start() + start_search) - mat.end()
            };
            if dist < best_dist {
                best_dist = dist;
                scope_name = name_caps.get(1).unwrap().as_str();
            }
        }

        if let Some(kw_caps) = kw_list_re.captures(kw_list_str) {
            for raw in kw_caps[1].split('|') {
                let kw = raw.trim().to_lowercase();
                if !is_identifier_like(&kw) {
                    continue;
                }

                if scope_name.contains("keyword.control") {
                    out.control.insert(kw);
                } else if scope_name.contains("keyword.operators.al") {
                    out.operator_words.insert(kw);
                } else if scope_name.contains("applicationobject") {
                    out.objects.insert(kw.clone());
                    out.control.insert(kw);
                } else if scope_name.contains("builtintypes") {
                    out.types.insert(kw.clone());
                    out.control.insert(kw);
                } else if scope_name.contains("metadata") {
                    out.metadata.insert(kw);
                } else if scope_name.contains("property") || 
                           scope_name.contains("variable.other") || 
                           scope_name.contains("support.variable") {
                    out.properties.insert(kw);
                }
            }
        }
    }

    // Process single keyword matches too
    for caps in single_kw_re.captures_iter(&xml) {
        let kw = caps[1].trim().to_lowercase();
        if is_identifier_like(&kw) {
             out.control.insert(kw);
        }
    }

    Ok(out)
}

fn read_encoding_aware(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16 LE
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&utf16).context("Failed to decode UTF-16LE")
    } else if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        // UTF-16 BE
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&utf16).context("Failed to decode UTF-16BE")
    } else {
        // Fallback to UTF-8
        String::from_utf8(bytes).context("Failed to decode UTF-8")
    }
}

fn extract_scope_name(syntax_file: &Path) -> Result<String> {
    let xml = read_encoding_aware(syntax_file)?;
    let re = Regex::new(r#"<key>scopeName</key>\s*<string>([^<]+)</string>"#)?;
    let caps = re
        .captures(&xml)
        .ok_or_else(|| anyhow!("scopeName not found in {}", syntax_file.display()))?;
    Ok(caps[1].trim().to_string())
}

fn is_identifier_like(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn print_keyword_stats(keywords: &Keywords) {
    let total = keywords_all(keywords).len();
    println!("   Control:   {}", keywords.control.len());
    println!("   Operators: {}", keywords.operator_words.len());
    println!("   Objects:   {}", keywords.objects.len());
    println!("   Types:     {}", keywords.types.len());
    println!("   Metadata:  {}", keywords.metadata.len());
    println!("   Property:  {}", keywords.properties.len());
    println!("   Total:     {}", total);
}

fn keywords_all(k: &Keywords) -> BTreeSet<String> {
    let mut all = BTreeSet::new();
    all.extend(k.control.iter().cloned());
    all.extend(k.operator_words.iter().cloned());
    all.extend(k.objects.iter().cloned());
    all.extend(k.types.iter().cloned());
    all.extend(k.metadata.iter().cloned());
    all.extend(k.properties.iter().cloned());
    all
}

fn generate_keywords_c(keywords: &Keywords, out_dir: &str) -> Result<()> {

    let all = keywords_all(keywords);

    let mut out = String::new();
    out.push_str("// Keyword lookup tables for AL external scanner\n");
    out.push_str("// AUTO-GENERATED - DO NOT EDIT\n");
    out.push_str("// Generated by: cargo run\n\n");
    out.push_str("#include <stdbool.h>\n");
    out.push_str("#include <stddef.h>\n");
    out.push_str("#include <string.h>\n\n");

    out.push_str(
        "static bool al_kw_binsearch(const char *word, const char *const *arr, size_t count) {\n",
    );
    out.push_str("  size_t lo = 0;\n");
    out.push_str("  size_t hi = count;\n");
    out.push_str("  while (lo < hi) {\n");
    out.push_str("    size_t mid = lo + (hi - lo) / 2;\n");
    out.push_str("    int cmp = strcmp(word, arr[mid]);\n");
    out.push_str("    if (cmp == 0) return true;\n");
    out.push_str("    if (cmp < 0) hi = mid; else lo = mid + 1;\n");
    out.push_str("  }\n");
    out.push_str("  return false;\n");
    out.push_str("}\n\n");

    write_kw_array(&mut out, "AL_KEYWORDS_ALL", &all);
    write_kw_array(&mut out, "AL_KEYWORDS_CONTROL", &keywords.control);
    write_kw_array(
        &mut out,
        "AL_KEYWORDS_OPERATOR_WORDS",
        &keywords.operator_words,
    );
    write_kw_array(&mut out, "AL_KEYWORDS_OBJECTS", &keywords.objects);
    write_kw_array(&mut out, "AL_KEYWORDS_TYPES", &keywords.types);
    write_kw_array(&mut out, "AL_KEYWORDS_METADATA", &keywords.metadata);
    write_kw_array(&mut out, "AL_KEYWORDS_PROPERTIES", &keywords.properties);

    out.push_str("static bool is_al_keyword(const char *word) {\n");
    out.push_str("  return al_kw_binsearch(word, AL_KEYWORDS_ALL, sizeof(AL_KEYWORDS_ALL) / sizeof(AL_KEYWORDS_ALL[0]));\n");
    out.push_str("}\n\n");

    out.push_str("static bool is_al_control_keyword(const char *word) {\n");
    out.push_str("  return al_kw_binsearch(word, AL_KEYWORDS_CONTROL, sizeof(AL_KEYWORDS_CONTROL) / sizeof(AL_KEYWORDS_CONTROL[0]));\n");
    out.push_str("}\n\n");

    out.push_str("static bool is_al_object_keyword(const char *word) {\n");
    out.push_str("  return al_kw_binsearch(word, AL_KEYWORDS_OBJECTS, sizeof(AL_KEYWORDS_OBJECTS) / sizeof(AL_KEYWORDS_OBJECTS[0]));\n");
    out.push_str("}\n\n");

    out.push_str("static bool is_al_type_keyword(const char *word) {\n");
    out.push_str("  return al_kw_binsearch(word, AL_KEYWORDS_TYPES, sizeof(AL_KEYWORDS_TYPES) / sizeof(AL_KEYWORDS_TYPES[0]));\n");
    out.push_str("}\n\n");

    out.push_str("static bool is_al_metadata_keyword(const char *word) {\n");
    out.push_str("  return al_kw_binsearch(word, AL_KEYWORDS_METADATA, sizeof(AL_KEYWORDS_METADATA) / sizeof(AL_KEYWORDS_METADATA[0]));\n");
    out.push_str("}\n\n");

    out.push_str("static bool is_al_property_keyword(const char *word) {\n");
    out.push_str("  return al_kw_binsearch(word, AL_KEYWORDS_PROPERTIES, sizeof(AL_KEYWORDS_PROPERTIES) / sizeof(AL_KEYWORDS_PROPERTIES[0]));\n");
    out.push_str("}\n");

    // Keep operator-words available if you want to add a distinct token later.
    out.push_str("\nstatic bool is_al_operator_word_keyword(const char *word) {\n");
    out.push_str("  return al_kw_binsearch(word, AL_KEYWORDS_OPERATOR_WORDS, sizeof(AL_KEYWORDS_OPERATOR_WORDS) / sizeof(AL_KEYWORDS_OPERATOR_WORDS[0]));\n");
    out.push_str("}\n");

    // Control keyword -> specific token lookup (kw_* externals).
    // NOTE: `TokenType` is defined in src/scanner.c before including this file.
    out.push_str("\n\ntypedef struct { const char *word; TokenType tok; } AlTokenMapEntry;\n");
    out.push_str("static bool al_kw_token_binsearch(const char *word, const AlTokenMapEntry *arr, size_t count, TokenType *out_tok) {\n");
    out.push_str("  size_t lo = 0;\n");
    out.push_str("  size_t hi = count;\n");
    out.push_str("  while (lo < hi) {\n");
    out.push_str("    size_t mid = lo + (hi - lo) / 2;\n");
    out.push_str("    int cmp = strcmp(word, arr[mid].word);\n");
    out.push_str("    if (cmp == 0) { *out_tok = arr[mid].tok; return true; }\n");
    out.push_str("    if (cmp < 0) hi = mid; else lo = mid + 1;\n");
    out.push_str("  }\n");
    out.push_str("  return false;\n");
    out.push_str("}\n\n");

    out.push_str("static const AlTokenMapEntry AL_CONTROL_KW_TOKENS[] = {\n");
    for kw in &keywords.control {
        out.push_str(&format!(
            "  {{\"{}\", KW_{}}},\n",
            c_escape(kw),
            kw.to_ascii_uppercase()
        ));
    }
    out.push_str("};\n\n");

    out.push_str(
        "static bool al_lookup_control_kw_token(const char *word, TokenType *out_tok) {\n",
    );
    out.push_str("  return al_kw_token_binsearch(word, AL_CONTROL_KW_TOKENS, sizeof(AL_CONTROL_KW_TOKENS) / sizeof(AL_CONTROL_KW_TOKENS[0]), out_tok);\n");
    out.push_str("}\n");

    // Operator word -> specific token lookup (op_* externals).
    out.push_str("\nstatic const AlTokenMapEntry AL_OPERATOR_WORD_TOKENS[] = {\n");
    for kw in &keywords.operator_words {
        out.push_str(&format!(
            "  {{\"{}\", OP_{}}},\n",
            c_escape(kw),
            kw.to_ascii_uppercase()
        ));
    }
    out.push_str("};\n\n");

    out.push_str(
        "static bool al_lookup_operator_word_token(const char *word, TokenType *out_tok) {\n",
    );
    out.push_str("  return al_kw_token_binsearch(word, AL_OPERATOR_WORD_TOKENS, sizeof(AL_OPERATOR_WORD_TOKENS) / sizeof(AL_OPERATOR_WORD_TOKENS[0]), out_tok);\n");
    out.push_str("}\n");

    let path = format!("{}/keywords.c", out_dir);
    fs::write(path, out)?;
    Ok(())
}

fn write_kw_array(out: &mut String, name: &str, words: &BTreeSet<String>) {
    out.push_str(&format!("static const char *const {}[] = {{\n", name));
    for w in words {
        out.push_str(&format!("  \"{}\",\n", c_escape(w)));
    }
    out.push_str("};\n\n");
}

fn c_escape(s: &str) -> String {
    // Keywords are identifier-like, but we still escape defensively.
    s.replace('\\', "\\\\").replace('\"', "\\\"")
}

fn generate_scanner_c(_keywords: &Keywords, external_tokens: &[ExternalTokenSpec], out_dir: &str) -> Result<()> {
    // out_dir is assumed to be "src" equivalent relative path
    // Templates are in the same directory as this source file
    let template_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/al-gen/templates");
    let template = fs::read_to_string(format!("{}/scanner.c.template", template_dir))?;

    let token_enum = gen_scanner_token_enum_fragment(external_tokens);
    let fastpath = gen_scanner_valid_symbols_fastpath_fragment(external_tokens);
    let directive_handling = gen_scanner_directive_handling_fragment();
    let keyword_dispatch = gen_scanner_keyword_dispatch_fragment();

    let rendered = render_template(
        &template,
        &[
            ("TOKEN_ENUM", &token_enum),
            ("VALID_SYMBOLS_FASTPATH", &fastpath),
            ("DIRECTIVE_HANDLING", &directive_handling),
            ("KEYWORD_DISPATCH", &keyword_dispatch),
        ],
    );

    let path = format!("{}/scanner.c", out_dir);
    fs::write(path, rendered)?;
    Ok(())
}

fn generate_grammar_js(keywords: &Keywords, external_tokens: &[ExternalTokenSpec], root: &str) -> Result<()> {
    let template_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/al-gen/templates");
    let template = fs::read_to_string(format!("{}/grammar.js.template", template_dir))?;
    let externals = gen_grammar_externals_fragment(external_tokens);
    let objects = gen_choice_fragment(&keywords.objects, &[]);
    let types_excluding_option = gen_choice_fragment(&keywords.types, &["option"]);
    
    // Build initial placeholder map
    let mut placeholders = vec![
        ("EXTERNALS_LIST".to_string(), externals),
        ("OBJECT_DECLARE_KIND".to_string(), objects),
        ("TYPE_REFERENCE_KIND".to_string(), types_excluding_option),
    ];

    // Build dynamic {{KW:name}} placeholders for every keyword found in the template
    let kw_placeholder_re = Regex::new(r"\{\{KW:([a-zA-Z_]+)\}\}")?;
    for caps in kw_placeholder_re.captures_iter(&template) {
        let kw_name = &caps[1].to_lowercase();
        let placeholder_key = format!("KW:{}", &caps[1]); // e.g. "KW:if"
        
        // Find the actual token name
        let token = if keywords.control.contains(kw_name) || 
                       keywords.operator_words.contains(kw_name) ||
                       keywords.objects.contains(kw_name) ||
                       keywords.types.contains(kw_name) ||
                       keywords.metadata.contains(kw_name) ||
                       keywords.properties.contains(kw_name) {
            format!("$.kw_{}", kw_name)
        } else {
            format!("$._kw_{}_missing", kw_name)
        };
        
        placeholders.push((placeholder_key, token));
    }

    // Convert to the required format for render_template
    let render_pairs: Vec<(&str, &str)> = placeholders.iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let rendered = render_template(&template, &render_pairs);
    let out_path = format!("{}grammar.js", root);
    
    let header = "// -------------------------------------------------------------------------\n\
                  // AUTO-GENERATED FILE - DO NOT EDIT MANUALLY\n\
                  // -------------------------------------------------------------------------\n\
                  // This file was automatically generated by `tree-sitter-al-gen`.\n\
                  // It extracts keywords dynamically from the official AL TextMate grammar.\n\
                  //\n\
                  // Any strict keyword tokens (e.g., $.kw_begin) are runtime artifacts\n\
                  // of that generation process, NOT hardcoded source values.\n\
                  // -------------------------------------------------------------------------\n\n";

    fs::write(out_path, format!("{}{}", header, rendered))?;
    Ok(())
}

fn gen_choice_fragment(elements: &BTreeSet<String>, exclude: &[&str]) -> String {
    let filtered: Vec<_> = elements.iter()
        .filter(|e| !exclude.contains(&e.as_str()))
        .collect();

    if filtered.is_empty() {
        return "$._dummy_never_match".to_string();
    }
    if filtered.len() == 1 {
        return format!("$.kw_{}", filtered[0]);
    }
    let mut out = "choice(\n".to_string();
    for kw in filtered {
        out.push_str(&format!("      $.kw_{},\n", kw));
    }
    out.push_str("    )");
    out
}

fn generate_highlights(keywords: &Keywords, out_path: &str) -> Result<()> {
    let template_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/al-gen/templates");
    let template = fs::read_to_string(format!("{}/highlights.scm.template", template_dir))?;

    let mut specific_tokens = String::new();

    // Only control keywords and operator words have individual grammar tokens (kw_* and op_*).
    // Other categories use the category tokens in the template (object_keyword, type_keyword, etc.)

    // Control keywords get kw_* tokens
    for kw in &keywords.control {
        specific_tokens.push_str(&format!("(kw_{}) @keyword.control\n", kw));
    }

    // Operator words get op_* tokens
    for kw in &keywords.operator_words {
        specific_tokens.push_str(&format!("(op_{}) @keyword.operator\n", kw));
    }

    let rendered = render_template(&template, &[("CONTROL_KW_TOKEN_HIGHLIGHTS", &specific_tokens)]);
    fs::write(out_path, rendered)?;
    Ok(())
}

fn run_tree_sitter_generate(root: &str) -> Result<()> {
    let root_abs = fs::canonicalize(root).context("Failed to canonicalize root")?;
    println!("   Generate root absolute: {}", root_abs.display());

    // Run tree-sitter generate in the root directory
    let output = Command::new("tree-sitter")
        .current_dir(&root_abs)
        .arg("generate")
        .output()
        .context("Failed to run tree-sitter")?;

    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        anyhow::bail!("tree-sitter generate failed");
    }

    if let Ok(metadata) = fs::metadata("src/parser.c") {
        let size_mb = metadata.len() / 1024 / 1024;
        println!("   parser.c: {}MB", size_mb);

        if size_mb < 3 {
            println!("   🎉 <3MB - GOOD!");
        }
    }

    Ok(())
}

fn run_tree_sitter_build(root: &str) -> Result<PathBuf> {
    let root_abs = fs::canonicalize(root).context("Failed to canonicalize root")?;
    println!("   Build root absolute: {}", root_abs.display());

    let out_path = root_abs.join("target/tree-sitter-al.so");
    
    // Ensure target dir exists relative to root
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let status = Command::new("tree-sitter")
        .current_dir(&root_abs)
        .arg("build")
        .arg("-o")
        .arg(&out_path)
        .status()
        .context("Failed to run tree-sitter build")?;

    if !status.success() {
        anyhow::bail!("tree-sitter build failed");
    }

    Ok(out_path)
}

#[derive(Debug, Deserialize)]
struct RepoList {
    #[serde(default)]
    repo: Vec<RepoConfig>,
}

#[derive(Debug, Deserialize)]
struct RepoConfig {
    name: String,
    url: String,
    #[serde(default = "default_branch")]
    branch: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    description: Option<String>,
}

fn default_branch() -> String {
    "main".to_string()
}

fn run_repo_tests(parser_lib: &Path, scope_name: &str, root: &str) -> Result<()> {
    // REPO_TEST_CONFIG is relative to generator execution or root?
    // It's in tests/test_repos.toml. Ideally we read it from root.
    
    let config_path = Path::new(root).join(REPO_TEST_CONFIG);
    let cfg_text = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let cfg: RepoList = toml::from_str(&cfg_text)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;

    let enabled: Vec<_> = cfg.repo.into_iter().filter(|r| r.enabled).collect();
    if enabled.is_empty() {
        println!("ℹ️  No enabled repos in {} ; skipping.", config_path.display());
        return Ok(());
    }

    let work_dir = Path::new(root).join(REPO_TEST_WORKDIR);
    fs::create_dir_all(&work_dir)?;
    
    let target_dir = Path::new(root).join("target");
    fs::create_dir_all(&target_dir)?;

    let mut total_files = 0usize;
    let mut total_ok = 0usize;

    for repo in enabled {
        println!("\n📦 Repository: {}", repo.name);
        if let Some(desc) = &repo.description {
            println!("   {}", desc);
        }
        println!("   URL: {}", repo.url);

        let repo_dir = work_dir.join(&repo.name);
        clone_or_update_repo(&repo, &repo_dir)?;

        // Canonicalize repo_dir so collected files have absolute paths
        let repo_dir_abs = fs::canonicalize(&repo_dir)
            .with_context(|| format!("Failed to canonicalize {}", repo_dir.display()))?;
        let files = collect_al_files(&repo_dir_abs)?;
        total_files += files.len();

        if files.is_empty() {
            println!("   ⚠️  No .al files found");
            continue;
        }

        let paths_file = target_dir.join(format!("paths-{}.txt", repo.name));
        write_paths_file(&paths_file, &files)?;

        // Canonicalize for tree-sitter which changes cwd to root
        let paths_file_abs = fs::canonicalize(&paths_file)
            .with_context(|| format!("Failed to canonicalize {}", paths_file.display()))?;

        let (ok, failed, samples) =
            parse_paths_with_tree_sitter(parser_lib, scope_name, &paths_file_abs, root)?;
        total_ok += ok;

        let pct = (ok as f64) * 100.0 / (files.len() as f64);
        println!("   Results:");
        println!("   ├─ Total files:  {}", files.len());
        println!("   ├─ Parsed OK:    {} ({:.2}%)", ok, pct);
        println!("   └─ Parse errors: {}", failed);
        if !samples.is_empty() {
            println!("   Failed examples:");
            for s in &samples {
                println!("   - {}", s);
            }
            print_failed_parse_details(parser_lib, scope_name, &samples)?;
        }
    }

    if total_files > 0 {
        let pct = (total_ok as f64) * 100.0 / (total_files as f64);
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║  OVERALL TEST RESULTS                                        ║");
        println!("╚══════════════════════════════════════════════════════════════╝\n");
        println!("Total AL files tested:     {}", total_files);
        println!("Successfully parsed:       {} ({:.2}%)", total_ok, pct);
        println!("Parse errors:              {}", total_files - total_ok);
    }

    Ok(())
}

fn run_fixture_tests(parser_lib: &Path, scope_name: &str, root: &str) -> Result<()> {
    let invalid_dir = Path::new(root).join(FIXTURES_INVALID_DIR);
    let valid_dir = Path::new(root).join(FIXTURES_VALID_DIR);
    let target_dir = Path::new(root).join("target");
    fs::create_dir_all(&target_dir)?;

    // Invalid fixtures must FAIL to parse.
    if invalid_dir.exists() {
        let invalid_dir_abs = fs::canonicalize(&invalid_dir)?;
        let invalid = collect_al_files(&invalid_dir_abs)?;
        if !invalid.is_empty() {
            println!("\n🧪 Fixture tests (invalid syntax must fail)...");
            let paths_file = target_dir.join("paths-fixtures-invalid.txt");

            // Files already have absolute paths from canonicalized directory
            write_paths_file(&paths_file, &invalid)?;
            let paths_file_abs = fs::canonicalize(&paths_file)?;

            let summaries =
                parse_paths_with_tree_sitter_detailed(parser_lib, scope_name, &paths_file_abs, root)?;

            let mut unexpected_ok = Vec::new();
            for s in summaries {
                if s.successful {
                    unexpected_ok.push(s.file);
                }
            }
            if !unexpected_ok.is_empty() {
                anyhow::bail!(
                    "Invalid fixtures unexpectedly parsed successfully:\n{}",
                    unexpected_ok
                        .into_iter()
                        .take(20)
                        .map(|p| format!("- {p}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            println!(
                "✅ Invalid fixtures: all failed as expected ({} files).",
                invalid.len()
            );
        }
    }

    // Valid fixtures must SUCCEED to parse.
    if valid_dir.exists() {
        let valid_dir_abs = fs::canonicalize(&valid_dir)?;
        let valid = collect_al_files(&valid_dir_abs)?;
        if !valid.is_empty() {
            println!("\n🧪 Fixture tests (valid syntax must succeed)...");
            let paths_file = target_dir.join("paths-fixtures-valid.txt");

            // Files already have absolute paths from canonicalized directory
            write_paths_file(&paths_file, &valid)?;
            let paths_file_abs = fs::canonicalize(&paths_file)?;

            let summaries =
                parse_paths_with_tree_sitter_detailed(parser_lib, scope_name, &paths_file_abs, root)?;

            let mut unexpected_failed = Vec::new();
            for s in summaries {
                if !s.successful {
                    unexpected_failed.push(s.file);
                }
            }
            if !unexpected_failed.is_empty() {
                anyhow::bail!(
                    "Valid fixtures unexpectedly failed to parse:\n{}",
                    unexpected_failed
                        .into_iter()
                        .take(20)
                        .map(|p| format!("- {p}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            println!(
                "✅ Valid fixtures: all parsed successfully ({} files).",
                valid.len()
            );
        }
    }

    Ok(())
}

fn print_failed_parse_details(
    parser_lib: &Path,
    scope_name: &str,
    samples: &[String],
) -> Result<()> {
    let limit = 3usize.min(samples.len());
    if limit == 0 {
        return Ok(());
    }
    println!("   Failure details (first {}):", limit);
    for path in samples.iter().take(limit) {
        let output = Command::new("tree-sitter")
            .arg("parse")
            .arg("--cst")
            .arg("--no-ranges")
            .arg("--lib-path")
            .arg(parser_lib)
            .arg("--lang-name")
            .arg("al")
            .arg("--scope")
            .arg(scope_name)
            .arg(path)
            .output()
            .with_context(|| format!("Failed to run tree-sitter parse on sample: {path}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let filtered = filter_tree_sitter_config_warning(&stdout);
        let snippet = truncate_lines(&filtered, 60);
        println!("   --- {}", path);
        for line in snippet.lines() {
            println!("   {}", line);
        }
    }
    Ok(())
}

fn filter_tree_sitter_config_warning(s: &str) -> String {
    // When using `tree-sitter parse --lib-path`, some versions still print a global
    // "no parser directories configured" warning. Strip it from our diagnostic snippets.
    s.lines()
        .filter(|line| {
            let l = line.trim();
            !(l.starts_with("Warning: You have not configured any parser directories!")
                || l.starts_with("Please run `tree-sitter init-config`")
                || l.starts_with("configuration file to indicate where we should look for")
                || l.starts_with("language grammars."))
        })
        .map(|l| format!("{l}\n"))
        .collect()
}

fn truncate_lines(s: &str, max_lines: usize) -> String {
    let mut out = String::new();
    for (i, line) in s.lines().enumerate() {
        if i >= max_lines {
            out.push_str("... (truncated)\n");
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn clone_or_update_repo(repo: &RepoConfig, dest: &Path) -> Result<()> {
    if dest.exists() {
        println!("   📂 Updating...");

        let status = Command::new("git")
            .arg("-C")
            .arg(dest)
            .arg("fetch")
            .arg("--all")
            .status()
            .context("git fetch failed")?;
        if !status.success() {
            anyhow::bail!("git fetch failed for {}", repo.name);
        }

        let status = Command::new("git")
            .arg("-C")
            .arg(dest)
            .arg("checkout")
            .arg(&repo.branch)
            .status()
            .context("git checkout failed")?;
        if !status.success() {
            anyhow::bail!("git checkout failed for {}", repo.name);
        }

        let status = Command::new("git")
            .arg("-C")
            .arg(dest)
            .arg("pull")
            .arg("--ff-only")
            .status()
            .context("git pull failed")?;
        if !status.success() {
            anyhow::bail!("git pull failed for {}", repo.name);
        }

        Ok(())
    } else {
        println!("   📥 Cloning...");
        let status = Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--branch")
            .arg(&repo.branch)
            .arg(&repo.url)
            .arg(dest)
            .status()
            .context("git clone failed")?;
        if !status.success() {
            anyhow::bail!("git clone failed for {}", repo.name);
        }
        Ok(())
    }
}

fn collect_al_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("al") || ext.eq_ignore_ascii_case("dal") {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

fn write_paths_file(paths_file: &Path, files: &[PathBuf]) -> Result<()> {
    let mut buf = String::new();
    for p in files {
        buf.push_str(&format!("{}\n", p.display()));
    }
    fs::write(paths_file, buf)?;
    Ok(())
}

fn parse_paths_with_tree_sitter(
    parser_lib: &Path,
    scope_name: &str,
    paths_file: &Path,
    root: &str
) -> Result<(usize, usize, Vec<String>)> {
    let summaries = parse_paths_with_tree_sitter_detailed(parser_lib, scope_name, paths_file, root)?;
    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut samples = Vec::new();
    for s in summaries {
        if s.successful {
            ok += 1;
        } else {
            failed += 1;
            if samples.len() < 10 {
                samples.push(s.file);
            }
        }
    }
    Ok((ok, failed, samples))
}

#[derive(Debug, Clone)]
struct FileParseSummary {
    file: String,
    successful: bool,
}

fn parse_paths_with_tree_sitter_detailed(
    parser_lib: &Path,
    scope_name: &str,
    paths_file: &Path,
    root: &str
) -> Result<Vec<FileParseSummary>> {
    let output = Command::new("tree-sitter")
        .current_dir(root)
        .arg("parse")
        .arg("--quiet")
        .arg("--json-summary")
        .arg("--lib-path")
        .arg(parser_lib)
        .arg("--lang-name")
        .arg("al")
        .arg("--scope")
        .arg(scope_name)
        .arg("--paths")
        .arg(paths_file)
        .output()
        .context("Failed to run tree-sitter parse")?;

    let stdout_full = String::from_utf8_lossy(&output.stdout);
    let stdout_json = extract_json_object_from_output(&stdout_full).unwrap_or(&stdout_full);

    let summary: serde_json::Value = match serde_json::from_str(stdout_json) {
        Ok(v) => v,
        Err(e) => {
            // If tree-sitter fails hard (bad args, no files, etc), it may not output JSON.
            // In that case, surface stderr and the JSON parse error.
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow!(
                "Failed to parse tree-sitter --json-summary output: {e}"
            ));
        }
    };

    if let Some(summaries) = summary.get("parse_summaries").and_then(|v| v.as_array()) {
        let mut out = Vec::new();
        for s in summaries {
            let file = s
                .get("file")
                .or_else(|| s.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>")
                .to_string();
            let successful = s
                .get("successful")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            out.push(FileParseSummary { file, successful });
        }
        return Ok(out);
    }

    if let Some(files) = summary.get("files").and_then(|v| v.as_array()) {
        let mut out = Vec::new();
        for f in files {
            let file = f
                .get("path")
                .or_else(|| f.get("file"))
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>")
                .to_string();
            let successful = f
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| {
                    f.get("error_count")
                        .and_then(|v| v.as_u64())
                        .map(|n| n == 0)
                        .unwrap_or(false)
                });
            out.push(FileParseSummary { file, successful });
        }
        return Ok(out);
    }

    anyhow::bail!("Unrecognized tree-sitter --json-summary output format");
}

fn extract_json_object_from_output(s: &str) -> Option<&str> {
    // tree-sitter may print per-file lines before the JSON summary.
    // Find the first line that begins a JSON object and slice from there.
    let mut offset = 0usize;
    for line in s.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('{') {
            // Compute byte offset of the '{' in the original string.
            let brace_idx_in_line = line.len() - trimmed.len();
            return Some(&s[offset + brace_idx_in_line..]);
        }
        offset += line.len() + 1; // + '\n'
    }
    None
}
