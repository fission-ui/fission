use crate::content::{load_goals, GoalDocument};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn build_all(project_dir: &Path) -> Result<()> {
    let content_dir = project_dir.join("content/goals");
    let documents = load_goals(&content_dir)?;
    eprintln!(
        "Building {} goal tutorial(s) from documentation source...",
        documents.len()
    );
    let mut compiled_previews = BTreeMap::<BTreeMap<String, String>, std::path::PathBuf>::new();
    for (index, document) in documents.iter().enumerate() {
        eprintln!(
            "Goal {}/{}: {}",
            index + 1,
            documents.len(),
            document.meta.title
        );
        build_goal(project_dir, document, &mut compiled_previews)?;
    }
    Ok(())
}

fn build_goal(
    project_dir: &Path,
    document: &GoalDocument,
    compiled_previews: &mut BTreeMap<BTreeMap<String, String>, std::path::PathBuf>,
) -> Result<()> {
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
        let stage_dir = project_dir.join("target/tutorials/shared-preview");
        if stage_dir.exists() {
            fs::remove_dir_all(&stage_dir)
                .with_context(|| format!("failed to replace {}", stage_dir.display()))?;
        }
        fs::create_dir_all(&stage_dir)?;
        for (relative, contents) in &files {
            let stage_path = if relative == "preview/lib.rs" {
                Path::new("src/lib.rs")
            } else {
                Path::new(relative)
            };
            let destination = stage_dir.join(stage_path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(destination, contents)?;
        }
        let package = "docs2-tutorial-preview";
        let output = project_dir
            .join("generated-assets/tutorials")
            .join(&document.meta.app)
            .join(&step.id);
        if output.exists() {
            fs::remove_dir_all(&output)?;
        }
        fs::create_dir_all(&output)?;
        if let Some(previous_output) = compiled_previews.get(&files) {
            copy_dir(previous_output, &output)?;
            eprintln!("  Source is unchanged; reusing the compiled Web preview.");
        } else {
            let include_charts = files
                .values()
                .any(|source| source.contains("use fission::charts"));
            compile_preview(
                &document.meta.app,
                &step.id,
                &package,
                &stage_dir,
                &output,
                &workspace_root,
                &cargo_target,
                include_charts,
            )?;
            compiled_previews.insert(files.clone(), output.clone());
        }
        fs::write(
            output.join("index.html"),
            tutorial_html(&document.meta.title, &step.title, &document.meta.goal),
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

fn compile_preview(
    app: &str,
    step: &str,
    package: &str,
    stage_dir: &Path,
    output: &Path,
    workspace_root: &Path,
    cargo_target: &Path,
    include_charts: bool,
) -> Result<()> {
    fs::write(
        stage_dir.join("Cargo.toml"),
        tutorial_manifest(package, workspace_root, include_charts),
    )?;
    let status = Command::new("wasm-pack")
        .arg("build")
        .arg(stage_dir)
        .args(["--target", "web", "--release", "--out-dir"])
        .arg(output.join("pkg"))
        .env("CARGO_TARGET_DIR", cargo_target)
        .status()
        .context("failed to launch wasm-pack; install it before building goal tutorials")?;
    if !status.success() {
        bail!("tutorial `{app}` step `{step}` failed to compile");
    }
    let module = package.replace('-', "_");
    fs::write(
        output.join("bootstrap.mjs"),
        format!("import init from './pkg/{module}.js';\nawait init();\n"),
    )?;
    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}

fn tutorial_manifest(package: &str, workspace_root: &Path, include_charts: bool) -> String {
    let fission = workspace_root.join("crates/authoring/fission");
    let fission_features = if include_charts {
        "\"web\", \"charts\""
    } else {
        "\"web\""
    };
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
fission = {{ path = "{}", default-features = false, features = [{fission_features}] }}
wasm-bindgen = "0.2"
"#,
        fission.display()
    )
}

fn tutorial_html(goal: &str, step: &str, task: &str) -> String {
    let goal = html_escape(goal);
    let step = html_escape(step);
    let task = html_escape(task);
    format!(
        r#"<!doctype html>
<html lang="en-GB"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{step} — {goal}</title><meta name="tutorial-goal" content="{task}"><style>html,body,#fission-web-mount{{width:100%;height:100%;margin:0;overflow:hidden}}body{{background:#f7f6f1}}</style></head>
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

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
