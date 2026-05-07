//! Integration tests for the `#[cocoindex::cached]` procedural macro.
//!
//! These tests verify that the macro correctly caches function results,
//! tracks cache_hits / cache_misses, and handles custom types.

use cocoindex::{App, CocoError, Ctx};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Custom struct returned by a cached function.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AnalysisResult {
    value: u64,
    label: String,
}

/// A cached async function with a struct return type.
#[cocoindex::cached]
async fn analyze_number(ctx: &Ctx, n: u32, label: &str) -> Result<AnalysisResult, CocoError> {
    // Simulate expensive work
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    Ok(AnalysisResult {
        value: (n as u64).wrapping_mul(2),
        label: label.to_string(),
    })
}

/// A cached sync function with a simple return type.
#[cocoindex::cached]
fn cached_add(ctx: &Ctx, a: i32, b: i32) -> Result<i32, CocoError> {
    Ok(a + b)
}

/// Helper to create a temp App.
fn make_app(dir: &Path) -> App {
    App::open("cached_test", dir).unwrap()
}

#[tokio::test]
async fn test_cached_macro_cold_and_warm() -> anyhow::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let app = make_app(db_dir.path());

    let (_, stats) = app
        .run(|ctx| async move {
            // Cold calls
            let r1 = analyze_number(&ctx, 42, "answer").await?;
            assert_eq!(r1.value, 84);
            assert_eq!(r1.label, "answer");

            let r2 = analyze_number(&ctx, 100, "test").await?;
            assert_eq!(r2.value, 200);

            // Warm calls (same args = cache hits)
            let r1_cached = analyze_number(&ctx, 42, "answer").await?;
            assert_eq!(r1_cached, r1);

            let r2_cached = analyze_number(&ctx, 100, "test").await?;
            assert_eq!(r2_cached, r2);

            // Different args = cache miss
            let r3 = analyze_number(&ctx, 99, "new").await?;
            assert_eq!(r3.value, 198);

            Ok(())
        })
        .await?;

    // 3 cold + 2 warm + 1 different = 5 total calls
    // cache_misses: 3 (42/answer, 100/test, 99/new)
    // cache_hits:   2 (42/answer repeated, 100/test repeated)
    assert_eq!(stats.cache_misses, 3, "should have 3 cache misses");
    assert_eq!(stats.cache_hits, 2, "should have 2 cache hits");

    Ok(())
}

#[tokio::test]
async fn test_cached_macro_sync_fn() -> anyhow::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let app = make_app(db_dir.path());

    let (_, stats) = app
        .run(|ctx| async move {
            // Cold calls
            let r1 = cached_add(&ctx, 2, 3)?;
            assert_eq!(r1, 5);

            let r2 = cached_add(&ctx, 10, 20)?;
            assert_eq!(r2, 30);

            // Warm calls
            let r1_cached = cached_add(&ctx, 2, 3)?;
            assert_eq!(r1_cached, 5);

            let r2_cached = cached_add(&ctx, 10, 20)?;
            assert_eq!(r2_cached, 30);

            Ok(())
        })
        .await?;

    assert_eq!(
        stats.cache_misses, 2,
        "should have 2 cache misses (2+3, 10+20)"
    );
    assert_eq!(stats.cache_hits, 2, "should have 2 cache hits (same args)");

    Ok(())
}

#[tokio::test]
async fn test_cached_cross_session_persistence() -> anyhow::Result<()> {
    // Verify that cached values persist across app.run() sessions.
    let db_dir = tempfile::tempdir()?;
    let app = make_app(db_dir.path());

    // Session 1: populate cache
    app.run(|ctx| async move {
        let r = cached_add(&ctx, 100, 200)?;
        assert_eq!(r, 300);
        Ok(())
    })
    .await?;

    // Session 2: read from cache
    let (_, stats) = app
        .run(|ctx| async move {
            let r = cached_add(&ctx, 100, 200)?;
            assert_eq!(r, 300);
            Ok(())
        })
        .await?;

    assert_eq!(stats.cache_hits, 1, "second session should hit cache");
    assert_eq!(stats.cache_misses, 0, "no misses in second session");

    Ok(())
}
