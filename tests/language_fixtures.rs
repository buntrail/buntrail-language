mod support;

fn logical_fixture_name(storage: &str) -> &str {
    match storage {
        "dotenv.fixture" => ".env",
        "dotenv-local.fixture" => ".env.local",
        "agent-instructions.fixture" => "AGENTS.md",
        name => name,
    }
}

use buntrail_core::contracts::*;
use buntrail_language::LanguageAdapter;
use std::{
    sync::Arc,
    task::{Context, Poll, Waker},
};
fn snapshot(language: &str, source: &str, revision: u64) -> DocumentSnapshot {
    DocumentSnapshot {
        document: DocumentID([7; 16]),
        revision: BufferRevision(revision),
        text: Arc::new(source.into()),
        encoding: Encoding::Utf8 { bom: false },
        line_endings: LineEndings::Lf,
        language: Some(language.into()),
    }
}
fn run(adapter: &LanguageAdapter, snapshot: DocumentSnapshot) -> LanguageSnapshot {
    let mut future = adapter.analyze(snapshot);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("worker-local analysis unexpectedly suspended"),
    }
}
#[test]
fn structured_configuration_has_real_properties() {
    let adapter = LanguageAdapter::default();
    let s = run(
        &adapter,
        snapshot("json", include_str!("../fixtures/languages/Nested.json"), 1),
    );
    assert_eq!(s.availability, LanguageAvailability::Available);
    assert!(!s.highlights.is_empty());
    let services = s
        .outline
        .iter()
        .find(|n| n.label == "services")
        .expect("services property");
    let env = &services.children[1].children[0];
    assert_eq!(env.label, "env");
    assert_eq!(env.children[0].label, "MODE");
}
fn all_nodes(nodes: &[OutlineNode]) -> Vec<&OutlineNode> {
    nodes
        .iter()
        .flat_map(|n| std::iter::once(n).chain(all_nodes(&n.children)))
        .collect()
}
#[test]
fn query_registry_compiles_and_associations_are_explicit() {
    for id in buntrail_language::registry::IDS {
        let s = run(&LanguageAdapter::default(), snapshot(id, "", 0));
        assert_eq!(s.availability, LanguageAvailability::Available, "{id}");
    }
    use buntrail_language::registry::select;
    for (file, id) in [
        ("Profile.blade.php", "blade"),
        ("Dockerfile.dev", "dockerfile"),
        ("Containerfile", "dockerfile"),
        (".env.local", "dotenv"),
        (".gitignore", "ignore"),
        ("Module.mjs", "javascript"),
        ("Types.cts", "typescript"),
        ("header.h", "c"),
        ("header.ipp", "cpp"),
        (".claude/CLAUDE.md", "markdown"),
    ] {
        assert_eq!(select(file, "", None), id);
    }
    assert_eq!(select("header.h", "", Some("cpp")), "cpp");
    assert_eq!(select("header.h", "", Some("custom")), "text");
    assert_eq!(select("script", "#!/usr/bin/env -S bash -e", None), "shell");
}
#[test]
fn configuration_duplicates_sequences_previews_and_jsonc() {
    for (file, id) in [
        ("compose.yaml", "yaml"),
        ("YAMLEdges.yml", "yaml"),
        ("settings.toml", "toml"),
        ("Settings.jsonc", "jsonc"),
    ] {
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let s = run(&LanguageAdapter::default(), snapshot(id, &source, 2));
        assert_eq!(s.availability, LanguageAvailability::Available, "{file}");
        assert!(!s.highlights.is_empty(), "{file}");
        assert!(!s.outline.is_empty(), "{file}");
        if file == "YAMLEdges.yml" {
            assert_eq!(s.outline.iter().filter(|n| n.label == "hello").count(), 2);
            let ids: std::collections::HashSet<_> =
                all_nodes(&s.outline).iter().map(|n| &n.id).collect();
            assert_eq!(ids.len(), all_nodes(&s.outline).len());
            assert!(s.outline.iter().any(|n| n.label == "[complex key]"));
        }
        for node in all_nodes(&s.outline) {
            assert!(
                node.selection.start >= node.range.start && node.selection.end <= node.range.end
            );
            assert!(
                node.preview
                    .as_ref()
                    .is_none_or(|p| p.chars().count() <= 81)
            );
        }
    }
}
#[test]
fn test_unicode_incremental_equivalence() {
    let source = include_str!("../fixtures/languages/Nested.json");
    let adapter = LanguageAdapter::default();
    let _ = run(&adapter, snapshot("json", source, 0));
    for (revision, value) in [
        source.replace("hello", "🐇e\u{301}日本\r\n"),
        source.replace("hello", "broken\""),
        source.to_string(),
    ]
    .into_iter()
    .enumerate()
    {
        let incremental = run(&adapter, snapshot("json", &value, revision as u64 + 1));
        let fresh = run(
            &LanguageAdapter::default(),
            snapshot("json", &value, revision as u64 + 1),
        );
        assert_eq!(incremental, fresh);
    }
}
#[test]
fn component_regions_are_independent_and_source_mapped() {
    for (file, id, expected) in [
        ("VueMixed.vue", "vue", "label"),
        ("SvelteModern.svelte", "svelte", "bump"),
        ("Profile.blade.php", "blade", "greet"),
    ] {
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let s = run(&LanguageAdapter::default(), snapshot(id, &source, 1));
        assert!(
            all_nodes(&s.outline).iter().any(|n| n.label == expected),
            "{file}: missing {expected}"
        );
        assert!(s.injections.len() >= 3, "{file}: embedded regions");
        for r in &s.injections {
            let bytes = r.range.to_bytes(&snapshot(id, &source, 1)).unwrap();
            assert!(bytes.end <= source.len() as u64);
        }
    }
}
#[test]
fn source_declarations_and_companions_have_selected_semantics() {
    for (file, id, names, token) in [
        (
            "Module.js",
            "javascript",
            vec!["greet", "Greeter"],
            "function",
        ),
        ("Panel.jsx", "jsx", vec!["Panel", "section"], "function"),
        (
            "Types.ts",
            "typescript",
            vec!["User", "greet", "Greeter"],
            "interface",
        ),
        ("Panel.tsx", "tsx", vec!["Props", "Panel"], "interface"),
        (
            "Page.php",
            "php",
            vec!["Example", "greet", "label"],
            "class",
        ),
        ("Example.java", "java", vec!["Example", "greet"], "class"),
        (
            "Example.swift",
            "swift",
            vec!["Example", "greet", "label"],
            "struct",
        ),
        ("example.c", "c", vec!["Example", "greet"], "struct"),
        (
            "example.cpp",
            "cpp",
            vec!["sample", "Example", "greet", "label"],
            "namespace",
        ),
        (
            "example.go",
            "go",
            vec!["Example", "Greet", "Label"],
            "func",
        ),
        (
            "example.dart",
            "dart",
            vec!["Example", "greet", "label"],
            "class",
        ),
        ("page.html", "html", vec!["article", "greet"], "article"),
        ("document.xml", "xml", vec!["root", "x:child"], "root"),
        ("styles.css", "css", vec!["article.hello"], "color"),
        ("styles.scss", "scss", vec![".hello", ".child"], "color"),
        ("commands.sh", "shell", vec!["hello"], "for"),
    ] {
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let s = run(&LanguageAdapter::default(), snapshot(id, &source, 1));
        assert_eq!(s.availability, LanguageAvailability::Available, "{file}");
        let nodes = all_nodes(&s.outline);
        for expected in names {
            assert!(
                nodes.iter().any(|n| n.label == expected),
                "{file} missing {expected}: {:?}",
                nodes.iter().map(|n| &n.label).collect::<Vec<_>>()
            );
        }
        assert!(!s.folds.is_empty(), "{file}: folds");
        let start = source.find(token).unwrap();
        let utf16 = source[..start].encode_utf16().count() as u64;
        assert!(
            s.highlights
                .iter()
                .any(|h| h.range.start <= utf16 && h.range.end >= utf16 + token.len() as u64),
            "{file} token {token}"
        );
    }
}
#[test]
fn markup_and_build_files_have_exact_structure() {
    let adapter = LanguageAdapter::default();
    for (file, id) in [
        ("AGENTS.md", "markdown"),
        ("SKILL.md", "markdown"),
        ("Dockerfile.multistage", "dockerfile"),
        ("Containerfile.dev", "dockerfile"),
        ("Dockerfile.escape", "dockerfile"),
        ("SvelteCurrentSyntax.svelte", "svelte"),
        ("VueFallback.vue", "vue"),
        ("BladeEscapes.blade.php", "blade"),
    ] {
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let s = run(&adapter, snapshot(id, &source, 1));
        assert_eq!(s.availability, LanguageAvailability::Available, "{file}");
        if file == "AGENTS.md" {
            let names = all_nodes(&s.outline);
            assert!(names.iter().any(|n| n.label == "Instructions"));
            assert!(!names.iter().any(|n| n.label.contains("Not a heading")));
            for lang in ["yaml", "tsx", "shell", "markdown-inline"] {
                assert!(s.injections.iter().any(|n| n.language == lang), "{lang}");
            }
        }
        if file == "Dockerfile.multistage" {
            let build = s.outline.iter().find(|n| n.label == "build").unwrap();
            let runtime = s.outline.iter().find(|n| n.label == "runtime").unwrap();
            assert_eq!(build.range.end, runtime.range.start);
            assert_eq!(build.children.len(), 3);
            assert!(!s.injections.iter().any(|r| {
                source[r.range.to_bytes(&snapshot(id, &source, 1)).unwrap().start as usize..]
                    .starts_with("hello\nMESSAGE")
            }));
        }
        if file == "VueFallback.vue" || file == "BladeEscapes.blade.php" {
            assert!(
                s.injections.is_empty(),
                "{file} must not fabricate executable regions"
            );
        }
    }
}
#[test]
fn complete_appendix_edit_coordinate_matrix() {
    let files: Vec<_> =
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/languages"))
            .unwrap()
            .map(Result::unwrap)
            .filter(|e| e.file_type().unwrap().is_file())
            .collect();
    for file in files {
        let name = logical_fixture_name(&file.file_name().to_string_lossy()).to_string();
        let source = std::fs::read_to_string(file.path()).unwrap();
        let override_id = (name == "example-cpp.h").then_some("cpp");
        let id = buntrail_language::registry::select(
            &name,
            source.lines().next().unwrap_or(""),
            override_id,
        );
        assert_ne!(id, "text", "unrecognized appendix fixture {name}");
        let adapter = LanguageAdapter::default();
        let mut doc = buntrail_core::Document::new(source.clone());
        for step in 0..7 {
            match step {
                1 => {
                    doc.edit(doc.revision(), 1, 0, 0, "🐇e\u{301}日本\r\n")
                        .unwrap();
                }
                2 => {
                    let len = doc.text().encode_utf16().count();
                    doc.edit(
                        doc.revision(),
                        1,
                        len.saturating_sub(5),
                        len.min(5),
                        "\r\n{broken",
                    )
                    .unwrap();
                }
                3 | 4 => {
                    doc.undo(doc.revision(), 1).unwrap();
                }
                5 => {
                    doc.redo(doc.revision(), 1).unwrap();
                }
                6 => {
                    doc.undo(doc.revision(), 1).unwrap();
                }
                _ => {}
            }
            let snap = snapshot(id, doc.text(), doc.revision());
            let actual = run(&adapter, snap.clone());
            let fresh = run(&LanguageAdapter::default(), snap.clone());
            assert_eq!(actual, fresh, "{name} edit {step}");
            assert_eq!(
                actual.availability,
                LanguageAvailability::Available,
                "{name}"
            );
            let ranges = actual
                .highlights
                .iter()
                .map(|n| &n.range)
                .chain(actual.folds.iter().map(|n| &n.range))
                .chain(actual.injections.iter().map(|n| &n.range))
                .chain(
                    all_nodes(&actual.outline)
                        .into_iter()
                        .flat_map(|n| [&n.range, &n.selection]),
                );
            for range in ranges {
                let bytes = range.to_bytes(&snap).unwrap();
                assert_eq!(bytes.to_utf16(&snap).unwrap(), *range, "{name}");
            }
        }
        for value in [
            source.replace('\n', "\r\n"),
            source.trim_end_matches('\n').into(),
        ] {
            let snap = snapshot(id, &value, 99);
            let analyzed = run(&LanguageAdapter::default(), snap.clone());
            assert_eq!(
                analyzed.availability,
                LanguageAvailability::Available,
                "{name} CRLF/no final newline"
            );
            for offset in 0..=value.len() {
                if value.is_char_boundary(offset) {
                    let b = TextByteRange {
                        revision: snap.revision,
                        start: offset as u64,
                        end: offset as u64,
                    };
                    assert_eq!(b.to_utf16(&snap).unwrap().to_bytes(&snap).unwrap(), b);
                }
            }
        }
    }
}
#[test]
fn component_named_hierarchy_and_region_shift() {
    for (file, id, expected) in [
        (
            "VueMixed.vue",
            "vue",
            vec!["script setup (TypeScript)", "label", "ul", "li"],
        ),
        (
            "SvelteModern.svelte",
            "svelte",
            vec!["row", "bump", "b", "p"],
        ),
        (
            "Profile.blade.php",
            "blade",
            vec!["content", "x-card", "x-slot:title", "greet"],
        ),
    ] {
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let adapter = LanguageAdapter::default();
        let first = run(&adapter, snapshot(id, &source, 0));
        for name in expected {
            assert!(
                all_nodes(&first.outline).iter().any(|n| n.label == name),
                "{file}: {name}"
            );
        }
        for region in first.injections {
            let start = region
                .range
                .to_bytes(&snapshot(id, &source, 0))
                .unwrap()
                .start as usize;
            let mut changed = source.clone();
            changed.insert_str(start, "🐇e\u{301}日本");
            let a = run(&adapter, snapshot(id, &changed, 1));
            let b = run(&LanguageAdapter::default(), snapshot(id, &changed, 1));
            assert_eq!(a, b, "{file} region insertion");
        }
    }
}
#[test]
fn scss_parent_suffix_recovery_keeps_navigation_and_highlights() {
    let source = include_str!("../fixtures/languages/styles-parent.scss");
    let snap = snapshot("scss", source, 7);
    let s = run(&LanguageAdapter::default(), snap.clone());
    assert_eq!(s.availability, LanguageAvailability::Available);
    for name in [".root", ".#{$suffix}", "&-#{$suffix}"] {
        let node = all_nodes(&s.outline)
            .into_iter()
            .find(|n| n.label == name)
            .expect(name);
        let bytes = node.selection.to_bytes(&snap).unwrap();
        assert_eq!(&source[bytes.start as usize..bytes.end as usize], name);
        assert!(s.folds.iter().any(|f| f.range == node.range));
        assert!(s.highlights.iter().any(|h|h.range.start<=node.selection.start&&h.range.end>=node.selection.end));
    }
    for node in all_nodes(&s.outline) {
        assert_eq!(
            node.range.to_bytes(&snap).unwrap().to_utf16(&snap).unwrap(),
            node.range
        );
    }
}
#[test]
fn nested_component_folds_and_lightweight_tokens() {
    for (file, id, labels) in [
        ("VueMixed.vue", "vue", vec!["template", "ul"]),
        ("SvelteModern.svelte", "svelte", vec!["row"]),
    ] {
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let s = run(&LanguageAdapter::default(), snapshot(id, &source, 1));
        for label in labels {
            let node = all_nodes(&s.outline)
                .into_iter()
                .find(|n| n.label == label)
                .expect(label);
            assert!(
                s.folds.iter().any(|f| f.range == node.range),
                "{file} {label} fold"
            );
        }
    }
    for (id, source, token, scope) in [
        ("dotenv", "# hello\nMODE=dev\n", "MODE", "property"),
        ("ignore", "# hello\n!keep.txt\n", "!keep.txt", "operator"),
    ] {
        let s = run(&LanguageAdapter::default(), snapshot(id, source, 1));
        let start = source.find(token).unwrap() as u64;
        assert!(s.highlights.iter().any(|h| h.range.start == start
            && h.range.end == start + token.len() as u64
            && h.scope == scope));
        assert!(s.outline.is_empty());
    }
}
#[test]
fn required_grammars_accept_authored_valid_syntax() {
    for entry in
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/languages")).unwrap()
    {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_file() {
            continue;
        }
        let filename = logical_fixture_name(&entry.file_name().to_string_lossy()).to_string();
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let selected = buntrail_language::registry::select(
            &filename,
            "",
            (filename == "example-cpp.h").then_some("cpp"),
        );
        let id = if selected == "dockerfile"
            && buntrail_language::docker_escape(&text)
                == Ok(buntrail_language::DockerEscape::Backtick)
        {
            "dockerfile_tick"
        } else {
            selected
        };
        let Some(grammar) = buntrail_language::registry::grammar(id) else {
            continue;
        };
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&grammar).unwrap();
        let tree = parser.parse(&text, None).unwrap();
        if filename == "styles-parent.scss" {
            assert!(
                tree.root_node().has_error(),
                "update recorded recovery evidence when the parent-suffix grammar is corrected"
            );
        } else if filename != "YAMLEdges.yml" {
            assert!(
                !tree.root_node().has_error(),
                "{filename}: {}",
                tree.root_node().to_sexp()
            );
        }
    }
}
#[test]
fn independently_authored_expectation_for_every_fixture() {
    for row in include_str!("fixture-expectations.txt")
        .lines()
        .filter(|r| !r.starts_with('#'))
    {
        let fields: Vec<_> = row.split('|').collect();
        let [file, id, token, names, injections] = fields.as_slice() else {
            panic!("invalid expectation")
        };
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let snap = snapshot(id, &source, 42);
        let result = run(&LanguageAdapter::default(), snap.clone());
        assert_eq!(
            result.availability,
            LanguageAvailability::Available,
            "{file}"
        );
        let byte = source.find(token).expect(file);
        let selected = TextByteRange {
            revision: snap.revision,
            start: byte as u64,
            end: (byte + token.len()) as u64,
        }
        .to_utf16(&snap)
        .unwrap();
        assert!(
            result
                .highlights
                .iter()
                .any(|h| h.range.start <= selected.start && h.range.end >= selected.end),
            "{file}: selected token {token}"
        );
        let nodes = all_nodes(&result.outline);
        for name in names.split(',').filter(|s| !s.is_empty()) {
            assert!(nodes.iter().any(|n| n.label == name), "{file}: {name}");
        }
        for language in injections.split(',').filter(|s| !s.is_empty()) {
            assert!(
                result.injections.iter().any(|r| r.language == language),
                "{file}: {language}"
            );
        }
        if injections.is_empty() {
            assert!(result.injections.is_empty(), "{file}: unexpected injection");
        }
    }
}
#[test]
fn setext_headings_preserve_source_order_and_stop_peer_folds() {
    let text =
        "# Root\ntext\n## Before\nbody\n\nSetext section\n--------------\nbody\n## After\nend";
    let snap = snapshot("markdown", text, 5);
    let s = run(&LanguageAdapter::default(), snap.clone());
    let root = &s.outline[0];
    assert_eq!(root.label, "Root");
    let labels: Vec<_> = root.children.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(labels, ["Before", "Setext section", "After"]);
    let before = &root.children[0];
    let setext = &root.children[1];
    let after = &root.children[2];
    assert_eq!(before.range.end, setext.range.start);
    assert_eq!(setext.range.end, after.range.start);
    assert!(s.folds.iter().any(|f| f.range == setext.range));
    let b = setext.selection.to_bytes(&snap).unwrap();
    assert_eq!(&text[b.start as usize..b.end as usize], "Setext section");
}
#[test]
fn json_escaped_keys_are_readable_without_losing_source_selection() {
    let text = r#"{"escaped\"key": 1, "\u65e5\u672c": 2}"#;
    let snap = snapshot("json", text, 1);
    let s = run(&LanguageAdapter::default(), snap.clone());
    assert_eq!(s.outline[0].label, "escaped\"key");
    assert_eq!(s.outline[1].label, "日本");
    let b = s.outline[0].selection.to_bytes(&snap).unwrap();
    assert_eq!(&text[b.start as usize..b.end as usize], r#""escaped\"key""#);
}
#[test]
fn vue_unknown_blocks_and_preprocessors_do_not_fabricate_children() {
    let source = "<template lang=\"pug\"><b>literal</b></template>\n<custom-block><script>function fabricated() {}</script></custom-block>";
    let s = run(&LanguageAdapter::default(), snapshot("vue", source, 1));
    assert!(s.injections.is_empty());
    assert!(s.outline.iter().all(|n| n.children.is_empty()));
    assert!(
        !all_nodes(&s.outline)
            .iter()
            .any(|n| n.label == "b" || n.label == "fabricated")
    );
}
#[test]
fn cpp_template_class_body_and_unique_fold_interiors() {
    for (file, language) in [
        ("example-cpp.h", "cpp"),
        ("Module.js", "javascript"),
        ("styles-parent.scss", "scss"),
    ] {
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let snap = snapshot(language, &source, 6);
        let s = run(&LanguageAdapter::default(), snap.clone());
        let mut interiors = std::collections::HashSet::new();
        for fold in &s.folds {
            let b = fold.range.to_bytes(&snap).unwrap();
            let first = source.as_bytes()[..b.start as usize]
                .iter()
                .filter(|b| **b == b'\n')
                .count();
            let last = source.as_bytes()[..(b.end as usize).saturating_sub(1)]
                .iter()
                .filter(|b| **b == b'\n')
                .count();
            eprintln!(
                "{file}: {} bytes {}..{} boundary lines {}..{}",
                fold.id,
                b.start,
                b.end,
                first + 1,
                last + 1
            );
            assert!(
                interiors.insert((first, last)),
                "{file}: duplicate hidden interior for boundary lines {}..{}",
                first + 1,
                last + 1
            );
        }
        if language == "cpp" {
            let member = source.find("T greet").unwrap();
            assert!(
                s.folds.iter().any(|f| {
                    let b = f.range.to_bytes(&snap).unwrap();
                    source.as_bytes()[..b.start as usize]
                        .iter()
                        .filter(|b| **b == b'\n')
                        .count()
                        == 2
                        && (b.start as usize) < member
                        && (b.end as usize) > member
                }),
                "explicit C++ template class must fold its member body"
            );
        }
    }
}
#[test]
fn explicit_cpp_template_has_class_fold() {
    let source = include_str!("../fixtures/languages/example-cpp.h");
    let snap = snapshot("cpp", source, 6);
    let s = run(&LanguageAdapter::default(), snap.clone());
    let member = source.find("T greet").unwrap();
    assert!(
        s.folds.iter().any(|f| {
            let b = f.range.to_bytes(&snap).unwrap();
            source.as_bytes()[..b.start as usize]
                .iter()
                .filter(|b| **b == b'\n')
                .count()
                == 2
                && (b.start as usize) < member
                && (b.end as usize) > member
        }),
        "explicit C++ template class must fold its member body"
    );
}

#[test]
fn outline_ids_are_unambiguous_and_stable_across_preceding_edits() {
    let source = r#"{"a":{"b":1},"a#0/property:b":2,"a":3,"/property:#🐇":4}"#;
    let adapter = LanguageAdapter::default();
    let before = run(&adapter, snapshot("json", source, 0));
    fn validate(
        nodes: &[OutlineNode],
        parent: Option<&str>,
        ids: &mut std::collections::HashSet<String>,
    ) {
        for node in nodes {
            assert!(
                ids.insert(node.id.clone()),
                "duplicate ID {} for {}",
                node.id,
                node.label
            );
            assert_eq!(node.parent_id.as_deref(), parent);
            validate(&node.children, Some(&node.id), ids);
        }
    }
    validate(&before.outline, None, &mut Default::default());
    let changed = format!("\r\n  {source}");
    let after = run(&adapter, snapshot("json", &changed, 1));
    validate(&after.outline, None, &mut Default::default());
    let identities = |s: &LanguageSnapshot| {
        all_nodes(&s.outline)
            .into_iter()
            .map(|n| (n.label.clone(), n.id.clone(), n.parent_id.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(identities(&before), identities(&after));
    assert_eq!(
        after,
        run(&LanguageAdapter::default(), snapshot("json", &changed, 1))
    );
}

#[test]
fn indented_atx_markers_preserve_hierarchy_selections_and_peer_fold_ends() {
    for indent in 1..=3 {
        let source = format!(
            "# Root\nbody\n{}## Indented\ninside\n## After\nend\n",
            " ".repeat(indent)
        );
        let adapter = LanguageAdapter::default();
        for (revision, text) in [
            (0, source.clone()),
            (1, format!("🐇e\u{301}日本\r\n\r\n{source}")),
        ] {
            let snap = snapshot("markdown", &text, revision);
            let parsed = run(&adapter, snap.clone());
            assert_eq!(parsed, run(&LanguageAdapter::default(), snap.clone()));
            assert_eq!(parsed.outline.len(), 1);
            let root = &parsed.outline[0];
            assert_eq!(root.label, "Root");
            assert_eq!(
                root.children
                    .iter()
                    .map(|n| n.label.as_str())
                    .collect::<Vec<_>>(),
                ["Indented", "After"]
            );
            assert_eq!(
                root.range,
                buntrail_language::coordinates::CoordinateMapper(&snap)
                    .native(text.find("# Root").unwrap()..text.len())
                    .unwrap()
            );
            for (i, label) in ["Indented", "After"].iter().enumerate() {
                let child = &root.children[i];
                assert_eq!(child.parent_id.as_deref(), Some(root.id.as_str()));
                let start = text.find(label).unwrap();
                assert_eq!(
                    child.selection,
                    buntrail_language::coordinates::CoordinateMapper(&snap)
                        .native(start..start + label.len())
                        .unwrap()
                );
                let end = if i == 0 {
                    text.find("## After").unwrap()
                } else {
                    text.len()
                };
                let opening = if i == 0 {
                    format!("{}## Indented", " ".repeat(indent))
                } else {
                    "## After".into()
                };
                let heading_start = text.find(&opening).unwrap();
                assert_eq!(
                    child.range,
                    buntrail_language::coordinates::CoordinateMapper(&snap)
                        .native(heading_start..end)
                        .unwrap()
                );
                assert!(parsed.folds.iter().any(|f| f.range == child.range));
            }
        }
    }
}

#[test]
fn blade_script_and_style_comments_are_not_php_echoes() {
    let source = "<script>\r\nconst start = '🐇';\r\n{{-- {{ $hidden }}\r\nfunction phantom() {} --}}\r\nconst name = '{{ $name }}';\r\nfunction visible() {}\r\n</script>\r\n<style>\r\n{{-- {!! $hidden !!}\r\n.hidden { color: red; } --}}\r\n.real { color: {{ $color }}; }\r\n</style>";
    let snap = snapshot("blade", source, 0);
    let parsed = run(&LanguageAdapter::default(), snap.clone());
    let php: Vec<_> = parsed
        .injections
        .iter()
        .filter(|r| r.language == "php-only")
        .map(|r| {
            let b = r.range.to_bytes(&snap).unwrap();
            &source[b.start as usize..b.end as usize]
        })
        .collect();
    assert_eq!(
        php,
        [" $name ", " $color "],
        "comments must be masked whole, never injected"
    );
    let mapper = buntrail_language::coordinates::CoordinateMapper(&snap);
    for (language, open, close) in [
        ("javascript", "<script>", "</script>"),
        ("css", "<style>", "</style>"),
        ("php-only", "const name = '{{", "}}"),
        ("php-only", "color: {{", "}}"),
    ] {
        let start = source.find(open).unwrap() + open.len();
        let end = start + source[start..].find(close).unwrap();
        assert!(
            parsed
                .injections
                .iter()
                .any(|r| r.language == language && r.range == mapper.native(start..end).unwrap())
        );
    }
    let mut offset = 0;
    while let Some(found) = source[offset..].find("{{--") {
        let start = offset + found;
        let end = start + source[start..].find("--}}").unwrap() + 4;
        let comment = mapper.native(start..end).unwrap();
        assert!(
            parsed
                .highlights
                .iter()
                .any(|h| h.scope == "comment" && h.range == comment)
        );
        assert!(
            parsed
                .highlights
                .iter()
                .filter(|h| h.range.start >= comment.start && h.range.end <= comment.end)
                .all(|h| h.scope == "comment")
        );
        offset = end;
    }
    let adapter = LanguageAdapter::default();
    run(&adapter, snap.clone());
    let changed = format!("<!-- e\u{301}日本🐇 -->\r\n{source}");
    let changed = snapshot("blade", &changed, 1);
    assert_eq!(
        run(&adapter, changed.clone()),
        run(&LanguageAdapter::default(), changed)
    );
    let labels: Vec<_> = all_nodes(&parsed.outline)
        .iter()
        .map(|n| n.label.as_str())
        .collect();
    assert!(labels.contains(&"visible"));
    assert!(labels.contains(&".real"));
    assert!(!labels.contains(&"phantom") && !labels.contains(&".hidden"));
    for region in &parsed.injections {
        let bytes = region.range.to_bytes(&snap).unwrap();
        assert_eq!(
            buntrail_language::coordinates::CoordinateMapper(&snap)
                .native(bytes.start as usize..bytes.end as usize)
                .unwrap(),
            region.range
        );
    }
}

#[test]
fn independently_expected_component_paths_children_selections_and_regions() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("component-expectations.json")).unwrap();
    fn strings(v: &serde_json::Value) -> Vec<&str> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect()
    }
    fn node_at<'a>(roots: &'a [OutlineNode], path: &[&str]) -> &'a OutlineNode {
        let mut nodes = roots;
        let mut parent: Option<&OutlineNode> = None;
        for label in path {
            let node = nodes
                .iter()
                .find(|n| &n.label == label)
                .unwrap_or_else(|| panic!("missing expected path {path:?}"));
            assert_eq!(node.parent_id.as_deref(), parent.map(|n| n.id.as_str()));
            if let Some(p) = parent {
                assert!(p.range.start <= node.range.start && p.range.end >= node.range.end);
            }
            parent = Some(node);
            nodes = &node.children;
        }
        parent.unwrap()
    }
    for fixture in fixtures.as_array().unwrap() {
        let file = fixture["file"].as_str().unwrap();
        let source = std::fs::read_to_string(support::fixture_path(file)).unwrap();
        let language = fixture["grammar"].as_str().unwrap();
        let adapter = LanguageAdapter::default();
        for (revision, text) in [
            (0, source.clone()),
            (1, format!("<!-- 🐇e\u{301}日本 -->\r\n{source}")),
        ] {
            let snap = snapshot(language, &text, revision);
            let parsed = run(&adapter, snap.clone());
            assert_eq!(parsed, run(&LanguageAdapter::default(), snap.clone()));
            assert_eq!(
                parsed
                    .outline
                    .iter()
                    .map(|n| n.label.as_str())
                    .collect::<Vec<_>>(),
                strings(&fixture["roots"]),
                "{file} roots"
            );
            let mapper = buntrail_language::coordinates::CoordinateMapper(&snap);
            for expected in fixture["nodes"].as_array().unwrap() {
                let path = strings(&expected["path"]);
                let node = node_at(&parsed.outline, &path);
                let open = expected["open"].as_str().unwrap();
                let close = expected["close"].as_str().unwrap();
                let start = text.find(open).unwrap();
                let end = start
                    + open.len()
                    + text[start + open.len()..].find(close).unwrap()
                    + close.len();
                assert_eq!(
                    node.range,
                    mapper.native(start..end).unwrap(),
                    "{file} {path:?} enclosing"
                );
                let selection = expected["selection"].as_str().unwrap();
                let offset = start + text[start..end].find(selection).unwrap();
                assert_eq!(
                    node.selection,
                    mapper.native(offset..offset + selection.len()).unwrap(),
                    "{file} {path:?} selected source"
                );
                assert_eq!(
                    node.children
                        .iter()
                        .map(|n| n.label.as_str())
                        .collect::<Vec<_>>(),
                    strings(&expected["children"]),
                    "{file} {path:?} child order"
                );
                if let Some(ancestor) = expected.get("foldAncestor") {
                    let ancestor = node_at(&parsed.outline, &strings(ancestor));
                    assert!(
                        parsed.folds.iter().any(|fold| fold.range == ancestor.range),
                        "{file} expected folded ancestor"
                    );
                    assert!(
                        ancestor.range.start < node.selection.start
                            && node.selection.end < ancestor.range.end
                    );
                }
            }
            for expected in fixture["regions"].as_array().unwrap() {
                let open = expected["open"].as_str().unwrap();
                let start = text.find(open).unwrap() + open.len();
                let end = start
                    + text[start..]
                        .find(expected["close"].as_str().unwrap())
                        .unwrap();
                let range = mapper.native(start..end).unwrap();
                assert!(
                    parsed
                        .injections
                        .iter()
                        .any(|r| r.language == expected["language"].as_str().unwrap()
                            && r.range == range),
                    "{file} expected {} region {:?}",
                    expected["language"],
                    &text[start..end]
                );
            }
        }
    }
}
