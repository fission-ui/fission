use crate::content::{load_goal, GoalDocument};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn build_all(project_dir: &Path) -> Result<()> {
    let content_dir = project_dir.join("content/goals");
    let mut documents = Vec::new();
    for entry in fs::read_dir(&content_dir)
        .with_context(|| format!("failed to read {}", content_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("mdx") {
            documents.push(load_goal(path)?);
        }
    }
    documents.sort_by_key(|document| document.meta.order);
    eprintln!(
        "Building {} goal tutorial(s) from documentation source...",
        documents.len()
    );
    for (index, document) in documents.iter().enumerate() {
        eprintln!(
            "Goal {}/{}: {}",
            index + 1,
            documents.len(),
            document.meta.title
        );
        build_goal(project_dir, document)?;
    }
    Ok(())
}

fn build_goal(project_dir: &Path, document: &GoalDocument) -> Result<()> {
    let workspace_root = project_dir
        .parent()
        .context("docs2 must live below the Fission workspace")?;
    let mut files = BTreeMap::<String, String>::new();
    let cargo_target = project_dir.join("target/tutorial-wasm");
    if !cargo_target.exists() {
        if let Some(first_step) = document.steps.first() {
            let legacy_target = project_dir
                .join("target/tutorials")
                .join(&document.meta.app)
                .join(&first_step.id)
                .join("target");
            if legacy_target.exists() {
                eprintln!("  Reusing the existing tutorial Web build cache...");
                fs::rename(&legacy_target, &cargo_target).with_context(|| {
                    format!(
                        "failed to move {} to {}",
                        legacy_target.display(),
                        cargo_target.display()
                    )
                })?;
            }
        }
    }
    for (index, step) in document.steps.iter().enumerate() {
        eprintln!(
            "  Step {}/{} `{}`: compiling interactive Web app...",
            index + 1,
            document.steps.len(),
            step.id
        );
        files.extend(step.files.clone());
        let stage_dir = project_dir
            .join("target/tutorials")
            .join(&document.meta.app)
            .join(&step.id);
        if stage_dir.exists() {
            fs::remove_dir_all(&stage_dir)
                .with_context(|| format!("failed to replace {}", stage_dir.display()))?;
        }
        for (relative, contents) in &files {
            let destination = stage_dir.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(destination, contents)?;
        }
        let package = format!("docs2-{}-{}", document.meta.app, step.id).replace('_', "-");
        fs::write(
            stage_dir.join("Cargo.toml"),
            tutorial_manifest(&package, workspace_root),
        )?;
        let output = project_dir
            .join("generated-assets/tutorials")
            .join(&document.meta.app)
            .join(&step.id);
        if output.exists() {
            fs::remove_dir_all(&output)?;
        }
        fs::create_dir_all(&output)?;
        let status = Command::new("wasm-pack")
            .arg("build")
            .arg(&stage_dir)
            .args(["--target", "web", "--dev", "--out-dir"])
            .arg(output.join("pkg"))
            .env("CARGO_TARGET_DIR", &cargo_target)
            .status()
            .context("failed to launch wasm-pack; install it before building goal tutorials")?;
        if !status.success() {
            bail!(
                "tutorial `{}` step `{}` failed to compile",
                document.meta.app,
                step.id
            );
        }
        let module = package.replace('-', "_");
        fs::write(
            output.join("bootstrap.mjs"),
            format!("import init from './pkg/{module}.js';\nawait init();\n"),
        )?;
        fs::write(
            output.join("index.html"),
            tutorial_html(&document.meta.title, &step.title),
        )?;
        eprintln!(
            "  Step {}/{} `{}`: ready",
            index + 1,
            document.steps.len(),
            step.id
        );
    }
    Ok(())
}

fn tutorial_manifest(package: &str, workspace_root: &Path) -> String {
    let fission = workspace_root.join("crates/authoring/fission");
    format!(
        r#"[workspace]

[package]
name = "{package}"
version = "0.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
anyhow = "1"
console_error_panic_hook = "0.1"
fission = {{ path = "{}", default-features = false, features = ["web"] }}
wasm-bindgen = "0.2"
"#,
        fission.display()
    )
}

fn tutorial_html(goal: &str, step: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en-GB"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{step} — {goal}</title><style>html,body,#fission-web-mount{{width:100%;height:100%;margin:0;overflow:hidden}}body{{background:#f7f6f1}}</style></head>
<body><main id="fission-web-mount" aria-label="Interactive result for {step}"></main><script>
// A preview must not steal focus or scroll its parent when its canvas starts.
// After the reader interacts, normal canvas focus and keyboard input resume.
let interacted = false;
addEventListener('pointerdown', () => interacted = true, {{capture: true, once: true}});
addEventListener('keydown', () => interacted = true, {{capture: true, once: true}});
const focusCanvas = HTMLCanvasElement.prototype.focus;
HTMLCanvasElement.prototype.focus = function(...args) {{
    if (interacted) return focusCanvas.apply(this, args);
}};
</script><script type="module" src="./bootstrap.mjs"></script></body></html>"#
    )
}
