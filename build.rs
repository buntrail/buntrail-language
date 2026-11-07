fn main() {
    println!("cargo:rustc-check-cfg=cfg(patched_grammars)");
    println!("cargo:rustc-cfg=patched_grammars");
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("vendor/languages");
    for name in [
        "vue",
        "svelte",
        "blade",
        "dockerfile",
        "dockerfile_tick",
        "jsonc",
        "scss",
    ] {
        let source = root.join(name).join("src");
        let mut build = cc::Build::new();
        build
            .include(&source)
            .file(source.join("parser.c"))
            .warnings(false);
        if source.join("scanner.c").exists() {
            build.file(source.join("scanner.c"));
        }
        build.compile(&format!("buntrail_{name}"));
        println!("cargo:rerun-if-changed={}", source.display());
    }
}
