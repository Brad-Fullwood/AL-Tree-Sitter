# AL Tree-Sitter Parser - Handover

## Purpose
This repo builds a Tree-sitter parser for AL (Business Central). Goal: 99%+ parse success on real-world AL while keeping parser size under 20MB.

## Current Status (2025-01-26)
- Parse success: 82.65% (12,064 / 14,597)
- Parser size: ~0.83MB (`src/parser.c`)
- Notes:
  - BCApps: 80.10% (3,688 / 4,604)
  - ALAppExtensions: 83.82% (8,376 / 9,993)

## Key Files
- `tools/al-gen/src/main.rs`: Rust generator (finds AL extension, extracts keywords, renders templates, runs tests).
- `templates/grammar.js.template`: Main grammar source (edit here).
- `templates/scanner.c.template`: External scanner source (edit here).
- `tests/fixtures/`: Small valid/invalid AL samples for quick validation.
- `tests/test_repos.toml`: Real-repo test configuration.
- Generated (do not edit): `grammar.js`, `src/parser.c`, `src/scanner.c`, `src/keywords.c`, `src/grammar.json`, `src/node-types.json`.

## Workflow
1. Edit grammar/scanner templates.
2. Run generation and fixture validation:
   ```bash
   cargo run --release
   ```
3. Run real-repo tests (clones to `tests/.repos/`):
   ```bash
   cargo run --release -- --test
   ```
   or:
   ```bash
   tests/run_repo_tests.sh
   ```

## Debugging
- Parse a specific file:
  ```bash
  tree-sitter parse --cst --no-ranges --lib-path target/tree-sitter-al.so --lang-name al --scope source.al <file.al>
  ```
- Look for `ERROR` nodes in CST output to identify grammar gaps.

## Source of Truth (Critical)
The AL extension is authoritative for syntax and keywords. Avoid hardcoding lists.
- Extension lives in `~/.cursor/extensions/` or `~/.vscode/extensions/` (latest `ms-dynamics-smb.al-*`).
- Use `syntaxes/alsyntax.tmlanguage` to extract keywords/patterns in `tools/al-gen/src/main.rs`.

## Extension-First Rules (Non-Negotiable)
- Do not hardcode language features, keywords, or syntax rules in templates or grammar.
- Always check the extension first and extract from it in `tools/al-gen/src/main.rs`.
- If the extension already defines a pattern, mirror it via extraction or template injection.
- When adding new syntax support, cite the source file in the extension (e.g., `syntaxes/alsyntax.tmlanguage`).

## Known Hotspots / Next Steps
- Identify top failing patterns from real-repo tests and add minimal fixtures.
- Prioritize high-impact fixes before optimizing size.
- Track size deltas after grammar changes.

## Quick Notes
- Parser size can balloon with overly permissive rules; simplify where possible.
- Keep fixtures small and representative.
