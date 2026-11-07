use tree_sitter::Language;
use tree_sitter_language::LanguageFn;
unsafe extern "C" {
    fn tree_sitter_vue() -> *const ();
    fn tree_sitter_svelte() -> *const ();
    fn tree_sitter_blade() -> *const ();
    fn tree_sitter_dockerfile() -> *const ();
    fn tree_sitter_dockerfile_tick() -> *const ();
    fn tree_sitter_jsonc() -> *const ();
    fn tree_sitter_scss() -> *const ();
}
pub const IDS: &[&str] = &[
    "javascript",
    "jsx",
    "typescript",
    "tsx",
    "php",
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
    "markdown",
    "yaml",
    "json",
    "jsonc",
    "toml",
    "css",
    "scss",
    "shell",
    "dotenv",
    "ignore",
    "text",
];
pub fn alias(value: &str) -> Option<&'static str> {
    Some(match value.to_ascii_lowercase().as_str() {
        "js" | "javascript" => "javascript",
        "jsx" => "jsx",
        "ts" | "typescript" => "typescript",
        "tsx" => "tsx",
        "bash" | "sh" | "zsh" | "shell" => "shell",
        "yml" | "yaml" => "yaml",
        "md" | "markdown" => "markdown",
        "c++" | "cxx" | "cpp" => "cpp",
        "php" => "php",
        "java" => "java",
        "swift" => "swift",
        "c" => "c",
        "go" => "go",
        "dart" => "dart",
        "html" => "html",
        "xml" => "xml",
        "vue" => "vue",
        "svelte" => "svelte",
        "blade" => "blade",
        "dockerfile" => "dockerfile",
        "json" => "json",
        "jsonc" => "jsonc",
        "toml" => "toml",
        "css" => "css",
        "scss" => "scss",
        "dotenv" => "dotenv",
        "ignore" => "ignore",
        "text" | "plaintext" => "text",
        _ => return None,
    })
}
/// Explicit override wins, then exact/compound filename associations, then shebang.
/// Unknown overrides deliberately remain plain text instead of silently using another grammar.
pub fn select(filename: &str, first_line: &str, override_id: Option<&str>) -> &'static str {
    if let Some(value) = override_id {
        return alias(value).unwrap_or("text");
    }
    let name = filename.rsplit('/').next().unwrap_or(filename);
    if name.ends_with(".blade.php") {
        return "blade";
    }
    if name == "Dockerfile"
        || name.starts_with("Dockerfile.")
        || name == "Containerfile"
        || name.starts_with("Containerfile.")
    {
        return "dockerfile";
    }
    if name == ".env" || name.starts_with(".env.") {
        return "dotenv";
    }
    if matches!(name, ".gitignore" | ".dockerignore") {
        return "ignore";
    }
    let ext = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    match ext {
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "jsx" => "jsx",
        "tsx" => "tsx",
        "h" | "c" => "c",
        "cc" | "cpp" | "cxx" | "c++" | "hh" | "hpp" | "hxx" | "inl" | "ipp" => "cpp",
        "htm" | "html" => "html",
        "php" => "php",
        "java" => "java",
        "swift" => "swift",
        "go" => "go",
        "dart" => "dart",
        "xml" => "xml",
        "vue" => "vue",
        "svelte" => "svelte",
        "md" | "markdown" => "markdown",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "jsonc" => "jsonc",
        "toml" => "toml",
        "css" => "css",
        "scss" => "scss",
        "sh" | "bash" | "zsh" => "shell",
        _ => {
            let words = first_line
                .strip_prefix("#!")
                .unwrap_or("")
                .split_whitespace()
                .collect::<Vec<_>>();
            if words
                .iter()
                .any(|w| matches!(w.rsplit('/').next().unwrap_or(w), "bash" | "sh" | "zsh"))
            {
                "shell"
            } else if words.iter().any(|w| w.ends_with("node")) {
                "javascript"
            } else {
                "text"
            }
        }
    }
}
pub fn grammar(id: &str) -> Option<Language> {
    Some(match id {
        "javascript" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        "php-only" => tree_sitter_php::LANGUAGE_PHP_ONLY.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "swift" => tree_sitter_swift::LANGUAGE.into(),
        "c" => tree_sitter_c::LANGUAGE.into(),
        "cpp" => tree_sitter_cpp::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "dart" => tree_sitter_dart::LANGUAGE.into(),
        "html" => tree_sitter_html::LANGUAGE.into(),
        "xml" => tree_sitter_xml::LANGUAGE_XML.into(),
        "markdown" => tree_sitter_md::LANGUAGE.into(),
        "markdown-inline" => tree_sitter_md::INLINE_LANGUAGE.into(),
        "yaml" => tree_sitter_yaml::LANGUAGE.into(),
        "json" => tree_sitter_json::LANGUAGE.into(),
        "toml" => tree_sitter_toml_ng::LANGUAGE.into(),
        "css" => tree_sitter_css::LANGUAGE.into(),
        "shell" => tree_sitter_bash::LANGUAGE.into(),
        name => {
            let function: unsafe extern "C" fn() -> *const () = match name {
                "vue" => tree_sitter_vue,
                "svelte" => tree_sitter_svelte,
                "blade" => tree_sitter_blade,
                "dockerfile" => tree_sitter_dockerfile,
                "dockerfile_tick" => tree_sitter_dockerfile_tick,
                "jsonc" => tree_sitter_jsonc,
                "scss" => tree_sitter_scss,
                _ => return None,
            };
            unsafe { LanguageFn::from_raw(function) }.into()
        }
    })
}
