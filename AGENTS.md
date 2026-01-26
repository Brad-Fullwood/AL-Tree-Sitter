# Repository Guidelines

## Project Structure & Module Organization
- `tools/al-gen/src/main.rs`: Rust generator that discovers the AL extension, extracts keywords, renders templates, and orchestrates generation/testing.
- `templates/`: Source templates for `grammar.js` and the C scanner; edit these for grammar changes.
- `src/`: Generated parser artifacts (`parser.c`, `grammar.json`, `node-types.json`, `scanner.c`, `keywords.c`).
- `tests/fixtures/`: Small valid/invalid AL samples used in quick validation.
- `tests/test_repos.toml`: Real-repo test configuration.
- `queries/`: Tree-sitter highlight queries.

## Build, Test, and Development Commands
- `cargo run --release`: One-command workflow; generates grammar, runs `tree-sitter generate`, and validates fixtures.
- `cargo run --release -- --test`: Full validation against real repositories in `tests/test_repos.toml`.
- `tests/run_repo_tests.sh`: Wrapper for the real-repo test run (clones into `tests/.repos/`).
- `tree-sitter parse --cst --no-ranges --lib-path target/tree-sitter-al.so --lang-name al --scope source.al <file.al>`: Parse a specific file with the built parser.

## Coding Style & Naming Conventions
- Rust code follows `rustfmt` defaults; keep functions focused and avoid hardcoded keyword lists.
- JavaScript grammar lives in `templates/grammar.js.template`; keep rule names consistent with existing grammar nodes.
- Generated files in `src/` and top-level `grammar.js` should not be edited directly.

## Testing Guidelines
- Fixture tests: keep small, focused `.al` files in `tests/fixtures/valid` and `tests/fixtures/invalid`.
- Real-repo tests: update `tests/test_repos.toml` to enable/disable repos; target 99%+ parse success.
- Use `tree-sitter parse` for debugging individual failures and to inspect CST output.

## Commit & Pull Request Guidelines
- Commit history is minimal and uses short subjects (e.g., `Initial`, `WIP`); prefer concise, descriptive messages and avoid `WIP` in final PRs.
- PRs should describe the grammar/scanner change, include before/after parse impact, and list any new fixtures or repo-test results.

## Extension-Driven Source of Truth
- The AL extension is the authority for keywords and syntax patterns; update extraction logic in `tools/al-gen/src/main.rs` rather than hardcoding lists.
