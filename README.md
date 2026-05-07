# CocoIndex Rust SDK

High-performance code indexing and caching library for Rust, built on LMDB.

[![crates.io](https://img.shields.io/crates/v/cocoindex.svg)](https://crates.io/crates/cocoindex)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Overview

CocoIndex provides native Rust access to CocoIndex's core features:
- **LMDB-backed caching** - Fast, persistent memoization
- **File fingerprinting** - Skip-unchanged logic for incremental sync
- **fs::walk()** - Efficient file traversal with glob/extension filtering
- **Proc macros** - `#[cached]` and `#[component]` for zero-cost abstraction

## Installation

```toml
[dependencies]
cocoindex = "0.1.0"
cocoindex_macro = "0.1.0"
```

Requires Rust 1.75+.

## Quick Start

```rust
use cocoindex::App;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::open("myindex", Path::new("/tmp/cocoindex_db"))?;

    app.run(|ctx| async move {
        // Your code here
        let content = ctx.read_file(Path::new("src/main.rs")).await?;
        Ok(content)
    }).await?;

    Ok(())
}
```

## Core Concepts

### App

The `App` struct is the entry point, owning the LMDB environment:

```rust
let app = App::open("myindex", Path::new("/tmp/cocoindex_db"))?;
```

### Ctx

`Ctx` provides runtime context for indexed code operations. It carries session ID, cache reference, and LMDB environment:

```rust
app.run(|ctx| async move {
    let content = ctx.read_file(path).await?;
    Ok(())
}).await?;
```

### fs::walk()

File traversal with fingerprint-based change detection:

```rust
use cocoindex::fs;

let walker = fs::walk(root)
    .extension("rs")
    .glob("**/*.rs")
    .include_hidden()
    .build();

for entry_result in walker {
    let entry = entry_result?;
    println!("{:?} (fingerprint: {:?})", entry.path(), entry.fingerprint());
}
```

### Proc Macros

#### #[cocoindex::cached]

Automatic cache key generation and LMDB lookup:

```rust
#[cocoindex::cached]
async fn process_file(ctx: &Ctx, path: &str) -> Result<String> {
    // Logic here - result cached based on path argument
    Ok(content)
}
```

With custom key expression:

```rust
#[cocoindex::cached(key_expr = { format!("{}:{}", path, hash) })]
async fn process_file(ctx: &Ctx, path: &str, hash: u64) -> Result<String> {
    Ok(content)
}
```

#### #[cocoindex::component]

Marks functions as pipeline components for stats tracking:

```rust
#[cocoindex::component("my_processor")]
async fn process(ctx: &Ctx, input: &str) -> Result<String> {
    Ok(result)
}
```

## Examples

```bash
# Run all examples
cargo run --example 01_hello_cocoindex
cargo run --example 02_code_walk
cargo run --example 03_multi_codebase
cargo run --example 04_with_llm
```

| Example | Description |
|---------|-------------|
| `01_hello_cocoindex` | Minimal App usage |
| `02_code_walk` | File traversal with fingerprints |
| `03_multi_codebase` | Indexing multiple projects |
| `04_with_llm` | LLM workflow integration (placeholder) |

## Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration_test

# Benchmarks
cargo bench
```

## Documentation

```bash
# Generate API docs
cargo doc --open

# Doc tests
cargo test --doc
```

## Architecture

```
cocoindex-rust-sdk (crate)
├── App          # Entry point, owns LMDB environment
├── Ctx<'_>      # Pipeline context, passed explicitly to all operations
├── fs::walk()   # File traversal with fingerprint-based change detection
├── cache        # LMDB-backed memoization layer
├── macros       # #[cocoindex::cached] and #[cocoindex::component] proc macros
└── RunStats     # Execution statistics (cache hits, files scanned, etc.)
```

## Tech Stack

- `lmdb-rkv` - LMDB bindings
- `tokio` - Async runtime
- `xxhash-rust` - Fingerprint calculation
- `thiserror` / `anyhow` - Error handling
- `serde` / `serde_json` - Serialization
- `walkdir` / `glob` - File traversal

## License

MIT License - see [LICENSE](LICENSE)

## Links

- [crates.io](https://crates.io/crates/cocoindex)
- [CocoIndex Python SDK](https://github.com/cocoindex-io/cocoindex)