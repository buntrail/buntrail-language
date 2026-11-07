use crate::{
    coordinates::{CoordinateMapper, point},
    registry,
    tree_sitter::analyze_tree,
};
use buntrail_core::contracts::*;
use tree_sitter::{Node, Parser, Range, Tree};
fn children(n: Node) -> Vec<Node> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}
fn descendants(n: Node) -> Vec<Node> {
    let mut stack = vec![n];
    let mut all = vec![];
    while let Some(n) = stack.pop() {
        if all.len() >= 100_000 {
            break;
        }
        stack.extend(children(n).into_iter().rev());
        all.push(n);
    }
    all
}
fn attr<'a>(n: Node, source: &'a str, name: &str) -> Option<&'a str> {
    let tag = children(n).into_iter().find(|n| n.kind() == "start_tag")?;
    for attr in children(tag) {
        if attr.kind() != "attribute" {
            continue;
        }
        let parts = descendants(attr);
        let key = parts.iter().find(|n| n.kind() == "attribute_name")?;
        if &source[key.byte_range()] == name {
            return Some(
                parts
                    .iter()
                    .find(|n| n.kind() == "attribute_value")
                    .map(|n| &source[n.byte_range()])
                    .unwrap_or(""),
            );
        }
    }
    None
}
fn label(id: &str) -> &str {
    match id {
        "typescript" => "TypeScript",
        "javascript" => "JavaScript",
        "scss" => "SCSS",
        "css" => "CSS",
        v => v,
    }
}
fn insert(nodes: &mut Vec<OutlineNode>, range: &BufferRange, mut additions: Vec<OutlineNode>) {
    if let Some(parent) = nodes
        .iter_mut()
        .find(|n| n.range.start <= range.start && n.range.end >= range.end)
    {
        insert(&mut parent.children, range, additions);
    } else {
        nodes.append(&mut additions);
        nodes.sort_by_key(|n| n.range.start);
    }
}
fn rename(nodes: &mut [OutlineNode], start: u64, name: &str) {
    for n in nodes {
        if n.range.start == start {
            n.label = name.into();
            return;
        }
        rename(&mut n.children, start, name);
    }
}
#[derive(Clone, Copy)]
pub(crate) struct AnalysisBudget {
    pub deadline: std::time::Instant,
    pub bounded_tree: bool,
}
fn inject(
    snapshot: &DocumentSnapshot,
    id: &str,
    ranges: Vec<std::ops::Range<usize>>,
    output: &mut LanguageSnapshot,
    depth: usize,
    source: Option<&str>,
    deadline: AnalysisBudget,
) -> Result<(), &'static str> {
    if std::time::Instant::now() >= deadline.deadline || output.injections.len() >= 512 {
        return Err("injection budget exceeded");
    }
    if ranges.is_empty() || depth > 8 {
        return Ok(());
    }
    let text = snapshot.text.as_str();
    let mut parser = Parser::new();
    let Some(grammar) = registry::grammar(id) else {
        return Ok(());
    };
    parser
        .set_language(&grammar)
        .map_err(|_| "injection grammar incompatible")?;
    let ranges: Vec<_> = ranges.into_iter().filter(|r| r.start < r.end).collect();
    if ranges.is_empty() {
        return Ok(());
    }
    let ts: Vec<_> = ranges
        .iter()
        .map(|r| Range {
            start_byte: r.start,
            end_byte: r.end,
            start_point: point(text, r.start),
            end_point: point(text, r.end),
        })
        .collect();
    parser
        .set_included_ranges(&ts)
        .map_err(|_| "overlapping injection ranges")?;
    let tree = crate::tree_sitter::bounded_parse_until(
        &mut parser,
        source.unwrap_or(text),
        None,
        deadline.deadline,
    )
    .ok_or("injection parse budget exceeded")?;
    let mut embedded = LanguageSnapshot {
        has_errors: false,
        truncated: false,
        document: snapshot.document,
        revision: snapshot.revision,
        grammar_id: id.into(),
        query_version: output.query_version.clone(),
        highlights: vec![],
        folds: vec![],
        injections: vec![],
        outline: vec![],
        availability: LanguageAvailability::Available,
    };
    analyze_tree(
        snapshot,
        id,
        &tree,
        &mut embedded,
        deadline.bounded_tree.then_some(deadline.deadline),
    )?;
    if source.is_none() {
        analyze(snapshot, id, &tree, &mut embedded, depth + 1, deadline)?;
    }
    let mapper = CoordinateMapper(snapshot);
    let enclosing = mapper
        .native(ranges.first().unwrap().start..ranges.last().unwrap().end)
        .map_err(|_| "injection boundary")?;
    for r in ranges {
        output.injections.push(InjectionRegion {
            range: mapper.native(r).map_err(|_| "injection boundary")?,
            language: id.into(),
        });
    }
    insert(&mut output.outline, &enclosing, embedded.outline);
    output.highlights.extend(embedded.highlights);
    output.folds.extend(embedded.folds);
    output.injections.extend(embedded.injections);
    if output.highlights.len() > 100_000
        || output.folds.len() > 100_000
        || output.injections.len() > 512
    {
        return Err("language result limit reached");
    }
    Ok(())
}
pub(crate) fn analyze(
    snapshot: &DocumentSnapshot,
    id: &str,
    tree: &Tree,
    output: &mut LanguageSnapshot,
    depth: usize,
    deadline: AnalysisBudget,
) -> Result<(), &'static str> {
    if depth > 8 {
        return Ok(());
    }
    let source = snapshot.text.as_str();
    let mapper = CoordinateMapper(snapshot);
    if id == "php" {
        let ranges = descendants(tree.root_node())
            .into_iter()
            .filter(|n| n.kind() == "text")
            .map(|n| n.byte_range())
            .collect();
        inject(snapshot, "html", ranges, output, depth, None, deadline)?;
    }
    for n in descendants(tree.root_node()) {
        if id == "vue"
            && let Some(block) = vue_fallback_block(n, source)
        {
            if n.id() == block.id() {
                let range = mapper.native(block.byte_range()).unwrap();
                fn clear(nodes: &mut [OutlineNode], range: &BufferRange, label: Option<&str>) {
                    for node in nodes {
                        if node.range == *range {
                            node.children.clear();
                            if let Some(label) = label {
                                node.label = label.into();
                            }
                        } else {
                            clear(&mut node.children, range, label);
                        }
                    }
                }
                let name = (block.kind() == "template_element").then(|| {
                    format!(
                        "template ({}){}",
                        attr(block, source, "lang").unwrap_or("html"),
                        if attr(block, source, "src").is_some() {
                            " external"
                        } else {
                            ""
                        }
                    )
                });
                clear(&mut output.outline, &range, name.as_deref());
                let parts = children(block);
                if let (Some(start), Some(end)) = (
                    parts.iter().find(|n| n.kind() == "start_tag"),
                    parts.iter().find(|n| n.kind() == "end_tag"),
                ) {
                    let body = mapper.native(start.end_byte()..end.start_byte()).unwrap();
                    output
                        .highlights
                        .retain(|n| !(n.range.start >= body.start && n.range.end <= body.end));
                    output
                        .folds
                        .retain(|n| !(n.range.start >= body.start && n.range.end <= body.end));
                }
            }
            continue;
        }
        match n.kind() {
            "script_element" | "style_element"
                if matches!(id, "html" | "vue" | "svelte" | "blade") =>
            {
                let script = n.kind() == "script_element";
                let language =
                    attr(n, source, "lang").unwrap_or(if script { "javascript" } else { "css" });
                let lang = registry::alias(language);
                let external = attr(n, source, "src").is_some();
                let name = format!(
                    "{}{} ({}){}",
                    if script { "script" } else { "style" },
                    if attr(n, source, "setup").is_some() {
                        " setup"
                    } else if attr(n, source, "module").is_some() {
                        " module"
                    } else {
                        ""
                    },
                    label(lang.unwrap_or(language)),
                    if external { " external" } else { "" }
                );
                rename(
                    &mut output.outline,
                    mapper.native(n.byte_range()).unwrap().start,
                    &name,
                );
                if !external
                    && let Some(raw) = children(n).into_iter().find(|n| n.kind() == "raw_text")
                    && let Some(lang) = lang
                {
                    if id == "blade" {
                        let (masked, holes, comments) = blade_holes(source, raw.byte_range());
                        for comment in comments {
                            output.highlights.push(HighlightSpan {
                                range: mapper.native(comment).unwrap(),
                                scope: "comment".into(),
                            });
                        }
                        inject(
                            snapshot,
                            lang,
                            vec![raw.byte_range()],
                            output,
                            depth,
                            Some(&masked),
                            deadline,
                        )?;
                        for hole in holes {
                            inject(
                                snapshot,
                                "php-only",
                                vec![hole],
                                output,
                                depth,
                                None,
                                deadline,
                            )?;
                        }
                    } else {
                        inject(
                            snapshot,
                            lang,
                            vec![raw.byte_range()],
                            output,
                            depth,
                            None,
                            deadline,
                        )?;
                    }
                }
            }
            "php_only" if id == "blade" => inject(
                snapshot,
                "php-only",
                vec![n.byte_range()],
                output,
                depth,
                None,
                deadline,
            )?,
            "interpolation" if id == "vue" => {
                if let Some(raw) = n.named_child(0) {
                    inject(
                        snapshot,
                        "typescript",
                        vec![raw.byte_range()],
                        output,
                        depth,
                        None,
                        deadline,
                    )?;
                }
            }
            "fenced_code_block" if id == "markdown" => {
                let nodes = descendants(n);
                let language = nodes
                    .iter()
                    .find(|n| n.kind() == "language")
                    .and_then(|n| registry::alias(&source[n.byte_range()]));
                if let (Some(lang), Some(body)) = (
                    language,
                    nodes.iter().find(|n| n.kind() == "code_fence_content"),
                ) {
                    inject(
                        snapshot,
                        lang,
                        vec![body.byte_range()],
                        output,
                        depth,
                        None,
                        deadline,
                    )?;
                }
            }
            "minus_metadata" if id == "markdown" => {
                let content = &source[n.byte_range()];
                if let Some(first) = content.find('\n') {
                    let end = content.trim_end().rfind('\n').unwrap_or(first);
                    if end > first {
                        inject(
                            snapshot,
                            "yaml",
                            std::iter::once(n.start_byte() + first + 1..n.start_byte() + end)
                                .collect(),
                            output,
                            depth,
                            None,
                            deadline,
                        )?;
                    }
                }
            }
            "inline" if id == "markdown" => inject(
                snapshot,
                "markdown-inline",
                vec![n.byte_range()],
                output,
                depth,
                None,
                deadline,
            )?,
            "shell_command" if id.starts_with("dockerfile") => {
                if let Some(parent) = n.parent() {
                    inject(
                        snapshot,
                        "shell",
                        std::iter::once(n.start_byte()..parent.end_byte()).collect(),
                        output,
                        depth,
                        None,
                        deadline,
                    )?;
                }
            }
            "json_string_array" if id.starts_with("dockerfile") => inject(
                snapshot,
                "json",
                vec![n.byte_range()],
                output,
                depth,
                None,
                deadline,
            )?,
            _ => {}
        }
    }
    Ok(())
}
/// Byte-identical source map: mask complete Blade comments and echo holes with
/// ASCII spaces, retaining CR/LF and every offset. Only echo bodies become PHP.
fn blade_holes(
    source: &str,
    range: std::ops::Range<usize>,
) -> (
    String,
    Vec<std::ops::Range<usize>>,
    Vec<std::ops::Range<usize>>,
) {
    let mut bytes = source.as_bytes().to_vec();
    let mut holes = vec![];
    let mut comments = vec![];
    let mut pos = range.start;
    while pos < range.end {
        let rest = &source[pos..range.end];
        let next = [("{{--", "--}}"), ("{{", "}}"), ("{!!", "!!}")]
            .iter()
            .filter_map(|(open, close)| rest.find(open).map(|i| (pos + i, *open, *close)))
            .min_by_key(|v| (v.0, std::cmp::Reverse(v.1.len())));
        let Some((start, open, close)) = next else {
            break;
        };
        let body = start + open.len();
        let Some(end) = source[body..range.end].find(close).map(|n| body + n) else {
            break;
        };
        if start > 0 && source.as_bytes()[start - 1] == b'@' {
            pos = end + close.len();
            continue;
        }
        if open == "{{--" {
            comments.push(start..end + close.len());
        } else {
            holes.push(body..end);
        }
        for b in &mut bytes[start..end + close.len()] {
            if *b != b'\n' && *b != b'\r' {
                *b = b' ';
            }
        }
        pos = end + close.len();
    }
    (String::from_utf8(bytes).unwrap(), holes, comments)
}

fn vue_fallback_block<'a>(mut node: Node<'a>, source: &str) -> Option<Node<'a>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "document" {
            return (node.kind() == "element"
                || (node.kind() == "template_element"
                    && (attr(node, source, "src").is_some()
                        || attr(node, source, "lang").is_some_and(|v| v != "html"))))
            .then_some(node);
        }
        node = parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::blade_holes;
    #[test]
    fn complete_blade_comment_masks_preserve_offsets_and_crlf() {
        for source in [
            "const before='🐇'; {{-- {{ $hidden }}\r\nfunction phantom() {} --}} const name='{{ $name }}';",
            "{{-- {!! $hidden !!}\r\n.hidden { color: red; } --}} .real { color: {{ $color }}; }",
        ] {
            let (masked, holes, comments) = blade_holes(source, 0..source.len());
            let comment_start = source.find("{{--").unwrap();
            let comment_end = source.find("--}}").unwrap() + 4;
            assert_eq!(comments.len(), 1);
            assert_eq!(comments[0], comment_start..comment_end);
            let echo_start = source[comment_end..].find("{{").unwrap() + comment_end;
            let echo_end = source[echo_start..].find("}}").unwrap() + echo_start + 2;
            assert_eq!(holes.len(), 1);
            assert_eq!(holes[0], echo_start + 2..echo_end - 2);
            assert_eq!(masked.len(), source.len());
            for (offset, original) in source.bytes().enumerate() {
                let expected = if (comments[0].contains(&offset)
                    || (echo_start..echo_end).contains(&offset))
                    && !matches!(original, b'\r' | b'\n')
                {
                    b' '
                } else {
                    original
                };
                assert_eq!(masked.as_bytes()[offset], expected, "source byte {offset}");
            }
        }
    }
}
