# AL Tree-Sitter Generator

**ONE command. Does everything.**

## Usage

```bash
cargo run --release
```

That's it! This single command:
1. Finds your AL extension automatically
2. Extracts 252 keywords
3. Renders templates + injects generated fragments (no hardcoded keyword lists)
4. Runs `tree-sitter generate`
5. Runs quick fixture validation (`tests/fixtures/*`)
6. (Optional) Tests against real Microsoft repositories (`--test`)

## What It Does

```
cargo run --release
│
├─ STEP 1: Generate Grammar Files
│  ├─ Find AL extension (auto)
│  ├─ Extract keywords → src/keywords.c
│  ├─ Render template → src/scanner.c (inject externals + preprocessor support)
│  └─ Render template → grammar.js (inject externals)
│
├─ STEP 2: Run tree-sitter generate
│  ├─ Generate src/parser.c
│  └─ Check size (<20MB target)
│
└─ STEP 3: Validate
   ├─ Fixture suite (always-on)
   ├─ Clone/update BCApps
   ├─ Clone/update ALAppExtensions
   ├─ Parse all .al files
   └─ Report success rate (target: 99%+)
```

## Preprocessor configuration

The external scanner evaluates `#if/#elif/#else/#endif` with a small boolean expression parser.

- **`AL_TS_DEFINES`**: a comma/semicolon/space-separated list of defined symbols (e.g. `AL_TS_DEFINES="CLEANROOM,BC,ONPREM"`). Used by `defined(X)` and bare identifiers.
- **`AL_TS_UNKNOWN_TRUE`**: default for unknown identifiers in `#if` expressions (`1`/`0`, `true`/`false`). Defaults to `true` to avoid hiding code accidentally.

## Project Structure

```
Source (EDIT THESE):
  ├── tools/al-gen/src/main.rs     # The generator (does everything!)
  ├── templates/
  │   ├── grammar.js.template     # AL grammar rules
  │   └── scanner.c.template      # External scanner
  └── tests/test_repos.toml       # Repo-test configuration

Generated (DON'T EDIT):
  ├── grammar.js
  ├── src/keywords.c
  ├── src/scanner.c
  ├── src/parser.c
  ├── src/grammar.json
  ├── src/node-types.json
  ├── queries/highlights.scm
  └── tree-sitter.json

Fixtures (validation):
  └── tests/fixtures/
      ├── valid/
      └── invalid/
```

## Configuration

### Test Repositories (`tests/test_repos.toml`)

```toml
[[repo]]
name = "BCApps"
url = "https://github.com/microsoft/BCApps"
branch = "main"
enabled = true
description = "Microsoft Business Central System Application"

[[repo]]
name = "ALAppExtensions"
url = "https://github.com/microsoft/ALAppExtensions"
branch = "main"
enabled = true
description = "Microsoft Business Central Application Extensions"
```

Add your own:
```toml
[[repo]]
name = "MyProject"
url = "https://github.com/yourname/your-al-project"
branch = "main"
enabled = true
description = "My AL project"
```

## Prerequisites

```bash
# Rust (for the generator)
rustc --version

# tree-sitter CLI (for parser generation)
npm install -g tree-sitter-cli

# git (for cloning test repos)
git --version
```

## Output Example

```
╔══════════════════════════════════════════════════════════════╗
║  AL Tree-Sitter Generator - ALL-IN-ONE                       ║
╚══════════════════════════════════════════════════════════════╝

🔧 STEP 1: Generate Grammar Files
================================================================

🔍 Finding AL extension...
✅ Found: /home/user/.cursor/extensions/ms-dynamics-smb.al-16.3.2065053

📖 Extracting keywords...
✅ Extracted 252 keywords

🔧 Generating C scanner files...
✅ Generated src/keywords.c and src/scanner.c

📝 Generating grammar.js...
✅ Generated grammar.js

🌳 STEP 2: Run tree-sitter generate
================================================================

Running: tree-sitter generate

✅ Generated src/parser.c: 18MB
   🎉 EXCELLENT! <20MB

🧪 STEP 3: Test Against Real Repositories
================================================================

📦 Repository: BCApps
   Microsoft Business Central System Application
   URL: https://github.com/microsoft/BCApps

   📂 Updating...
   🔍 Testing .al files...
   [========================================] 3245/3245 Complete

   Results:
   ├─ Total files:    3,245
   ├─ Parsed OK:      3,238 (99.78%)
   └─ Parse errors:   7

📦 Repository: ALAppExtensions
   ...

╔══════════════════════════════════════════════════════════════╗
║  OVERALL TEST RESULTS                                        ║
╚══════════════════════════════════════════════════════════════╝

Total AL files tested:     18,456
Successfully parsed:       18,389 (99.64%)
Parse errors:              67

Target: 99%+ success rate (original: 99.48%)
Status: 🎉 BETTER than original!

╔══════════════════════════════════════════════════════════════╗
║  ✅ ALL DONE!                                                ║
╚══════════════════════════════════════════════════════════════╝
```

## Development

### Update Grammar Rules
Edit `templates/grammar.js.template`, then:
```bash
cargo run --release
```

### Update Scanner Logic
Edit `templates/scanner.c.template`, then:
```bash
cargo run --release
```

### Skip Testing (faster iteration)
Comment out repos in `tests/test_repos.toml`:
```toml
[[repo]]
enabled = false  # Set to false
```

## Why ONE File?

- **Simple**: One command to run
- **Fast**: Rust compiles to native binary
- **Complete**: Generate + build + test all in one
- **Clear**: All logic in `tools/al-gen/src/main.rs` (easy to understand)
- **No scripts**: No bash scripts to maintain

## File Ownership

| File | Edit? | Why? |
|------|-------|------|
| `tools/al-gen/src/main.rs` | ✅ YES | All the logic |
| `templates/*.template` | ✅ YES | Source templates |
| `tests/test_repos.toml` | ✅ YES | Test config |
| `grammar.js` | ❌ NO | Generated |
| `src/keywords.c` | ❌ NO | Generated |
| `src/scanner.c` | ❌ NO | Generated |
| `src/parser.c` | ❌ NO | Generated |

## Summary

- ✅ ONE Rust file (`tools/al-gen/src/main.rs`)
- ✅ ONE command (cargo run)
- ✅ Does everything (generate + build + test)
- ✅ Templates (clean, editable)
- ✅ Dynamic (zero hardcoding)
- ✅ Real-world tested (Microsoft repos)
- ✅ Measurable (success rate %)

**Simple. Complete. Production-ready.**
