//! Application structure for CocoIndex

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use lmdb::Environment;
use crate::Ctx;
use crate::cache::Cache;
use crate::stats::RunStats;
use crate::Result;

/// App is the main entry point for CocoIndex
pub struct App {
    env: Arc<Environment>,
    cache: Cache,
    db_path: PathBuf,
}

impl App {
    /// Open or create a CocoIndex database
    pub fn open(name: &str, db_path: &Path) -> Result<Self> {
        // Ensure the database directory exists
        std::fs::create_dir_all(db_path)?;

        // Open or create LMDB environment
        let env = Environment::new()
            .set_map_size(1024 * 1024 * 1024) // 1GB
            .set_max_dbs(16)
            .set_max_readers(128)
            .open(db_path)
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        let env = Arc::new(env);

        // Open cache database
        let cache = Cache::open(&env)?;

        eprintln!("Opened CocoIndex '{}' at {:?}", name, db_path);
        Ok(App { env, cache, db_path: db_path.to_path_buf() })
    }

    /// Run a task with the given context
    pub async fn run<F, Fut, T>(&self, f: F) -> Result<(T, RunStats)>
    where
        F: FnOnce(Ctx) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let start = std::time::Instant::now();

        // Create shared stats that can be accessed after Ctx is consumed
        let stats_shared = Arc::new(Mutex::new(RunStats::default()));

        // Create context with session and shared stats
        let session_id = uuid::Uuid::new_v4();
        let ctx = Ctx::new(
            session_id,
            self.cache.clone(),
            self.env.clone(),
            self.db_path.clone(),
            stats_shared.clone(),
        );

        // Run the task
        let result = f(ctx).await?;

        // Extract final stats from shared handle
        let elapsed = start.elapsed();
        let mut final_stats = stats_shared.lock().unwrap();
        final_stats.elapsed_ms = elapsed.as_millis() as u64;
        let stats = final_stats.clone();

        Ok((result, stats))
    }

    /// Get the database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Get a reference to the cache
    pub fn cache(&self) -> &Cache {
        &self.cache
    }
}