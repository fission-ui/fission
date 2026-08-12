use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use fission_skia_artifacts::{
    self as artifacts, ArtifactKind, CANVASKIT_PROFILE, CANVASKIT_TARGET,
};

const ARTIFACT_DIR_ENV: &str = "FISSION_CANVASKIT_ARTIFACT_DIR";
const ALLOW_UNQUALIFIED_ENV: &str = "FISSION_CANVASKIT_ALLOW_UNQUALIFIED_ARTIFACT";
const MAX_TREE_DEPTH: usize = 64;
static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn stage_canvaskit(project_dir: &Path) -> Result<()> {
    let explicit = env::var_os(ARTIFACT_DIR_ENV);
    let allow_unqualified =
        artifacts::boolean_from_environment(ALLOW_UNQUALIFIED_ENV).map_err(anyhow::Error::msg)?;
    if allow_unqualified && explicit.is_none() {
        bail!("{ALLOW_UNQUALIFIED_ENV} applies only with an explicit {ARTIFACT_DIR_ENV}");
    }

    let artifact_root = if let Some(path) = explicit {
        if path.is_empty() {
            bail!("{ARTIFACT_DIR_ENV} must not be empty");
        }
        let path = PathBuf::from(path);
        require_real_directory(&path, ARTIFACT_DIR_ENV)?;
        artifacts::verify_local(
            &path,
            ArtifactKind::CanvasKit,
            CANVASKIT_TARGET,
            CANVASKIT_PROFILE,
            allow_unqualified,
        )
        .map_err(anyhow::Error::msg)?;
        path
    } else {
        let cache_root = artifacts::cache_root_from_environment().map_err(anyhow::Error::msg)?;
        let offline = artifacts::offline_from_environment().map_err(anyhow::Error::msg)?;
        artifacts::resolve_bundled(
            ArtifactKind::CanvasKit,
            CANVASKIT_TARGET,
            CANVASKIT_PROFILE,
            &cache_root,
            offline,
        )
        .map_err(anyhow::Error::msg)?
    };

    let destination = project_dir.join("platforms/web/canvaskit");
    stage_verified_tree(&artifact_root, &destination, allow_unqualified)
}

fn stage_verified_tree(source: &Path, destination: &Path, allow_unqualified: bool) -> Result<()> {
    artifacts::verify_local(
        source,
        ArtifactKind::CanvasKit,
        CANVASKIT_TARGET,
        CANVASKIT_PROFILE,
        allow_unqualified,
    )
    .map_err(anyhow::Error::msg)?;

    if destination.exists() {
        require_real_directory(destination, "staged CanvasKit directory")?;
        let source_manifest = fs::read(source.join("manifest.json"))?;
        let destination_manifest = fs::read(destination.join("manifest.json")).ok();
        if destination_manifest.as_deref() == Some(source_manifest.as_slice())
            && artifacts::verify_local(
                destination,
                ArtifactKind::CanvasKit,
                CANVASKIT_TARGET,
                CANVASKIT_PROFILE,
                allow_unqualified,
            )
            .is_ok()
        {
            return Ok(());
        }
    }

    let parent = destination
        .parent()
        .context("CanvasKit destination has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create CanvasKit Web directory {}",
            parent.display()
        )
    })?;
    require_real_directory(parent, "CanvasKit Web directory")?;

    let staging = TemporaryDirectory::new(parent, ".canvaskit-staging")?;
    copy_tree(source, staging.path())?;
    artifacts::verify_local(
        staging.path(),
        ArtifactKind::CanvasKit,
        CANVASKIT_TARGET,
        CANVASKIT_PROFILE,
        allow_unqualified,
    )
    .map_err(anyhow::Error::msg)?;

    if !destination.exists() {
        fs::rename(staging.path(), destination).with_context(|| {
            format!(
                "failed to publish CanvasKit Web assets at {}",
                destination.display()
            )
        })?;
        staging.keep();
        return Ok(());
    }

    let backup = unique_child(parent, ".canvaskit-previous")?;
    fs::rename(destination, &backup).with_context(|| {
        format!(
            "failed to move previous CanvasKit Web assets from {}",
            destination.display()
        )
    })?;
    if let Err(error) = fs::rename(staging.path(), destination) {
        let restore = fs::rename(&backup, destination);
        return match restore {
            Ok(()) => Err(error).context("failed to publish replacement CanvasKit Web assets"),
            Err(restore_error) => Err(error).context(format!(
                "failed to publish replacement CanvasKit Web assets and failed to restore {}: {restore_error}",
                destination.display()
            )),
        };
    }
    staging.keep();
    fs::remove_dir_all(&backup).with_context(|| {
        format!(
            "CanvasKit Web assets were replaced, but the previous tree could not be removed from {}",
            backup.display()
        )
    })?;
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    let mut stack = vec![(source.to_path_buf(), destination.to_path_buf(), 0usize)];
    while let Some((from, to, depth)) = stack.pop() {
        if depth > MAX_TREE_DEPTH {
            bail!("CanvasKit artifact tree exceeds {MAX_TREE_DEPTH} directory levels");
        }
        if depth > 0 {
            fs::create_dir(&to).with_context(|| {
                format!("failed to create CanvasKit directory {}", to.display())
            })?;
        }
        for entry in fs::read_dir(&from)
            .with_context(|| format!("failed to read CanvasKit directory {}", from.display()))?
        {
            let entry = entry?;
            let input = entry.path();
            let output = to.join(entry.file_name());
            let metadata = fs::symlink_metadata(&input)?;
            if metadata.file_type().is_symlink() {
                bail!(
                    "CanvasKit artifact contains a symbolic link: {}",
                    input.display()
                );
            }
            if metadata.is_dir() {
                stack.push((input, output, depth + 1));
            } else if metadata.is_file() {
                fs::copy(&input, &output).with_context(|| {
                    format!(
                        "failed to copy CanvasKit asset {} to {}",
                        input.display(),
                        output.display()
                    )
                })?;
            } else {
                bail!(
                    "CanvasKit artifact contains a special file: {}",
                    input.display()
                );
            }
        }
    }
    Ok(())
}

fn require_real_directory(path: &Path, description: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {description} at {}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!("{description} must be a real directory: {}", path.display());
    }
    Ok(())
}

fn unique_child(parent: &Path, prefix: &str) -> Result<PathBuf> {
    for _ in 0..32 {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!("{prefix}-{}-{time}-{sequence}", std::process::id()));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("failed to allocate a unique CanvasKit staging path")
}

struct TemporaryDirectory {
    path: PathBuf,
    keep: bool,
}

impl TemporaryDirectory {
    fn new(parent: &Path, prefix: &str) -> Result<Self> {
        for _ in 0..32 {
            let path = unique_child(parent, prefix)?;
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path, keep: false }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to create CanvasKit staging path {}", path.display())
                    });
                }
            }
        }
        bail!("failed to allocate a private CanvasKit staging directory")
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use serde_json::json;
    use sha2::{Digest, Sha256};

    use super::*;

    const ASSETS: &[(&str, &str)] = &[
        ("canvaskit_js", "web/canvaskit.js"),
        ("canvaskit_wasm", "web/canvaskit.wasm"),
        ("fission_web_bridge", "web/fission_skia_web.js"),
        ("fission_command_decoder", "web/fission_skia_commands.js"),
        ("fission_frame_executor", "web/fission_skia_executor.js"),
        (
            "fission_paragraph_wire",
            "web/fission_skia_paragraph_wire.js",
        ),
        (
            "fission_paragraph_unicode",
            "web/fission_skia_paragraph_unicode.js",
        ),
        ("fission_paragraph_host", "web/fission_skia_paragraph.js"),
    ];
    const LICENSES: &[&str] = &[
        "brotli",
        "fission",
        "freetype",
        "harfbuzz",
        "icu",
        "libjpeg-turbo",
        "libpng",
        "libwebp",
        "skia",
        "woff2",
        "wuffs",
        "zlib",
    ];

    fn write_fixture(root: &Path, marker: &str) {
        let mut files = Vec::new();
        let mut assets = BTreeMap::new();
        for (name, relative) in ASSETS {
            let bytes = if *relative == "web/canvaskit.wasm" {
                b"\0asm\x01\0\0\0fixture".to_vec()
            } else {
                format!("// {marker} {relative}\n").into_bytes()
            };
            let record = write_payload(root, relative, &bytes);
            assets.insert(
                *name,
                json!({
                    "path": relative,
                    "sha256": record["sha256"],
                    "size": record["size"],
                    "role": "fixture",
                    "media_type": "application/octet-stream",
                }),
            );
            files.push(record);
        }
        for name in LICENSES {
            files.push(write_payload(
                root,
                &format!("licenses/{name}.txt"),
                format!("{marker} {name}\n").as_bytes(),
            ));
        }
        let manifest = json!({
            "schema_version": 1,
            "artifact_id": "fission-canvaskit-0.10.1-wasm32-unknown-unknown-canvaskit-production-abi13-wire1",
            "fission_version": "0.10.1",
            "origin": "local-build",
            "qualified": false,
            "profile": "canvaskit-production",
            "target": "wasm32-unknown-unknown",
            "platform": "Web",
            "lane": "webgl-ganesh",
            "source": {
                "repository": "https://skia.googlesource.com/skia.git",
                "revision": artifacts::SKIA_REVISION,
            },
            "abi": {
                "bridge_abi_version": artifacts::BRIDGE_ABI_VERSION,
                "web_protocol_version": artifacts::CANVASKIT_PROTOCOL_VERSION,
            },
            "assets": assets,
            "files": files,
        });
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn write_payload(root: &Path, relative: &str, bytes: &[u8]) -> serde_json::Value {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        json!({
            "path": relative,
            "sha256": format!("{:x}", Sha256::digest(bytes)),
            "size": bytes.len(),
        })
    }

    #[test]
    fn stages_the_complete_verified_artifact_tree() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        let destination = temporary.path().join("project/platforms/web/canvaskit");
        fs::create_dir(&source).unwrap();
        write_fixture(&source, "first");

        stage_verified_tree(&source, &destination, true).unwrap();

        assert_eq!(
            fs::read(destination.join("manifest.json")).unwrap(),
            fs::read(source.join("manifest.json")).unwrap()
        );
        assert!(destination.join("web/canvaskit.wasm").is_file());
        assert!(destination.join("licenses/skia.txt").is_file());
        artifacts::verify_local(
            &destination,
            ArtifactKind::CanvasKit,
            CANVASKIT_TARGET,
            CANVASKIT_PROFILE,
            true,
        )
        .unwrap();
    }

    #[test]
    fn replaces_a_stale_tree_without_mixing_artifact_versions() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        let destination = temporary.path().join("project/platforms/web/canvaskit");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        write_fixture(&source, "replacement");
        write_fixture(&destination, "stale");

        stage_verified_tree(&source, &destination, true).unwrap();

        assert_eq!(
            fs::read(destination.join("manifest.json")).unwrap(),
            fs::read(source.join("manifest.json")).unwrap()
        );
        assert!(fs::read_to_string(destination.join("web/canvaskit.js"))
            .unwrap()
            .contains("replacement"));
        assert!(!fs::read_to_string(destination.join("web/canvaskit.js"))
            .unwrap()
            .contains("stale"));
    }
}
