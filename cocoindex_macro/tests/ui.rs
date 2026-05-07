//! Trybuild tests for CocoIndex macros

use std::path::PathBuf;

fn get_rust_files() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ui");
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn test_macro_ui() {
    let files = get_rust_files();
    if files.is_empty() {
        eprintln!("No UI test files found, skipping trybuild");
        return;
    }

    let mut tc = trybuild::TestCases::new();
    for file in files {
        if file.file_stem()
            .map(|s| s.to_str().unwrap_or("").ends_with("_pass"))
            .unwrap_or(false)
        {
            tc.pass(file);
        } else if file.file_stem()
            .map(|s| s.to_str().unwrap_or("").ends_with("_fail"))
            .unwrap_or(false)
        {
            tc.compile_fail(file);
        }
    }
}
