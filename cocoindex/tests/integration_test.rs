//! Integration tests for CocoIndex

use cocoindex::{App, Result};
use std::path::Path;
use tempfile::TempDir;

#[tokio::test]
async fn test_app_open_and_close() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("cocoindex_test");

    let app = App::open("test", &db_path)?;
    assert_eq!(app.db_path(), db_path.as_path());

    Ok(())
}

#[tokio::test]
async fn test_cache_operations() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("cocoindex_cache_test");

    let app = App::open("cache_test", &db_path)?;
    let cache = app.cache();

    // Test set and get
    cache.set("key1", b"value1")?;
    let result = cache.get("key1")?;
    assert_eq!(result, Some(b"value1".to_vec()));

    // Test overwrite
    cache.set("key1", b"value2")?;
    let result = cache.get("key1")?;
    assert_eq!(result, Some(b"value2".to_vec()));

    // Test delete
    cache.delete("key1")?;
    let result = cache.get("key1")?;
    assert_eq!(result, None);

    Ok(())
}

#[tokio::test]
async fn test_app_run() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("cocoindex_run_test");

    let app = App::open("run_test", &db_path)?;

    let (result, stats) = app.run(|_ctx| async move {
        Ok("success".to_string())
    }).await?;

    assert_eq!(result, "success");
    assert!(stats.elapsed_ms >= 0);

    Ok(())
}

#[tokio::test]
async fn test_ctx_read_file() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("cocoindex_ctx_test");
    let test_file = temp_dir.path().join("test.txt");

    // Create a test file
    tokio::fs::write(&test_file, "Hello, CocoIndex!").await?;

    let app = App::open("ctx_test", &db_path)?;
    let (content, stats) = app.run(|ctx| async move {
        let content = ctx.read_file(&test_file).await?;
        Ok(content)
    }).await?;

    assert_eq!(content, "Hello, CocoIndex!");
    assert!(stats.elapsed_ms >= 0);

    Ok(())
}

#[test]
fn test_error_types() {
    use cocoindex::CocoError;

    // Test IO error
    let io_err = CocoError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
    assert!(matches!(io_err, CocoError::Io(_)));
    assert!(io_err.to_string().contains("file not found"));

    // Test LMDB error
    let lmdb_err = CocoError::Lmdb("database corrupted".to_string());
    assert!(matches!(lmdb_err, CocoError::Lmdb(_)));
    assert!(lmdb_err.to_string().contains("database corrupted"));

    // Test Serde error
    let serde_err: CocoError = serde_json::from_slice::<serde_json::Value>(b"invalid").unwrap_err().into();
    assert!(matches!(serde_err, CocoError::Serde(_)));

    // Test User error
    let user_err = CocoError::User("custom error".to_string());
    assert!(matches!(user_err, CocoError::User(_)));
    assert!(user_err.to_string().contains("custom error"));
}