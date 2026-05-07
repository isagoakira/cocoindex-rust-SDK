//! Walk directory iterator with fingerprint-based change detection
//!
//! The main entry point is [`walk()`], which returns a [`WalkBuilder`] for
//! configuration. Call `.build()` for a simple walk, or `.walk_with_ctx(ctx)`
//! / `.walk_with_cache(cache)` for fingerprint-based skip-unchanged iteration.
//!
//! # Skip-unchanged behaviour
//!
//! When a cache is provided, each file is fingerprinted and compared against
//! the previous run. Files whose fingerprint is identical are **skipped**
//! (not yielded). This lets downstream consumers process only changed or new
//! files.
//!
//! ```rust,no_run
//! use std::path::Path;
//! use cocoindex::fs;
//!
//! // Simple walk – no caching, every file is yielded
//! for entry in fs::walk(Path::new(".")).build() {
//!     let entry = entry.unwrap();
//!     println!("{}", entry.path().display());
//! }
//! ```

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_64;

use crate::cache::Cache;
use crate::stats::RunStats;
use crate::Ctx;
use crate::Result;

/// Fingerprint for skip-unchanged logic.
///
/// Content hash identifies file content, code hash identifies code structure.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fingerprint {
    /// Hash of file content (xxh3_64 of raw bytes)
    pub content_hash: u64,
    /// Hash of code structure (line count, first/last line, total chars)
    pub code_hash: u64,
}

impl Fingerprint {
    /// Calculate fingerprint from file content.
    pub fn from_content(content: &[u8]) -> Self {
        let content_hash = xxh3_64(content);
        let code_hash = Self::compute_code_hash(content);
        Fingerprint {
            content_hash,
            code_hash,
        }
    }

    /// Compute code hash from content (structure-based).
    fn compute_code_hash(content: &[u8]) -> u64 {
        let content_str = String::from_utf8_lossy(content);
        let lines: Vec<&str> = content_str.lines().collect();

        if lines.is_empty() {
            return 0;
        }

        // Hash structure: line count + first line + last line + total chars
        let mut struct_data = Vec::new();
        struct_data.extend_from_slice(&(lines.len() as u64).to_le_bytes());
        if let Some(first) = lines.first() {
            struct_data.extend_from_slice(first.as_bytes());
        }
        if let Some(last) = lines.last() {
            struct_data.extend_from_slice(last.as_bytes());
        }
        struct_data.extend_from_slice(&(content.len() as u64).to_le_bytes());

        xxh3_64(&struct_data)
    }

    /// Serialize fingerprint to bytes (16 bytes) for cache storage.
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&self.content_hash.to_le_bytes());
        bytes[8..].copy_from_slice(&self.code_hash.to_le_bytes());
        bytes
    }

    /// Deserialize fingerprint from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 16 {
            return None;
        }
        let content_hash = u64::from_le_bytes(bytes[..8].try_into().ok()?);
        let code_hash = u64::from_le_bytes(bytes[8..].try_into().ok()?);
        Some(Fingerprint {
            content_hash,
            code_hash,
        })
    }
}

/// A file entry from directory walking with fingerprint.
pub struct FileEntry {
    /// Absolute path to the file
    pub(crate) path: PathBuf,
    /// Whether this is a directory
    pub(crate) is_dir: bool,
    /// Eagerly calculated fingerprint (None for directories)
    pub(crate) fingerprint: Option<Fingerprint>,
    /// File size in bytes
    pub(crate) size: u64,
}

impl FileEntry {
    /// Get the path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if this is a directory.
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Get the fingerprint.
    pub fn fingerprint(&self) -> Option<&Fingerprint> {
        self.fingerprint.as_ref()
    }

    /// Get file size.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Read file content lazily (only when needed).
    pub fn read_content(&self) -> Result<Vec<u8>> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);
        let mut content = Vec::new();
        reader.read_to_end(&mut content)?;
        Ok(content)
    }

    /// Get file name as string.
    pub fn file_name(&self) -> Option<String> {
        self.path
            .file_name()
            .and_then(|n| n.to_str().map(String::from))
    }
}

/// Cache key for file fingerprints.
fn fingerprint_key(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    format!("fingerprint:{}", rel.to_string_lossy())
}

/// Builder for configuring file traversal.
///
/// Create one via [`walk()`] or [`WalkBuilder::new()`], then chain
/// configuration methods, and finally call one of the terminal methods:
///
/// | Method | Cache | Result |
/// |--------|-------|--------|
/// | `.build()` | None | `WalkIter` (no skip-unchanged) |
/// | `.walk_with_cache(cache)` | Explicit `&Cache` | `Result<WalkIter>` |
/// | `.walk_with_ctx(ctx)` | From `&Ctx` | `Result<WalkIter>` |
pub struct WalkBuilder {
    root: PathBuf,
    glob_pattern: Option<String>,
    extension: Option<String>,
    include_hidden: bool,
}

impl WalkBuilder {
    /// Create a new WalkBuilder for the given root path.
    pub fn new(root: PathBuf) -> Self {
        WalkBuilder {
            root,
            glob_pattern: None,
            extension: None,
            include_hidden: false,
        }
    }

    /// Set a glob pattern to filter files (e.g., `"**/*.rs"`).
    ///
    /// The pattern is matched against the **relative path** from the root,
    /// not just the file name. For example, `glob("src/**/*.rs")` matches
    /// both `src/main.rs` and `src/sub/mod.rs`.
    pub fn glob(mut self, pattern: &str) -> Self {
        self.glob_pattern = Some(pattern.to_string());
        self
    }

    /// Set a file extension filter (e.g., `"rs"` for `*.rs` files).
    pub fn extension(mut self, ext: &str) -> Self {
        self.extension = Some(ext.to_string());
        self
    }

    /// Include hidden files and directories (those starting with `.`).
    pub fn include_hidden(mut self) -> Self {
        self.include_hidden = true;
        self
    }

    /// Build a no-cache walker.
    ///
    /// No fingerprint caching is used — every file is always yielded.
    /// Use [`walk_with_ctx`](Self::walk_with_ctx) for skip-unchanged behaviour.
    pub fn build(self) -> WalkIter<'static> {
        let compiled = self
            .glob_pattern
            .as_ref()
            .and_then(|p| glob::Pattern::new(p).ok());
        WalkIter {
            root: self.root,
            glob_pattern: compiled,
            extension: self.extension,
            include_hidden: self.include_hidden,
            cache: None,
            walker: None,
            pending: Vec::new(),
            stats: None,
        }
    }

    /// Walk with an explicit `&Cache` for fingerprint-based change detection.
    ///
    /// Unchanged files are skipped (not yielded).
    /// Returns an error if the glob pattern is invalid.
    pub fn walk_with_cache<'a>(self, cache: &'a Cache) -> Result<WalkIter<'a>> {
        let compiled = match self.glob_pattern {
            None => None,
            Some(ref p) => Some(glob::Pattern::new(p).map_err(|e| {
                crate::CocoError::User(format!("invalid glob pattern '{}': {}", p, e))
            })?),
        };
        Ok(WalkIter {
            root: self.root,
            glob_pattern: compiled,
            extension: self.extension,
            include_hidden: self.include_hidden,
            cache: Some(cache),
            walker: None,
            pending: Vec::new(),
            stats: None,
        })
    }

    /// Walk with a `&Ctx`, tracking stats automatically.
    ///
    /// Unchanged files are skipped (not yielded). `files_processed` and
    /// `bytes_read` track all files that were fingerprinted (including
    /// skipped ones).
    pub fn walk_with_ctx<'a>(self, ctx: &'a Ctx) -> Result<WalkIter<'a>> {
        let compiled = match self.glob_pattern {
            None => None,
            Some(ref p) => Some(glob::Pattern::new(p).map_err(|e| {
                crate::CocoError::User(format!("invalid glob pattern '{}': {}", p, e))
            })?),
        };
        Ok(WalkIter {
            root: self.root,
            glob_pattern: compiled,
            extension: self.extension,
            include_hidden: self.include_hidden,
            cache: Some(ctx.cache()),
            walker: None,
            pending: Vec::new(),
            stats: Some(ctx.stats_handle()),
        })
    }
}

/// Iterator that yields [`FileEntry`] items during directory traversal.
///
/// Obtained from [`WalkBuilder::build`], [`WalkBuilder::walk_with_cache`],
/// or [`WalkBuilder::walk_with_ctx`].
///
/// When a cache is available, files whose fingerprint matches the cached
/// value are **skipped** (not yielded). This allows downstream consumers
/// to process only changed or new files.
pub struct WalkIter<'a> {
    root: PathBuf,
    /// Pre-compiled glob pattern (compiled once in the constructor).
    glob_pattern: Option<glob::Pattern>,
    extension: Option<String>,
    include_hidden: bool,
    /// When `None`, no skip-unchanged logic is applied.
    cache: Option<&'a Cache>,
    walker: Option<walkdir::IntoIter>,
    /// Files queued from directory matching that haven't been yielded yet.
    pending: Vec<PathBuf>,
    stats: Option<Arc<Mutex<RunStats>>>,
}

impl<'a> Iterator for WalkIter<'a> {
    type Item = Result<FileEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Process any queued pending files first
            if let Some(path) = self.pending.pop() {
                match self.process_path(&path) {
                    Some(Ok(entry)) => return Some(Ok(entry)),
                    Some(Err(e)) => return Some(Err(e)),
                    None => continue, // skipped (unchanged or filtered)
                }
            }

            // Get next entry from the directory walker
            let walker = self
                .walker
                .get_or_insert_with(|| WalkDir::new(&self.root).follow_links(false).into_iter());

            let entry = match walker.next() {
                Some(Ok(e)) => e,
                Some(Err(e)) => {
                    // Skip permission errors and continue
                    if e.io_error()
                        .map(|e| e.kind() == io::ErrorKind::PermissionDenied)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let err_msg = format!("walkdir error: {}", e);
                    return Some(Err(crate::CocoError::User(err_msg)));
                }
                None => return None,
            };

            let path = entry.path().to_path_buf();

            // Filter hidden files
            if !self.include_hidden {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
            }

            // Process the path — may return None for skipped/unchanged files
            match self.process_path(&path) {
                Some(Ok(file_entry)) => return Some(Ok(file_entry)),
                Some(Err(e)) => return Some(Err(e)),
                None => continue,
            }
        }
    }
}

impl<'a> WalkIter<'a> {
    /// Process a single path, returning a `FileEntry` if it passes all
    /// filters and has changed content.
    ///
    /// Returns `None` when:
    /// - The path has been deleted since walkdir saw it
    /// - The path doesn't match the glob/extension filter
    /// - The file's fingerprint matches the cached value (skip-unchanged)
    fn process_path(&self, path: &Path) -> Option<Result<FileEntry>> {
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    return None;
                }
                if e.kind() == io::ErrorKind::PermissionDenied {
                    return None;
                }
                return Some(Err(crate::CocoError::Io(e)));
            }
        };

        let is_dir = metadata.is_dir();

        // Apply glob pattern filter against the relative path
        if let Some(ref pattern) = self.glob_pattern {
            if !is_dir {
                let rel_path = self.relative_path(path);
                let rel_str = rel_path.to_string_lossy();
                // On Windows the path may use backslashes; glob uses forward slashes.
                let rel_str = rel_str.replace('\\', "/");
                if !pattern.matches(&rel_str) {
                    return None;
                }
            }
        }

        // Apply extension filter
        if let Some(ref ext) = self.extension {
            if !is_dir {
                let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if file_ext != ext {
                    return None;
                }
            }
        }

        // If it's a directory, yield it directly (no fingerprint, no skip)
        if is_dir {
            return Some(Ok(FileEntry {
                path: path.to_path_buf(),
                is_dir: true,
                fingerprint: None,
                size: metadata.len(),
            }));
        }

        // --- File processing starts here ---

        // Track files processed
        if let Some(ref stats) = self.stats {
            let mut s = stats.lock().unwrap();
            s.files_processed += 1;
        }

        let cache_key = fingerprint_key(&self.root, path);

        // Attempt to retrieve the cached fingerprint (if cache is available)
        let cached_fp: Option<Fingerprint> = self.cache.and_then(|c| {
            c.get(&cache_key)
                .ok()
                .flatten()
                .and_then(|bytes| Fingerprint::from_bytes(&bytes))
        });

        // Read current file content
        let content = match fs::read(path) {
            Ok(c) => c,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    return None;
                }
                if e.kind() == io::ErrorKind::PermissionDenied {
                    return None;
                }
                return Some(Err(crate::CocoError::Io(e)));
            }
        };

        // Track bytes read
        if let Some(ref stats) = self.stats {
            let mut s = stats.lock().unwrap();
            s.bytes_read += content.len() as u64;
        }

        // Compute fingerprint of current content
        let fp = Fingerprint::from_content(&content);

        // Skip-unchanged: if the cached fingerprint matches, don't yield
        if let Some(ref cached) = cached_fp {
            if *cached == fp {
                // Content unchanged — update cache timestamp but skip yield
                // (we already read the file, but downstream processing is avoided)
                if let Some(c) = self.cache {
                    let _ = c.set(&cache_key, &fp.to_bytes());
                }
                return None;
            }
        }

        // Store updated fingerprint in cache (if available)
        if let Some(c) = self.cache {
            let _ = c.set(&cache_key, &fp.to_bytes());
        }

        Some(Ok(FileEntry {
            path: path.to_path_buf(),
            is_dir: false,
            fingerprint: Some(fp),
            size: metadata.len(),
        }))
    }

    /// Compute the relative path from the walk root.
    fn relative_path(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.root).unwrap_or(path).to_path_buf()
    }
}

/// Create a new [`WalkBuilder`] for the given root path.
///
/// This is the primary entry point for directory traversal.
///
/// # Example
///
/// ```rust,no_run
/// use std::path::Path;
/// use cocoindex::fs;
///
/// // Simple walk without caching
/// for entry in fs::walk(Path::new("/tmp")).build() {
///     let entry = entry.unwrap();
///     println!("{}", entry.path().display());
/// }
/// ```
pub fn walk(root: &Path) -> WalkBuilder {
    WalkBuilder::new(root.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_from_content() {
        let content = b"Hello, World!";
        let fp = Fingerprint::from_content(content);

        // Same content should produce same fingerprint
        let fp2 = Fingerprint::from_content(content);
        assert_eq!(fp, fp2);

        // Different content should produce different fingerprint
        let fp3 = Fingerprint::from_content(b"Different");
        assert_ne!(fp, fp3);
    }

    #[test]
    fn test_fingerprint_serialization() {
        let fp = Fingerprint::from_content(b"test content");
        let bytes = fp.to_bytes();
        let recovered = Fingerprint::from_bytes(&bytes).unwrap();
        assert_eq!(fp, recovered);
    }

    #[test]
    fn test_fingerprint_bytes_roundtrip() {
        let fp = Fingerprint {
            content_hash: 12345,
            code_hash: 67890,
        };
        let bytes = fp.to_bytes();
        let recovered = Fingerprint::from_bytes(&bytes).unwrap();
        assert_eq!(fp.content_hash, recovered.content_hash);
        assert_eq!(fp.code_hash, recovered.code_hash);
    }

    #[test]
    fn test_fingerprint_invalid_bytes() {
        assert!(Fingerprint::from_bytes(&[0u8; 8]).is_none());
        assert!(Fingerprint::from_bytes(&[0u8; 0]).is_none());
        assert!(Fingerprint::from_bytes(&[0u8; 32]).is_none());
    }

    #[test]
    fn test_relative_path() {
        let root = PathBuf::from("/project/src");
        let walker = WalkIter {
            root,
            glob_pattern: None,
            extension: None,
            include_hidden: false,
            cache: None,
            walker: None,
            pending: Vec::new(),
            stats: None,
        };

        let rel = walker.relative_path(Path::new("/project/src/main.rs"));
        assert_eq!(rel, PathBuf::from("main.rs"));

        let rel = walker.relative_path(Path::new("/project/src/sub/mod.rs"));
        assert_eq!(rel, PathBuf::from("sub/mod.rs"));

        // Path outside root is returned as-is
        let rel = walker.relative_path(Path::new("/other/file.rs"));
        assert_eq!(rel, PathBuf::from("/other/file.rs"));
    }
}
