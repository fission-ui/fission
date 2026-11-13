//! Build the actual HTML project for Static site tutorials, not a Canvas imitation.
use super::{copy_dir, PreviewBuild, PreviewCacheHasher};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub(super) fn is_static(files: &BTreeMap<String, String>) -> bool {
    files.contains_key("fission.toml")
        && files.get("src/lib.rs").is_some_and(|source| {
            source.contains("pub fn build_site(") && source.contains("fission::site::")
        })
}

pub(super) fn build(
    workspace: &Path,
    target: &Path,
    preview: PreviewBuild,
    cache_context: &str,
) -> Result<()> {
    let started = Instant::now();
    let first = preview
        .outputs
        .first()
        .context("Static site preview has no output")?;
    let mut hasher = PreviewCacheHasher::default();
    hasher.field(b"docs2-static-tutorial-v1");
    hasher.field(cache_context.as_bytes());
    // HTML generation is a dependency of this preview, unlike the Web renderer.
    // A shell change must invalidate rendered pages even when authored files match.
    super::hash_directory(
        &mut hasher,
        workspace,
        &workspace.join("crates/shell/fission-shell-site"),
    )?;
    for (path, source) in &preview.files {
        hasher.field(path.as_bytes());
        hasher.field(source.as_bytes());
    }
    let fingerprint = format!("{:016x}", hasher.finish());
    let package = format!("docs2-site-{fingerprint}");
    let directory = target.join("tutorial-sites").join(&fingerprint);
    let output = directory.join("dist/site");
    let marker = directory.join(".complete");
    let cached = output_is_current(&output, &marker, &fingerprint)?;
    if !cached {
        for (relative, source) in &preview.files {
            let destination = directory.join(relative);
            fs::create_dir_all(
                destination
                    .parent()
                    .context("tutorial source has no parent")?,
            )?;
            fs::write(destination, source)?;
        }
        let runner = directory.join("src/bin/docs2_site_preview.rs");
        fs::create_dir_all(runner.parent().unwrap())?;
        fs::write(
            &runner,
            format!(
                "fn main() -> anyhow::Result<()> {{ {}::build_site() }}\n",
                package.replace('-', "_")
            ),
        )?;
        let mut manifest = format!(
            "[package]\nname = {package:?}\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             [workspace]\n\n[lib]\npath = \"src/lib.rs\"\n\n\
             [[bin]]\nname = \"site-preview\"\npath = \"src/bin/docs2_site_preview.rs\"\n\n\
             [dependencies]\nfission = {{ path = {:?}, default-features = false, features = [\"site\"] }}\n\
             anyhow = \"1\"\nserde = {{ version = \"1\", features = [\"derive\"] }}\n\
             serde_json = \"1\"\nserde_yaml = \"0.9\"\n",
            workspace.join("crates/authoring/fission")
        );
        if preview
            .files
            .values()
            .any(|source| source.contains("fission_design_system_codegen"))
        {
            manifest.push_str(&format!(
                "\n[build-dependencies]\nfission-design-system-codegen = {{ path = {:?} }}\n",
                workspace.join("crates/tools/fission-design-system-codegen")
            ));
        }
        let root_manifest = fs::read_to_string(workspace.join("Cargo.toml"))?;
        let profiles = root_manifest
            .find("[profile.dev]")
            .context("workspace profiles are missing")?;
        manifest.push_str(&root_manifest[profiles..]);
        fs::write(directory.join("Cargo.toml"), manifest)?;
        fs::copy(workspace.join("Cargo.lock"), directory.join("Cargo.lock"))?;
        eprintln!(
            "Building actual Static site preview: {} / {}",
            first.app, first.step
        );
        let status = Command::new("cargo")
            .args(["run", "--offline", "--manifest-path"])
            .arg(directory.join("Cargo.toml"))
            .args(["--bin", "site-preview", "--", "build", "--project-dir"])
            .arg(&directory)
            .env("CARGO_TARGET_DIR", target)
            .env("CARGO_BUILD_JOBS", "2")
            .status()
            .context("failed to run the tutorial's real Static site builder")?;
        if !status.success() {
            bail!(
                "Static site tutorial {} / {} failed ({status})",
                first.app,
                first.step
            );
        }
        if !output.join("index.html").is_file() {
            bail!("Static site tutorial must write its actual pages under dist/site");
        }
        fs::write(
            marker,
            format!("{fingerprint}\n{}", output_fingerprint(&output)?),
        )?;
    }
    eprintln!(
        "Static site {} / {}: cache {}, ready in {:.2?}.",
        first.app,
        first.step,
        if cached { "hit" } else { "miss" },
        started.elapsed()
    );
    for step in preview.outputs {
        if step.path.exists() {
            // Only the generated preview for this exact tutorial checkpoint.
            fs::remove_dir_all(&step.path)?;
        }
        copy_dir(&output, &step.path)?;
        eprintln!("  Ready (actual HTML): {} / {}", step.app, step.step);
    }
    Ok(())
}

fn output_fingerprint(output: &Path) -> Result<String> {
    let mut hasher = PreviewCacheHasher::default();
    super::hash_directory(&mut hasher, output, output)?;
    Ok(format!("{:016x}", hasher.finish()))
}

fn output_is_current(output: &Path, marker: &Path, source: &str) -> Result<bool> {
    if !output.join("index.html").is_file() {
        return Ok(false);
    }
    let Ok(marker) = fs::read_to_string(marker) else {
        return Ok(false);
    };
    let Some((cached_source, cached_output)) = marker.split_once('\n') else {
        return Ok(false);
    };
    Ok(cached_source == source && cached_output == output_fingerprint(output)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_preview_requires_the_project_and_real_website_entrypoint() {
        let mut files = BTreeMap::new();
        files.insert(
            "src/lib.rs".into(),
            "pub fn build_site() { fission::site::build_from_cli(site) }".into(),
        );
        assert!(!is_static(&files));
        files.insert("fission.toml".into(), "targets = [\"site\"]".into());
        assert!(is_static(&files));
        files.insert("src/lib.rs".into(), "pub fn run_web() {}".into());
        assert!(!is_static(&files));
    }

    #[test]
    fn missing_cached_child_page_requires_a_rebuild() {
        let directory = super::super::tests::temporary_directory("static-output");
        let output = directory.join("site");
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("index.html"), "home").unwrap();
        fs::write(output.join("guide.html"), "guide").unwrap();
        let marker = directory.join(".complete");
        fs::write(
            &marker,
            format!("source\n{}", output_fingerprint(&output).unwrap()),
        )
        .unwrap();
        assert!(output_is_current(&output, &marker, "source").unwrap());
        fs::remove_file(output.join("guide.html")).unwrap();
        assert!(!output_is_current(&output, &marker, "source").unwrap());
        fs::remove_dir_all(directory).unwrap();
    }
}
