//! CocoIndex Rust SDK
//!
//! High-performance code indexing and caching library.
//!
//! # Example
//!
//! ```rust
//! use cocoindex::App;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let app = App::open("myindex", Path::new("/tmp/cocoindex_db"))?;
//!     app.run(|ctx| async move {
//!         // Your code here
//!         Ok(())
//!     }).await?;
//!     Ok(())
//! }
//! ```

// Public API re-exports
pub mod app;
pub mod cache;
pub mod ctx;
pub mod error;
pub mod fs;
pub mod macros;
pub mod stats;

pub use app::App;
pub use ctx::Ctx;
pub use error::CocoError;
pub use fs::{FileEntry, Fingerprint, WalkBuilder};
pub use macros::{cached, component};
pub use stats::RunStats;

/// Result type alias using CocoError
pub type Result<T> = std::result::Result<T, CocoError>;
