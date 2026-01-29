; AL highlights for Zed/tree-sitter
; AUTO-GENERATED - DO NOT EDIT (copied by AL Tree Sitter)

(comment) @comment
(string) @string
(integer) @number

(control_keyword) @keyword.control

; Specific keyword highlights
(kw_option) @type.builtin
(kw_record) @type.builtin
(kw_page) @keyword.storage.type
(kw_table) @keyword.storage.type
(kw_codeunit) @keyword.storage.type
(kw_report) @keyword.storage.type
(kw_xmlport) @keyword.storage.type
(kw_enum) @keyword.storage.type
(kw_query) @keyword.storage.type
(kw_interface) @keyword.storage.type
(kw_tableextension) @keyword.storage.type
(kw_pageextension) @keyword.storage.type
(kw_enumextension) @keyword.storage.type
(kw_permissionset) @keyword.storage.type
(kw_permissionsetextension) @keyword.storage.type

(kw_array) @keyword.control
(kw_asserterror) @keyword.control
(kw_begin) @keyword.control
(kw_break) @keyword.control
(kw_case) @keyword.control
(kw_codeunit) @keyword.control
(kw_continue) @keyword.control
(kw_do) @keyword.control
(kw_downto) @keyword.control
(kw_else) @keyword.control
(kw_end) @keyword.control
(kw_enum) @keyword.control
(kw_enumextension) @keyword.control
(kw_event) @keyword.control
(kw_exit) @keyword.control
(kw_for) @keyword.control
(kw_foreach) @keyword.control
(kw_function) @keyword.control
(kw_if) @keyword.control
(kw_in) @keyword.control
(kw_indataset) @keyword.control
(kw_interface) @keyword.control
(kw_internal) @keyword.control
(kw_local) @keyword.control
(kw_of) @keyword.control
(kw_option) @keyword.control
(kw_page) @keyword.control
(kw_pageextension) @keyword.control
(kw_permissionset) @keyword.control
(kw_permissionsetextension) @keyword.control
(kw_procedure) @keyword.control
(kw_program) @keyword.control
(kw_protected) @keyword.control
(kw_query) @keyword.control
(kw_record) @keyword.control
(kw_repeat) @keyword.control
(kw_report) @keyword.control
(kw_runonclient) @keyword.control
(kw_securityfiltering) @keyword.control
(kw_suppressdispose) @keyword.control
(kw_table) @keyword.control
(kw_tableextension) @keyword.control
(kw_temporary) @keyword.control
(kw_then) @keyword.control
(kw_to) @keyword.control
(kw_trigger) @keyword.control
(kw_until) @keyword.control
(kw_var) @keyword.control
(kw_while) @keyword.control
(kw_with) @keyword.control
(kw_withevents) @keyword.control
(kw_xmlport) @keyword.control


(operator_word) @operator
(object_keyword) @keyword
(type_keyword) @type
(metadata_keyword) @keyword.directive
(property_keyword) @property
(keyword) @keyword

; --- Punctuation & Operators ---
(operator) @operator
(semicolon) @punctuation.delimiter
(comma) @punctuation.delimiter
["(" ")" "[" "]" "{" "}"] @punctuation.bracket

; --- Names & Calls ---
; Function calls
(call_suffix) @function.call
(member_call_suffix member: (_) @function.call)
(scope_call_suffix member: (_) @function.call)

; Definitions
(procedure_declaration name: (_) @function.method)
(trigger_declaration name: (_) @function.method)
(event_declaration name: (_) @function.method)

(identifier) @variable
