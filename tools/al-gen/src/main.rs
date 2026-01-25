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

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO_TEST_CONFIG: &str = "tests/test_repos.toml";
const REPO_TEST_WORKDIR: &str = "tests/.repos";

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  AL Tree-Sitter Generator - Dynamic from Extension          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("🔍 Finding AL extension...");
    let extension_path = find_al_extension()?;
    println!("✅ Found: {}", extension_path.display());

    let syntax_file = find_syntax_file(&extension_path)?;
    println!("📄 Using syntax: {}\n", syntax_file.display());

    println!("📖 Extracting keywords from TextMate grammar...");
    let keywords = extract_keywords(&syntax_file)?;
    let scope_name = extract_scope_name(&syntax_file).unwrap_or_else(|_| "source.al".to_string());
    print_keyword_stats(&keywords);

    println!("\n🔧 Generating C scanner files...");
    generate_keywords_c(&keywords)?;
    generate_scanner_c()?;
    println!("✅ Generated src/keywords.c and src/scanner.c");

    println!("\n📝 Generating grammar.js...");
    generate_grammar_js()?;
    println!("✅ Generated grammar.js");

    println!("\n🎨 Generating highlight queries...");
    generate_highlights()?;
    println!("✅ Generated queries/highlights.scm");

    println!("\n🌳 Running tree-sitter generate...");
    run_tree_sitter_generate()?;

    // Build a parser library once; used for fast `tree-sitter parse` runs.
    println!("\n🔨 Building parser library...");
    let lib_path = run_tree_sitter_build()?;
    println!("✅ Built: {}", lib_path.display());

    // Optional: real-world validation against Microsoft repos.
    // Opt-in only: `cargo run --release -- --test`
    let args: Vec<String> = std::env::args().collect();
    let do_tests = args.iter().any(|a| a == "--test" || a == "--tests");

    if do_tests {
        if Path::new(REPO_TEST_CONFIG).exists() {
            println!("\n🧪 Testing against real repositories...");
            run_repo_tests(&lib_path, &scope_name)?;
        } else {
            println!("\nℹ️  {} not found; skipping repo tests.", REPO_TEST_CONFIG);
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

            al_extensions.sort_by(|a, b| al_extension_version_key(a).cmp(&al_extension_version_key(b)));
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
        anyhow::bail!("No syntaxes directory in extension: {}", extension_path.display());
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
    let xml = fs::read_to_string(syntax_file)
        .with_context(|| format!("Failed to read {}", syntax_file.display()))?;

    // Find match/name pairs. This is a pragmatic parser for the AL TextMate plist:
    // we only care about `match` patterns that include `(?i:(...))` keyword lists.
    let pattern_re = Regex::new(
        r#"<key>match</key>\s*<string>(.*?)</string>[\s\S]*?<key>name</key>\s*<string>(.*?)</string>"#,
    )?;
    let keyword_list_re = Regex::new(r"\(\?i:\((.*?)\)\)")?;

    let mut out = Keywords::default();

    for caps in pattern_re.captures_iter(&xml) {
        let pattern = &caps[1];
        let scope_name = &caps[2];

        let Some(kw_caps) = keyword_list_re.captures(pattern) else {
            continue;
        };

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
                out.objects.insert(kw);
            } else if scope_name.contains("builtintypes") {
                out.types.insert(kw);
            } else if scope_name.contains("keyword.other.metadata") {
                out.metadata.insert(kw);
            } else if scope_name.contains("keyword.other.property") {
                out.properties.insert(kw);
            }
        }
    }

    Ok(out)
}

fn extract_scope_name(syntax_file: &Path) -> Result<String> {
    let xml = fs::read_to_string(syntax_file)
        .with_context(|| format!("Failed to read {}", syntax_file.display()))?;
    let re = Regex::new(r#"<key>scopeName</key>\s*<string>([^<]+)</string>"#)?;
    let caps = re
        .captures(&xml)
        .ok_or_else(|| anyhow!("scopeName not found in {}", syntax_file.display()))?;
    Ok(caps[1].trim().to_string())
}

fn is_identifier_like(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else { return false };
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

fn generate_keywords_c(keywords: &Keywords) -> Result<()> {
    fs::create_dir_all("src")?;

    let all = keywords_all(keywords);

    let mut out = String::new();
    out.push_str("// Keyword lookup tables for AL external scanner\n");
    out.push_str("// AUTO-GENERATED - DO NOT EDIT\n");
    out.push_str("// Generated by: cargo run\n\n");
    out.push_str("#include <stdbool.h>\n");
    out.push_str("#include <stddef.h>\n");
    out.push_str("#include <string.h>\n\n");

    out.push_str("static bool al_kw_binsearch(const char *word, const char *const *arr, size_t count) {\n");
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
    write_kw_array(&mut out, "AL_KEYWORDS_OPERATOR_WORDS", &keywords.operator_words);
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

    fs::write("src/keywords.c", out)?;
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

fn generate_scanner_c() -> Result<()> {
    fs::create_dir_all("src")?;
    let template = fs::read_to_string("templates/scanner.c.template")?;
    fs::write("src/scanner.c", template)?;
    Ok(())
}

fn generate_grammar_js() -> Result<()> {
    let template = fs::read_to_string("templates/grammar.js.template")?;
    fs::write("grammar.js", template)?;
    Ok(())
}

fn generate_highlights() -> Result<()> {
    fs::create_dir_all("queries")?;
    let template = fs::read_to_string("templates/highlights.scm.template")?;
    fs::write("queries/highlights.scm", template)?;
    Ok(())
}

fn run_tree_sitter_generate() -> Result<()> {
    let output = Command::new("tree-sitter")
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

        if size_mb < 30 {
            println!("   🎉 <30MB - GOOD!");
        }
    }

    Ok(())
}

fn run_tree_sitter_build() -> Result<PathBuf> {
    fs::create_dir_all("target")?;
    let out_path = PathBuf::from("target").join("tree-sitter-al.so");

    let status = Command::new("tree-sitter")
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

fn run_repo_tests(parser_lib: &Path, scope_name: &str) -> Result<()> {
    let cfg_text = fs::read_to_string(REPO_TEST_CONFIG)
        .with_context(|| format!("Failed to read {}", REPO_TEST_CONFIG))?;
    let cfg: RepoList = toml::from_str(&cfg_text)
        .with_context(|| format!("Failed to parse {}", REPO_TEST_CONFIG))?;

    let enabled: Vec<_> = cfg.repo.into_iter().filter(|r| r.enabled).collect();
    if enabled.is_empty() {
        println!("ℹ️  No enabled repos in {} ; skipping.", REPO_TEST_CONFIG);
        return Ok(());
    }

    fs::create_dir_all(REPO_TEST_WORKDIR)?;
    fs::create_dir_all("target")?;

    let mut total_files = 0usize;
    let mut total_ok = 0usize;

    for repo in enabled {
        println!("\n📦 Repository: {}", repo.name);
        if let Some(desc) = &repo.description {
            println!("   {}", desc);
        }
        println!("   URL: {}", repo.url);

        let repo_dir = PathBuf::from(REPO_TEST_WORKDIR).join(&repo.name);
        clone_or_update_repo(&repo, &repo_dir)?;

        let files = collect_al_files(&repo_dir)?;
        total_files += files.len();

        if files.is_empty() {
            println!("   ⚠️  No .al files found");
            continue;
        }

        let paths_file = PathBuf::from("target").join(format!("paths-{}.txt", repo.name));
        write_paths_file(&paths_file, &files)?;

        let (ok, failed, samples) = parse_paths_with_tree_sitter(parser_lib, scope_name, &paths_file)?;
        total_ok += ok;

        let pct = (ok as f64) * 100.0 / (files.len() as f64);
        println!("   Results:");
        println!("   ├─ Total files:  {}", files.len());
        println!("   ├─ Parsed OK:    {} ({:.2}%)", ok, pct);
        println!("   └─ Parse errors: {}", failed);
        if !samples.is_empty() {
            println!("   Failed examples:");
            for s in samples {
                println!("   - {}", s);
            }
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
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
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
) -> Result<(usize, usize, Vec<String>)> {
    let output = Command::new("tree-sitter")
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
            return Err(anyhow!("Failed to parse tree-sitter --json-summary output: {e}"));
        }
    };

    // tree-sitter's JSON summary shape varies by version; handle known formats:
    //
    // tree-sitter 0.26+:
    // {
    //   "parse_summaries": [{ "file": "...", "successful": true/false, ... }],
    //   "cumulative_stats": { "successful_parses": N, "total_parses": M, ... },
    //   ...
    // }
    if let Some(summaries) = summary.get("parse_summaries").and_then(|v| v.as_array()) {
        let mut ok = 0usize;
        let mut failed = 0usize;
        let mut samples = Vec::new();
        for s in summaries {
            match s.get("successful").and_then(|v| v.as_bool()) {
                Some(true) => ok += 1,
                Some(false) => {
                    failed += 1;
                    if samples.len() < 10 {
                        if let Some(f) = s.get("file").and_then(|v| v.as_str()) {
                            samples.push(f.to_string());
                        }
                    }
                }
                None => {
                    failed += 1;
                }
            }
        }
        return Ok((ok, failed, samples));
    }

    // Older / alternate formats:
    // - { "files": [ { "path": "...", "success": true/false } ] }
    if let Some(files) = summary.get("files").and_then(|v| v.as_array()) {
        let mut ok = 0usize;
        let mut failed = 0usize;
        let mut samples = Vec::new();
        for f in files {
            if let Some(s) = f.get("success").and_then(|v| v.as_bool()) {
                if s {
                    ok += 1
                } else {
                    failed += 1;
                    if samples.len() < 10 {
                        if let Some(p) = f.get("path").and_then(|v| v.as_str()) {
                            samples.push(p.to_string());
                        }
                    }
                }
            } else if let Some(ec) = f.get("error_count").and_then(|v| v.as_u64()) {
                if ec == 0 {
                    ok += 1
                } else {
                    failed += 1;
                    if samples.len() < 10 {
                        if let Some(p) = f.get("path").and_then(|v| v.as_str()) {
                            samples.push(p.to_string());
                        }
                    }
                }
            } else {
                failed += 1;
            }
        }
        return Ok((ok, failed, samples));
    }

    if let Some(stats) = summary.get("cumulative_stats").and_then(|v| v.as_object()) {
        let ok = stats
            .get("successful_parses")
            .or_else(|| stats.get("success_count"))
            .or_else(|| stats.get("ok_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let failed = stats
            .get("total_parses")
            .and_then(|v| v.as_u64())
            .map(|t| t as usize)
            .unwrap_or(ok)
            .saturating_sub(ok);
        return Ok((ok, failed, Vec::new()));
    }

    if let Some(stats) = summary.get("file_stats").and_then(|v| v.as_object()) {
        let ok = stats
            .get("success_count")
            .or_else(|| stats.get("ok_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let failed = stats
            .get("error_count")
            .or_else(|| stats.get("failed_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        return Ok((ok, failed, Vec::new()));
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

