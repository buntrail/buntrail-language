use crate::coordinates::CoordinateMapper;
use buntrail_core::contracts::*;
use tree_sitter::Node;
fn text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}
fn children(node: Node) -> Vec<Node> {
    let mut c = node.walk();
    node.named_children(&mut c).collect()
}
fn find<'a>(node: Node<'a>, kinds: &[&str], depth: usize) -> Option<Node<'a>> {
    if depth > 32 {
        return None;
    }
    if kinds.contains(&node.kind()) {
        return Some(node);
    }
    children(node)
        .into_iter()
        .find_map(|n| find(n, kinds, depth + 1))
}
fn short(s: &str) -> String {
    let mut v = s.chars().take(80).collect::<String>();
    if s.chars().count() > 80 {
        v.push('…');
    }
    v
}
fn make(
    snapshot: &DocumentSnapshot,
    node: Node,
    selection: Node,
    label: String,
    kind: &str,
    children: Vec<OutlineNode>,
    preview: Option<String>,
) -> OutlineNode {
    let mapper = CoordinateMapper(snapshot);
    OutlineNode {
        id: String::new(),
        kind: kind.into(),
        parent_id: None,
        label,
        range: mapper.native(node.byte_range()).unwrap(),
        selection: mapper.native(selection.byte_range()).unwrap(),
        preview,
        children,
    }
}
pub(crate) fn identify(nodes: &mut [OutlineNode], parent: Option<&str>) {
    let mut counts = std::collections::HashMap::new();
    for node in nodes {
        let count = counts
            .entry((node.kind.clone(), node.label.clone()))
            .or_insert(0);
        // Length-prefixed components keep arbitrary display labels from becoming
        // ancestor separators. Occurrence distinguishes same-named siblings.
        node.id = format!(
            "/{}:{}{}:{}#{count}",
            node.kind.len(),
            node.kind,
            node.label.len(),
            node.label
        );
        node.id.insert_str(0, parent.unwrap_or(""));
        *count += 1;
        node.parent_id = parent.map(str::to_owned);
        identify(&mut node.children, Some(&node.id));
    }
}
pub(crate) fn extract(
    snapshot: &DocumentSnapshot,
    id: &str,
    root: Node,
    output: &mut LanguageSnapshot,
) {
    output
        .outline
        .extend(if id == "dockerfile" || id == "dockerfile_tick" {
            docker(snapshot, root)
        } else if id == "markdown" {
            markdown(snapshot, root)
        } else {
            walk(snapshot, id, root, 0)
        });
    if id == "markdown" {
        fn heading_folds(nodes: &[OutlineNode], folds: &mut Vec<FoldRange>) {
            for n in nodes {
                if n.range.end > n.selection.end {
                    folds.push(FoldRange {
                        id: format!("heading:{}", n.range.start),
                        range: n.range.clone(),
                    });
                }
                heading_folds(&n.children, folds);
            }
        }
        heading_folds(&output.outline, &mut output.folds);
    }
    if id.starts_with("dockerfile") {
        for stage in &output.outline {
            if stage.kind == "stage" && !stage.children.is_empty() {
                output.folds.push(FoldRange {
                    id: format!("stage:{}", stage.label),
                    range: stage.range.clone(),
                });
            }
        }
    }
    let mut stack = vec![(root, 0)];
    let mapper = CoordinateMapper(snapshot);
    while let Some((n, depth)) = stack.pop() {
        if depth > 256 {
            continue;
        }
        if n.end_position().row > n.start_position().row
            && foldable(n.kind())
            && !(id == "markdown" && n.kind() == "section")
        {
            let r = mapper.native(n.byte_range()).unwrap();
            output.folds.push(FoldRange {
                id: format!("{}:{}:{}", id, n.kind(), r.start),
                range: r,
            });
        }
        stack.extend(children(n).into_iter().map(|n| (n, depth + 1)));
    }
}
fn foldable(k: &str) -> bool {
    matches!(
        k,
        "element"
            | "template_element"
            | "script_element"
            | "style_element"
            | "statement_block"
            | "block"
            | "class_body"
            | "field_declaration_list"
            | "declaration_list"
            | "function_body"
            | "compound_statement"
            | "object"
            | "array"
            | "block_mapping"
            | "block_sequence"
            | "flow_mapping"
            | "flow_sequence"
            | "block_scalar"
            | "section"
            | "fenced_code_block"
            | "table"
            | "table_array_element"
            | "rule_set"
            | "if_statement"
            | "each_statement"
            | "await_statement"
            | "snippet_statement"
            | "section_directive"
            | "conditional"
            | "loop"
            | "preproc_if"
            | "preproc_ifdef"
            | "run_instruction"
            | "copy_instruction"
            | "heredoc_block"
            | "function_definition"
            | "function_declaration"
            | "class_declaration"
            | "interface_declaration"
    )
}
fn walk(snapshot: &DocumentSnapshot, id: &str, n: Node, depth: usize) -> Vec<OutlineNode> {
    if depth > 128 {
        return vec![];
    }
    let source = snapshot.text.as_str();
    let k = n.kind();
    if matches!(id, "json" | "jsonc" | "yaml" | "toml") {
        if matches!(k, "pair" | "block_mapping_pair" | "flow_pair") {
            let key = n.child_by_field_name("key").or_else(|| n.named_child(0));
            let value = n.child_by_field_name("value").or_else(|| n.named_child(1));
            if let Some(key) = key {
                let raw = text(key, source);
                let label = if raw.contains('\n') || raw.starts_with('[') || raw.starts_with('{') {
                    "[complex key]".into()
                } else {
                    if matches!(id, "json" | "jsonc") {
                        serde_json::from_str::<String>(raw).unwrap_or_else(|_| short(raw))
                    } else {
                        raw.trim_matches('"').trim_matches('\'').to_string()
                    }
                };
                let kids = value
                    .map(|v| walk(snapshot, id, v, depth + 1))
                    .unwrap_or_default();
                let preview = value
                    .filter(|_| kids.is_empty())
                    .map(|v| short(text(v, source)));
                return vec![make(snapshot, n, key, label, "property", kids, preview)];
            }
        }
        if matches!(k, "array" | "block_sequence" | "flow_sequence") {
            return children(n)
                .into_iter()
                .filter(|v| v.kind() != "comment")
                .enumerate()
                .map(|(i, item)| {
                    let kids = walk(snapshot, id, item, depth + 1);
                    let preview = kids
                        .is_empty()
                        .then(|| short(text(item, source).trim_start_matches("- ")));
                    make(
                        snapshot,
                        item,
                        item,
                        format!("[{i}]"),
                        "index",
                        kids,
                        preview,
                    )
                })
                .collect();
        }
        if matches!(k, "table" | "table_array_element") {
            let key = n.named_child(0).unwrap_or(n);
            let kids = children(n)
                .into_iter()
                .skip(1)
                .flat_map(|v| walk(snapshot, id, v, depth + 1))
                .collect();
            return vec![make(
                snapshot,
                n,
                key,
                text(key, source).into(),
                "table",
                kids,
                None,
            )];
        }
    }
    if id == "blade" && k == "section" {
        let parameter = children(n).into_iter().find(|c| c.kind() == "parameter");
        if let Some(parameter) = parameter {
            let label = text(parameter, source)
                .trim_matches(|c| matches!(c, '(' | ')' | '\'' | '"'))
                .to_string();
            let kids = children(n)
                .into_iter()
                .filter(|c| c.id() != parameter.id())
                .flat_map(|v| walk(snapshot, id, v, depth + 1))
                .collect();
            return vec![make(snapshot, n, parameter, label, "section", kids, None)];
        }
    }
    let element = matches!(
        k,
        "element"
            | "template_element"
            | "script_element"
            | "style_element"
            | "jsx_element"
            | "jsx_self_closing_element"
    );
    let declaration = matches!(
        k,
        "function_declaration"
            | "function_definition"
            | "generator_function_declaration"
            | "class_declaration"
            | "class_definition"
            | "class_specifier"
            | "interface_declaration"
            | "type_alias_declaration"
            | "method_definition"
            | "method_declaration"
            | "constructor_declaration"
            | "struct_specifier"
            | "enum_specifier"
            | "namespace_definition"
            | "type_spec"
            | "extension_declaration"
            | "protocol_declaration"
            | "property_declaration"
            | "mixin_statement"
            | "mixin_definition"
            | "function_signature"
            | "snippet_statement"
    );
    let selector = k == "rule_set";
    let name = if element {
        find(n, &["tag_name", "Name", "identifier"], 0)
    } else if selector {
        find(n, &["selectors"], 0)
    } else if declaration {
        n.child_by_field_name("name")
            .or_else(|| {
                n.child_by_field_name("declarator")
                    .and_then(|d| find(d, &["identifier", "field_identifier"], 0))
            })
            .or_else(|| {
                find(
                    n,
                    &[
                        "identifier",
                        "type_identifier",
                        "simple_identifier",
                        "snippet_name",
                        "name",
                    ],
                    0,
                )
            })
    } else {
        None
    };
    let kids: Vec<_> = children(n)
        .into_iter()
        .flat_map(|v| walk(snapshot, id, v, depth + 1))
        .collect();
    if let Some(name) = name {
        return vec![make(
            snapshot,
            n,
            name,
            short(text(name, source)),
            if element {
                "element"
            } else if selector {
                "rule"
            } else {
                k
            },
            kids,
            None,
        )];
    }
    kids
}
fn docker(snapshot: &DocumentSnapshot, root: Node) -> Vec<OutlineNode> {
    let source = snapshot.text.as_str();
    let mut stages: Vec<OutlineNode> = vec![];
    for n in children(root) {
        let k = n.kind();
        if k == "from_instruction" {
            if let Some(last) = stages.last_mut() {
                last.range.end = CoordinateMapper(snapshot)
                    .native(n.start_byte()..n.start_byte())
                    .unwrap()
                    .start;
            }
            let name = n
                .child_by_field_name("as")
                .or_else(|| find(n, &["image_alias"], 0));
            let label = name
                .map(|n| text(n, source).to_string())
                .unwrap_or_else(|| short(text(n, source)));
            stages.push(make(
                snapshot,
                n,
                name.unwrap_or(n),
                label,
                "stage",
                vec![],
                None,
            ));
        } else if k.ends_with("_instruction") {
            let label = short(text(n, source).lines().next().unwrap_or(""));
            let child = make(snapshot, n, n, label, "instruction", vec![], None);
            if let Some(stage) = stages.last_mut() {
                stage.range.end = child.range.end;
                stage.children.push(child);
            } else {
                stages.push(child);
            }
        }
    }
    stages
}
pub(crate) fn lightweight(snapshot: &DocumentSnapshot, id: &str, output: &mut LanguageSnapshot) {
    if id == "text" {
        return;
    }
    let mapper = CoordinateMapper(snapshot);
    let mut start = 0;
    for line in snapshot.text.as_str().split_inclusive('\n').take(100_001) {
        let trim = line.trim();
        let offset = line.len() - line.trim_start().len();
        let scope = if trim.starts_with('#') {
            "comment"
        } else if id == "dotenv" && trim.contains('=') {
            "property"
        } else if trim.starts_with('!') {
            "operator"
        } else {
            "string"
        };
        if !trim.is_empty() {
            let end = if scope == "property" {
                start + line.find('=').unwrap()
            } else {
                start + line.trim_end().len()
            };
            output.highlights.push(HighlightSpan {
                range: mapper.native(start + offset..end).unwrap(),
                scope: scope.into(),
            });
        }
        start += line.len();
    }
}

fn markdown(snapshot: &DocumentSnapshot, root: Node) -> Vec<OutlineNode> {
    let mut stack = vec![root];
    let mut headings = vec![];
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "atx_heading" | "setext_heading") {
            let level = if node.kind() == "atx_heading" {
                let marker = children(node)
                    .into_iter()
                    .find_map(|child| match child.kind() {
                        "atx_h1_marker" => Some(1),
                        "atx_h2_marker" => Some(2),
                        "atx_h3_marker" => Some(3),
                        "atx_h4_marker" => Some(4),
                        "atx_h5_marker" => Some(5),
                        "atx_h6_marker" => Some(6),
                        _ => None,
                    });
                // An unexpected/recovered heading must not truncate later headings.
                let Some(level) = marker else { continue };
                level
            } else if children(node)
                .iter()
                .any(|n| n.kind() == "setext_h1_underline")
            {
                1
            } else {
                2
            };
            headings.push((level, node));
        }
        stack.extend(children(node).into_iter().rev());
    }
    fn collect(
        snapshot: &DocumentSnapshot,
        headings: &[(usize, Node)],
        index: &mut usize,
        parent_level: usize,
        end: usize,
    ) -> Vec<OutlineNode> {
        let mut nodes = vec![];
        while *index < headings.len() && headings[*index].0 > parent_level {
            let (level, heading) = headings[*index];
            *index += 1;
            let children = collect(snapshot, headings, index, level, end);
            let stop = headings.get(*index).map_or(end, |(_, h)| h.start_byte());
            let name = find(heading, &["inline"], 0).unwrap_or(heading);
            let mut node = make(
                snapshot,
                heading,
                name,
                short(text(name, snapshot.text.as_str()).trim()),
                "heading",
                children,
                None,
            );
            node.range = CoordinateMapper(snapshot)
                .native(heading.start_byte()..stop)
                .unwrap();
            nodes.push(node);
        }
        nodes
    }
    collect(snapshot, &headings, &mut 0, 0, root.end_byte())
}

/// Several grammars expose both a declaration and its body with different token
/// starts but identical folded lines. Preserve the outer source range once; keep
/// genuinely nested folds whose first or last boundary line differs.
pub(crate) fn canonicalize_folds(snapshot: &DocumentSnapshot, output: &mut LanguageSnapshot) {
    let source = snapshot.text.as_str();
    let mut lines = vec![0];
    lines.extend(
        source
            .bytes()
            .enumerate()
            .filter_map(|(i, b)| (b == b'\n').then_some(i + 1)),
    );
    output
        .folds
        .sort_by_key(|f| (f.range.start, std::cmp::Reverse(f.range.end)));
    let mut seen = std::collections::HashSet::new();
    output.folds.retain(|fold| {
        let bytes = fold
            .range
            .to_bytes(snapshot)
            .expect("syntax fold coordinate");
        let first = lines.partition_point(|start| *start <= bytes.start as usize);
        let last = lines.partition_point(|start| *start <= (bytes.end as usize).saturating_sub(1));
        seen.insert((first, last))
    });
}
