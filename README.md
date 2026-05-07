# CocoIndex Rust SDK

[![crates.io](https://img.shields.io/crates/v/cocoindex.svg)](https://crates.io/crates/cocoindex)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**CocoIndex** is an incremental data sync engine with native Rust SDK support. This crate provides LMDB-backed caching, file fingerprinting for skip-unchanged logic, and procedural macros for ergonomic cache management.

## Quick Start

```rust
use cocoindex::App;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::open("myindex", Path::new("/tmp/cocoindex_db"))?;

    let (result, stats) = app.run(|ctx| async move {
        ctx.cache_set("greeting", b"Hello from CocoIndex!")?;
        let value = ctx.cache_get("greeting")?.unwrap_or_default();
        println!("Cached: {}", String::from_utf8_lossy(&value));
        Ok("done")
    }).await?;

    println!("Cache hits: {}, misses: {}", stats.cache_hits, stats.cache_misses);
    Ok(())
}
```

Requires Rust 1.75+.

## API Overview

### App — entry point, owns LMDB environment

```rust
let app = App::open("myindex", Path::new("/tmp/cocoindex_db"))?;
```

| Method | Description |
|--------|-------------|
| `App::open(name, path)` | Open or create a CocoIndex database |
| `app.run(f)` | Execute a task with a `Ctx` context; returns `(T, RunStats)` |
| `app.db_path()` | Get the database directory path |
| `app.cache()` | Get a reference to the underlying cache |

### Ctx — pipeline context

`Ctx` is passed to all operations inside `app.run()`. It carries session ID, cache, LMDB environment reference, and shared `RunStats`.

```rust
app.run(|ctx| async move {
    let content = ctx.read_file(path).await?;
    let stats = ctx.stats();
    // ...
}).await?;
```

| Method | Description |
|--------|-------------|
| `ctx.read_file(path)` | Read a file as `String` |
| `ctx.read_file_bytes(path)` | Read a file as `Vec<u8>` |
| `ctx.session_id()` | Get the session UUID |
| `ctx.db_path()` | Get the database path |
| `ctx.cache_get(key)` | Get from cache with auto hit/miss counting |
| `ctx.cache_set(key, value)` | Set cache value |
| `ctx.stats()` | Get a snapshot of current `RunStats` |
| `ctx.stats_mut()` | Get mutable access to `RunStats` |

### fs::walk() — file traversal with fingerprinting

```rust
use cocoindex::fs;

let walker = fs::walk(Path::new("./src"))
    .extension("rs")
    .glob("**/*.rs")
    .build();

for entry in walker {
    let entry = entry?;
    println!("{:?} (fingerprint: {:?})", entry.path(), entry.fingerprint());
}
```

| Method | Description |
|--------|-------------|
| `fs::walk(root)` | Create a `WalkBuilder` for the given root path |
| `.glob(pattern)` | Filter by glob pattern |
| `.extension(ext)` | Filter by file extension |
| `.include_hidden()` | Include hidden files (default: excluded) |
| `.build()` | Build iterator (no caching) |
| `.walk_with_cache(cache)` | Build iterator with fingerprint caching for skip-unchanged |
| `.walk_with_ctx(ctx)` | Build iterator using Ctx for cache + stats |

**FileEntry** fields: `path`, `is_dir`, `fingerprint: Option<Fingerprint>`, `size`

**Fingerprint** fields: `content_hash: u64` (xxh3 of content), `code_hash: u64`

### #[cocoindex::cached] — automatic caching

The `#[cocoindex::cached]` procedural macro wraps a function to automatically cache results in LMDB:

```rust
#[cocoindex::cached]
async fn process(ctx: &Ctx, input: &str) -> Result<String, CocoError> {
    // Expensive logic here — result is cached by input parameter
    Ok(result)
}
```

Key behavior:
- Cache key is derived from **all non-Ctx parameters** via serde + xxh3 hashing
- First call with fresh arguments = cache miss (function body runs)
- Subsequent calls with same arguments = cache hit (function body skipped)
- `cache_hits` / `cache_misses` are automatically counted via `ctx.cache_get()`
- Works with both `async fn` and sync `fn`
- Return type must be `Result<T, E>` where `T: Serialize + Deserialize`

### #[cocoindex::component] — stats tracking

Marks a function as a pipeline component, incrementing `components_executed` in `RunStats`:

```rust
#[cocoindex::component("my_processor")]
async fn process(ctx: &Ctx, input: &str) -> Result<String, CocoError> {
    Ok(input.to_string())
}
```

### RunStats — execution statistics

Returned by `app.run()`. Also accessible via `ctx.stats()` and `ctx.stats_mut()` during execution.

| Field | Description |
|-------|-------------|
| `cache_hits` | Number of cache lookups that found a value |
| `cache_misses` | Number of cache lookups that found no value |
| `files_processed` | Total files read (including skipped-unchanged) |
| `bytes_read` | Total bytes of file content read |
| `components_executed` | Number of `#[component]` functions executed |
| `elapsed_ms` | Total run time in milliseconds |

## Examples

```bash
cargo run --example 01_hello_cocoindex
cargo run --example 02_code_walk
cargo run --example 03_cached_compute
cargo run --example 04_with_llm
```

| Example | Description |
|---------|-------------|
| `01_hello_cocoindex` | Minimal App usage with cache read/write and RunStats |
| `02_code_walk` | File traversal with fingerprint-based skip-unchanged (3 walks) |
| `03_cached_compute` | `#[cocoindex::cached]` macro with struct return and stats |
| `04_with_llm` | Simulated LLM API call caching with `#[cocoindex::cached]` |

## Testing

```bash
# All tests (31 tests across 4 test targets + doc tests)
cargo test

# Specific test suites
cargo test --test integration_test   # App, Ctx, Cache infrastructure
cargo test --test fs_walk            # File traversal and skip-unchanged
cargo test --test cached_macro       # #[cached] macro end-to-end
cargo test --test trybuild           # Procedural macro compile tests
cargo test --doc                     # Doc tests

# Benchmarks
cargo bench
```

## Architecture

```
cocoindex-rust-sdk
├── App          # Entry point, owns LMDB environment
├── Ctx          # Pipeline context (session ID, cache, stats)
├── fs::walk()   # File traversal with fingerprint change detection
├── cache        # LMDB-backed memoization
├── macros       # #[cocoindex::cached] and #[cocoindex::component] proc macros
└── RunStats     # Execution statistics
```

All pipeline operations take `&Ctx` as an explicit parameter rather than using global state. The `#[cocoindex::cached]` macro automatically handles cache key generation and LMDB lookups based on non-Ctx parameters.

## Tech Stack

- `lmdb-rkv` — LMDB bindings
- `tokio` — Async runtime
- `xxhash-rust` — Fingerprint calculation (xxh3)
- `walkdir` + `glob` — File traversal
- `serde` / `serde_json` — Serialization for cache keys and values
- `thiserror` / `anyhow` — Error handling
- `proc-macro2` / `quote` / `syn` — Procedural macros

## License

MIT License — see [LICENSE](LICENSE)

## Links

- [Crates.io](https://crates.io/crates/cocoindex)
- [CocoIndex Python SDK](https://github.com/cocoindex-io/cocoindex)
- [GitHub Repository](https://github.com/cocoindex-io/cocoindex-rust)
