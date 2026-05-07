//! Error types for CocoIndex

use thiserror::Error;

/// Main error type for CocoIndex
#[derive(Error, Debug)]
pub enum CocoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("LMDB error: {0}")]
    Lmdb(String),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("User error: {0}")]
    User(String),
}
