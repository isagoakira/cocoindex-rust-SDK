//! Example 02: Code Walk with skip-unchanged
//!
//! Demonstrates file system traversal with fingerprint-based change detection:
//!   1. First walk = all files are yielded (cold cache)
//!   2. Second walk = 0 files yielded (all fingerprints match)
//!   3. Modify one file, third walk = only the changed file is yielded

use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;
use cocoindex::{fs, cache::Cache, CocoError};
use lmdb::Environment;

fn open_temp_cache(dir: &Path) -> (Arc<Environment>, Cache) {
    std::fs::create_dir_all(dir).unwrap();
    let env = Arc::new(
        Environment::new()
            .set_map_size(1024 * 1024)
            .set_max_dbs(16)
            .set_max_readers(8)
            .open(dir)
            .unwrap(),
    );
    let cache = Cache::open(&env).unwrap();
    (env, cache)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --- Setup: create a temp directory with a few source files ---
    let src_dir = tempdir()?;
    let root = src_dir.path();

    std::fs::write(root.join("main.rs"), "fn main() {\n    println!(\"hello\");\n}\n")?;
    std::fs::write(root.join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n")?;
    std::fs::write(root.join("README.md"), "# Example Project\n")?;

    // --- Setup: LMDB cache for fingerprint storage ---
    let cache_dir = tempdir()?;
    let (_env, cache) = open_temp_cache(cache_dir.path());

    println!("=== Walk 1: cold cache (all files yielded) ===");
    let entries1: Vec<_> = fs::walk(root)
        .glob("**/*.rs")
        .walk_with_cache(&cache)?
        .filter(|e| e.as_ref().map(|e| !e.is_dir()).unwrap_or(true))
        .collect::<Result<Vec<_>, CocoError>>()?;
    println!("  Yielded {} files:", entries1.len());
    for entry in &entries1 {
        println!("    - {}  (fingerprint: {:016x})",
            entry.path().display(),
            entry.fingerprint().map(|f| f.content_hash).unwrap_or(0));
    }

    println!("\n=== Walk 2: same content (0 files yielded — all skipped) ===");
    let entries2: Vec<_> = fs::walk(root)
        .glob("**/*.rs")
        .walk_with_cache(&cache)?
        .filter(|e| e.as_ref().map(|e| !e.is_dir()).unwrap_or(true))
        .collect::<Result<Vec<_>, CocoError>>()?;
    println!("  Yielded {} files (expected 0)", entries2.len());

    println!("\n=== Modify main.rs ===");
    std::fs::write(root.join("main.rs"), "fn main() {\n    println!(\"modified!\");\n}\n")?;

    println!("\n=== Walk 3: only changed file yielded ===");
    let entries3: Vec<_> = fs::walk(root)
        .glob("**/*.rs")
        .walk_with_cache(&cache)?
        .filter(|e| e.as_ref().map(|e| !e.is_dir()).unwrap_or(true))
        .collect::<Result<Vec<_>, CocoError>>()?;
    println!("  Yielded {} files (expected 1):", entries3.len());
    for entry in &entries3 {
        println!("    - {}  (fingerprint: {:016x})",
            entry.path().display(),
            entry.fingerprint().map(|f| f.content_hash).unwrap_or(0));
    }

    assert_eq!(entries1.len(), 2, "first walk: 2 .rs files");
    assert_eq!(entries2.len(), 0, "second walk: all skipped");
    assert_eq!(entries3.len(), 1, "third walk: only main.rs changed");
    println!("\nAll assertions passed — skip-unchanged is working correctly!");

    Ok(())
}
