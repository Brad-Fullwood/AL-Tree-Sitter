//! This crate provides AL (Microsoft Dynamics 365 Business Central) language
//! support for the [tree-sitter] parsing library.
//!
//! Typical usage builds a [`tree_sitter::Parser`] with the [`LANGUAGE`] constant:
//!
//! ```
//! let mut parser = tree_sitter::Parser::new();
//! parser
//!     .set_language(&tree_sitter_al_bc::LANGUAGE.into())
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

/// The outline/symbol query for this grammar.
pub const OUTLINE_QUERY: &str = include_str!("../../queries/outline.scm");

/// The local scope and variable resolution query for this grammar.
pub const LOCALS_QUERY: &str = include_str!("../../queries/locals.scm");

/// The code-folding query for this grammar.
pub const FOLDS_QUERY: &str = include_str!("../../queries/folds.scm");

/// The indentation query for this grammar.
pub const INDENTS_QUERY: &str = include_str!("../../queries/indents.scm");

/// The bracket-matching query for this grammar.
pub const BRACKETS_QUERY: &str = include_str!("../../queries/brackets.scm");

/// The text-object query for this grammar.
pub const TEXTOBJECTS_QUERY: &str = include_str!("../../queries/textobjects.scm");

/// Generated AL language-data tables embedded for downstream consumers.
pub mod data {
    pub const KEYWORDS: &str = include_str!("../../data/keywords.json");
    pub const BUILTIN_FUNCTIONS: &str = include_str!("../../data/builtin_functions.json");
    pub const OBJECT_TYPES: &str = include_str!("../../data/object_types.json");
    pub const IMPLICIT_VARIABLES: &str = include_str!("../../data/implicit_variables.json");
    pub const PAGE_CONTROLS: &str = include_str!("../../data/page_controls.json");
    pub const SINGLE_STMT_OPENERS: &str = include_str!("../../data/single_stmt_openers.json");
    pub const RUNTIME_ENUMS: &str = include_str!("../../data/runtime_enums.json");
    pub const NAV_TYPE_KINDS: &str = include_str!("../../data/nav_type_kinds.json");
    pub const SYSTEM_OBJECTS: &str = include_str!("../../data/system_objects.json");
    pub const TOKEN_CLASSIFICATION: &str = include_str!("../../data/token_classification.json");
}

#[cfg(test)]
mod tests {
    use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator, Tree};

    /// One representative declaration per AL object kind, in the single-object
    /// form real `.al` files use. The last object in a file used to be the case
    /// that lost its `name:` field, so each of these is parsed on its own.
    const OBJECT_SOURCES: &[(&str, &str)] = &[
        ("table", "table 50100 MyName\n{\n}\n"),
        ("table quoted", "table 50100 \"My Name\"\n{\n}\n"),
        ("page", "page 50101 MyName\n{\n}\n"),
        ("report", "report 50102 MyName\n{\n}\n"),
        ("query", "query 50103 MyName\n{\n}\n"),
        ("xmlport", "xmlport 50104 MyName\n{\n}\n"),
        ("codeunit", "codeunit 50105 MyName\n{\n}\n"),
        ("enum", "enum 50106 MyName\n{\n}\n"),
        ("interface", "interface MyName\n{\n}\n"),
        ("permissionset", "permissionset 50107 MyName\n{\n}\n"),
        ("entitlement", "entitlement 50108 MyName\n{\n}\n"),
        ("controladdin", "controladdin MyName\n{\n}\n"),
        ("profile", "profile \"My Name\"\n{\n}\n"),
        (
            "tableextension",
            "tableextension 50109 MyName extends Customer\n{\n}\n",
        ),
        (
            "pageextension",
            "pageextension 50110 MyName extends \"Customer Card\"\n{\n}\n",
        ),
        (
            "reportextension",
            "reportextension 50111 MyName extends MyReport\n{\n}\n",
        ),
        (
            "enumextension",
            "enumextension 50112 MyName extends MyEnum\n{\n}\n",
        ),
        (
            "permissionsetextension",
            "permissionsetextension 50113 MyName extends MyPerm\n{\n}\n",
        ),
        (
            "profileextension",
            "profileextension MyName extends \"Base Profile\"\n{\n}\n",
        ),
        (
            "pagecustomization",
            "pagecustomization MyName customizes \"Customer Card\"\n{\n}\n",
        ),
    ];

    fn parse(source: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading AL parser");
        let tree = parser.parse(source, None).expect("parse tree");
        assert!(
            !tree.root_node().has_error(),
            "unexpected parse error in:\n{source}"
        );
        tree
    }

    fn query(source: &str) -> Query {
        Query::new(&super::LANGUAGE.into(), source).expect("query compiles")
    }

    /// Texts captured by `capture` when `query` runs over `source`.
    fn captured_texts<'a>(query_source: &str, capture: &str, source: &'a str) -> Vec<&'a str> {
        let tree = parse(source);
        let query = query(query_source);
        let index = query
            .capture_index_for_name(capture)
            .unwrap_or_else(|| panic!("query has no @{capture} capture"));

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        let mut out = Vec::new();
        while let Some(m) = matches.next() {
            for c in m.captures.iter().filter(|c| c.index == index) {
                out.push(&source[c.node.byte_range()]);
            }
        }
        out
    }

    fn find_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        let found = node
            .children(&mut cursor)
            .find_map(|child| find_kind(child, kind));
        found
    }

    #[test]
    fn can_load_grammar() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading AL parser");
    }

    #[test]
    fn every_object_kind_exposes_its_name_field() {
        for (label, source) in OBJECT_SOURCES {
            let tree = parse(source);
            let object = find_kind(tree.root_node(), "object_declaration")
                .unwrap_or_else(|| panic!("{label}: no object_declaration"));

            let name = object
                .child_by_field_name("name")
                .unwrap_or_else(|| panic!("{label}: object_declaration has no name field"));

            assert_eq!(name.kind(), "name_or_keyword", "{label}");
            let text = &source[name.byte_range()];
            assert!(
                text == "MyName" || text == "\"My Name\"",
                "{label}: name field covers {text:?}"
            );
            assert!(
                object.child_by_field_name("kind").is_some(),
                "{label}: object_declaration has no kind field"
            );
        }
    }

    #[test]
    fn outline_query_names_every_object_kind() {
        for (label, source) in OBJECT_SOURCES {
            let names = captured_texts(super::OUTLINE_QUERY, "name", source);
            assert!(
                names.iter().any(|n| *n == "MyName" || *n == "\"My Name\""),
                "{label}: outline captured {names:?}"
            );
        }
    }

    #[test]
    fn outline_query_covers_nested_declarations() {
        let source = r#"
codeunit 50100 MyCodeunit
{
    event procedure OnFoo(Value: Integer);

    procedure Ordinary()
    begin
    end;

    trigger OnRun()
    begin
    end;

    event OnLegacy(Value: Integer)
    begin
    end;
}

enum 50101 MyEnum
{
    value(0; Draft)
    {
    }
}
"#;
        let names = captured_texts(super::OUTLINE_QUERY, "name", source);
        for expected in [
            "MyCodeunit",
            "OnFoo",
            "Ordinary",
            "OnRun",
            "OnLegacy",
            "MyEnum",
            "Draft",
        ] {
            assert!(
                names.contains(&expected),
                "outline missing {expected}: {names:?}"
            );
        }
    }

    #[test]
    fn highlights_query_titles_object_names() {
        for (label, source) in OBJECT_SOURCES {
            let titles = captured_texts(super::HIGHLIGHTS_QUERY, "title", source);
            assert!(
                titles.iter().any(|t| *t == "MyName" || *t == "\"My Name\""),
                "{label}: @title captured {titles:?}"
            );
        }
    }

    #[test]
    fn locals_query_defines_the_object_type() {
        for (label, source) in OBJECT_SOURCES {
            let defs = captured_texts(super::LOCALS_QUERY, "local.definition.type", source);
            assert!(
                defs.iter().any(|d| *d == "MyName" || *d == "\"My Name\""),
                "{label}: local.definition.type captured {defs:?}"
            );
        }
    }

    #[test]
    fn every_shipped_query_compiles() {
        for (name, source) in [
            ("highlights", super::HIGHLIGHTS_QUERY),
            ("outline", super::OUTLINE_QUERY),
            ("locals", super::LOCALS_QUERY),
            ("folds", super::FOLDS_QUERY),
            ("indents", super::INDENTS_QUERY),
            ("brackets", super::BRACKETS_QUERY),
            ("textobjects", super::TEXTOBJECTS_QUERY),
        ] {
            Query::new(&super::LANGUAGE.into(), source)
                .unwrap_or_else(|e| panic!("{name}.scm does not compile: {e}"));
        }
    }

    /// An unterminated single-line literal must not swallow the following
    /// lines. CR-only line endings are covered too: excluding just LF would let
    /// the literal run to the end of a classic-Mac-style file.
    #[test]
    fn an_unterminated_literal_is_confined_to_its_line() {
        for (label, newline) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
            for broken in ["Message('oops);", "Rec.\"Oops := 1;"] {
                let source = [
                    "codeunit 50100 T",
                    "{",
                    "    procedure Broken()",
                    "    begin",
                    &format!("        {broken}"),
                    "    end;",
                    "",
                    "    procedure StillParses()",
                    "    begin",
                    "        Message('fine');",
                    "    end;",
                    "}",
                    "",
                ]
                .join(newline);

                let mut parser = Parser::new();
                parser
                    .set_language(&super::LANGUAGE.into())
                    .expect("Error loading AL parser");
                let tree = parser.parse(&source, None).expect("parse tree");

                let mut procedures = 0;
                let mut error_lines = 0;
                let mut stack = vec![tree.root_node()];
                while let Some(node) = stack.pop() {
                    if node.kind() == "procedure_declaration" {
                        procedures += 1;
                    }
                    if node.is_error() {
                        error_lines += node.end_position().row - node.start_position().row + 1;
                    }
                    let mut cursor = node.walk();
                    stack.extend(node.children(&mut cursor));
                }

                assert_eq!(
                    procedures, 2,
                    "{label}/{broken}: the following procedure must still parse"
                );
                assert!(
                    error_lines <= 2,
                    "{label}/{broken}: error spans {error_lines} lines"
                );
            }
        }
    }

    /// Attributes on global variables attach to the declaration they precede.
    ///
    /// This is what the scanner's zero-width `_var_attribute_marker` buys: at
    /// the `[` one token of lookahead cannot separate an attributed global from
    /// an attributed member, so the scanner reads past the balanced brackets and
    /// only marks the position when a `name:` declaration follows.
    #[test]
    fn attributes_attach_to_global_variables() {
        let source = r#"
page 50100 MyPage
{
    var
        Plain: Integer;
        [InDataSet]
        Visible: Boolean;
        [InDataSet]
        [Obsolete('gone', '24.0')]
        Multi: Boolean;
        [InDataSet]
        A, B : Boolean;
        [InDataSet]
        "Quoted Var": Boolean;

    [Test]
    procedure AfterSection()
    begin
    end;

    [TryFunction]
    local procedure Another()
    begin
    end;
}
"#;
        let tree = parse(source);
        let section =
            find_kind(tree.root_node(), "object_var_section").expect("global var section");

        let mut cursor = section.walk();
        let declarations: Vec<_> = section
            .children(&mut cursor)
            .filter(|c| c.kind() == "object_variable_declaration")
            .collect();
        assert_eq!(declarations.len(), 5, "every global stays in the section");

        let attribute_counts: Vec<usize> = declarations
            .iter()
            .map(|declaration| {
                let mut walker = declaration.walk();
                declaration
                    .children(&mut walker)
                    .filter(|c| c.kind() == "attribute")
                    .count()
            })
            .collect();
        assert_eq!(
            attribute_counts,
            vec![0, 1, 2, 1, 1],
            "attributes must be children of the declaration they precede"
        );

        // The section still ends at the attributed members that follow it, and
        // their attributes stay with them.
        let mut procedures = 0;
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if node.kind() == "procedure_declaration" {
                procedures += 1;
                assert!(
                    find_kind(node, "attribute").is_some(),
                    "the member kept its own attribute"
                );
            }
            let mut walker = node.walk();
            stack.extend(node.children(&mut walker));
        }
        assert_eq!(procedures, 2);
    }

    /// A table `keys { … }` block produces real `key_section`/`key_declaration`
    /// nodes, so the outline and fold captures for them are reachable.
    #[test]
    fn key_declarations_are_real_nodes() {
        let source = r#"
table 50100 MyTable
{
    fields
    {
        field(1; "No."; Code[20]) { }
    }

    keys
    {
        key(PK; "No.")
        {
            Clustered = true;
        }
        key(Key2; Name, Description)
        {
        }
    }
}
"#;
        let tree = parse(source);
        let section = find_kind(tree.root_node(), "key_section").expect("keys block");
        assert_eq!(
            section
                .child_by_field_name("keyword")
                .expect("keys keyword")
                .kind(),
            "kw_keys"
        );

        let body = section.child_by_field_name("body").expect("keys body");
        let mut cursor = body.walk();
        let keys: Vec<_> = body
            .children(&mut cursor)
            .filter(|c| c.kind() == "key_declaration")
            .collect();
        assert_eq!(keys.len(), 2);

        let names: Vec<&str> = keys
            .iter()
            .map(|k| {
                &source[k
                    .child_by_field_name("name")
                    .expect("key name")
                    .byte_range()]
            })
            .collect();
        assert_eq!(names, vec!["PK", "Key2"]);

        // Single- and multi-field key field lists, and key properties.
        let fields: Vec<&str> = keys
            .iter()
            .map(|k| {
                &source[k
                    .child_by_field_name("fields")
                    .expect("key fields")
                    .byte_range()]
            })
            .collect();
        assert_eq!(fields, vec!["\"No.\"", "Name, Description"]);
        assert!(find_kind(keys[0], "property_assignment").is_some());
    }

    #[test]
    fn outline_and_fold_queries_reach_key_declarations() {
        let source = r#"
table 50100 MyTable
{
    keys
    {
        key(PK; "No.") { }
        key(Key2; Name) { }
    }
}
"#;
        let names = captured_texts(super::OUTLINE_QUERY, "name", source);
        assert!(names.contains(&"PK"), "outline captured {names:?}");
        assert!(names.contains(&"Key2"), "outline captured {names:?}");

        let folds = captured_texts(super::FOLDS_QUERY, "fold", source);
        assert!(
            folds.iter().filter(|f| f.starts_with("key(")).count() == 2,
            "fold captured {folds:?}"
        );
    }

    /// `key`/`keys` only become dedicated tokens in front of `(`/`{`, so their
    /// ordinary metadata-keyword uses are untouched.
    #[test]
    fn key_keyword_is_not_hijacked_elsewhere() {
        let source = r#"
codeunit 50100 T
{
    var
        Keys: List of [Text];

    procedure P()
    var
        Key: Text;
    begin
        Keys := Dict.Keys();
        Key := Keys.Get(1);
    end;
}
"#;
        let tree = parse(source);
        assert!(
            find_kind(tree.root_node(), "key_section").is_none(),
            "no keys block here"
        );
        assert!(
            find_kind(tree.root_node(), "key_declaration").is_none(),
            "no key declaration here"
        );
    }

    /// A `directive` node has to span the whole `#...` line. It used to start
    /// after the directive name, which left region folding and directive
    /// highlighting with a node that omitted `#region`/`#pragma`.
    #[test]
    fn directive_nodes_span_the_whole_line() {
        let source = "#pragma warning disable AA0072\ncodeunit 50100 T\n{\n    #region Helpers\n    #endregion\n}\n";
        let tree = parse(source);

        let mut directives = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if node.kind() == "directive" {
                directives.push(&source[node.byte_range()]);
            }
            let mut cursor = node.walk();
            stack.extend(node.children(&mut cursor));
        }
        directives.sort();

        assert_eq!(
            directives,
            vec![
                "#endregion",
                "#pragma warning disable AA0072",
                "#region Helpers"
            ]
        );
    }

    #[test]
    fn folds_query_pairs_preprocessor_regions() {
        let source = "codeunit 50100 T\n{\n    #region Helpers\n    procedure P()\n    begin\n    end;\n    #endregion\n}\n";

        let starts = captured_texts(super::FOLDS_QUERY, "fold.region.start", source);
        let ends = captured_texts(super::FOLDS_QUERY, "fold.region.end", source);

        assert_eq!(starts, vec!["#region Helpers"]);
        assert_eq!(ends, vec!["#endregion"]);
    }

    /// The region patterns match on directive text, so they need a name
    /// boundary: `#regional` is not a region and must not open a fold.
    #[test]
    fn folds_query_ignores_directives_that_merely_start_with_region() {
        let source = "codeunit 50100 T\n{\n    #regional Something\n    #endregionExtra\n    #pragma warning disable AA0072\n}\n";

        assert!(captured_texts(super::FOLDS_QUERY, "fold.region.start", source).is_empty());
        assert!(captured_texts(super::FOLDS_QUERY, "fold.region.end", source).is_empty());
    }

    /// A global `var` section keeps every declaration that follows it, the way
    /// a local `var_section` does. It used to close after the first one and
    /// leave the rest as detached `variable_declaration` siblings.
    #[test]
    fn object_var_section_holds_every_global_declaration() {
        let source = r#"
codeunit 50100 T
{
    var
        GlobalRec: Record Customer;
        GlobalInt: Integer;
        GlobalText: Text[100];

    [Test]
    procedure P()
    begin
    end;
}
"#;
        let tree = parse(source);
        let section =
            find_kind(tree.root_node(), "object_var_section").expect("global var section");

        let mut cursor = section.walk();
        let declarations = section
            .children(&mut cursor)
            .filter(|c| c.kind() == "object_variable_declaration")
            .count();

        assert_eq!(
            declarations,
            3,
            "section holds {:?}",
            &source[section.byte_range()]
        );
        // The section must stop at the attributed procedure that follows it.
        assert!(find_kind(tree.root_node(), "procedure_declaration").is_some());
    }

    /// Every bracket pattern must contribute both halves of a pair. The Zed
    /// language package used to ship an `(if_statement (kw_if) @open)` pattern
    /// with no `@close`, and no `{`/`}` pair at all.
    #[test]
    fn bracket_pairs_are_balanced() {
        let source = r#"
codeunit 50100 Brackets
{
    procedure P(Values: List of [Integer])
    var
        Index: Integer;
    begin
        case Index of
            1:
                begin
                    while Index > 0 do
                        Index -= 1;
                    for Index := 1 to 2 do
                        Index += 0;
                    foreach Index in Values do
                        Index += 0;
                    repeat
                        Index += 1;
                    until Index > 3;
                end;
        end;
    end;
}
"#;
        let opens = captured_texts(super::BRACKETS_QUERY, "open", source);
        let closes = captured_texts(super::BRACKETS_QUERY, "close", source);
        assert_eq!(
            opens.len(),
            closes.len(),
            "every @open needs a matching @close\nopen: {opens:?}\nclose: {closes:?}"
        );
        for expected in [
            "{", "(", "[", "begin", "case", "while", "for", "foreach", "repeat",
        ] {
            assert!(
                opens.iter().any(|o| o.eq_ignore_ascii_case(expected)),
                "brackets query never opened {expected}: {opens:?}"
            );
        }
    }

    #[test]
    fn complete_local_procedure_is_not_claimed_by_legacy_recovery() {
        let source = r#"
codeunit 50100 "Procedure Boundary"
{
    local procedure Empty()
    begin
    end;

    local procedure Successor()
    begin
        Message('still a sibling');
    end;
}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading AL parser");
        let tree = parser.parse(source, None).expect("parse tree");
        assert!(!tree.root_node().has_error(), "{:?}", tree.root_node());

        let mut procedures = 0;
        let mut recoveries = 0;
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            match node.kind() {
                "procedure_declaration" => procedures += 1,
                "legacy_local_incomplete_procedure_pair" => recoveries += 1,
                _ => {}
            }
            let mut cursor = node.walk();
            stack.extend(node.children(&mut cursor));
        }

        assert_eq!(procedures, 2, "both complete procedures must stay siblings");
        assert_eq!(
            recoveries, 0,
            "bounded recovery must not claim valid begin/end syntax"
        );
    }
}
