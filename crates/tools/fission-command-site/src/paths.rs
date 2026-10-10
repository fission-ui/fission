//! Lexical paths for local serving, including Windows mapped and WebDAV roots.
//!
//! The explicitly selected output root is trusted and may itself be reached
//! through a mount or link. URL-derived descendants must be normal components;
//! existing descendant symlinks and Windows reparse points are rejected rather
//! than resolved. The output tree must remain under the owner's control while
//! serving: these metadata checks are not a filesystem sandbox against races.
use anyhow::{bail, Context, Result};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

pub(super) fn absolute_root(root: &Path) -> Result<PathBuf> {
    let root = if root.is_absolute() {
        root.to_owned()
    } else {
        std::env::current_dir()
            .context("cannot determine serving working directory")?
            .join(root)
    };
    if !root.is_dir() {
        bail!("serving output directory is missing: {}", root.display());
    }
    Ok(root)
}

pub(super) fn asset_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = root.to_owned();
    for part in relative.split('/') {
        if part.is_empty() {
            continue;
        }
        if part == "."
            || part == ".."
            || part.contains(['\\', ':', '%', '\0'])
            || !matches!(
                Path::new(part).components().collect::<Vec<_>>().as_slice(),
                [Component::Normal(_)]
            )
        {
            bail!("invalid static path `{relative}`");
        }
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_link(&metadata) => {
                bail!("static path contains a link or reparse point: `{relative}`");
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("cannot inspect static path `{relative}`"))
            }
        }
    }
    Ok(path)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_url_components_that_can_escape_or_alias_windows_paths() {
        let root = std::env::temp_dir();
        for relative in [
            "../secret",
            "./secret",
            "a/../secret",
            "C:/secret",
            "a\\secret",
            "file:stream",
            "%2e%2e/secret",
            "a\0b",
        ] {
            assert!(asset_path(&root, relative).is_err(), "{relative}");
        }
        assert_eq!(
            asset_path(&root, "nested/file.css").unwrap(),
            root.join("nested/file.css")
        );
    }
}
