// AL Tree-Sitter Grammar - Keyword-driven & tolerant
// Edit this file then run: cargo run

module.exports = grammar({
  name: 'al',

  externals: $ => [
    // Keyword categories come from the Cursor/VSCode AL extension's TextMate grammar
    // (syntaxes/alsyntax.tmlanguage). No keyword strings are hardcoded here.
    $.keyword,
    $.control_keyword,
    $.object_keyword,
    $.type_keyword,
    $.metadata_keyword,
    $.property_keyword,
  ],

  extras: $ => [
    /\s/,
    $.comment,
    $.directive,
  ],

  word: $ => $.identifier,

  rules: {
    source_file: $ => repeat($._item),

    _item: $ => choice(
      $.object_declaration,
      $.braced_block,
      $.parenthesized_block,
      $.bracketed_block,
      $._atom,
    ),

    // Object declarations are the most common top-level AL construct.
    // We intentionally keep this tolerant while still using a keyword-derived object kind.
    object_declaration: $ => prec(1, seq(
      field('kind', $.object_keyword),
      field('id', optional($.integer)),
      field('name', $.name),
      // Conditional compilation in AL often repeats/varies the object header (e.g. #if/#else enum ...).
      // We stay tolerant and consume header-related tokens until the opening '{'.
      repeat($._pre_object_body),
      field('body', $.braced_block),
    )),

    // e.g. pageextension 50100 MyExt extends "Customer Card" { ... }
    object_modifier: $ => seq(
      field('modifier', $.metadata_keyword),
      field('target', $.name),
    ),

    _pre_object_body: $ => choice(
      $.object_modifier,
      // Allow directives between header pieces
      $.directive,
      // Header can include things like `implements (...)` or attribute lists
      $.parenthesized_block,
      $.bracketed_block,
      // Anything else *except* a bare metadata_keyword (to avoid conflicts with object_modifier)
      $.string,
      $.verbatim_string,
      $.quoted_identifier,
      $.integer,
      $.identifier,
      $.control_keyword,
      $.object_keyword,
      $.type_keyword,
      $.property_keyword,
      $.keyword,
      $.operator,
      $.punctuation,
    ),

    braced_block: $ => prec.right(seq(
      '{',
      repeat(choice(
        $.braced_block,
        $.parenthesized_block,
        $.bracketed_block,
        $._atom,
      )),
      '}',
    )),

    parenthesized_block: $ => prec.right(seq(
      '(',
      repeat(choice(
        $.braced_block,
        $.parenthesized_block,
        $.bracketed_block,
        $._atom,
      )),
      ')',
    )),

    bracketed_block: $ => prec.right(seq(
      '[',
      repeat(choice(
        $.braced_block,
        $.parenthesized_block,
        $.bracketed_block,
        $._atom,
      )),
      ']',
    )),

    name: $ => choice(
      $.identifier,
      $.quoted_identifier,
    ),

    _atom: $ => choice(
      $.string,
      $.verbatim_string,
      $.quoted_identifier,
      $.integer,
      $.identifier,

      // Keyword categories (from external scanner)
      $.control_keyword,
      $.object_keyword,
      $.type_keyword,
      $.metadata_keyword,
      $.property_keyword,
      $.keyword,

      $.operator,
      $.punctuation,
    ),

    // AL identifiers can include non-ASCII letters in practice (e.g. demo datasets).
    // Keep keywords ASCII-only via the external scanner, but accept Unicode in identifiers for robustness.
    identifier: _ => /[A-Za-z_\u0080-\uFFFF][A-Za-z0-9_\u0080-\uFFFF]*/,
    integer: _ => /[0-9]+/,

    // Strings: AL uses single quotes; escaping is done by doubling ''.
    // Use `token(...)` to keep lexing robust in free-form contexts.
    string: _ => token(seq("'", repeat(choice(/[^']/, "''")), "'")),
    verbatim_string: _ => token(seq("@'", repeat(choice(/[^']/, "''")), "'")),

    // Quoted identifiers: "My Field"; escaping is done by doubling "".
    quoted_identifier: _ => token(seq('"', repeat(choice(/[^"]/, '""')), '"')),

    // Directives are line-oriented and start with '#'
    directive: _ => token(seq('#', /[^\n]*/)),

    comment: _ => token(choice(
      seq('//', /[^\n]*/),
      seq(
        '/*',
        /[^*]*\*+([^/*][^*]*\*+)*/,
        '/'
      ),
    )),

    // Catch most operator/symbol runs (including % placeholders in label strings).
    operator: _ => token(/[!$%&*+\-./:<=>?@^|~]+/),

    punctuation: _ => token(choice(
      ';',
      ',',
    )),
  }
});
