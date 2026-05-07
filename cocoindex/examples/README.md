# CocoIndex Rust SDK Examples

This directory contains examples demonstrating how to use the CocoIndex Rust SDK.

## Getting Started

```rust
use cocoindex::App;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::open("myindex", Path::new("/tmp/cocoindex_db"))?;
    app.run(|ctx| async move {
        // Your code here
        Ok(())
    }).await?;
    Ok(())
}
```

## Examples

### 01 - Hello CocoIndex
`01_hello_cocoindex.rs`

A minimal example showing the basic App structure including cache read/write and RunStats inspection.

### 02 - Code Walk
`02_code_walk.rs`

Demonstrates file system traversal with fingerprint-based change detection:
- First walk yields all files (cold cache)
- Second walk yields zero files (fingerprints unchanged)
- After modification, only the changed file is yielded

### 03 - Cached Compute
`03_cached_compute.rs`

Demonstrates the `#[cocoindex::cached]` procedural macro with:
- `async` and `sync` cached function support
- Custom struct return types (Serialize + Deserialize)
- Cold vs warm cache behavior and RunStats inspection

### 04 - Multi-Codebase
`03_multi_codebase.rs`

Shows how to index multiple codebases using a shared CocoIndex app. Includes:
- Walking multiple project paths
- Extension filtering (.rs files)
- Statistics collection

### 05 - With LLM (placeholder)
`04_with_llm.rs`

Placeholder for future LLM workflow integration examples.

## Running Examples

```bash
# Run a specific example
cargo run --example 01_hello_cocoindex
cargo run --example 02_code_walk
cargo run --example 03_cached_compute
cargo run --example 03_multi_codebase
cargo run --example 04_with_llm
```

## Core Concepts

- **App** - Entry point, owns LMDB environment
- **Ctx** - Pipeline context passed to all operations
- **fs::walk()** - File traversal with fingerprint-based change detection
- **Cache** - LMDB-backed memoization layer
- **#[cocoindex::cached]** - Automatic cache key generation and LMDB lookup
- **#[cocoindex::component]** - Marks pipeline components for stats tracking

## Documentation

Full API documentation is available at:
```bash
cargo doc --open
```
