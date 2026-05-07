//! Run statistics for CocoIndex

use serde::{Deserialize, Serialize};

/// Statistics collected during a run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStats {
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    /// Total files processed
    pub files_processed: u64,
    /// Total bytes read
    pub bytes_read: u64,
    /// Number of components executed
    pub components_executed: u64,
    /// Elapsed time in milliseconds
    pub elapsed_ms: u64,
}

impl Default for RunStats {
    fn default() -> Self {
        Self {
            cache_hits: 0,
            cache_misses: 0,
            files_processed: 0,
            bytes_read: 0,
            components_executed: 0,
            elapsed_ms: 0,
        }
    }
}
