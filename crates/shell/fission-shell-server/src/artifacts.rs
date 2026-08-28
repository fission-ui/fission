use crate::{FissionServerApp, ProgressiveWorker, WasmIsland};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Kind of independently compiled browser artifact.
pub enum BrowserArtifactKind {
    /// Progressive Web Worker attached to a route.
    Worker,
    /// Hydrated WebAssembly island mounted into the document.
    Island,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Validated build plan for one generated browser artifact shim.
pub struct BrowserArtifactPlan {
    /// Stable worker or island identifier.
    pub id: String,
    /// Artifact execution kind.
    pub kind: BrowserArtifactKind,
    /// Rust entry path exported by the application package.
    pub entry: String,
    /// Public artifact path expected by the rendered document.
    pub artifact: String,
    /// Directory into which the generated shim crate is written.
    pub shim_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs controlling browser artifact generation and compilation.
pub struct BrowserArtifactBuildOptions {
    /// Application project root.
    pub project_dir: PathBuf,
    /// Root directory for generated shims and compiled assets.
    pub output_dir: PathBuf,
    /// Cargo package that exports worker and island entry functions.
    pub package_name: String,
    /// Whether that package's default features remain enabled.
    pub package_default_features: bool,
    /// Additional package features enabled for artifact builds.
    pub package_features: Vec<String>,
    /// Whether to compile with Cargo's release profile.
    pub release: bool,
    /// Whether `write_shims` also invokes compilation.
    pub compile: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Collection of browser artifacts discovered from an SSR application.
pub struct BrowserArtifactBuild {
    /// Validated worker and island plans in route declaration order.
    pub plans: Vec<BrowserArtifactPlan>,
}

impl BrowserArtifactBuild {
    /// Discovers and validates every worker and island registered by `app`.
    pub fn from_app(app: &FissionServerApp, options: &BrowserArtifactBuildOptions) -> Result<Self> {
        let mut plans = Vec::new();
        for route in app.routes() {
            for worker in route.workers {
                plans.push(worker_plan(&worker, options)?);
            }
            for island in route.islands {
                plans.push(island_plan(&island, options)?);
            }
        }
        Ok(Self { plans })
    }

    /// Writes generated shim crates and optionally compiles their Wasm artifacts.
    pub fn write_shims(&self, options: &BrowserArtifactBuildOptions) -> Result<()> {
        for plan in &self.plans {
            write_shim(plan, options)?;
            if options.compile {
                compile_shim(plan, options)?;
            }
        }
        Ok(())
    }
}

fn worker_plan(
    worker: &ProgressiveWorker,
    options: &BrowserArtifactBuildOptions,
) -> Result<BrowserArtifactPlan> {
    validate_artifact_id("worker", &worker.id)?;
    let entry = worker
        .entry
        .clone()
        .with_context(|| format!("worker `{}` is missing an entry path", worker.id))?;
    validate_entry_path("worker", &worker.id, &entry)?;
    validate_package_name(&options.package_name)?;
    Ok(BrowserArtifactPlan {
        id: worker.id.clone(),
        kind: BrowserArtifactKind::Worker,
        entry,
        artifact: worker.artifact.clone(),
        shim_dir: options
            .output_dir
            .join("generated/workers")
            .join(&worker.id),
    })
}

fn island_plan(
    island: &WasmIsland,
    options: &BrowserArtifactBuildOptions,
) -> Result<BrowserArtifactPlan> {
    validate_artifact_id("island", &island.id)?;
    let entry = island
        .entry
        .clone()
        .with_context(|| format!("island `{}` is missing an entry path", island.id))?;
    validate_entry_path("island", &island.id, &entry)?;
    validate_artifact_id("island mount", &island.mount_id)?;
    validate_package_name(&options.package_name)?;
    Ok(BrowserArtifactPlan {
        id: island.id.clone(),
        kind: BrowserArtifactKind::Island,
        entry,
        artifact: island.artifact.clone(),
        shim_dir: options
            .output_dir
            .join("generated/islands")
            .join(&island.id),
    })
}

fn write_shim(plan: &BrowserArtifactPlan, options: &BrowserArtifactBuildOptions) -> Result<()> {
    let src_dir = plan.shim_dir.join("src");
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("failed to create {}", src_dir.display()))?;
    fs::write(
        plan.shim_dir.join("Cargo.toml"),
        shim_manifest(plan, options),
    )
    .with_context(|| format!("failed to write shim manifest for {}", plan.id))?;
    fs::write(src_dir.join("lib.rs"), shim_source(plan))
        .with_context(|| format!("failed to write shim source for {}", plan.id))?;
    Ok(())
}

fn shim_manifest(plan: &BrowserArtifactPlan, options: &BrowserArtifactBuildOptions) -> String {
    let crate_name = shim_crate_name(plan);
    let dependency_name = options.package_name.replace('-', "_");
    let dependency = dependency_spec(&dependency_name, options, &plan.shim_dir);
    format!(
        r#"[package]
name = "{crate_name}"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
{dependency}

[workspace]
"#,
    )
}

fn dependency_spec(
    dependency_name: &str,
    options: &BrowserArtifactBuildOptions,
    shim_dir: &Path,
) -> String {
    let mut fields = Vec::new();
    if dependency_name != options.package_name {
        fields.push(format!(r#"package = "{}""#, options.package_name));
    }
    fields.push(format!(
        r#"path = "{}""#,
        path_for_manifest(&options.project_dir, shim_dir)
    ));
    if !options.package_default_features {
        fields.push("default-features = false".to_string());
    }
    if !options.package_features.is_empty() {
        let features = options
            .package_features
            .iter()
            .map(|feature| format!(r#""{feature}""#))
            .collect::<Vec<_>>()
            .join(", ");
        fields.push(format!("features = [{features}]"));
    }
    format!("{dependency_name} = {{ {} }}", fields.join(", "))
}

fn shim_source(plan: &BrowserArtifactPlan) -> String {
    let entry_export = match plan.kind {
        BrowserArtifactKind::Worker => "fission_worker_entry",
        BrowserArtifactKind::Island => "fission_island_entry",
    };
    let event_export = match plan.kind {
        BrowserArtifactKind::Worker => "fission_worker_event",
        BrowserArtifactKind::Island => "fission_island_event",
    };
    format!(
        r###"use std::cell::RefCell;

thread_local! {{
    static FISSION_BRIDGE_OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}}

#[no_mangle]
pub extern "C" fn fission_bridge_alloc(len: usize) -> *mut u8 {{
    let mut buffer = Vec::with_capacity(len);
    buffer.resize(len, 0);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}}

#[no_mangle]
pub unsafe extern "C" fn fission_bridge_dealloc(ptr: *mut u8, len: usize) {{
    if !ptr.is_null() {{
        drop(Vec::from_raw_parts(ptr, len, len));
    }}
}}

#[no_mangle]
pub extern "C" fn fission_bridge_output_ptr() -> *const u8 {{
    FISSION_BRIDGE_OUTPUT.with(|output| output.borrow().as_ptr())
}}

#[no_mangle]
pub extern "C" fn fission_bridge_output_len() -> usize {{
    FISSION_BRIDGE_OUTPUT.with(|output| output.borrow().len())
}}

#[no_mangle]
pub unsafe extern "C" fn {entry_export}(ptr: *const u8, len: usize) -> i32 {{
    fission_call_entry(ptr, len)
}}

#[no_mangle]
pub unsafe extern "C" fn {event_export}(ptr: *const u8, len: usize) -> i32 {{
    fission_call_entry(ptr, len)
}}

unsafe fn fission_call_entry(ptr: *const u8, len: usize) -> i32 {{
    let input = if len == 0 {{
        ""
    }} else if ptr.is_null() {{
        fission_store_output(r#"{{"messages":[{{"type":"error","message":"null bridge input pointer","stack":null}}],"bindings":[]}}"#);
        return 1;
    }} else {{
        match std::str::from_utf8(std::slice::from_raw_parts(ptr, len)) {{
            Ok(input) => input,
            Err(_) => {{
                fission_store_output(r#"{{"messages":[{{"type":"error","message":"bridge input is not valid UTF-8","stack":null}}],"bindings":[]}}"#);
                return 1;
            }}
        }}
    }};
    let output = {entry}(input);
    fission_store_output(output);
    0
}}

fn fission_store_output(output: impl AsRef<str>) {{
    FISSION_BRIDGE_OUTPUT.with(|buffer| {{
        let mut buffer = buffer.borrow_mut();
        buffer.clear();
        buffer.extend_from_slice(output.as_ref().as_bytes());
    }});
}}
"###,
        entry = plan.entry
    )
}

fn compile_shim(plan: &BrowserArtifactPlan, options: &BrowserArtifactBuildOptions) -> Result<()> {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(plan.shim_dir.join("Cargo.toml"))
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--message-format=json-render-diagnostics")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if options.release {
        command.arg("--release");
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to compile browser artifact `{}`", plan.id))?;
    let stdout = child
        .stdout
        .take()
        .context("cargo did not expose browser artifact build output")?;
    let expected_crate = shim_crate_name(plan).replace('-', "_");
    let mut compiled_wasm = None;
    for line in BufReader::new(stdout).lines() {
        let line = line.context("failed to read browser artifact build output")?;
        if let Some(path) = compiled_wasm_from_cargo_message(&line, &expected_crate) {
            compiled_wasm = Some(path);
        }
        if let Some(rendered) = rendered_cargo_diagnostic(&line) {
            eprint!("{rendered}");
        }
    }
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for browser artifact `{}`", plan.id))?;
    if !status.success() {
        bail!("browser artifact `{}` failed with {status}", plan.id);
    }
    let compiled_wasm = compiled_wasm.with_context(|| {
        format!(
            "cargo did not report a wasm artifact for browser artifact `{}`",
            plan.id
        )
    })?;
    copy_compiled_artifact(plan, options, &compiled_wasm)?;
    Ok(())
}

fn copy_compiled_artifact(
    plan: &BrowserArtifactPlan,
    options: &BrowserArtifactBuildOptions,
    wasm: &Path,
) -> Result<()> {
    let relative = safe_artifact_output_path(&plan.artifact)
        .with_context(|| format!("browser artifact path `{}` is invalid", plan.artifact))?;
    let output = options.output_dir.join(relative);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(&wasm, &output).with_context(|| {
        format!(
            "failed to copy browser artifact {} to {}",
            wasm.display(),
            output.display()
        )
    })?;
    Ok(())
}

fn compiled_wasm_from_cargo_message(line: &str, expected_crate: &str) -> Option<PathBuf> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message.get("reason")?.as_str()? != "compiler-artifact"
        || message.get("target")?.get("name")?.as_str()? != expected_crate
    {
        return None;
    }
    message
        .get("filenames")?
        .as_array()?
        .iter()
        .filter_map(|path| path.as_str())
        .map(PathBuf::from)
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wasm")
        })
}

fn rendered_cargo_diagnostic(line: &str) -> Option<String> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message.get("reason")?.as_str()? != "compiler-message" {
        return None;
    }
    message
        .get("message")?
        .get("rendered")?
        .as_str()
        .map(str::to_owned)
}

fn shim_crate_name(plan: &BrowserArtifactPlan) -> String {
    format!(
        "fission_{}_{}",
        kind_name(plan.kind),
        sanitize_ident(&plan.id)
    )
}

fn safe_artifact_output_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() || trimmed.contains('\\') {
        return None;
    }
    let path = Path::new(trimmed);
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(segment) => out.push(segment),
            _ => return None,
        }
    }
    Some(out)
}

fn kind_name(kind: BrowserArtifactKind) -> &'static str {
    match kind {
        BrowserArtifactKind::Worker => "worker",
        BrowserArtifactKind::Island => "island",
    }
}

fn sanitize_ident(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn path_for_manifest(project_dir: &Path, shim_dir: &Path) -> String {
    let _ = shim_dir;
    project_dir.to_string_lossy().replace('\\', "/")
}

fn validate_artifact_id(kind: &str, id: &str) -> Result<()> {
    if id.is_empty() {
        bail!("{kind} id must not be empty");
    }
    if id == "." || id == ".." || id.contains('/') || id.contains('\\') {
        bail!("{kind} id `{id}` must be a plain path segment");
    }
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("{kind} id `{id}` may only contain ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}

fn validate_package_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("package name must not be empty");
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("package name `{name}` is not safe for generated Cargo manifests");
    }
    Ok(())
}

fn validate_entry_path(kind: &str, id: &str, entry: &str) -> Result<()> {
    let mut segments = entry.split("::").peekable();
    if segments.peek().is_none() {
        bail!("{kind} `{id}` entry path must not be empty");
    }
    for segment in segments {
        if !is_rust_ident(segment) {
            bail!("{kind} `{id}` entry path `{entry}` must be a Rust item path");
        }
    }
    Ok(())
}

fn is_rust_ident(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FissionServerApp, ProgressiveWorker, ServerRenderPolicy, WasmIsland, WebRouteMode,
    };
    use fission_core::ui::Text;
    use fission_core::{GlobalState, Widget};

    #[derive(Debug, Default)]
    struct State;
    impl GlobalState for State {}

    #[derive(Clone)]
    struct Page;
    impl From<Page> for Widget {
        fn from(_component: Page) -> Self {
            let (_ctx, _view) = fission_core::build::current::<State>();
            Text::new("artifact page").into()
        }
    }
    #[test]
    fn writes_one_shim_per_worker_and_island() {
        let root = std::env::temp_dir().join(format!("fission-artifacts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let app = FissionServerApp::new("Artifacts")
            .route_widget::<State, _>(
                "/",
                "Home",
                None,
                WebRouteMode::Server(ServerRenderPolicy::default()),
                Page,
            )
            .worker(
                "/",
                ProgressiveWorker::new("filters", "/filters.wasm").entry("demo::filters::boot"),
            )
            .island(
                "/",
                WasmIsland::new("cart", "/cart.wasm", "cart-root").entry("demo::cart::boot"),
            );
        let options = BrowserArtifactBuildOptions {
            project_dir: root.clone(),
            output_dir: root.join("target/fission/server"),
            package_name: "demo".into(),
            package_default_features: true,
            package_features: Vec::new(),
            release: false,
            compile: false,
        };
        let build = BrowserArtifactBuild::from_app(&app, &options).unwrap();
        build.write_shims(&options).unwrap();
        assert!(options
            .output_dir
            .join("generated/workers/filters/src/lib.rs")
            .exists());
        assert!(options
            .output_dir
            .join("generated/islands/cart/src/lib.rs")
            .exists());
        let worker = fs::read_to_string(
            options
                .output_dir
                .join("generated/workers/filters/src/lib.rs"),
        )
        .unwrap();
        assert!(worker.contains("demo::filters::boot"));
        assert!(worker.contains("fission_bridge_alloc"));
        assert!(worker.contains("fission_worker_entry"));
        assert!(worker.contains("fission_worker_event"));
        assert!(worker.contains("fission_call_entry(ptr, len)"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_path_traversal_ids_and_source_injection_entries() {
        let root =
            std::env::temp_dir().join(format!("fission-artifacts-invalid-{}", std::process::id()));
        let app = FissionServerApp::new("Artifacts")
            .route_widget::<State, _>(
                "/",
                "Home",
                None,
                WebRouteMode::Server(ServerRenderPolicy::default()),
                Page,
            )
            .worker(
                "/",
                ProgressiveWorker::new("../escape", "/escape.wasm").entry("demo::filters::boot"),
            );
        let options = BrowserArtifactBuildOptions {
            project_dir: root.clone(),
            output_dir: root.join("target/fission/server"),
            package_name: "demo".into(),
            package_default_features: true,
            package_features: Vec::new(),
            release: false,
            compile: false,
        };
        assert!(BrowserArtifactBuild::from_app(&app, &options).is_err());

        let app = FissionServerApp::new("Artifacts")
            .route_widget::<State, _>(
                "/",
                "Home",
                None,
                WebRouteMode::Server(ServerRenderPolicy::default()),
                Page,
            )
            .worker(
                "/",
                ProgressiveWorker::new("filters", "/filters.wasm")
                    .entry("demo::filters::boot(); std::process::exit(1)"),
            );
        assert!(BrowserArtifactBuild::from_app(&app, &options).is_err());
    }

    #[test]
    fn finds_compiled_wasm_in_cargo_artifact_messages_without_assuming_target_dir() {
        let line = serde_json::json!({
            "reason": "compiler-artifact",
            "target": {
                "name": "fission_worker_filters",
            },
            "filenames": [
                "/custom/shared/target/wasm32-unknown-unknown/debug/fission_worker_filters.wasm",
            ],
        })
        .to_string();

        assert_eq!(
            compiled_wasm_from_cargo_message(&line, "fission_worker_filters"),
            Some(PathBuf::from(
                "/custom/shared/target/wasm32-unknown-unknown/debug/fission_worker_filters.wasm"
            ))
        );
        assert_eq!(
            compiled_wasm_from_cargo_message(&line, "fission_island_filters"),
            None
        );
    }
}
