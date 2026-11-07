use crate::{
    coordinates::{CoordinateMapper, point},
    registry, structure,
};
use ::tree_sitter::{InputEdit, Parser, Query, QueryCursor, Tree};
use buntrail_core::contracts::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use streaming_iterator::StreamingIterator;
#[derive(Default)]
struct Previous {
    text: String,
    grammar: String,
    tree: Option<Tree>,
    parser: Parser,
}
#[derive(Default)]
pub struct LanguageAdapter {
    documents: Mutex<HashMap<DocumentID, Arc<Mutex<Previous>>>>,
}
impl LanguageService for LanguageAdapter {
    fn analyze(
        &self,
        snapshot: DocumentSnapshot,
    ) -> async_types::ServiceFuture<'_, LanguageSnapshot> {
        Box::pin(async move { self.analyze_snapshot(snapshot) })
    }
}
impl LanguageAdapter {
    /// Called only by a background worker. The cache holds at most eight document trees;
    /// eviction trades incremental reuse for a fresh parse, without changing semantics.
    pub fn analyze_snapshot(&self, snapshot: DocumentSnapshot) -> LanguageSnapshot {
        self.analyze_snapshot_with_limits(snapshot, std::time::Duration::from_secs(2), 100_000)
    }
    /// The host may retain its smaller analysis budget while using the same cached parser.
    pub fn analyze_snapshot_with_limits(
        &self,
        snapshot: DocumentSnapshot,
        budget: std::time::Duration,
        highlights: usize,
    ) -> LanguageSnapshot {
        let started = std::time::Instant::now();
        let deadline = started + budget;
        let id = registry::alias(snapshot.language.as_deref().unwrap_or("text")).unwrap_or("text");
        let mut output = LanguageSnapshot {
            has_errors: false,
            truncated: false,
            document: snapshot.document,
            revision: snapshot.revision,
            grammar_id: id.into(),
            query_version: "buntrail-1".into(),
            highlights: vec![],
            folds: vec![],
            injections: vec![],
            outline: vec![],
            availability: LanguageAvailability::Available,
        };
        let previous = {
            let mut documents = self.documents.lock().unwrap();
            if documents.len() >= 8 && !documents.contains_key(&snapshot.document) {
                if let Some(id) = documents
                    .iter()
                    .find(|(_, v)| Arc::strong_count(v) == 1)
                    .map(|(id, _)| *id)
                {
                    documents.remove(&id);
                } else {
                    output.availability = LanguageAvailability::Unavailable(CoreError::new(
                        ErrorCode::Busy,
                        "parser document cache is busy",
                    ));
                    return output;
                }
            }
            documents.entry(snapshot.document).or_default().clone()
        };
        let result = (|| {
            if snapshot.text.len() > 16 * 1024 * 1024 {
                return Err("language input exceeds 16 MiB");
            }
            let text = snapshot.text.as_str();
            if matches!(id, "text" | "dotenv" | "ignore") {
                structure::lightweight(&snapshot, id, &mut output);
                return if output.highlights.len() > 100_000 {
                    Err("highlight limit reached")
                } else {
                    Ok(())
                };
            }
            let dialect = if id == "dockerfile"
                && crate::docker_escape(text) == Ok(crate::DockerEscape::Backtick)
            {
                "dockerfile_tick"
            } else {
                id
            };
            let language = registry::grammar(dialect).ok_or("unknown grammar")?;
            let mut previous = previous.lock().unwrap();
            previous
                .parser
                .set_language(&language)
                .map_err(|_| "incompatible grammar")?;
            let mut old = previous.tree.take().filter(|_| previous.grammar == dialect);
            if let Some(tree) = old.as_mut() {
                edit(tree, &previous.text, text);
            }
            let tree = if budget >= std::time::Duration::from_secs(2) {
                bounded_parse(&mut previous.parser, text, old.as_ref())
            } else {
                bounded_parse_until(
                    &mut previous.parser,
                    text,
                    old.as_ref(),
                    started + budget.min(std::time::Duration::from_millis(200)),
                )
            }
            .ok_or("parse budget exceeded")?;
            output.has_errors = tree.root_node().has_error();
            analyze_tree_limited(
                &snapshot,
                dialect,
                &tree,
                &mut output,
                (budget < std::time::Duration::from_secs(2)).then_some(deadline),
                highlights,
            )?;
            crate::injections::analyze(
                &snapshot,
                dialect,
                &tree,
                &mut output,
                0,
                crate::injections::AnalysisBudget {
                    deadline: if budget < std::time::Duration::from_secs(2) {
                        deadline
                    } else {
                        std::time::Instant::now() + std::time::Duration::from_secs(2)
                    },
                    bounded_tree: budget < std::time::Duration::from_secs(2),
                },
            )?;
            structure::identify(&mut output.outline, None);
            structure::canonicalize_folds(&snapshot, &mut output);
            output
                .highlights
                .sort_by_key(|s| (s.range.start, s.range.end));
            output.highlights.dedup();
            previous.text = text.into();
            previous.grammar = dialect.into();
            previous.tree = Some(tree);
            Ok::<(), &str>(())
        })();
        if let Err(message) = result {
            if budget < std::time::Duration::from_secs(2)
                && message.starts_with("injection")
                && message.contains("budget")
            {
                output.truncated = true;
                output.highlights.truncate(highlights);
                return output;
            }
            output.highlights.clear();
            output.folds.clear();
            output.injections.clear();
            output.outline.clear();
            output.availability =
                LanguageAvailability::Unavailable(CoreError::new(ErrorCode::Unavailable, message));
        }
        output
    }
}
fn edit(tree: &mut Tree, old: &str, new: &str) {
    if old == new {
        return;
    }
    let mut start = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
        start -= 1;
    }
    let mut suffix = old[start..]
        .bytes()
        .rev()
        .zip(new[start..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) {
        suffix -= 1;
    }
    tree.edit(&InputEdit {
        start_byte: start,
        old_end_byte: old.len() - suffix,
        new_end_byte: new.len() - suffix,
        start_position: point(old, start),
        old_end_position: point(old, old.len() - suffix),
        new_end_position: point(new, new.len() - suffix),
    });
}
pub fn query(id: &str) -> &'static str {
    macro_rules! queries {($($id:literal),*)=>{match id {$($id=>include_str!(concat!("../resources/languages/queries/",$id,".scm")),)*_=>""}};}
    queries!(
        "javascript",
        "jsx",
        "typescript",
        "tsx",
        "php",
        "php-only",
        "java",
        "swift",
        "c",
        "cpp",
        "go",
        "dart",
        "html",
        "xml",
        "vue",
        "svelte",
        "blade",
        "dockerfile",
        "dockerfile_tick",
        "markdown",
        "markdown-inline",
        "yaml",
        "json",
        "jsonc",
        "toml",
        "css",
        "scss",
        "shell"
    )
}
pub(crate) fn analyze_tree(
    snapshot: &DocumentSnapshot,
    id: &str,
    tree: &Tree,
    output: &mut LanguageSnapshot,
    deadline: Option<std::time::Instant>,
) -> Result<(), &'static str> {
    analyze_tree_limited(snapshot, id, tree, output, deadline, 100_000)
}
fn analyze_tree_limited(
    snapshot: &DocumentSnapshot,
    id: &str,
    tree: &Tree,
    output: &mut LanguageSnapshot,
    deadline: Option<std::time::Instant>,
    highlights: usize,
) -> Result<(), &'static str> {
    let mut cursor = tree.walk();
    let mut count = 0;
    let mut depth = 0;
    loop {
        count += 1;
        if count > 100_000
            || depth > 128
            || deadline.is_some_and(|end| std::time::Instant::now() >= end)
        {
            if highlights < 100_000 {
                output.truncated = true;
                return Ok(());
            }
            return Err("syntax structure limit reached");
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                break;
            }
            depth -= 1;
        }
        if depth == 0 && cursor.node().id() == tree.root_node().id() {
            break;
        }
    }
    let lang = registry::grammar(id).ok_or("unknown grammar")?;
    let q = Query::new(&lang, query(id)).map_err(|_| "query incompatible with pinned grammar")?;
    let text = snapshot.text.as_str();
    let mapper = CoordinateMapper(snapshot);
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&q, tree.root_node(), text.as_bytes());
    while let Some((m, i)) = captures.next() {
        let capture = m.captures()[*i];
        output.highlights.push(HighlightSpan {
            range: mapper
                .native(capture.node.byte_range())
                .map_err(|_| "invalid capture boundary")?,
            scope: q.capture_names()[capture.index as usize].into(),
        });
        if output.highlights.len() > highlights
            || deadline.is_some_and(|end| std::time::Instant::now() >= end)
        {
            if highlights >= 100_000 {
                return Err("highlight limit reached");
            }
            output.highlights.truncate(highlights);
            output.truncated = true;
            break;
        }
    }
    structure::extract(snapshot, id, tree.root_node(), output);
    Ok(())
}

/// Parse budget bounds cancellation/shutdown delay; it is not a performance acceptance target.
pub(crate) fn bounded_parse(parser: &mut Parser, text: &str, old: Option<&Tree>) -> Option<Tree> {
    bounded_parse_until(
        parser,
        text,
        old,
        std::time::Instant::now() + std::time::Duration::from_secs(2),
    )
}
pub(crate) fn bounded_parse_until(
    parser: &mut Parser,
    text: &str,
    old: Option<&Tree>,
    deadline: std::time::Instant,
) -> Option<Tree> {
    let mut progress = |_: &::tree_sitter::ParseState| {
        if std::time::Instant::now() >= deadline {
            std::ops::ControlFlow::Break(())
        } else {
            std::ops::ControlFlow::Continue(())
        }
    };
    let tree = parser.parse_with_options(
        &mut |offset, _| &text.as_bytes()[offset..],
        old,
        Some(::tree_sitter::ParseOptions::new().progress_callback(&mut progress)),
    );
    if tree.is_none() {
        parser.reset();
    }
    tree
}
