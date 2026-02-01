# Tree-sitter Grammar for AL

A [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammar for the **AL (Application Language)** programming language used in **Microsoft Dynamics 365 Business Central** development.

## Features

- Parses all AL object types (tables, pages, codeunits, reports, etc.)
- Supports modern AL syntax including interfaces, enums, and permission sets
- Handles preprocessor directives (`#if`, `#else`, `#endif`, `#region`)
- Includes queries for syntax highlighting, code outline, indentation, and bracket matching
- **98%+ parse success rate** against Microsoft's BCApps repository

## Usage

This grammar is used by the [AL Extension for Zed](https://github.com/Brad-Fullwood/al.language.zed). Zed automatically clones this repository when installing the extension.

### Manual Usage

```bash
# Parse an AL file
tree-sitter parse myfile.al

# Generate parser (if modifying grammar)
tree-sitter generate
```

## Included Queries

| Query | Purpose |
|-------|---------|
| `highlights.scm` | Syntax highlighting |
| `outline.scm` | Code outline / document symbols |
| `indents.scm` | Auto-indentation |
| `brackets.scm` | Bracket matching |

## Regenerating the Grammar

The grammar is generated from Microsoft's VS Code AL extension TextMate grammar to ensure compatibility.

### Prerequisites

- Rust toolchain
- VS Code or Cursor with AL Language extension installed

### Generate

```bash
cd generator
cargo run --release
```

This extracts keywords and patterns from the VS Code extension and generates:
- `grammar.js` - Tree-sitter grammar definition
- `src/parser.c` - Generated parser
- `src/scanner.c` - Custom scanner for complex tokens
- `queries/highlights.scm` - Syntax highlighting queries

### Test Against Microsoft Repos

```bash
cd generator
cargo run --release -- --test
```

This clones Microsoft's BCApps and AL-Go repositories and tests parse success rates.

## Project Structure

```
AL-Tree-Sitter/
├── grammar.js              # Tree-sitter grammar (generated)
├── tree-sitter.json        # Tree-sitter configuration
├── src/
│   ├── parser.c            # Generated parser
│   ├── scanner.c           # Custom scanner
│   └── keywords.c          # Keyword definitions
├── queries/
│   ├── highlights.scm      # Syntax highlighting
│   ├── outline.scm         # Document symbols
│   ├── indents.scm         # Indentation rules
│   └── brackets.scm        # Bracket matching
├── generator/
│   └── tools/al-gen/
│       ├── src/main.rs     # Generator tool
│       └── templates/      # Generation templates
└── tests/
    ├── fixtures/           # Test AL files
    └── test_repos.toml     # Test repository config
```

## Supported Constructs

### Object Types
- `table`, `tableextension`
- `page`, `pageextension`, `pagecustomization`
- `codeunit`
- `report`, `reportextension`
- `query`, `xmlport`
- `enum`, `enumextension`
- `interface`, `controladdin`
- `permissionset`, `permissionsetextension`
- `entitlement`, `profile`, `profileextension`

### Statements
- `if`/`then`/`else`, `case`
- `for`/`to`/`downto`/`do`
- `foreach`/`in`/`do`
- `while`/`do`
- `repeat`/`until`
- `begin`/`end`

### Declarations
- `procedure`, `trigger`, `event`
- `var`, `local`, `protected`, `internal`
- Fields, keys, actions, views

## Related Projects

- [AL Extension for Zed](https://github.com/Brad-Fullwood/al.language.zed) - Zed editor extension using this grammar
- [AL Language Extension for VS Code](https://marketplace.visualstudio.com/items?itemName=ms-dynamics-smb.al) - Microsoft's official extension

## License

MIT
