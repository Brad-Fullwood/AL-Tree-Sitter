//! This crate provides AL (Microsoft Dynamics 365 Business Central) language
//! support for the [tree-sitter] parsing library.
//!
//! Typical usage builds a [`tree_sitter::Parser`] with the [`LANGUAGE`] constant:
//!
//! ```
//! let mut parser = tree_sitter::Parser::new();
//! parser
//!     .set_language(&tree_sitter_al::LANGUAGE.into())
//!     .expect("Error loading AL parser");
//! ```
//!
//! [tree-sitter]: https://tree-sitter.github.io/

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_al() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
///
/// Convert to a [`tree_sitter::Language`] with `.into()`.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_al) };

/// The content of this grammar's `node-types.json` file.
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

/// The syntax-highlighting query for this grammar.
pub const HIGHLIGHTS_QUERY: &str = include_str!("../../queries/highlights.scm");

#[cfg(test)]
mod tests {
    #[test]
    fn test_can_load_grammar() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading AL parser");
    }
}
