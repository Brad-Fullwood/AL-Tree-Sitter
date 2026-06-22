//! Compiles the generated tree-sitter parser (`src/parser.c`) and its external
//! scanner (`src/scanner.c`, which `#include`s `src/keywords.c`) into a static
//! library linked by the Rust binding.

fn main() {
    let src_dir = std::path::Path::new("src");

    let mut build = cc::Build::new();
    build.include(src_dir);
    build.file(src_dir.join("parser.c"));
    println!("cargo:rerun-if-changed=src/parser.c");

    let scanner = src_dir.join("scanner.c");
    if scanner.exists() {
        build.file(&scanner);
        println!("cargo:rerun-if-changed=src/scanner.c");
        // scanner.c #includes keywords.c, so changes to it must retrigger.
        println!("cargo:rerun-if-changed=src/keywords.c");
    }

    // The generated parser.c emits warnings we do not control.
    build.warnings(false).flag_if_supported("-w");

    build.compile("tree-sitter-al");
}
