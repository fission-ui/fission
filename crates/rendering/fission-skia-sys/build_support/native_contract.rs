use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) const MAX_ARTIFACT_FILES: usize = 8_192;
pub(crate) const MAX_EXPANDED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub(crate) const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TREE_ENTRIES: usize = MAX_ARTIFACT_FILES + 1_024;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_PATH_COMPONENTS: usize = 64;

const NATIVE_RASTER_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "x86_64-linux-android",
    "i686-linux-android",
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "x86_64-apple-ios",
];

pub(crate) fn supports_native_raster(target: &str) -> bool {
    NATIVE_RASTER_TARGETS.contains(&target)
}

pub(crate) fn artifact_id(version: &str, abi: u32, profile: &str, target: &str) -> String {
    format!("fission-skia-{version}-abi{abi}-{profile}-{target}")
}

pub(crate) fn parse_boolean(value: &str, variable: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => Err(format!(
            "{variable} must be one of 1, 0, true, false, yes, or no"
        )),
    }
}

pub(crate) fn validate_link_name(value: &str, description: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'+' | b'.' | b'-'))
    {
        return Err(format!(
            "{description} {value:?} contains characters unsafe for a Cargo link directive"
        ));
    }
    Ok(())
}

pub(crate) fn parse_link_override(value: &str) -> Result<Vec<&str>, String> {
    if value.trim().is_empty() {
        return Err("FISSION_SKIA_LINK_LIBS must not be empty".to_owned());
    }
    value
        .split(',')
        .map(str::trim)
        .map(|library| {
            validate_link_name(library, "Skia static-library override")?;
            Ok(library)
        })
        .collect()
}

pub(crate) fn safe_relative(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.len() > MAX_PATH_BYTES || value.contains('\\') {
        return Err(format!("artifact manifest contains unsafe path {value:?}"));
    }
    let path = Path::new(value);
    let mut components = 0usize;
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!("artifact manifest contains unsafe path {value:?}"));
        }
        components += 1;
        if components > MAX_PATH_COMPONENTS {
            return Err(format!(
                "artifact manifest path has more than {MAX_PATH_COMPONENTS} components: {value:?}"
            ));
        }
    }
    if components == 0 {
        return Err(format!("artifact manifest contains unsafe path {value:?}"));
    }
    Ok(path.to_path_buf())
}

pub(crate) fn declared_file_set<'a>(
    paths: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<String>, String> {
    let mut declared = BTreeSet::new();
    let mut folded = BTreeSet::new();
    for path in paths {
        safe_relative(path)?;
        if declared.len() >= MAX_ARTIFACT_FILES {
            return Err(format!(
                "artifact manifest declares more than {MAX_ARTIFACT_FILES} payload files"
            ));
        }
        if !declared.insert(path.to_owned()) || !folded.insert(path.to_lowercase()) {
            return Err(format!(
                "artifact manifest contains a duplicate or case-colliding path {path:?}"
            ));
        }
    }
    if declared.is_empty() {
        return Err("Skia artifact manifest has no files".to_owned());
    }
    Ok(declared)
}

pub(crate) fn inspect_artifact_tree(root: &Path) -> Result<BTreeSet<String>, String> {
    let root_metadata = fs::symlink_metadata(root).map_err(|error| {
        format!(
            "failed to inspect artifact root {}: {error}",
            root.display()
        )
    })?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(format!(
            "artifact root is not a real directory: {}",
            root.display()
        ));
    }

    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut files = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut entries = 0usize;
    let mut total_bytes = 0u64;
    while let Some((directory, depth)) = stack.pop() {
        let children = fs::read_dir(&directory).map_err(|error| {
            format!(
                "failed to read artifact directory {}: {error}",
                directory.display()
            )
        })?;
        for child in children {
            let child = child.map_err(|error| {
                format!(
                    "failed to enumerate artifact directory {}: {error}",
                    directory.display()
                )
            })?;
            entries = entries
                .checked_add(1)
                .ok_or_else(|| "artifact tree entry count overflowed usize".to_owned())?;
            if entries > MAX_TREE_ENTRIES {
                return Err(format!(
                    "artifact tree contains more than {MAX_TREE_ENTRIES} entries"
                ));
            }
            let path = child.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "artifact tree contains a symbolic link: {}",
                    path.display()
                ));
            }
            let relative = canonical_tree_path(root, &path)?;
            if metadata.is_dir() {
                let next_depth = depth + 1;
                if next_depth > MAX_PATH_COMPONENTS {
                    return Err(format!(
                        "artifact tree exceeds the {MAX_PATH_COMPONENTS}-component depth limit"
                    ));
                }
                stack.push((path, next_depth));
            } else if metadata.is_file() {
                total_bytes = total_bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| "artifact tree size overflowed u64".to_owned())?;
                if total_bytes > MAX_EXPANDED_BYTES {
                    return Err(format!(
                        "artifact tree exceeds the {MAX_EXPANDED_BYTES}-byte expanded limit"
                    ));
                }
                if !files.insert(relative.clone()) || !folded.insert(relative.to_lowercase()) {
                    return Err(format!(
                        "artifact tree contains a duplicate or case-colliding path {relative:?}"
                    ));
                }
            } else {
                return Err(format!(
                    "artifact tree contains a special file: {}",
                    path.display()
                ));
            }
        }
    }

    if !files.contains("manifest.json") {
        return Err("artifact tree does not contain manifest.json".to_owned());
    }
    let manifest = root.join("manifest.json");
    let manifest_metadata = fs::symlink_metadata(&manifest)
        .map_err(|error| format!("failed to inspect {}: {error}", manifest.display()))?;
    if !manifest_metadata.is_file()
        || manifest_metadata.file_type().is_symlink()
        || manifest_metadata.len() > MAX_MANIFEST_BYTES
    {
        return Err("artifact manifest is not a bounded regular file".to_owned());
    }
    Ok(files)
}

pub(crate) fn verify_payload_set(
    mut actual: BTreeSet<String>,
    declared: &BTreeSet<String>,
) -> Result<(), String> {
    actual.remove("manifest.json");
    if &actual != declared {
        return Err(format!(
            "Skia artifact payload set differs from its manifest; missing={:?}, undeclared={:?}",
            declared.difference(&actual).collect::<Vec<_>>(),
            actual.difference(declared).collect::<Vec<_>>()
        ));
    }
    Ok(())
}

fn canonical_tree_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "artifact tree escaped its root".to_owned())?;
    let text = relative
        .to_str()
        .ok_or_else(|| "artifact tree contains a non-UTF-8 path".to_owned())?;
    if text.contains('\\') {
        return Err(format!(
            "artifact tree contains a non-canonical path {text:?}"
        ));
    }
    let canonical = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("artifact tree contains an unsafe path {text:?}"))?
        .join("/");
    safe_relative(&canonical)?;
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fission-skia-native-contract-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn native_raster_target_vocabulary_is_closed() {
        assert!(supports_native_raster("x86_64-unknown-linux-gnu"));
        assert!(supports_native_raster("aarch64-linux-android"));
        assert!(supports_native_raster("aarch64-apple-ios-sim"));
        assert!(!supports_native_raster("wasm32-wasip1"));
        assert!(!supports_native_raster("powerpc64le-unknown-linux-gnu"));
    }

    #[test]
    fn native_artifact_identity_binds_every_selection_input() {
        assert_eq!(
            artifact_id("0.10.1", 14, "native-raster", "x86_64-unknown-linux-gnu"),
            "fission-skia-0.10.1-abi14-native-raster-x86_64-unknown-linux-gnu"
        );
    }

    #[test]
    fn booleans_are_values_not_presence_flags() {
        assert_eq!(parse_boolean("1", "FLAG").unwrap(), true);
        assert_eq!(parse_boolean("FALSE", "FLAG").unwrap(), false);
        assert!(parse_boolean("", "FLAG").is_err());
        assert!(parse_boolean("enabled", "FLAG").is_err());
    }

    #[test]
    fn link_overrides_reject_empty_or_directive_shaped_values() {
        assert_eq!(
            parse_link_override("fission_skia_bridge, skia").unwrap(),
            ["fission_skia_bridge", "skia"]
        );
        for value in [
            "",
            "fission_skia_bridge,,skia",
            "skia\nother",
            "static=skia",
        ] {
            assert!(parse_link_override(value).is_err(), "accepted {value:?}");
        }
    }

    #[test]
    fn declared_paths_reject_duplicates_collisions_and_escapes() {
        assert!(declared_file_set(["lib/libskia.a", "include/fission_skia.h"]).is_ok());
        assert!(declared_file_set(["lib/libskia.a", "lib/libskia.a"]).is_err());
        assert!(declared_file_set(["lib/Skia.a", "lib/skia.a"]).is_err());
        assert!(declared_file_set(["../escape"]).is_err());
        assert!(declared_file_set(["windows\\separator"]).is_err());
    }

    #[test]
    fn artifact_tree_is_exact_bounded_and_iterative() {
        let root = TestDirectory::new();
        fs::write(root.path().join("manifest.json"), b"{}\n").unwrap();
        fs::create_dir(root.path().join("lib")).unwrap();
        fs::write(root.path().join("lib/libskia.a"), b"bytes").unwrap();
        let files = inspect_artifact_tree(root.path()).unwrap();
        assert_eq!(
            files,
            BTreeSet::from(["lib/libskia.a".to_owned(), "manifest.json".to_owned()])
        );
        assert!(
            verify_payload_set(files.clone(), &BTreeSet::from(["lib/libskia.a".to_owned()]))
                .is_ok()
        );
        assert!(verify_payload_set(files, &BTreeSet::from(["other".to_owned()])).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn artifact_tree_rejects_symlinks_and_special_files() {
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let root = TestDirectory::new();
        fs::write(root.path().join("payload"), b"bytes").unwrap();
        symlink(
            root.path().join("payload"),
            root.path().join("manifest.json"),
        )
        .unwrap();
        assert!(inspect_artifact_tree(root.path())
            .unwrap_err()
            .contains("symbolic link"));

        fs::remove_file(root.path().join("manifest.json")).unwrap();
        fs::write(root.path().join("manifest.json"), b"{}\n").unwrap();
        fs::create_dir(root.path().join("lib")).unwrap();
        symlink(
            root.path().join("payload"),
            root.path().join("lib/libskia.a"),
        )
        .unwrap();
        assert!(inspect_artifact_tree(root.path())
            .unwrap_err()
            .contains("symbolic link"));
        fs::remove_file(root.path().join("lib/libskia.a")).unwrap();
        let _listener = UnixListener::bind(root.path().join("socket")).unwrap();
        assert!(inspect_artifact_tree(root.path())
            .unwrap_err()
            .contains("special file"));
    }

    #[cfg(unix)]
    #[test]
    fn artifact_root_must_not_be_a_symlink() {
        use std::os::unix::fs::symlink;

        let parent = TestDirectory::new();
        let real = parent.path().join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("manifest.json"), b"{}\n").unwrap();
        let linked = parent.path().join("linked");
        symlink(&real, &linked).unwrap();
        assert!(inspect_artifact_tree(&linked)
            .unwrap_err()
            .contains("not a real directory"));
    }

    #[test]
    fn artifact_tree_rejects_excessive_depth() {
        let root = TestDirectory::new();
        fs::write(root.path().join("manifest.json"), b"{}\n").unwrap();
        let mut directory = root.path().to_path_buf();
        for _ in 0..=MAX_PATH_COMPONENTS {
            directory.push("d");
            fs::create_dir(&directory).unwrap();
        }
        let error = inspect_artifact_tree(root.path()).unwrap_err();
        assert!(
            error.contains("depth limit") || error.contains("components"),
            "unexpected error: {error}"
        );
    }
}
