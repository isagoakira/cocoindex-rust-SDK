//! File system walk integration tests
//!
//! These tests verify the skip-unchanged behaviour, glob path matching,
//! and the no-cache walk API.

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;
    use cocoindex::App;
    use cocoindex::fs;
    use cocoindex::cache::Cache;
    use lmdb::Environment;

    /// Helper: open a temporary LMDB environment + cache.
    fn open_temp_cache(dir: &Path) -> (Arc<Environment>, Cache) {
        std::fs::create_dir_all(dir).unwrap();
        let env = Arc::new(
            Environment::new()
                .set_map_size(1024 * 1024) // 1MB
                .set_max_dbs(16)
                .set_max_readers(8)
                .open(dir)
                .unwrap()
        );
        let cache = Cache::open(&env).unwrap();
        (env, cache)
    }

    // ---------------------------------------------------------------
    //  Skip-unchanged behaviour
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_skip_unchanged_first_walk_yields_all() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("a.txt"), b"hello")?;
        std::fs::write(root.join("b.txt"), b"world")?;

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        let results: Vec<_> = fs::walk(root)
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        // 2 files, no directories (temp dir name starts with '.', filtered as hidden)
        assert_eq!(results.len(), 2, "first walk should yield all 2 files");
        Ok(())
    }

    #[tokio::test]
    async fn test_skip_unchanged_second_walk_empty() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("a.txt"), b"hello")?;
        std::fs::write(root.join("b.txt"), b"world")?;

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        // First walk: populate cache
        let _: Vec<_> = fs::walk(root)
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Second walk: same content -> should yield 0
        let results: Vec<_> = fs::walk(root)
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert_eq!(results.len(), 0, "second walk with same content should yield 0 files");
        Ok(())
    }

    #[tokio::test]
    async fn test_skip_unchanged_modify_one_file() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("a.txt"), b"hello")?;
        std::fs::write(root.join("b.txt"), b"world")?;

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        // First walk: populate cache
        let _: Vec<_> = fs::walk(root)
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Modify one file
        std::fs::write(root.join("a.txt"), b"modified")?;

        // Third walk: should yield only the modified file
        let results: Vec<_> = fs::walk(root)
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert_eq!(results.len(), 1, "should yield 1 modified file");
        assert_eq!(results[0].file_name(), Some("a.txt".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_skip_unchanged_within_ctx() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("a.txt"), b"hello")?;
        std::fs::write(root.join("b.txt"), b"world")?;
        std::fs::write(root.join("c.txt"), b"coco")?;

        let db_dir = tempdir()?;
        let app = App::open("test_ctx", db_dir.path())?;

        let root2 = root.to_path_buf();
        let (_, stats) = app.run(move |ctx| async move {
            let ctx_local = &ctx;

            // First walk: 3 files yielded
            let results: Vec<_> = fs::walk(&root2)
                .walk_with_ctx(ctx_local)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            assert_eq!(results.len(), 3, "first walk should yield 3 files");

            // Second walk: same content -> 0 files yielded (all skipped)
            let results: Vec<_> = fs::walk(&root2)
                .walk_with_ctx(ctx_local)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            assert_eq!(results.len(), 0, "second walk should yield 0 files");

            // Modify one file
            std::fs::write(root2.join("b.txt"), b"modified")?;

            // Third walk: 1 modified file yielded
            let results: Vec<_> = fs::walk(&root2)
                .walk_with_ctx(ctx_local)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            assert_eq!(results.len(), 1, "third walk should yield 1 modified file");
            assert_eq!(results[0].file_name(), Some("b.txt".to_string()));

            // Stats: each walk processes ALL files (fingerprinted), regardless of skip
            // Walk 1: 3 files processed, 3 yielded
            // Walk 2: 3 files processed, 0 yielded (all skipped)
            // Walk 3: 3 files processed, 1 yielded (1 changed, 2 skipped)
            // Total files_processed = 9
            let s = ctx_local.stats();
            assert_eq!(s.files_processed, 9, "should have processed 9 files total (3 per walk)");

            Ok(())
        }).await?;

        // Also verify via the returned stats
        assert_eq!(stats.files_processed, 9);
        Ok(())
    }

    // ---------------------------------------------------------------
    //  Glob path matching (against relative path, not just filename)
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_glob_path_matching_nested() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::create_dir_all(root.join("src"))?;
        std::fs::create_dir_all(root.join("src/sub"))?;
        std::fs::write(root.join("src/main.rs"), b"fn main() {}")?;
        std::fs::write(root.join("src/lib.rs"), b"pub fn foo() {}")?;
        std::fs::write(root.join("src/sub/mod.rs"), b"mod stuff;")?;
        std::fs::write(root.join("README.md"), b"docs")?;
        std::fs::write(root.join("src/notes.txt"), b"notes")?;

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        // Glob "src/**/*.rs" should match only .rs files nested under src/
        let results: Vec<_> = fs::walk(root)
            .glob("src/**/*.rs")
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Filter for files only (directories: src/, src/sub/ are also yielded)
        let files: Vec<_> = results.iter().filter(|e| !e.is_dir()).collect();
        assert_eq!(files.len(), 3, "should match 3 .rs files under src/");
        let names: Vec<String> = files.iter().map(|e| e.file_name().unwrap()).collect();
        assert!(names.contains(&"main.rs".to_string()));
        assert!(names.contains(&"lib.rs".to_string()));
        assert!(names.contains(&"mod.rs".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_glob_matches_all_depths() -> anyhow::Result<()> {
        // `glob::Pattern::matches("*.md")` matches .md files at ANY depth.
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("readme.md"), b"docs")?;
        std::fs::write(root.join("license.md"), b"mit")?;
        std::fs::write(root.join("src.rs"), b"code")?;
        std::fs::create_dir(root.join("sub"))?;
        std::fs::write(root.join("sub/ignore.md"), b"hidden")?;

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        // Glob "*.md" matches any file ending in .md regardless of depth
        let results: Vec<_> = fs::walk(root)
            .glob("*.md")
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Filter for files only (sub/ dir is also yielded)
        let files: Vec<_> = results.iter().filter(|e| !e.is_dir()).collect();
        assert_eq!(files.len(), 3, "*.md should match 3 .md files at any depth");
        let names: Vec<String> = files.iter().map(|e| e.file_name().unwrap()).collect();
        assert!(names.contains(&"readme.md".to_string()));
        assert!(names.contains(&"license.md".to_string()));
        assert!(names.contains(&"ignore.md".to_string()));
        Ok(())
    }

    // ---------------------------------------------------------------
    //  No-cache build() always yields all files
    // ---------------------------------------------------------------

    #[test]
    fn test_build_no_cache_yields_all() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("x.txt"), b"x")?;
        std::fs::write(root.join("y.txt"), b"y")?;

        let results: Vec<_> = fs::walk(root)
            .build()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        // Both files yielded (no cache = no skip-unchanged)
        let files: Vec<_> = results.iter().filter(|e| !e.is_dir()).collect();
        assert_eq!(files.len(), 2, "build() should yield all files");
        Ok(())
    }

    // ---------------------------------------------------------------
    //  Extension filter
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_extension_filter() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("a.rs"), b"fn a() {}")?;
        std::fs::write(root.join("b.py"), b"def b(): pass")?;
        std::fs::write(root.join("c.rs"), b"fn c() {}")?;

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        let results: Vec<_> = fs::walk(root)
            .extension("rs")
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let files: Vec<_> = results.iter().filter(|e| !e.is_dir()).collect();
        assert_eq!(files.len(), 2, "should match 2 .rs files");
        Ok(())
    }

    // ---------------------------------------------------------------
    //  Walk + Ctx with glob + extension filters combined
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_walk_with_ctx_and_glob() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("a.rs"), b"fn a() {}")?;
        std::fs::write(root.join("b.rs"), b"fn b() {}")?;
        std::fs::write(root.join("c.py"), b"def c(): pass")?;

        let db_dir = tempdir()?;
        let app = App::open("test_walk_ctx", db_dir.path())?;

        let root2 = root.to_path_buf();
        app.run(move |ctx| async move {
            let results: Vec<_> = fs::walk(&root2)
                .glob("*.rs")
                .walk_with_ctx(&ctx)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let files: Vec<_> = results.iter().filter(|e| !e.is_dir()).collect();
            assert_eq!(files.len(), 2, "should match 2 .rs files");

            let s = ctx.stats();
            assert_eq!(s.files_processed, 2);
            Ok(())
        }).await?;

        Ok(())
    }

    // ---------------------------------------------------------------
    //  Multi-file fingerprint consistency
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_fingerprint_consistency_multi_pass() -> anyhow::Result<()> {
        // Verify that different filter configurations produce identical
        // fingerprints for the same underlying file.
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("a.rs"), b"fn a() {}")?;
        std::fs::write(root.join("b.rs"), b"fn b() {}")?;
        std::fs::write(root.join("notes.txt"), b"plain text")?;

        // Use separate cache instances for each walk to avoid
        // skip-unchanged interfering.
        let cache_dir1 = tempdir()?;
        let cache_dir2 = tempdir()?;
        let (_env1, cache1) = open_temp_cache(cache_dir1.path());
        let (_env2, cache2) = open_temp_cache(cache_dir2.path());

        // Walk 1: extension filter
        let results1: Vec<_> = fs::walk(root)
            .extension("rs")
            .walk_with_cache(&cache1)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let fp_a1 = results1.iter().find(|e| e.file_name() == Some("a.rs".into()))
            .and_then(|e| e.fingerprint().cloned());
        let fp_b1 = results1.iter().find(|e| e.file_name() == Some("b.rs".into()))
            .and_then(|e| e.fingerprint().cloned());
        assert!(fp_a1.is_some(), "a.rs should have a fingerprint");
        assert!(fp_b1.is_some(), "b.rs should have a fingerprint");

        // Walk 2: glob pattern — same files, fresh cache
        let results2: Vec<_> = fs::walk(root)
            .glob("*.rs")
            .walk_with_cache(&cache2)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let fp_a2 = results2.iter().find(|e| e.file_name() == Some("a.rs".into()))
            .and_then(|e| e.fingerprint().cloned());
        let fp_b2 = results2.iter().find(|e| e.file_name() == Some("b.rs".into()))
            .and_then(|e| e.fingerprint().cloned());

        // Fingerprints should be identical regardless of filter used
        assert_eq!(fp_a1, fp_a2, "fingerprint of a.rs should be consistent");
        assert_eq!(fp_b1, fp_b2, "fingerprint of b.rs should be consistent");

        Ok(())
    }

    // ---------------------------------------------------------------
    //  Error paths
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_walk_nonexistent_directory() -> anyhow::Result<()> {
        // Walking a non-existent directory should return an error.
        let bogus = Path::new("/tmp/cocoindex_test_nonexistent_dir_that_should_not_exist");
        // Ensure it really doesn't exist
        let _ = std::fs::remove_dir_all(bogus);

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        let mut iter = fs::walk(bogus).walk_with_cache(&cache)?;
        let result = iter.next();
        assert!(result.is_some(), "walking non-existent dir should yield an error");
        assert!(result.unwrap().is_err(), "should be an Err variant");

        Ok(())
    }

    #[tokio::test]
    async fn test_walk_symlink_not_followed() -> anyhow::Result<()> {
        // symlinks should not be followed (follow_links: false).
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("real.txt"), b"real content")?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt"))?;
        }
        #[cfg(windows)]
        {
            // On Windows symlinks may not be available; skip
            return Ok(());
        }

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        // Walk should yield the real file but not the symlink
        // (walkdir treats symlinks to files as files by default; we set
        // follow_links = false, so symlink entries appear but are NOT followed)
        let results: Vec<_> = fs::walk(root)
            .walk_with_cache(&cache)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let files: Vec<_> = results.iter().filter(|e| !e.is_dir()).collect();
        let names: Vec<String> = files.iter().map(|e| e.file_name().unwrap()).collect();
        assert!(names.contains(&"real.txt".to_string()));
        // The symlink appears as a file entry (walkdir yields the symlink itself)
        assert!(names.contains(&"link.txt".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_walk_file_disappears() -> anyhow::Result<()> {
        // If a file is listed by walkdir but deleted before we read it,
        // the walk should skip it gracefully.
        let dir = tempdir()?;
        let root = dir.path();

        std::fs::write(root.join("stay.txt"), b"stay")?;
        let gone_path = root.join("gone.txt");
        std::fs::write(&gone_path, b"gone")?;

        let cache_dir = tempdir()?;
        let (_env, cache) = open_temp_cache(cache_dir.path());

        // Collect entries into a Vec to force eager iteration
        // WalkIter processes eagerly so we need to trigger processing
        let mut results = Vec::new();
        {
            let mut iter = fs::walk(root).walk_with_cache(&cache)?;

            // Get the first file
            if let Some(Ok(entry)) = iter.next() {
                results.push(entry);
                // Immediately delete gone.txt before the iterator processes it
                let _ = std::fs::remove_file(&gone_path);
            }

            // Collect the rest
            for entry in iter {
                match entry {
                    Ok(e) => results.push(e),
                    Err(_) => {} // skip errors
                }
            }
        }

        let files: Vec<_> = results.iter().filter(|e| !e.is_dir()).collect();
        assert_eq!(files.len(), 1, "only stay.txt should remain after deletion");
        assert_eq!(files[0].file_name(), Some("stay.txt".to_string()));

        Ok(())
    }
}
