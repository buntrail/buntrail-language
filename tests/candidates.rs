//! Smoke probes, not the full independently reviewed release expectations.
mod support;
use tree_sitter::{InputEdit, Language, Parser, Point, Query};
fn point(text: &str, byte: usize) -> Point {
    let prefix = &text[..byte];
    Point::new(
        prefix.bytes().filter(|b| *b == b'\n').count(),
        prefix.rsplit('\n').next().unwrap().len(),
    )
}
fn probe(language: Language, query: &str, fixture: &str, expected: &str) {
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let query = Query::new(&language, query).unwrap();
    assert!(!query.capture_names().is_empty());
    let original = std::fs::read_to_string(support::fixture_path(fixture)).unwrap();
    let mut tree = parser.parse(&original, None).unwrap();
    assert!(
        !tree.root_node().has_error(),
        "{fixture}: {}",
        tree.root_node().to_sexp()
    );
    assert!(
        tree.root_node().to_sexp().contains(expected),
        "{fixture} missing {expected}"
    );
    let start = original.find("hello").unwrap();
    let changed = original.replacen("hello", "😀e\u{301}日本", 1);
    let end = start + "😀e\u{301}日本".len();
    tree.edit(&InputEdit {
        start_byte: start,
        old_end_byte: start + 5,
        new_end_byte: end,
        start_position: point(&original, start),
        old_end_position: point(&original, start + 5),
        new_end_position: point(&changed, end),
    });
    let incremental = parser.parse(&changed, Some(&tree)).unwrap();
    let fresh = parser.parse(&changed, None).unwrap();
    assert_eq!(
        incremental.root_node().to_sexp(),
        fresh.root_node().to_sexp(),
        "{fixture}"
    );
    assert!(
        !fresh.root_node().has_error(),
        "{fixture} Unicode replacement"
    );
    println!(
        "PASS probe {fixture} ABI {} query compiled + expected node + Unicode incremental/fresh",
        language.abi_version()
    );
}
macro_rules! case {
    ($name:ident,$lang:expr,$query:expr,$file:literal,$node:literal) => {
        #[test]
        fn $name() {
            probe($lang, $query, $file, $node);
        }
    };
}
case!(
    javascript,
    tree_sitter_javascript::LANGUAGE.into(),
    tree_sitter_javascript::HIGHLIGHT_QUERY,
    "Module.js",
    "function_declaration"
);
case!(
    jsx,
    tree_sitter_javascript::LANGUAGE.into(),
    tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
    "Panel.jsx",
    "jsx_element"
);
case!(
    typescript,
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    tree_sitter_typescript::HIGHLIGHTS_QUERY,
    "Types.ts",
    "interface_declaration"
);
case!(
    tsx,
    tree_sitter_typescript::LANGUAGE_TSX.into(),
    tree_sitter_typescript::HIGHLIGHTS_QUERY,
    "Panel.tsx",
    "jsx_element"
);
case!(
    php,
    tree_sitter_php::LANGUAGE_PHP.into(),
    tree_sitter_php::HIGHLIGHTS_QUERY,
    "Page.php",
    "method_declaration"
);
case!(
    java,
    tree_sitter_java::LANGUAGE.into(),
    tree_sitter_java::HIGHLIGHTS_QUERY,
    "Example.java",
    "class_declaration"
);
case!(
    swift,
    tree_sitter_swift::LANGUAGE.into(),
    tree_sitter_swift::HIGHLIGHTS_QUERY,
    "Example.swift",
    "function_declaration"
);
case!(
    c,
    tree_sitter_c::LANGUAGE.into(),
    tree_sitter_c::HIGHLIGHT_QUERY,
    "example.c",
    "function_definition"
);
case!(
    cpp,
    tree_sitter_cpp::LANGUAGE.into(),
    tree_sitter_cpp::HIGHLIGHT_QUERY,
    "example.cpp",
    "namespace_definition"
);
case!(
    go,
    tree_sitter_go::LANGUAGE.into(),
    tree_sitter_go::HIGHLIGHTS_QUERY,
    "example.go",
    "method_declaration"
);
case!(
    dart,
    tree_sitter_dart::LANGUAGE.into(),
    tree_sitter_dart::HIGHLIGHTS_QUERY,
    "example.dart",
    "class_declaration"
);
case!(
    html,
    tree_sitter_html::LANGUAGE.into(),
    tree_sitter_html::HIGHLIGHTS_QUERY,
    "page.html",
    "script_element"
);
case!(
    xml,
    tree_sitter_xml::LANGUAGE_XML.into(),
    tree_sitter_xml::XML_HIGHLIGHT_QUERY,
    "document.xml",
    "CDSect"
);
case!(
    markdown,
    tree_sitter_md::LANGUAGE.into(),
    tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
    "AGENTS.md",
    "fenced_code_block"
);
case!(
    yaml,
    tree_sitter_yaml::LANGUAGE.into(),
    tree_sitter_yaml::HIGHLIGHTS_QUERY,
    "compose.yaml",
    "block_mapping_pair"
);
case!(
    json,
    tree_sitter_json::LANGUAGE.into(),
    tree_sitter_json::HIGHLIGHTS_QUERY,
    "Nested.json",
    "pair"
);
case!(
    toml,
    tree_sitter_toml_ng::LANGUAGE.into(),
    tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
    "settings.toml",
    "table"
);
case!(
    css,
    tree_sitter_css::LANGUAGE.into(),
    tree_sitter_css::HIGHLIGHTS_QUERY,
    "styles.css",
    "rule_set"
);
case!(
    scss,
    buntrail_language::registry::grammar("scss").unwrap(),
    include_str!("../resources/languages/queries/scss.scm"),
    "styles.scss",
    "rule_set"
);
case!(
    shell,
    tree_sitter_bash::LANGUAGE.into(),
    tree_sitter_bash::HIGHLIGHT_QUERY,
    "commands.sh",
    "function_definition"
);
#[test]
fn jsonc_trailing_comma_is_a_known_candidate_blocker() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_json::LANGUAGE.into())
        .unwrap();
    let text = include_str!("../fixtures/languages/Settings.jsonc");
    assert!(
        parser.parse(text, None).unwrap().root_node().has_error(),
        "Remove blocker only with reviewed JSONC expectations"
    );
}
