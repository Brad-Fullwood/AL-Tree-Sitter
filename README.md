# tree-sitter-al-bc

A [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammar for the AL
language used by Microsoft Dynamics 365 Business Central.

The repository contains the generated parser, highlighting and editor queries,
a Rust binding, and the generator used to update them from Microsoft's AL
extension. Generated parser files are committed so consumers do not need the
generator or the AL extension.

## Build and test

The Rust binding requires a stable Rust toolchain:

```sh
cargo test
```

Fixture and repository validation also require the
[`tree-sitter` CLI](https://tree-sitter.github.io/tree-sitter/cli/installation.html)
at the exact version recorded in `.tree-sitter-cli-version`. The exact pin is
part of the generated-parser contract; older releases lack the prebuilt-library
JSON-summary interface used by the corpus runner.

```sh
cargo install tree-sitter-cli \
  --version "$(cat .tree-sitter-cli-version)" --locked
```

The generator runs the fixture suite after regeneration:

```sh
cd generator
cargo run --release
```

Pass `--test` to additionally clone and parse the repositories configured in
`tests/test_repos.toml`:

```sh
cargo run --release -- --test
```

To validate the committed generated parser without locating or regenerating
from Microsoft's AL extension, use the repository runner:

```bash
tests/run_repo_tests.sh
```

It builds the committed parser, runs the focused valid/invalid fixtures, and
parses every pinned external corpus revision. Internally it uses
`--repo-tests-only`; that mode deliberately performs no generator-owned source
rewrite.

## Regenerating the grammar

The generator reads `syntaxes/alsyntax.tmlanguage` from the newest installed
`ms-dynamics-smb.al-*` extension under either `~/.vscode/extensions` or
`~/.cursor/extensions`. It updates:

- `grammar.js` and the generated files under `src/`
- queries under `queries/`
- language metadata under `data/`
- the AL language package files consumed by the parent Zed extension

Edit the sources under `generator/tools/al-gen/templates/` rather than generated
grammar, scanner, or query files. Run the generator and commit the corresponding
generated changes.

`queries/folds.scm`, `queries/locals.scm`, and `queries/textobjects.scm` are
derived from `src/node-types.json` alone, so they can be refreshed after a
grammar change without locating the AL extension:

```sh
tree-sitter generate
cargo run --release --manifest-path generator/Cargo.toml -- --structural-queries-only
```

`queries/brackets.scm`, `queries/indents.scm`, and `queries/outline.scm` are
hand-maintained here and copied verbatim into the parent extension's
`languages/al` directory. Edit them in `queries/`; never edit the copies.

## Validation fixtures

Files under `tests/fixtures/valid` must parse without errors. Files under
`tests/fixtures/invalid` must produce parse errors. The optional repository suite
validates the grammar against larger AL codebases; it reports parse results but
does not replace focused fixtures for grammar changes.

`test/corpus/` holds tree-sitter corpus tests, which pin the exact tree shape
(including field names) rather than just "parses / does not parse":

```sh
tree-sitter test
```

## Preprocessor symbols

`#define` and `#undef` in the source update the scanner's symbol table, so a
later `#if` sees them. Symbols can also be supplied externally through the
`AL_TS_DEFINES` environment variable (whitespace-, comma-, or semicolon-
separated); an in-source `#define`/`#undef` overrides the environment value for
the rest of the file. `AL_TS_UNKNOWN_TRUE` controls whether an unknown symbol
evaluates to true (default: false).

## Rust binding

```rust
let mut parser = tree_sitter::Parser::new();
parser.set_language(&tree_sitter_al::LANGUAGE.into())?;
# Ok::<(), tree_sitter::LanguageError>(())
```

The binding also embeds the generated node types, highlighting query, and AL
language-data JSON files.

## License

MIT. See [LICENSE](LICENSE).
