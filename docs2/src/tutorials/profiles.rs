//! Generated preview workspaces inherit the repository's existing Cargo profiles.
use super::PreviewCacheHasher;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub(super) fn append_root_profiles(workspace: &Path, manifest: &mut String) -> Result<()> {
    let root_manifest = fs::read_to_string(workspace.join("Cargo.toml"))?;
    // Profiles are the final section of this repository's manifest. Preserve
    // its dev/package/release settings verbatim, as Static site previews did
    // already, rather than inventing a second release profile for Web apps.
    let start = root_manifest
        .find("[profile.dev]")
        .context("workspace profiles are missing")?;
    manifest.push('\n');
    manifest.push_str(&root_manifest[start..]);
    Ok(())
}

pub(super) fn hash_root_manifest(workspace: &Path, hasher: &mut PreviewCacheHasher) -> Result<()> {
    // Both Web packages and Static site outputs derive their context from this
    // input. A root profile edit must invalidate a package built with old flags.
    hasher.field(b"Cargo.toml");
    hasher.field(&fs::read(workspace.join("Cargo.toml"))?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tutorials::{cache, preview_workspace_manifest, static_site};

    const PROFILES: &str = "[profile.dev]\ndebug = \"line-tables-only\"\n\n\
        [profile.dev.package.\"*\"]\ndebug = false\nopt-level = 1\n\n\
        [profile.release]\nstrip = true\nlto = true\nopt-level = \"z\"\n\
        codegen-units = 1\npanic = \"abort\"\n";

    fn context(workspace: &Path) -> String {
        let mut hasher = PreviewCacheHasher::default();
        hash_root_manifest(workspace, &mut hasher).unwrap();
        format!("{:016x}", hasher.finish())
    }

    #[test]
    fn web_and_static_manifests_copy_the_exact_repository_profiles() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let expected = &source[source.find("[profile.dev]").unwrap()..];
        for manifest in [
            preview_workspace_manifest(root, "\"preview-test\"").unwrap(),
            static_site::preview_manifest(root, "site-test", false).unwrap(),
            static_site::preview_manifest(root, "site-codegen-test", true).unwrap(),
        ] {
            assert_eq!(
                &manifest[manifest.find("[profile.dev]").unwrap()..],
                expected
            );
            assert!(manifest.contains("[profile.release]\n"));
        }
    }

    #[test]
    fn root_profile_changes_invalidate_complete_cached_packages() {
        let root = super::super::tests::temporary_directory("profile-cache");
        let package = root.join("package");
        fs::create_dir_all(&package).unwrap();
        fs::write(root.join("Cargo.toml"), format!("[workspace]\n{PROFILES}")).unwrap();
        fs::write(package.join("preview.js"), "javascript").unwrap();
        fs::write(package.join("preview_bg.wasm"), "wasm").unwrap();
        let original = context(&root);
        cache::mark_complete(&package, &original).unwrap();
        assert!(cache::package_is_current(&package, &original, "preview").unwrap());

        for (before, after) in [
            ("lto = true", "lto = false"),
            ("opt-level = \"z\"", "opt-level = 3"),
            ("codegen-units = 1", "codegen-units = 16"),
            ("panic = \"abort\"", "panic = \"unwind\""),
            ("strip = true", "strip = false"),
        ] {
            let changed = PROFILES.replace(before, after);
            fs::write(root.join("Cargo.toml"), format!("[workspace]\n{changed}")).unwrap();
            let current = context(&root);
            assert_ne!(original, current, "profile change was not hashed: {before}");
            assert!(!cache::package_is_current(&package, &current, "preview").unwrap());
            assert!(preview_workspace_manifest(&root, "\"preview-test\"")
                .unwrap()
                .ends_with(&changed));
            assert!(static_site::preview_manifest(&root, "site-test", false)
                .unwrap()
                .ends_with(&changed));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
