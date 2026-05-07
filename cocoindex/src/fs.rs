//! File system traversal utilities

pub mod walk;
pub use walk::{WalkBuilder, FileEntry, Fingerprint};

/// Create a walker for the given root path
pub fn walk(root: &std::path::Path) -> WalkBuilder {
    WalkBuilder::new(root.to_path_buf())
}
