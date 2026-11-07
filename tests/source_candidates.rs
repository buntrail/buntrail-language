extern crate buntrail_language;
use tree_sitter::{Language, Parser, Query};
use tree_sitter_language::LanguageFn;
unsafe extern "C" {
    fn tree_sitter_vue() -> *const ();
    fn tree_sitter_svelte() -> *const ();
    fn tree_sitter_blade() -> *const ();
    fn tree_sitter_dockerfile() -> *const ();
}
fn probe(name: &str, function: unsafe extern "C" fn() -> *const (), fixture: &str) {
    let language: Language = unsafe { LanguageFn::from_raw(function) }.into();
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let text = std::fs::read_to_string(format!("fixtures/languages/{fixture}")).unwrap();
    let tree = parser.parse(&text, None).unwrap();
    let suffix = if name == "vue" {
        "queries/vue"
    } else {
        "queries"
    };
    let path = std::path::PathBuf::from(format!("vendor/languages/{name}/{suffix}"));
    let mut errors = vec![];
    if path.exists() {
        for file in std::fs::read_dir(path).unwrap() {
            let file = file.unwrap().path();
            if file.extension().is_some_and(|e| e == "scm") {
                let query = std::fs::read_to_string(&file).unwrap();
                if let Err(error) = Query::new(&language, &query) {
                    errors.push(format!("{}: {error}", file.display()));
                }
            }
        }
    } else {
        errors.push("No upstream queries directory".into());
    }
    println!(
        "CANDIDATE {name}: ABI={} parseErrors={} queryErrors={errors:?} tree={}",
        language.abi_version(),
        tree.root_node().has_error(),
        tree.root_node().to_sexp()
    );
    if fixture == "BladeEscapes.blade.php" {
        assert!(
            !tree.root_node().to_sexp().contains("php_statement"),
            "verbatim/escaped expressions are literal"
        );
        assert_eq!(
            tree.root_node().to_sexp().matches("(conditional ").count(),
            1,
            "unknown directives must not absorb the recognized @if"
        );
    }
    if fixture == "SvelteCurrentSyntax.svelte" {
        assert!(tree.root_node().to_sexp().contains("attach_tag"));
    }
    inspect(tree.root_node(), &text);
    assert!(
        !tree.root_node().has_error(),
        "{name} required syntax is not accepted"
    );
    assert!(
        errors.is_empty(),
        "{name} query compatibility is not proven: {errors:?}"
    );
}
#[test]
fn vue() {
    probe("vue", tree_sitter_vue, "VueMixed.vue");
}
#[test]
fn svelte() {
    probe("svelte", tree_sitter_svelte, "SvelteModern.svelte");
}
#[test]
fn blade() {
    probe("blade", tree_sitter_blade, "Profile.blade.php");
}
#[test]
fn dockerfile() {
    probe(
        "dockerfile",
        tree_sitter_dockerfile,
        "Dockerfile.multistage",
    );
}

#[test]
fn vue_fallback() {
    probe("vue", tree_sitter_vue, "VueFallback.vue");
}
#[test]
fn svelte_current_syntax() {
    probe("svelte", tree_sitter_svelte, "SvelteCurrentSyntax.svelte");
}
#[test]
fn blade_escapes() {
    probe("blade", tree_sitter_blade, "BladeEscapes.blade.php");
}
#[test]
fn dockerfile_escape_copy_heredoc() {
    #[cfg(patched_grammars)]
    probe(
        "dockerfile_tick",
        tree_sitter_dockerfile_tick,
        "Dockerfile.escape",
    );
    #[cfg(not(patched_grammars))]
    probe("dockerfile", tree_sitter_dockerfile, "Dockerfile.escape");
}

fn inspect(node: tree_sitter::Node, text: &str) {
    if node.is_error()
        || node.is_missing()
        || node.kind() == "directive_start"
        || node.kind() == "php_only"
    {
        println!(
            "NODE {} {:?} {:?}",
            node.kind(),
            node.range(),
            &text[node.byte_range()]
        );
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        inspect(child, text);
    }
}

#[cfg(patched_grammars)]
unsafe extern "C" {
    fn tree_sitter_jsonc() -> *const ();
}
#[cfg(patched_grammars)]
#[test]
fn jsonc_patch_keeps_properties_and_rejects_double_commas() {
    let language: Language = unsafe { LanguageFn::from_raw(tree_sitter_jsonc) }.into();
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    Query::new(&language, tree_sitter_json::HIGHLIGHTS_QUERY).unwrap();
    let fixture = include_str!("../fixtures/languages/Settings.jsonc");
    let tree = parser.parse(fixture, None).unwrap();
    assert!(!tree.root_node().has_error());
    assert_eq!(tree.root_node().to_sexp().matches("(pair ").count(), 1);
    for invalid in ["{,}", "[1,,]", "{\"a\":1,,}"] {
        assert!(parser.parse(invalid, None).unwrap().root_node().has_error());
    }
}

#[cfg(patched_grammars)]
unsafe extern "C" {
    fn tree_sitter_dockerfile_tick() -> *const ();
}
#[cfg(patched_grammars)]
#[test]
fn docker_dialect_selection_and_header_edits_preserve_ranges() {
    use buntrail_language::{DockerEscape, docker_escape};
    let cases = [
        (
            "# escape=`\r\nFROM alpine AS hello\r\nRUN echo hello `\r\n && true\r\nCOPY <<END /message\r\nhello\r\nEND\r\n",
            DockerEscape::Backtick,
            true,
        ),
        (
            "FROM alpine\nRUN echo `date`\n",
            DockerEscape::Backslash,
            true,
        ),
        (
            "# ordinary comment\n# escape=`\nFROM alpine\nRUN echo hello `\n && true\n",
            DockerEscape::Backslash,
            false,
        ),
        (
            "\n# escape=`\nFROM alpine\nRUN echo hello `\n && true\n",
            DockerEscape::Backslash,
            false,
        ),
        (
            "FROM alpine\n# escape=`\nRUN echo hello `\n && true\n",
            DockerEscape::Backslash,
            false,
        ),
    ];
    for (text, expected, valid) in cases {
        let mode = docker_escape(text).unwrap();
        assert_eq!(mode, expected);
        let function = if mode == DockerEscape::Backtick {
            tree_sitter_dockerfile_tick
        } else {
            tree_sitter_dockerfile
        };
        let language: Language = unsafe { LanguageFn::from_raw(function) }.into();
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(text, None).unwrap();
        assert_eq!(
            !tree.root_node().has_error(),
            valid,
            "{text}\n{}",
            tree.root_node().to_sexp()
        );
        assert_eq!(tree.root_node().end_byte(), text.len());
    }
    // Header edit changes dialect: discard the old grammar's tree entirely.
    let original = "# escape=`\nFROM alpine\nRUN echo hello `\n && true\n";
    let changed = format!("# ordinary comment\n{original}");
    assert_ne!(
        docker_escape(original).unwrap(),
        docker_escape(&changed).unwrap()
    );
    let mut parser = Parser::new();
    parser
        .set_language(&unsafe { LanguageFn::from_raw(tree_sitter_dockerfile_tick) }.into())
        .unwrap();
    let old = parser.parse(original, None).unwrap();
    assert!(!old.root_node().has_error());
    parser
        .set_language(&unsafe { LanguageFn::from_raw(tree_sitter_dockerfile) }.into())
        .unwrap();
    let fresh = parser.parse(&changed, None).unwrap();
    assert!(fresh.root_node().has_error());
}
