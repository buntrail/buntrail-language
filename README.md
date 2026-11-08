# buntrail-language

Language analysis using Tree-sitter.

## Features

Highlighting, folding, symbols, coordinate mapping and bundled patched grammars.

## Usage

Add to your Cargo.toml after the initial registry release:

```toml
[dependencies]
buntrail-language = "0.2.0"
```

```rust
let adapter = buntrail_language::LanguageAdapter::default();
drop(adapter);
```

Rust 1.99.1 (edition 2024). A C compiler builds the included grammars; no grammar downloads or Node.js are needed.

```sh
cargo fetch
scripts/check.sh
```

## Technologies and support

Rust, Cargo, Tree-sitter and generated C grammars. The core library is portable. Target-native acceptance must be verified separately.

API documentation is built with `cargo doc --no-deps`. Public dependency types
remain in their originating Buntrail crates; depend on those crates directly
when constructing those types. No API stability beyond the declared 0.x version
is promised. See [release instructions](docs/releasing.md).

## Collaborators and contributing

Maintained by Bunga Mungil for the [Buntrail organization](https://github.com/buntrail).
Contributions are welcome through issues and small pull requests in
https://github.com/buntrail/buntrail-language. Read [CONTRIBUTING.md](CONTRIBUTING.md)
and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Contact: bungamungil@icloud.com.
Report vulnerabilities privately using [SECURITY.md](SECURITY.md).

## License

First-party code is [MIT licensed](LICENSE), Copyright (c) 2026 Bunga Mungil.
Third-party dependencies retain their own licenses. See
[third-party notices](THIRD_PARTY_NOTICES.md).
