//! Example 03: Cached Computation with `#[cocoindex::cached]`
//!
//! Demonstrates the `#[cocoindex::cached]` procedural macro:
//!   1. Define an expensive function annotated with `#[cocoindex::cached]`
//!   2. First call with fresh arguments = cold cache, function body runs
//!   3. Second call with same arguments = cache hit, function body skipped
//!   4. Inspect RunStats to observe cache_hits vs cache_misses

use cocoindex::{App, CocoError, Ctx};
use serde::{Deserialize, Serialize};

/// The result of our cached computation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ComputeResult {
    /// The computed value
    value: u64,
    /// Description of what was computed
    label: String,
}

/// An expensive computation annotated with `#[cocoindex::cached]`.
///
/// The macro automatically:
///   - Generates a cache key from all non-Ctx parameters (serialized via serde)
///   - Checks the LMDB cache before calling the function body
///   - Stores the serialized result on cache miss
///   - Counts cache_hits / cache_misses via ctx.cache_get()
///
/// The function signature must be `async fn name(ctx: &Ctx, ...) -> Result<T>`
/// where T: Serialize + Deserialize.
#[cocoindex::cached]
async fn compute_fibonacci(ctx: &Ctx, n: u32) -> Result<u64, CocoError> {
    // Simulate an expensive computation
    let result = fibonacci(n);
    println!("    [computed] fibonacci({}) = {} (cold cache)", n, result);
    Ok(result)
}

/// A cached function returning a struct.
#[cocoindex::cached]
async fn analyze_number(ctx: &Ctx, n: u32, label: &str) -> Result<ComputeResult, CocoError> {
    // Simulate expensive analysis
    println!("    [computed] analyze({}, {}) (cold cache)", n, label);
    Ok(ComputeResult {
        value: (n as u64).wrapping_mul(2).wrapping_add(n as u64 / 3),
        label: label.to_string(),
    })
}

fn fibonacci(n: u32) -> u64 {
    if n <= 1 {
        return n as u64;
    }
    let mut a = 0u64;
    let mut b = 1u64;
    for _ in 2..=n {
        let next = a.wrapping_add(b);
        a = b;
        b = next;
    }
    b
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let app = App::open("cached-compute", db_dir.path())?;
    println!("CocoIndex cached compute example\n");

    let (_, stats) = app
        .run(|ctx| async move {
            // --- Cold calls ---
            println!("--- Cold cache (first calls) ---");
            let fib10 = compute_fibonacci(&ctx, 10).await?;
            let fib20 = compute_fibonacci(&ctx, 20).await?;
            let analysis = analyze_number(&ctx, 42, "answer").await?;
            println!("  fibonacci(10) = {}", fib10);
            println!("  fibonacci(20) = {}", fib20);
            println!("  analysis      = {:?}", analysis);

            // --- Hot calls (same arguments = cache hits) ---
            println!("\n--- Warm cache (cache hits expected) ---");
            let fib10_cached = compute_fibonacci(&ctx, 10).await?;
            let fib20_cached = compute_fibonacci(&ctx, 20).await?;
            let analysis_cached = analyze_number(&ctx, 42, "answer").await?;
            println!("  fibonacci(10) = {} (from cache)", fib10_cached);
            println!("  fibonacci(20) = {} (from cache)", fib20_cached);
            println!("  analysis      = {:?} (from cache)", analysis_cached);

            // Verify values match
            assert_eq!(fib10, fib10_cached);
            assert_eq!(fib20, fib20_cached);
            assert_eq!(analysis, analysis_cached);

            println!("\n--- Stats from ctx ---");
            let s = ctx.stats();
            println!("  cache_hits:   {}", s.cache_hits);
            println!("  cache_misses: {}", s.cache_misses);
            println!("  files_processed: {}", s.files_processed);

            Ok(())
        })
        .await?;

    println!("\n--- Final stats ---");
    println!("  cache_hits:   {}", stats.cache_hits);
    println!("  cache_misses: {}", stats.cache_misses);
    assert!(stats.cache_hits >= 2, "should have at least 2 cache hits");
    assert!(
        stats.cache_misses >= 2,
        "should have at least 2 cache misses"
    );

    println!("\nAll assertions passed — cached macro is working correctly!");
    Ok(())
}
