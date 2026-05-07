//! Context structure for CocoIndex

use crate::cache::Cache;
use crate::stats::RunStats;
use crate::Result;
use lmdb::Environment;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Ctx provides runtime context for indexed code operations
pub struct Ctx {
    session_id: Uuid,
    cache: Cache,
    env: Arc<Environment>,
    db_path: PathBuf,
    stats: Arc<Mutex<RunStats>>,
}

impl Ctx {
    /// Create a new context with shared stats
    pub(crate) fn new(
        session_id: Uuid,
        cache: Cache,
        env: Arc<Environment>,
        db_path: PathBuf,
        stats: Arc<Mutex<RunStats>>,
    ) -> Self {
        Self {
            session_id,
            cache,
            env,
            db_path,
            stats,
        }
    }

    /// Read a file as string
    pub async fn read_file(&self, path: &Path) -> Result<String> {
        let bytes = self.read_file_bytes(path).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Read a file as bytes
    pub async fn read_file_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        tokio::fs::read(path).await.map_err(crate::CocoError::Io)
    }

    /// Get the session ID
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Get the database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Get a reference to the cache
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// Get a reference to the environment
    pub fn env(&self) -> &Arc<Environment> {
        &self.env
    }

    /// Get a cloned snapshot of current stats
    pub fn stats(&self) -> RunStats {
        self.stats.lock().unwrap().clone()
    }

    /// Get mutable access to stats
    pub fn stats_mut(&self) -> std::sync::MutexGuard<'_, RunStats> {
        self.stats.lock().unwrap()
    }

    /// Cache get with automatic stats counting (hits vs misses)
    pub fn cache_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let result = self.cache.get(key)?;
        let mut stats = self.stats.lock().unwrap();
        match &result {
            Some(_) => stats.cache_hits += 1,
            None => stats.cache_misses += 1,
        }
        Ok(result)
    }

    /// Cache set (delegates to underlying cache)
    pub fn cache_set(&self, key: &str, value: &[u8]) -> Result<()> {
        self.cache.set(key, value)
    }

    /// Return a clone of the stats handle for sharing
    pub(crate) fn stats_handle(&self) -> Arc<Mutex<RunStats>> {
        self.stats.clone()
    }
}
