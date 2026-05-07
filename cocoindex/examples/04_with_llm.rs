//! LLM Caching Example
//!
//! Demonstrates caching expensive LLM API calls using `#[cocoindex::cached]`.
//! The simulated LLM call just formats a response, but the pattern works
//! identically with real API calls.
//!
//! Key takeaways:
//!   1. Wrap LLM API calls with `#[cocoindex::cached]` to avoid redundant calls
//!   2. Cache key is derived from all non-Ctx parameters (prompt + model)
//!   3. First call = cold cache (simulated API runs)
//!   4. Repeated calls with same args = cache hit (instant, no API call)
//!   5. Different model name = new cache entry (different key)

use cocoindex::{App, CocoError, Ctx};
use serde::{Deserialize, Serialize};

/// The response from our simulated LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct LlmResponse {
    /// The original prompt text
    prompt: String,
    /// The simulated LLM response text
    response: String,
    /// Which model generated the response
    model: String,
}

/// A cached function simulating an LLM API call.
///
/// The `#[cocoindex::cached]` macro:
///   - Generates a cache key from `prompt` and `model` (all non-Ctx params)
///   - Checks LMDB cache before calling the function body
///   - Stores the serialized result on cache miss
///   - Counts cache_hits / cache_misses via ctx
#[cocoindex::cached]
async fn call_llm(ctx: &Ctx, prompt: &str, model: &str) -> Result<LlmResponse, CocoError> {
    // Simulate an expensive LLM API call
    println!(
        "    [LLM API call] model={}, prompt={:?} (cold cache)",
        model, prompt
    );

    // Simulate some processing time (in a real app this would be an HTTP request)
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    Ok(LlmResponse {
        prompt: prompt.to_string(),
        response: format!("Simulated response for: {}", prompt),
        model: model.to_string(),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let app = App::open("llm-cache", db_dir.path())?;
    println!("CocoIndex LLM Caching Example\n");

    let prompts = vec!["What is Rust?", "Explain CocoIndex", "How does LMDB work?"];

    let (_, stats) = app
        .run(|ctx| async move {
            // --- First pass: all cold cache (simulated LLM calls) ---
            println!("--- First pass: Cold cache (simulated LLM calls) ---");
            let mut results = Vec::new();
            for prompt in &prompts {
                let response = call_llm(&ctx, prompt, "gpt-4").await?;
                println!("  Response: {}", response.response);
                results.push(response);
            }

            // --- Second pass: same prompts, should all be cache hits ---
            println!("\n--- Second pass: Warm cache (no LLM calls) ---");
            for (i, prompt) in prompts.iter().enumerate() {
                let response = call_llm(&ctx, prompt, "gpt-4").await?;
                println!("  Cached response: {}", response.response);
                // Verify cached result matches original
                assert_eq!(response, results[i]);
            }

            // --- Third pass: different model should miss cache (different key) ---
            println!("\n--- Third pass: Different model (new cache entry) ---");
            let response = call_llm(&ctx, "What is Rust?", "gpt-3.5").await?;
            println!("  Response: {}", response.response);

            // Verify that the same prompt with a different model produces a different cache entry
            let response_gpt4 = call_llm(&ctx, "What is Rust?", "gpt-4").await?;
            assert_ne!(
                response, response_gpt4,
                "different models should give different results"
            );

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
    assert!(
        stats.cache_hits >= 3,
        "should have at least 3 cache hits (same prompts)"
    );
    assert!(
        stats.cache_misses >= 3,
        "should have at least 3 cache misses (first calls)"
    );

    println!("\nAll assertions passed -- LLM caching is working correctly!");
    Ok(())
}
