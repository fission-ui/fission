use crate::content::load_goals;
use anyhow::{bail, Context, Result};
use std::collections::{hash_map::DefaultHasher, BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;

const TUTORIAL_BUILD_PARALLELISM_ENV: &str = "FISSION_TUTORIAL_BUILD_PARALLELISM";
const DEFAULT_TUTORIAL_BUILD_PARALLELISM: usize = 10;
const MAX_TUTORIAL_BUILD_PARALLELISM: usize = 64;

struct PreviewOutput {
    app: String,
    step: String,
    goal_title: String,
    step_title: String,
    goal: String,
    path: std::path::PathBuf,
}

struct PreviewBuild {
    files: BTreeMap<String, String>,
    include_charts: bool,
    outputs: Vec<PreviewOutput>,
}

pub fn build_all(project_dir: &Path) -> Result<()> {
    let content_dir = project_dir.join("content/goals");
    let documents = load_goals(&content_dir)?;
    let parallelism = tutorial_build_parallelism()?;
    eprintln!(
        "Preparing {} goal tutorial(s) from documentation source...",
        documents.len(),
    );
    let mut previews = Vec::<PreviewBuild>::new();
    let mut preview_indices = HashMap::<u64, Vec<usize>>::new();

    for (index, document) in documents.iter().enumerate() {
        eprintln!(
            "Goal {}/{}: {}",
            index + 1,
            documents.len(),
            document.meta.title
        );
        let mut files = BTreeMap::<String, String>::new();
        for step in &document.steps {
            files.extend(step.files.clone());
            let mut hasher = DefaultHasher::new();
            files.hash(&mut hasher);
            let fingerprint = hasher.finish();
            let existing = preview_indices.get(&fingerprint).and_then(|candidates| {
                candidates
                    .iter()
                    .copied()
                    .find(|candidate| previews[*candidate].files == files)
            });
            let preview_index = if let Some(existing) = existing {
                existing
            } else {
                let preview_index = previews.len();
                previews.push(PreviewBuild {
                    include_charts: files
                        .values()
                        .any(|source| source.contains("use fission::charts")),
                    files: files.clone(),
                    outputs: Vec::new(),
                });
                preview_indices
                    .entry(fingerprint)
                    .or_default()
                    .push(preview_index);
                preview_index
            };
            previews[preview_index].outputs.push(PreviewOutput {
                app: document.meta.app.clone(),
                step: step.id.clone(),
                goal_title: document.meta.title.clone(),
                step_title: step.title.clone(),
                goal: document.meta.goal.clone(),
                path: project_dir
                    .join("generated-assets/tutorials")
                    .join(&document.meta.app)
                    .join(&step.id),
            });
        }
    }

    if previews.is_empty() {
        return Ok(());
    }
    let workers = parallelism.min(previews.len());
    eprintln!(
        "Compiling {} unique Web preview(s) with {workers} worker(s); set {TUTORIAL_BUILD_PARALLELISM_ENV} to change concurrency.",
        previews.len(),
    );
    let workspace_root = project_dir
        .parent()
        .context("docs2 must live below the Fission workspace")?;

    let first_target = project_dir.join("target/tutorial-wasm");
    if !first_target.exists() {
        if let Some(first_step) = documents.first().and_then(|doc| doc.steps.first()) {
            let legacy_target = project_dir
                .join("target/tutorials")
                .join(&documents[0].meta.app)
                .join(&first_step.id)
                .join("target");
            if legacy_target.exists() {
                eprintln!("Reusing the existing tutorial Web build cache...");
                fs::rename(&legacy_target, &first_target).with_context(|| {
                    format!(
                        "failed to move {} to {}",
                        legacy_target.display(),
                        first_target.display()
                    )
                })?;
            }
        }
    }

    let next_preview = AtomicUsize::new(0);
    let cancelled = AtomicBool::new(false);
    let errors = Mutex::new(Vec::<String>::new());
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for worker in 0..workers {
            let next_preview = &next_preview;
            let cancelled = &cancelled;
            let errors = &errors;
            let previews = &previews;
            handles.push(scope.spawn(move || {
                while !cancelled.load(Ordering::Acquire) {
                    let preview_index = next_preview.fetch_add(1, Ordering::Relaxed);
                    let Some(preview) = previews.get(preview_index) else {
                        break;
                    };
                    if let Err(error) = build_preview(
                        project_dir,
                        workspace_root,
                        preview,
                        preview_index,
                        previews.len(),
                        worker,
                    ) {
                        errors
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push(format!("{error:#}"));
                        cancelled.store(true, Ordering::Release);
                        break;
                    }
                }
            }));
        }
        for handle in handles {
            if handle.join().is_err() {
                cancelled.store(true, Ordering::Release);
                errors
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push("a tutorial preview build worker panicked".into());
            }
        }
    });

    let errors = errors
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !errors.is_empty() {
        bail!(
            "{} tutorial preview build(s) failed:\n{}",
            errors.len(),
            errors.join("\n")
        );
    }
    Ok(())
}

fn tutorial_build_parallelism() -> Result<usize> {
    let value = match std::env::var(TUTORIAL_BUILD_PARALLELISM_ENV) {
        Ok(value) => value
            .parse::<usize>()
            .with_context(|| format!("{TUTORIAL_BUILD_PARALLELISM_ENV} must be an integer"))?,
        Err(std::env::VarError::NotPresent) => DEFAULT_TUTORIAL_BUILD_PARALLELISM,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {TUTORIAL_BUILD_PARALLELISM_ENV}"))
        }
    };
    if !(1..=MAX_TUTORIAL_BUILD_PARALLELISM).contains(&value) {
        bail!(
            "{TUTORIAL_BUILD_PARALLELISM_ENV} must be between 1 and {MAX_TUTORIAL_BUILD_PARALLELISM}"
        );
    }
    Ok(value)
}

fn build_preview(
    project_dir: &Path,
    workspace_root: &Path,
    preview: &PreviewBuild,
    preview_index: usize,
    preview_count: usize,
    worker: usize,
) -> Result<()> {
    let first = preview
        .outputs
        .first()
        .context("tutorial preview has no output paths")?;
    eprintln!(
        "Preview {}/{} on worker {}: {} / {}",
        preview_index + 1,
        preview_count,
        worker + 1,
        first.app,
        first.step
    );

    let worker_name = format!("worker-{worker:02}");
    let stage_dir = project_dir
        .join("target/tutorials")
        .join(&worker_name)
        .join("shared-preview");
    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir)
            .with_context(|| format!("failed to replace {}", stage_dir.display()))?;
    }
    fs::create_dir_all(&stage_dir)?;
    for (relative, contents) in &preview.files {
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
    let output = &first.path;
    if output.exists() {
        fs::remove_dir_all(output)
            .with_context(|| format!("failed to replace {}", output.display()))?;
    }
    fs::create_dir_all(output)?;
    let cargo_target = if worker == 0 {
        project_dir.join("target/tutorial-wasm")
    } else {
        project_dir.join(format!("target/tutorial-wasm-parallel-{worker:02}"))
    };
    compile_preview(
        &first.app,
        &first.step,
        package,
        &stage_dir,
        output,
        workspace_root,
        &cargo_target,
        preview.include_charts,
    )?;

    for step in &preview.outputs {
        if step.path != *output {
            if step.path.exists() {
                fs::remove_dir_all(&step.path)
                    .with_context(|| format!("failed to replace {}", step.path.display()))?;
            }
            copy_dir(output, &step.path)?;
        }
        fs::write(
            step.path.join("index.html"),
            tutorial_html(&step.goal_title, &step.step_title, &step.goal),
        )?;
        eprintln!("  Ready: {} / {}", step.app, step.step);
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
    let output_result = Command::new("wasm-pack")
        .arg("build")
        .arg(stage_dir)
        .args(["--target", "web", "--release", "--out-dir"])
        .arg(output.join("pkg"))
        .env("CARGO_TARGET_DIR", cargo_target)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to launch wasm-pack; install it before building goal tutorials")?;
    if !output_result.status.success() {
        let stdout = String::from_utf8_lossy(&output_result.stdout);
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        bail!(
            "tutorial `{app}` step `{step}` failed to compile ({}):\n{stdout}{stderr}",
            output_result.status
        );
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
