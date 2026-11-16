//! Disposable, complete packages. A marker alone never proves a cache hit.
use super::{write_if_changed, PreviewCacheHasher};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

pub(super) fn package_is_current(directory: &Path, key: &str, module: &str) -> Result<bool> {
    if !directory.join(format!("{module}.js")).is_file()
        || !directory.join(format!("{module}_bg.wasm")).is_file()
    {
        return Ok(false);
    }
    let marker = directory.join(".complete");
    let Ok(saved) = fs::read_to_string(&marker) else {
        return Ok(false);
    };
    // Hash every packaged file, including wasm-bindgen snippets, not just the two entrypoints.
    Ok(saved == format!("{key}\n{}", package_fingerprint(directory)?))
}

pub(super) fn mark_complete(directory: &Path, key: &str) -> Result<()> {
    fs::write(
        directory.join(".complete"),
        format!("{key}\n{}", package_fingerprint(directory)?),
    )?;
    Ok(())
}

fn package_fingerprint(directory: &Path) -> Result<String> {
    fn visit(root: &Path, directory: &Path, hasher: &mut PreviewCacheHasher) -> Result<()> {
        let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path == root.join(".complete") {
                continue;
            }
            anyhow::ensure!(
                !entry.file_type()?.is_symlink(),
                "preview cache contains a symlink"
            );
            if path.is_dir() {
                visit(root, &path, hasher)?;
            } else {
                hasher.field(path.strip_prefix(root)?.to_string_lossy().as_bytes());
                hasher.field(&fs::read(path)?);
            }
        }
        Ok(())
    }
    let mut hasher = PreviewCacheHasher::default();
    visit(directory, directory, &mut hasher)?;
    Ok(format!("{:016x}", hasher.finish()))
}

/// Keep unchanged files (and their mtimes); remove files no longer authored.
pub(super) fn sync_sources(directory: &Path, files: &BTreeMap<String, String>) -> Result<()> {
    fs::create_dir_all(directory)?;
    for relative in files.keys() {
        anyhow::ensure!(
            Path::new(relative)
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
            "generated source path must be relative: {relative}"
        );
    }
    // Only files listed in the previous generated-source manifest are eligible for removal.
    let marker = directory.join(".docs2-sources.json");
    let previous: Vec<String> = match fs::read(&marker) {
        Ok(bytes) => serde_json::from_slice(&bytes).context("invalid generated-source manifest")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    for relative in previous {
        anyhow::ensure!(
            Path::new(&relative)
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
            "invalid previous generated-source path"
        );
        if !files.contains_key(&relative) && directory.join(&relative).is_file() {
            fs::remove_file(directory.join(relative))?;
        }
    }
    for (relative, source) in files {
        write_if_changed(&directory.join(relative), source.as_bytes())?;
    }
    write_if_changed(
        &marker,
        &serde_json::to_vec(&files.keys().collect::<Vec<_>>())?,
    )?;
    Ok(())
}

pub(super) fn source_identity(files: &BTreeMap<String, String>) -> String {
    let mut hasher = PreviewCacheHasher::default();
    for (path, contents) in files {
        hasher.field(path.as_bytes());
        hasher.field(contents.as_bytes());
    }
    format!("{:016x}", hasher.finish())
}

pub(super) fn seed_lock(directory: &Path, root_lock: &[u8]) -> Result<()> {
    let mut hasher = PreviewCacheHasher::default();
    hasher.field(root_lock);
    let key = format!("{:016x}", hasher.finish());
    let marker = directory.join(".docs2-root-lock");
    if !directory.join("Cargo.lock").is_file()
        || fs::read_to_string(&marker).ok().as_deref() != Some(&key)
    {
        write_if_changed(&directory.join("Cargo.lock"), root_lock)?;
        fs::write(marker, key)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::temporary_directory;
    use super::*;

    #[test]
    fn package_needs_complete_matching_contents_including_snippets() {
        let root = temporary_directory("package-integrity");
        fs::create_dir_all(root.join("snippets")).unwrap();
        fs::write(root.join("preview.js"), "js").unwrap();
        fs::write(root.join("preview_bg.wasm"), "wasm").unwrap();
        fs::write(root.join("snippets/import.js"), "snippet").unwrap();
        assert!(!package_is_current(&root, "key", "preview").unwrap());
        mark_complete(&root, "key").unwrap();
        assert!(package_is_current(&root, "key", "preview").unwrap());
        assert!(!package_is_current(&root, "different", "preview").unwrap());
        fs::remove_file(root.join("snippets/import.js")).unwrap();
        assert!(!package_is_current(&root, "key", "preview").unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unchanged_sources_keep_mtime_and_obsolete_files_are_removed() {
        let root = temporary_directory("stable-source");
        let first = BTreeMap::from([
            ("src/lib.rs".into(), "first".into()),
            ("build.rs".into(), "old".into()),
        ]);
        sync_sources(&root, &first).unwrap();
        let modified = fs::metadata(root.join("src/lib.rs"))
            .unwrap()
            .modified()
            .unwrap();
        let next = BTreeMap::from([("src/lib.rs".into(), "first".into())]);
        sync_sources(&root, &next).unwrap();
        assert_eq!(
            modified,
            fs::metadata(root.join("src/lib.rs"))
                .unwrap()
                .modified()
                .unwrap()
        );
        assert!(!root.join("build.rs").exists());
        assert_eq!(source_identity(&next), source_identity(&next));
        assert_ne!(source_identity(&first), source_identity(&next));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_paths_cannot_escape_generated_project() {
        let root = temporary_directory("source-path");
        assert!(sync_sources(&root, &BTreeMap::from([("../escape".into(), "no".into())])).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_repository_lock_reseeds_but_unchanged_lock_keeps_generated_resolution() {
        let root = temporary_directory("root-lock");
        fs::create_dir_all(&root).unwrap();
        seed_lock(&root, b"root-v1").unwrap();
        fs::write(root.join("Cargo.lock"), "resolved-with-lessons").unwrap();
        seed_lock(&root, b"root-v1").unwrap();
        assert_eq!(
            fs::read(root.join("Cargo.lock")).unwrap(),
            b"resolved-with-lessons"
        );
        seed_lock(&root, b"root-v2").unwrap();
        assert_eq!(fs::read(root.join("Cargo.lock")).unwrap(), b"root-v2");
        fs::remove_dir_all(root).unwrap();
    }
}
