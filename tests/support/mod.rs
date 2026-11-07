use std::path::PathBuf;

// Synthetic fixtures retain logical names without occupying protected project paths.
pub fn fixture_path(logical: &str) -> PathBuf {
    let storage = match logical {
        ".env" => "dotenv.fixture",
        ".env.local" => "dotenv-local.fixture",
        "AGENTS.md" => "agent-instructions.fixture",
        name => name,
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/languages")
        .join(storage)
}
