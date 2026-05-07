//! Multi-codebase example - demonstrating walking multiple codebases

use std::collections::HashMap;
use std::path::PathBuf;
use cocoindex::{App, fs, Result};

/// Index results for a single codebase
#[derive(Debug)]
struct CodebaseIndex {
    root: PathBuf,
    total_files: usize,
    total_size: u64,
    by_extension: HashMap<String, usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Multi-Codebase Indexing Example ===\n");

    // Define multiple codebases to index
    // In a real scenario, these would be actual project paths
    let codebases = vec![
        PathBuf::from("."),
        PathBuf::from("./cocoindex"),
        PathBuf::from("./cocoindex_macro"),
    ];

    // Create a shared CocoIndex app for all codebases
    let app = App::open("multi-codebase", &std::env::temp_dir().join("cocoindex-multi"))?;

    let mut results: HashMap<String, CodebaseIndex> = HashMap::new();

    // Index each codebase
    for codebase_root in &codebases {
        if !codebase_root.exists() {
            println!("Skipping non-existent path: {:?}", codebase_root);
            continue;
        }

        println!("Indexing: {:?}", codebase_root);

        let (index, _stats) = app.run(|ctx| async move {
            index_codebase(&ctx, codebase_root).await
        }).await?;

        let name = codebase_root.to_string_lossy().to_string();
        results.insert(name, index);
    }

    // Print summary
    println!("\n=== Index Summary ===");
    for (name, index) in &results {
        println!("\nCodebase: {}", name);
        println!("  Total files: {}", index.total_files);
        println!("  Total size: {} bytes", index.total_size);
        println!("  By extension:");
        for (ext, count) in &index.by_extension {
            println!("    .{}: {} files", ext, count);
        }
    }

    println!("\n=== Done ===");
    Ok(())
}

/// Index a single codebase
async fn index_codebase(ctx: &cocoindex::Ctx, root: &std::path::Path) -> Result<CodebaseIndex> {
    let mut total_files = 0;
    let mut total_size = 0u64;
    let mut by_extension: HashMap<String, usize> = HashMap::new();

    // Walk the codebase with Rust source filtering
    let walker = fs::walk(root)
        .extension("rs")
        .include_hidden()
        .build();

    // Use walk_with_cache for change detection (if cache is available)
    // For demonstration, we use the basic iterator
    for entry_result in walker {
        let entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Error walking {:?}: {}", root, e);
                continue;
            }
        };

        total_files += 1;
        total_size += entry.size();

        // Track by extension
        let ext = entry.path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_string();

        *by_extension.entry(ext).or_insert(0) += 1;

        // Read file content to trigger content caching
        let _content = ctx.read_file(entry.path()).await.ok();
    }

    Ok(CodebaseIndex {
        root: root.to_path_buf(),
        total_files,
        total_size,
        by_extension,
    })
}