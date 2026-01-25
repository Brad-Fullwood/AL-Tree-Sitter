//! AL Tree-Sitter Generator - Dynamic Grammar from VS Code Extension
//!
//! Extracts keywords from VS Code AL extension and generates case-insensitive
//! regex patterns in the grammar. NO hardcoding!

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  AL Tree-Sitter Generator - Dynamic from Extension          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("🔍 Finding AL extension...");
    let extension_path = find_al_extension()?;
    println!("✅ Found: {}\n", extension_path.display());

    println!("📖 Extracting keywords by category...");
    let keywords = extract_keywords_by_category(&extension_path)?;
    print_keyword_stats(&keywords);

    println!("\n🔧 Generating grammar.js with dynamic keywords...");
    generate_grammar(&keywords)?;
    println!("✅ Generated grammar.js\n");

    println!("🌳 Running tree-sitter generate...");
    run_tree_sitter_generate()?;
    
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  ✅ DONE!                                                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

#[derive(Debug, Default)]
struct Keywords {
    control: Vec<String>,
    operators: Vec<String>,
    objects: Vec<String>,
    types: Vec<String>,
    metadata: Vec<String>,
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

            al_extensions.sort();
            if let Some(latest) = al_extensions.last() {
                return Ok(latest.clone());
            }
        }
    }

    anyhow::bail!("AL extension not found")
}

fn extract_keywords_by_category(extension_path: &Path) -> Result<Keywords> {
    let syntax_file = extension_path.join("syntaxes/alsyntax.tmlanguage");
    let xml = fs::read_to_string(&syntax_file)?;

    let pattern_re = Regex::new(
        r#"<key>match</key>\s*<string>(.*?)</string>[\s\S]*?<key>name</key>\s*<string>(.*?)</string>"#,
    )?;
    let keyword_re = Regex::new(r"\(\?i:\((.*?)\)\)")?;

    let mut keywords = Keywords::default();

    for caps in pattern_re.captures_iter(&xml) {
        let pattern = &caps[1];
        let name = &caps[2];

        if let Some(kw_caps) = keyword_re.captures(pattern) {
            let kw_list: Vec<String> = kw_caps[1].split('|').map(|s| s.to_lowercase()).collect();

            if name.contains("keyword.control") {
                keywords.control.extend(kw_list);
            } else if name.contains("keyword.operators") {
                keywords.operators.extend(kw_list);
            } else if name.contains("applicationobject") {
                keywords.objects.extend(kw_list);
            } else if name.contains("builtintypes") {
                keywords.types.extend(kw_list);
            } else if name.contains("metadata") {
                keywords.metadata.extend(kw_list);
            }
        }
    }

    Ok(keywords)
}

fn print_keyword_stats(keywords: &Keywords) {
    println!("   Control:  {} keywords", keywords.control.len());
    println!("   Operators: {} keywords", keywords.operators.len());
    println!("   Objects:   {} keywords", keywords.objects.len());
    println!("   Types:     {} keywords", keywords.types.len());
    println!("   Metadata:  {} keywords", keywords.metadata.len());
}

fn case_insensitive_regex(word: &str) -> String {
    word.chars()
        .map(|c| {
            if c.is_alphabetic() {
                format!("[{}{}]", c.to_lowercase(), c.to_uppercase())
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn generate_keyword_choice(keywords: &[String]) -> String {
    keywords
        .iter()
        .map(|kw| format!("/{}/", case_insensitive_regex(kw)))
        .collect::<Vec<_>>()
        .join(",\n      ")
}

fn generate_grammar(keywords: &Keywords) -> Result<()> {
    let template = fs::read_to_string("templates/grammar.js.template")?;
    
    // Replace placeholders with dynamic keywords
    let grammar = template
        .replace("{{OBJECT_KEYWORDS}}", &generate_keyword_choice(&keywords.objects))
        .replace("{{TYPE_KEYWORDS}}", &generate_keyword_choice(&keywords.types))
        .replace("{{CONTROL_KEYWORDS}}", &generate_keyword_choice(&keywords.control));

    fs::write("grammar.js", grammar)?;
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
