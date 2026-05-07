//! Example 01: Hello CocoIndex
//!
//! Minimal example showing the core CocoIndex workflow:
//!   - App::open() to create or open an index database
//!   - app.run() to execute a task with a Ctx context
//!   - Cache read/write operations
//!   - RunStats inspection

use cocoindex::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Use a temporary directory for the LMDB database.
    // In production you would use a persistent path.
    let db_dir = tempfile::tempdir()?;

    // Open (or create) a CocoIndex app with a human-readable name and a
    // filesystem path for the LMDB-backed database.
    let app = App::open("hello-cocoindex", db_dir.path())?;
    println!("Database at: {:?}", app.db_path());

    // app.run() creates a session with a fresh Ctx and shared RunStats.
    // The closure receives &Ctx and can perform any cached operations.
    let (result, stats) = app
        .run(|ctx| async move {
            // --- Cache basic read/write ---
            ctx.cache_set("greeting", b"Hello from CocoIndex!")?;
            let value = ctx.cache_get("greeting")?;
            let value_bytes = value.unwrap_or_default();
            let text = String::from_utf8_lossy(&value_bytes);
            println!("Cached value: {}", text);

            // --- The Ctx also exposes read_file / read_file_bytes ---
            // (not used here, see examples 02 and 03)
            Ok("all good")
        })
        .await?;

    // --- Inspect statistics ---
    println!("\nResult: {}", result);
    println!("Cache hits:   {}", stats.cache_hits);
    println!("Cache misses: {}", stats.cache_misses);
    println!("Elapsed:      {} ms", stats.elapsed_ms);

    Ok(())
}
