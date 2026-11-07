//! Language analysis using Tree-sitter.
//!
//! See the repository README for usage, prerequisites and platform limitations.
use ::tree_sitter::{Language, Node, Parser, Query, QueryCursor};
use buntrail_core::Snapshot;
use streaming_iterator::StreamingIterator;

#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub length: usize,
    pub kind: u32,
}
pub struct Analysis {
    pub revision: u64,
    pub spans: Vec<Span>,
}

fn span(text: &str, node: Node, kind: u32) -> Span {
    Span {
        start: text[..node.start_byte()].encode_utf16().count(),
        length: text[node.byte_range()].encode_utf16().count(),
        kind,
    }
}
fn highlights(
    text: &str,
    tree: &::tree_sitter::Tree,
    language: &Language,
    query: &str,
    spans: &mut Vec<Span>,
    kind: u32,
) -> Result<(), String> {
    let query = Query::new(language, query).map_err(|e| e.to_string())?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures() {
            spans.push(span(text, capture.node, kind));
        }
    }
    Ok(())
}
fn folds(text: &str, node: Node, spans: &mut Vec<Span>) {
    if matches!(
        node.kind(),
        "element" | "script_element" | "statement_block"
    ) && node.end_position().row > node.start_position().row
    {
        spans.push(span(text, node, 1));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        folds(text, child, spans);
    }
}

/// T02 HTML/JavaScript seam only. Other required grammars remain unadopted.
pub fn analyze(snapshot: Snapshot) -> Result<Analysis, String> {
    let text = snapshot.text.as_str();
    let html: Language = tree_sitter_html::LANGUAGE.into();
    let js: Language = tree_sitter_javascript::LANGUAGE.into();
    let mut parser = Parser::new();
    parser.set_language(&html).map_err(|e| e.to_string())?;
    let tree = parser.parse(text, None).ok_or("parse cancelled")?;
    let mut spans = vec![];
    highlights(
        text,
        &tree,
        &html,
        tree_sitter_html::HIGHLIGHTS_QUERY,
        &mut spans,
        2,
    )?;
    folds(text, tree.root_node(), &mut spans);
    let query =
        Query::new(&html, "(script_element (raw_text) @script)").map_err(|e| e.to_string())?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let mut injected = Parser::new();
            injected.set_language(&js).map_err(|e| e.to_string())?;
            injected
                .set_included_ranges(&[capture.node.range()])
                .map_err(|e| e.to_string())?;
            let parsed = injected.parse(text, None).ok_or("injection cancelled")?;
            highlights(
                text,
                &parsed,
                &js,
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                &mut spans,
                3,
            )?;
            folds(text, parsed.root_node(), &mut spans);
        }
    }
    Ok(Analysis {
        revision: snapshot.revision,
        spans,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum DockerEscape {
    Backslash,
    Backtick,
}
/// Select a source-preserving grammar dialect; directives stop at the first
/// blank line, ordinary comment or instruction. Never rewrite the document.
pub fn docker_escape(source: &str) -> Result<DockerEscape, &'static str> {
    let mut escape = DockerEscape::Backslash;
    let mut seen = std::collections::HashSet::new();
    for line in source.lines() {
        let Some(comment) = line.trim().strip_prefix('#') else {
            break;
        };
        let Some((name, value)) = comment.trim().split_once('=') else {
            break;
        };
        let name = name.trim().to_ascii_lowercase();
        if !matches!(name.as_str(), "syntax" | "escape" | "check") {
            break;
        }
        if !seen.insert(name.clone()) {
            return Err("duplicate parser directive");
        }
        if name == "escape" {
            escape = match value.trim() {
                "\\" => DockerEscape::Backslash,
                "`" => DockerEscape::Backtick,
                _ => return Err("invalid escape directive"),
            };
        }
    }
    Ok(escape)
}

pub mod coordinates;
mod injections;
pub mod registry;
mod structure;
mod tree_sitter;
pub use tree_sitter::LanguageAdapter;
