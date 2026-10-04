//! Runs the shipped queries over every entry in `test/corpus/` and checks that
//! each shape is captured the way its older equivalent is: a quoted or keyword
//! name like an identifier, a signed case label like an unsigned one, a List
//! element type like a plain type. Every failure names the corpus file, the
//! entry, the node kind, its parent and its text.
//!
//! Highlights are resolved the way Zed resolves them. Zed pushes each capture
//! tree-sitter yields onto a stack and paints a position with the last pushed
//! capture that covers it. tree-sitter yields a capture on a wrapper node
//! before the capture on the leaf inside it, so the leaf's own capture is the
//! one Zed shows. A capture on a wrapper node only shows on leaves inside it
//! that have no capture of their own.
//!
//! Entries whose tree has an error (`recovery.txt`) are skipped, since the
//! leaves inside an `ERROR` node have no role to check.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator, Tree};

struct Entry {
    file: String,
    name: String,
    source: String,
}

struct Parsed {
    entry: Entry,
    tree: Tree,
}

/// Every corpus entry without a parse error, in file order.
fn corpus() -> Vec<Parsed> {
    let dir = format!("{}/test/corpus", env!("CARGO_MANIFEST_DIR"));
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {dir}: {e}"))
        .map(|entry| entry.expect("corpus directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort();

    let mut parser = Parser::new();
    parser
        .set_language(&super::LANGUAGE.into())
        .expect("Error loading AL parser");

    let mut out = Vec::new();
    for path in paths {
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("corpus file name")
            .to_string();
        let text = std::fs::read_to_string(&path).expect("corpus file");
        for entry in split_entries(&file, &text) {
            let tree = parser.parse(&entry.source, None).expect("parse tree");
            if !tree.root_node().has_error() {
                out.push(Parsed { entry, tree });
            }
        }
    }
    assert!(out.len() > 100, "only {} corpus entries parsed", out.len());
    out
}

/// Splits a corpus file into entries: a `===` line, the name, a `===` line,
/// optional `:attribute` lines, the input, then a `---` line.
fn split_entries(file: &str, text: &str) -> Vec<Entry> {
    let lines: Vec<&str> = text.lines().collect();
    let mut entries = Vec::new();
    let mut i = 0;
    while i + 2 < lines.len() {
        if !(lines[i].starts_with("===") && lines[i + 2].starts_with("===")) {
            i += 1;
            continue;
        }
        let name = lines[i + 1].trim().to_string();
        let mut j = i + 3;
        let mut skip = false;
        while j < lines.len() && lines[j].starts_with(':') {
            skip |= lines[j].starts_with(":skip") || lines[j].starts_with(":error");
            j += 1;
        }
        let start = j;
        while j < lines.len() && !lines[j].starts_with("---") {
            j += 1;
        }
        if !skip {
            entries.push(Entry {
                file: file.to_string(),
                name,
                source: lines[start..j].join("\n"),
            });
        }
        i = j;
    }
    entries
}

fn query(source: &str) -> Query {
    Query::new(&super::LANGUAGE.into(), source).expect("query compiles")
}

/// The captures of `query` over `parsed` as (node, capture name), in the order
/// tree-sitter yields them.
fn captures<'t>(query: &'t Query, parsed: &'t Parsed) -> Vec<(Node<'t>, &'t str)> {
    let mut cursor = QueryCursor::new();
    let mut iter = cursor.captures(
        query,
        parsed.tree.root_node(),
        parsed.entry.source.as_bytes(),
    );
    let mut out = Vec::new();
    while let Some((m, index)) = iter.next() {
        let capture = m.captures[*index];
        let name = query.capture_names()[capture.index as usize];
        // A capture named with a leading underscore only feeds a predicate.
        // Zed draws nothing for it.
        if name.starts_with('_') {
            continue;
        }
        out.push((capture.node, name));
    }
    out
}

fn nodes<'t>(parsed: &'t Parsed) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack = vec![parsed.tree.root_node()];
    while let Some(node) = stack.pop() {
        out.push(node);
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    out
}

fn leaves<'t>(parsed: &'t Parsed) -> Vec<Node<'t>> {
    nodes(parsed)
        .into_iter()
        .filter(|node| node.child_count() == 0 && node.end_byte() > node.start_byte())
        .collect()
}

fn covers(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// The highlight Zed shows for `leaf`: the last capture yielded that covers it.
fn highlight<'q>(captures: &[(Node, &'q str)], leaf: Node) -> Option<&'q str> {
    let range = leaf.byte_range();
    captures
        .iter()
        .rev()
        .find(|(node, _)| covers(&node.byte_range(), &range))
        .map(|(_, name)| *name)
}

fn text<'s>(parsed: &'s Parsed, node: Node) -> &'s str {
    &parsed.entry.source[node.byte_range()]
}

fn field_name(node: Node) -> Option<&'static str> {
    let parent = node.parent()?;
    let mut cursor = parent.walk();
    for (index, child) in parent.children(&mut cursor).enumerate() {
        if child.id() == node.id() {
            return parent.field_name_for_child(index as u32);
        }
    }
    None
}

/// `file "entry" row:col kind in parent 'text'`, with a one-based row.
fn locate(parsed: &Parsed, node: Node) -> String {
    let parent = node.parent().map(|p| p.kind()).unwrap_or("-");
    let field = field_name(node)
        .map(|f| format!(" field {f}"))
        .unwrap_or_default();
    let position = node.start_position();
    let snippet: String = text(parsed, node).chars().take(50).collect();
    format!(
        "{} \"{}\" {}:{} {} in {}{} {:?}",
        parsed.entry.file,
        parsed.entry.name,
        position.row + 1,
        position.column + 1,
        node.kind(),
        parent,
        field,
        snippet
    )
}

fn assert_no_failures(what: &str, failures: Vec<String>) {
    if !failures.is_empty() {
        panic!(
            "{what}: {} failure(s)\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

/// Leaf kinds that can spell a name: an identifier, a quoted identifier, or a
/// word the scanner classifies as a keyword where a name is also valid.
const NAME_KINDS: &[&str] = &[
    "identifier",
    "quoted_identifier",
    "keyword",
    "kw_function",
    "metadata_keyword",
    "object_keyword",
    "property_keyword",
    "type_keyword",
    "control_keyword",
];

fn is_name_kind(node: Node) -> bool {
    NAME_KINDS.contains(&node.kind())
}

/// The node a name stands for: a leaf inside `name` or `name_or_keyword`
/// stands for that wrapper.
fn name_wrapper(leaf: Node) -> Node {
    let mut node = leaf;
    if let Some(parent) = node.parent().filter(|p| p.kind() == "name") {
        node = parent;
    }
    if let Some(parent) = node.parent().filter(|p| p.kind() == "name_or_keyword") {
        node = parent;
    }
    node
}

/// The position a name holds, such as `parameter.name` or
/// `primary_expression before index_suffix`. Names with no field in their
/// parent and outside an expression have no role and are skipped.
fn name_role(leaf: Node) -> Option<String> {
    let wrapper = name_wrapper(leaf);
    let parent = wrapper.parent()?;
    if parent.kind() == "primary_expression" {
        let next = parent
            .next_named_sibling()
            .map(|s| s.kind())
            .unwrap_or("nothing");
        return Some(format!("primary_expression before {next}"));
    }
    let field = field_name(wrapper)?;
    Some(format!("{}.{}", parent.kind(), field))
}

/// Roles whose capture depends on spelling on purpose.
///
/// - Before a member or scope suffix a keyword kind may be the object itself
///   (`Page.RunModal(...)`, `Codeunit::"Sales-Post"`) or a variable named after
///   it (`Page.ToUpper`). Only a symbol table tells them apart, so the keyword
///   capture stays.
/// - A property value that is a quoted name is an object reference
///   (`TableRelation = "Sales Header"`) and a bare name is an option value
///   (`DataClassification = CustomerContent`).
/// - An object section header keyword (`field`, `area`, `addafter`) is a
///   keyword whatever kind the scanner gives it.
const SPELLING_ROLES: &[&str] = &[
    "primary_expression before member_call_suffix",
    "primary_expression before member_suffix",
    "primary_expression before scope_suffix",
    "primary_expression before scope_call_suffix",
    "property_assignment.value",
    "object_section.keyword",
    // What precedes `::` decides: an option or enum member after a type
    // (`Status::Released`), an object after an object kind (`Codeunit::"X"`).
    "scope_suffix.member",
    // A built-in call (`Rec.SetRange`, `Error`, `Report.Run`) is a built-in
    // function, any other call a function. The name or the receiver decides.
    "member_call_suffix.member",
    "primary_expression before call_suffix",
];

fn is_boolean_literal(parsed: &Parsed, leaf: Node) -> bool {
    leaf.kind() == "identifier"
        && ["true", "false"].contains(&text(parsed, leaf).to_ascii_lowercase().as_str())
}

#[test]
fn every_corpus_leaf_has_a_highlight() {
    let highlights = query(super::HIGHLIGHTS_QUERY);
    let mut failures = Vec::new();
    for parsed in &corpus() {
        let captures = captures(&highlights, parsed);
        for leaf in leaves(parsed) {
            if highlight(&captures, leaf).is_none() {
                failures.push(locate(parsed, leaf));
            }
        }
    }
    assert_no_failures("leaves with no highlight capture", failures);
}

#[test]
fn highlight_captures_land_on_leaves() {
    let highlights = query(super::HIGHLIGHTS_QUERY);
    let mut failures = Vec::new();
    for parsed in &corpus() {
        for (node, name) in captures(&highlights, parsed) {
            // A documentation comment's parts have their own captures, and the
            // comment's capture colors the spaces between them.
            if node.child_count() > 0 && node.kind() != "comment" {
                failures.push(format!("@{name} on {}", locate(parsed, node)));
            }
        }
    }
    assert_no_failures(
        "highlight captures on wrapper nodes, which the leaf capture inside hides in Zed",
        failures,
    );
}

#[test]
fn a_name_is_highlighted_by_its_role_and_not_its_spelling() {
    let highlights = query(super::HIGHLIGHTS_QUERY);
    // role -> capture -> first example
    let mut seen: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for parsed in &corpus() {
        let captures = captures(&highlights, parsed);
        for leaf in leaves(parsed) {
            if !is_name_kind(leaf) || is_boolean_literal(parsed, leaf) {
                continue;
            }
            let Some(role) = name_role(leaf) else {
                continue;
            };
            if SPELLING_ROLES.contains(&role.as_str()) {
                continue;
            }
            let capture = highlight(&captures, leaf).unwrap_or("none").to_string();
            seen.entry(role)
                .or_default()
                .entry(capture)
                .or_insert_with(|| locate(parsed, leaf));
        }
    }
    let failures = seen
        .into_iter()
        .filter(|(_, by_capture)| by_capture.len() > 1)
        .map(|(role, by_capture)| {
            let examples: Vec<_> = by_capture
                .into_iter()
                .map(|(capture, example)| format!("@{capture}: {example}"))
                .collect();
            format!("{role}\n    {}", examples.join("\n    "))
        })
        .collect();
    assert_no_failures("names in one role with different highlights", failures);
}

/// Leaves inside a type that are not part of the type name: brackets,
/// separators, the dots of a namespace, lengths and array sizes, the word
/// `of`, and `temporary`.
fn is_type_syntax(parsed: &Parsed, leaf: Node) -> bool {
    matches!(
        leaf.kind(),
        "[" | "]" | "(" | ")" | "." | "comma" | "integer" | "kw_of" | "kw_temporary"
    ) || text(parsed, leaf).eq_ignore_ascii_case("of")
}

#[test]
fn type_names_are_highlighted_as_types() {
    let highlights = query(super::HIGHLIGHTS_QUERY);
    let mut failures = Vec::new();
    for parsed in &corpus() {
        let captures = captures(&highlights, parsed);
        for leaf in leaves(parsed) {
            let mut in_type = false;
            let mut in_option = false;
            let mut ancestor = leaf.parent();
            while let Some(node) = ancestor {
                match node.kind() {
                    "option_type" => in_option = true,
                    "type_reference" => {
                        in_type = true;
                        break;
                    }
                    _ => {}
                }
                ancestor = node.parent();
            }
            // Option member names are values, not types.
            if !in_type || in_option || is_type_syntax(parsed, leaf) {
                continue;
            }
            // A builtin type keyword (`Integer`, `Record`) is type.builtin.al,
            // which BC themes draw in the keyword color. A type's name is
            // type.builtin.
            let capture = highlight(&captures, leaf);
            if !matches!(capture, Some("type.builtin" | "type.builtin.al")) {
                failures.push(format!(
                    "@{} {}",
                    capture.unwrap_or("none"),
                    locate(parsed, leaf)
                ));
            }
        }
    }
    assert_no_failures("type names without the type capture", failures);
}

/// Shapes with one expected highlight: every leaf in the entry whose text is
/// `text` has `capture`.
const EXPECTED_HIGHLIGHTS: &[(&str, &str, &str, &str)] = &[
    // A sign is an operator, and a signed case label is one token that reads
    // as a number when it is one.
    (SIGNS, SIGN_ENTRY, "-", "operator"),
    (CASE_LABELS, SIGNED_LABELS, "-1", "number"),
    (CASE_LABELS, SIGNED_LABELS, "- 2", "number"),
    (
        CASE_LABELS,
        SIGNED_LABELS,
        "-Level::Gold.AsInteger()",
        "variable",
    ),
    (STATEMENTS, SIGNED_NAME_LABEL, "-Limit", "variable"),
    // The `of` inside a nested List or Dictionary type stays a keyword.
    (COLLECTIONS, ELEMENT_TYPES, "of", "keyword.control"),
    // Trigger and property names show their own capture, not the identifier's.
    (DECLARATIONS_FILE, TRIGGERS, "OnRun", "function"),
    (DECLARATIONS_FILE, KEYS, "Clustered", "property"),
    // Key sections and layout move directives are keywords.
    (DECLARATIONS_FILE, KEYS, "keys", "keyword"),
    (DECLARATIONS_FILE, KEYS, "key", "keyword"),
    (EXTENSIONS, MOVES, "movefirst", "keyword"),
    (EXTENSIONS, MOVES, "moveafter", "keyword"),
    (EXTENSIONS, MOVES, "movebefore", "keyword"),
    (EXTENSIONS, MOVES, "movelast", "keyword"),
];

const SIGNS: &str = "sign_operators.txt";
const SIGN_ENTRY: &str = "sign after an assignment, a multiplication and a subtraction";
const CASE_LABELS: &str = "case_labels.txt";
const SIGNED_LABELS: &str = "signed case labels, negative ranges and a subtraction ending an arm";
const STATEMENTS: &str = "statements.txt";
const SIGNED_NAME_LABEL: &str = "case labels with a leading minus on a number and a name";
const COLLECTIONS: &str = "collections.txt";
const ELEMENT_TYPES: &str = "element types of nested List, Dictionary and array types";
const DECLARATIONS_FILE: &str = "declarations.txt";
const TRIGGERS: &str = "triggers and events";
const KEYS: &str = "table keys with single-field, multi-field and quoted keys";
const EXTENSIONS: &str = "object_extensions.txt";
const MOVES: &str = "pageextension layout with add, modify and move directives";

#[test]
fn listed_shapes_have_their_highlight() {
    let highlights = query(super::HIGHLIGHTS_QUERY);
    let corpus = corpus();
    let mut failures = Vec::new();
    for &(file, entry, wanted_text, wanted) in EXPECTED_HIGHLIGHTS {
        let Some(parsed) = corpus
            .iter()
            .find(|p| p.entry.file == file && p.entry.name == entry)
        else {
            failures.push(format!("no corpus entry {file} \"{entry}\""));
            continue;
        };
        let captures = captures(&highlights, parsed);
        let matching: Vec<_> = leaves(parsed)
            .into_iter()
            .filter(|leaf| text(parsed, *leaf) == wanted_text)
            .collect();
        if matching.is_empty() {
            failures.push(format!("no leaf {wanted_text:?} in {file} \"{entry}\""));
        }
        for leaf in matching {
            let capture = highlight(&captures, leaf);
            if capture != Some(wanted) {
                failures.push(format!(
                    "@{} where @{wanted} was expected: {}",
                    capture.unwrap_or("none"),
                    locate(parsed, leaf)
                ));
            }
        }
    }
    assert_no_failures("expected highlights", failures);
}

/// For each query, the captures that tag a whole construct. A node kind that
/// gets one of them in one place must get it everywhere.
const CONSTRUCT_CAPTURES: &[(&str, &str, &[&str])] = &[
    ("folds", super::FOLDS_QUERY, &["fold"]),
    ("indents", super::INDENTS_QUERY, &["indent"]),
    ("outline", super::OUTLINE_QUERY, &["item"]),
    (
        "textobjects",
        super::TEXTOBJECTS_QUERY,
        &["function.around", "class.around", "comment.around"],
    ),
    ("locals", super::LOCALS_QUERY, &["local.scope"]),
];

#[test]
fn a_construct_is_captured_in_every_context() {
    let corpus = corpus();
    let mut failures = Vec::new();
    for &(query_name, source, construct_captures) in CONSTRUCT_CAPTURES {
        let query = query(source);
        // (capture, kind) -> captured somewhere
        let mut captured_kinds: BTreeSet<(&str, &str)> = BTreeSet::new();
        // per entry: (capture, start byte, end byte) captured
        let mut per_entry: Vec<BTreeSet<(&str, usize, usize)>> = Vec::new();
        for parsed in &corpus {
            let mut spans = BTreeSet::new();
            for (node, name) in captures(&query, parsed) {
                if let Some(&capture) = construct_captures.iter().find(|c| **c == name) {
                    captured_kinds.insert((capture, node.kind()));
                    spans.insert((capture, node.start_byte(), node.end_byte()));
                }
            }
            per_entry.push(spans);
        }
        for (parsed, spans) in corpus.iter().zip(&per_entry) {
            for node in nodes(parsed) {
                for &capture in construct_captures {
                    // A node counts as captured when a node with the same span
                    // is, as for a procedure_declaration that only wraps an
                    // event_procedure_declaration.
                    if captured_kinds.contains(&(capture, node.kind()))
                        && !spans.contains(&(capture, node.start_byte(), node.end_byte()))
                    {
                        failures.push(format!(
                            "{query_name} @{capture} missing on {}",
                            locate(parsed, node)
                        ));
                    }
                }
            }
        }
    }
    assert_no_failures(
        "constructs captured in one context and not another",
        failures,
    );
}

#[test]
fn every_outline_item_has_a_name() {
    let outline = query(super::OUTLINE_QUERY);
    let mut failures = Vec::new();
    for parsed in &corpus() {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(
            &outline,
            parsed.tree.root_node(),
            parsed.entry.source.as_bytes(),
        );
        while let Some(m) = matches.next() {
            let names = |wanted: &str| {
                m.captures
                    .iter()
                    .filter(|c| outline.capture_names()[c.index as usize] == wanted)
                    .map(|c| c.node)
                    .collect::<Vec<_>>()
            };
            for item in names("item") {
                if names("name")
                    .iter()
                    .all(|n| text(parsed, *n).trim().is_empty())
                {
                    failures.push(locate(parsed, item));
                }
            }
        }
    }
    assert_no_failures("outline items without a name", failures);
}

/// Declarations whose `name` field is a local definition.
const DECLARATIONS: &[&str] = &[
    "object_declaration",
    "procedure_declaration",
    "trigger_declaration",
    "event_declaration",
    "regular_variable_declaration",
    "label_declaration",
    "parameter",
];

#[test]
fn every_declared_name_is_a_local_definition() {
    let locals = query(super::LOCALS_QUERY);
    let mut failures = Vec::new();
    for parsed in &corpus() {
        let definitions: Vec<Range<usize>> = captures(&locals, parsed)
            .into_iter()
            .filter(|(_, name)| name.starts_with("local.definition."))
            .map(|(node, _)| node.byte_range())
            .collect();
        for node in nodes(parsed) {
            if !DECLARATIONS.contains(&node.kind()) {
                continue;
            }
            let Some(name) = node.child_by_field_name("name") else {
                continue;
            };
            if !definitions.iter().any(|d| covers(&name.byte_range(), d)) {
                failures.push(locate(parsed, name));
            }
        }
    }
    assert_no_failures("declared names that locals.scm does not define", failures);
}

#[test]
fn a_variable_highlight_is_a_local_reference_or_definition() {
    let highlights = query(super::HIGHLIGHTS_QUERY);
    let locals = query(super::LOCALS_QUERY);
    let mut failures = Vec::new();
    for parsed in &corpus() {
        let highlight_captures = captures(&highlights, parsed);
        let local_ranges: Vec<Range<usize>> = captures(&locals, parsed)
            .into_iter()
            .filter(|(_, name)| *name == "local.reference" || name.starts_with("local.definition."))
            .map(|(node, _)| node.byte_range())
            .collect();
        for leaf in leaves(parsed) {
            if !is_name_kind(leaf) {
                continue;
            }
            let is_variable = highlight(&highlight_captures, leaf)
                .is_some_and(|name| name.starts_with("variable"));
            if is_variable && !local_ranges.contains(&leaf.byte_range()) {
                failures.push(locate(parsed, leaf));
            }
        }
    }
    assert_no_failures(
        "names highlighted as variables that locals.scm does not reference",
        failures,
    );
}

/// A multi-line attribute is the first child of the declaration it
/// decorates, so the declaration's fold starts where the attribute starts. A
/// fold on the attribute would start on the same line as that fold, which is
/// why `folds.scm` has no `(attribute)` entry. This fails if the grammar moves
/// attributes out of their declaration or the attribute gets a fold of its own.
#[test]
fn a_multi_line_attribute_starts_the_fold_of_its_declaration() {
    let folds = query(super::FOLDS_QUERY);
    let mut multi_line = 0;
    let mut failures = Vec::new();
    for parsed in &corpus() {
        let folded: Vec<Range<usize>> = captures(&folds, parsed)
            .into_iter()
            .filter(|(_, name)| *name == "fold")
            .map(|(node, _)| node.byte_range())
            .collect();
        for node in nodes(parsed) {
            if node.kind() != "attribute" || node.start_position().row == node.end_position().row {
                continue;
            }
            multi_line += 1;
            if folded.contains(&node.byte_range()) {
                failures.push(format!(
                    "the attribute folds on its own: {}",
                    locate(parsed, node)
                ));
            }
            let declaration_fold = node.parent().filter(|declaration| {
                declaration.start_byte() == node.start_byte()
                    && folded.contains(&declaration.byte_range())
            });
            if declaration_fold.is_none() {
                failures.push(format!(
                    "no fold of the declaration starts at the attribute: {}",
                    locate(parsed, node)
                ));
            }
        }
    }
    assert!(
        multi_line > 0,
        "the corpus has no attribute that spans several lines"
    );
    assert_no_failures("multi-line attributes", failures);
}
