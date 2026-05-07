# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the **CocoIndex Rust SDK** - an alpha-stage Rust SDK for the CocoIndex incremental data sync engine. The SDK provides native Rust access to CocoIndex's core features (LMDB-backed caching, file fingerprinting, incremental sync) without requiring Python bindings.

Key references:
- [CocoIndex Python SDK](https://github.com/cocoindex-io/cocoindex)
- [Issue #1667: Ergonomic Rust SDK](https://github.com/cocoindex-io/cocoindex/issues/1667)

## Build & Test Commands

```bash
# Unit tests
cargo test

# Integration tests (requires Python SDK for equivalence comparison)
cargo test --test integration

# Benchmarks
cargo bench

# Documentation
cargo doc --open
```

## Architecture

```
cocoindex-rust-sdk (crate)
├── App          # Entry point, owns LMDB environment
├── Ctx<'_>      # Pipeline context, passed explicitly to all operations
├── fs::walk()   # File traversal with fingerprint-based change detection
├── cache        # LMDB-backed memoization
├── macros       # #[cocoindex::cached] and #[cocoindex::component] proc macros
└── RunStats     # Execution statistics (cache hits, files scanned, etc.)
```

Core design: All pipeline operations take `&Ctx` as an explicit parameter rather than using global state. The `#[cocoindex::cached]` macro automatically handles cache key generation and LMDB lookups based on non-Ctx parameters.

## Key Data Structures

- `App::open(name, db_path)` - Opens or creates a CocoIndex app
- `Ctx<'_>` - Carries session ID, LMDB env reference, config, and stats
- `FileEntry` - File path + lazy content loading + eager fingerprint
- `Fingerprint` - content_hash + code_hash for skip-unchanged logic

## Tech Stack

- Rust 1.75+ (async traits, proc-macro2 stable)
- lmdb-rkv for LMDB bindings
- tokio async runtime
- xxhash-rust for fingerprints
- thiserror + anyhow for error handling

## Module Structure (src/)

| File | Responsibility |
|------|----------------|
| `lib.rs` | Public API exports |
| `app.rs` | App struct and LMDB lifecycle |
| `ctx.rs` | Ctx struct and context operations |
| `fs.rs` | WalkBuilder and FileEntry |
| `cache.rs` | LMDB memoization layer |
| `macros.rs` | proc macro definitions |
| `stats.rs` | RunStats struct |

## Phase Timeline

- Phase 1 (Week 1-2): Infrastructure - App, Ctx, LMDB integration
- Phase 2 (Week 2-3): fs::walk() with fingerprint
- Phase 3 (Week 3-4): Proc macros (#[cached], #[component])
- Phase 4 (Week 4-5): Integration + examples
- Phase 5 (Week 5-6): Docs + crates.io publish