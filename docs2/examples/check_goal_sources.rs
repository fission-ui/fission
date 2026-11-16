//! Compile the authored app and its documentation preview independently.
//! Example: cargo run -p fission-docs2 --example check_goal_sources -- rust-login-page
use anyhow::{bail, Context, Result};
use fission_docs2::content::load_goals;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<()> {
    let mut selected: Vec<String> = std::env::args().skip(1).collect();
    if selected
        .first()
        .is_some_and(|argument| argument == "--all-previews")
    {
        if selected.len() != 1 {
            bail!("--all-previews does not accept tutorial slugs");
        }
        return fission_docs2::tutorials::build_all(std::path::Path::new(env!(
            "CARGO_MANIFEST_DIR"
        )));
    }
    let package_final = selected
        .first()
        .is_some_and(|argument| argument == "--preview");
    if package_final {
        selected.remove(0);
    }
    if selected.is_empty() {
        bail!("provide one or more tutorial app slugs to check");
    }
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if package_final {
        for app in &selected {
            fission_docs2::tutorials::build_goal_preview(&project, app)?;
        }
        return Ok(());
    }
    let root = project.parent().context("docs2 has no workspace parent")?;
    let target = PathBuf::from(
        std::env::var_os("CARGO_TARGET_DIR").context("set the shared CARGO_TARGET_DIR first")?,
    );
    let stage = target.join("tutorial-source-audit");
    fs::create_dir_all(&stage)?;
    let documents = load_goals(project.join("content/goals"))?;
    for slug in &selected {
        if !documents.iter().any(|document| &document.meta.app == slug) {
            bail!("unknown tutorial app: {slug}");
        }
    }
    let mut checked_sources = Vec::new();
    let mut failures = Vec::new();
    for document in documents
        .iter()
        .filter(|doc| selected.contains(&doc.meta.app))
    {
        let result = (|| -> Result<()> {
            if document.steps.first().is_some_and(|step| {
                step.files.contains_key("fission.toml")
                    && step
                        .files
                        .get("src/lib.rs")
                        .is_some_and(|source| source.contains("pub fn build_site("))
            }) {
                fission_docs2::tutorials::check_static_goal(&project, &document.meta.app)?;
                return Ok(());
            }
            let mut files = BTreeMap::new();
            for step in &document.steps {
                if step.id.is_empty()
                    || !step
                        .id
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
                {
                    bail!("unsafe generated-source step directory: {:?}", step.id);
                }
                files.extend(step.files.clone());
                let label = format!("{} / {}", document.meta.app, step.id);
                if checked_sources.iter().any(|checked| checked == &files) {
                    eprintln!("Checked {label}: exact source already passed in this invocation");
                    continue;
                }
                if !files.contains_key("src/lib.rs") {
                    bail!("{label}: the tutorial does not wire its normal src/lib.rs");
                }
                let directory = stage.join(&document.meta.app).join(&step.id);
                if directory.exists() {
                    // This directory contains only this tool's generated sources.
                    // Removed tutorial files must not survive to mask a bad import.
                    fs::remove_dir_all(&directory)?;
                }
                fs::create_dir_all(&directory)?;
                for (relative, source) in &files {
                    let relative = if relative == "preview/lib.rs" {
                        "src/preview.rs"
                    } else {
                        relative
                    };
                    let destination = directory.join(relative);
                    fs::create_dir_all(destination.parent().context("source has no parent")?)?;
                    fs::write(destination, source)?;
                }
                let sources = files.values().map(String::as_str).collect::<Vec<_>>();
                let charts = sources
                    .iter()
                    .any(|source| source.contains("fission::charts"));
                let mut features =
                    fission_docs2::tutorials::tutorial_fission_features(&sources, charts);
                features.push("desktop");
                let features = features
                    .iter()
                    .map(|feature| format!("\"{feature}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                let preview = files.contains_key("preview/lib.rs");
                let preview_manifest = if preview {
                    "\n[[example]]\nname = \"preview\"\npath = \"src/preview.rs\"\ncrate-type = [\"rlib\"]\n"
                } else {
                    ""
                };
                let mut manifest = format!(
                "[package]\nname = \"docs2-source-{}-{}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n\n[lib]\nname = {:?}\npath = \"src/lib.rs\"\n\n[dependencies]\nfission = {{ path = {:?}, default-features = false, features = [{features}] }}\nanyhow = \"1\"\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\nserde_yaml = \"0.9\"\nwasm-bindgen = \"0.2\"\nconsole_error_panic_hook = \"0.1\"\n{preview_manifest}",
                document.meta.app,
                step.id,
                document.meta.app.replace('-', "_"),
                root.join("crates/authoring/fission"),
            );
                if sources.iter().any(|source| source.contains("web_sys::")) {
                    manifest.push_str("\n[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nweb-sys = { version = \"0.3\", features = [\"Window\", \"Location\"] }\n");
                }
                let mut native_features = Vec::new();
                if sources.iter().any(|source| {
                    source.contains(".store()")
                        || source.contains(".sql()")
                        || source.contains("StoreKey<")
                        || source.contains("SqlStatement")
                }) {
                    native_features.push("store-sqlite-native");
                }
                if sources
                    .iter()
                    .any(|source| source.contains("fission::site::"))
                {
                    native_features.push("site");
                }
                if sources.iter().any(|source| source.contains("pollster::")) {
                    if !native_features.contains(&"store-sqlite-native") {
                        native_features.push("store-sqlite-native");
                    }
                    manifest.push_str("\n[dev-dependencies]\npollster = \"0.3\"\n");
                }
                if !native_features.is_empty() {
                    manifest.push_str(&format!(
                    "\n[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\nfission = {{ path = {:?}, default-features = false, features = {:?} }}\n",
                    root.join("crates/authoring/fission"),
                    native_features,
                ));
                }
                if sources
                    .iter()
                    .any(|source| source.contains("fission_design_system_codegen"))
                {
                    manifest.push_str(&format!(
                    "\n[build-dependencies]\nfission-design-system-codegen = {{ path = {:?} }}\n",
                    root.join("crates/tools/fission-design-system-codegen"),
                ));
                }
                let workspace_manifest = fs::read_to_string(root.join("Cargo.toml"))?;
                let profile_start = workspace_manifest
                    .find("[profile.dev]")
                    .context("workspace profile settings are missing")?;
                manifest.push_str(&workspace_manifest[profile_start..]);
                let manifest_path = directory.join("Cargo.toml");
                fs::write(&manifest_path, manifest)?;
                // Start from the repository's dependency choices, rather than
                // resolving newer releases independently for every checkpoint.
                fs::copy(root.join("Cargo.lock"), directory.join("Cargo.lock"))?;
                let has_tests = sources.iter().any(|source| source.contains("#[test]"));
                eprintln!(
                    "Checking {label}: authored native app{}",
                    if has_tests { " and its tests" } else { "" }
                );
                let status = Command::new("cargo")
                    .args([
                        if has_tests { "test" } else { "check" },
                        "--offline",
                        "--lib",
                        "--manifest-path",
                    ])
                    .arg(&manifest_path)
                    .env("CARGO_TARGET_DIR", &target)
                    .status()?;
                if !status.success() {
                    bail!("{label}: native source tests failed");
                }
                // Authored test hosts are separate from the browser preview and
                // must compile against the project's original public crate name.
                for path in files
                    .keys()
                    .filter(|path| path.starts_with("examples/") && path.ends_with(".rs"))
                {
                    let example = PathBuf::from(path);
                    let name = example.file_stem().context("example has no name")?;
                    if !Command::new("cargo")
                        .args(["test", "--offline", "--manifest-path"])
                        .arg(&manifest_path)
                        .arg("--example")
                        .arg(name)
                        .env("CARGO_TARGET_DIR", &target)
                        .status()?
                        .success()
                    {
                        bail!("{label}: authored example {path} failed");
                    }
                }
                eprintln!("Checking {label}: authored Web app and documentation preview");
                let mut command = Command::new("cargo");
                command
                    .args([
                        "check",
                        "--offline",
                        "--lib",
                        "--target",
                        "wasm32-unknown-unknown",
                        "--manifest-path",
                    ])
                    .arg(&manifest_path)
                    .env("CARGO_TARGET_DIR", target.join("tutorial-wasm"));
                if preview {
                    command.args(["--example", "preview"]);
                }
                if !command.status()?.success() {
                    bail!("{label}: Web source check failed");
                }
                checked_sources.push(files.clone());
            }
            Ok(())
        })();
        match result {
            Ok(()) => eprintln!("SOURCE AUDIT PASSED: {}", document.meta.app),
            Err(error) => {
                eprintln!("SOURCE AUDIT FAILED: {}: {error:#}", document.meta.app);
                failures.push(format!("{}: {error:#}", document.meta.app));
            }
        }
    }
    if !failures.is_empty() {
        bail!("authored-source checks failed:\n{}", failures.join("\n"));
    }
    Ok(())
}
