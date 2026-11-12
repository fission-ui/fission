use crate::content::load_goals;
use anyhow::{bail, Context, Result};
use std::collections::{hash_map::DefaultHasher, BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;

const TUTORIAL_BUILD_PARALLELISM_ENV: &str = "FISSION_TUTORIAL_BUILD_PARALLELISM";
const WEB_WASM_SIMD_ENV: &str = "FISSION_WEB_WASM_SIMD";
const WEB_WASM_TARGET_RUSTFLAGS: &str = "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS";
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

struct PreparedPreview {
    package: String,
    stage_dir: std::path::PathBuf,
    cache_marker: std::path::PathBuf,
    cache_key: String,
    cache_context: String,
    outputs: Vec<PreviewOutput>,
}

struct PreviewWorkspace {
    manifest_path: std::path::PathBuf,
    preview_count: usize,
    include_charts: bool,
    preview_indices: Vec<usize>,
}

pub fn build_all(project_dir: &Path) -> Result<()> {
    let content_dir = project_dir.join("content/goals");
    let documents = load_goals(&content_dir)?;
    let parallelism = tutorial_build_parallelism()?;
    let (wasm_simd, wasm_target_rustflags) = tutorial_web_wasm_simd()?;
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
    let workspace_root = project_dir
        .parent()
        .context("docs2 must live below the Fission workspace")?;
    let cache_context = preview_cache_context(
        workspace_root,
        project_dir,
        wasm_simd,
        wasm_target_rustflags.as_deref(),
    )?;

    let build_cache_root = tutorial_build_cache_root(project_dir);
    let (previews, workspaces) =
        prepare_preview_workspaces(workspace_root, &build_cache_root, previews, &cache_context)?;
    let cargo_target = build_cache_root.join("tutorial-wasm");
    compile_preview_workspaces(
        &workspaces,
        &previews,
        &cargo_target,
        parallelism,
        wasm_target_rustflags.as_deref(),
    )?;
    let workers = parallelism.min(previews.len());
    eprintln!(
        "Packaging {} unique Web preview(s) with {workers} worker(s); set {TUTORIAL_BUILD_PARALLELISM_ENV} to change concurrency.",
        previews.len(),
    );
    package_previews(
        &previews,
        &cargo_target,
        workers,
        wasm_target_rustflags.as_deref(),
    )
}

fn prepare_preview_workspaces(
    workspace_root: &Path,
    build_cache_root: &Path,
    mut previews: Vec<PreviewBuild>,
    cache_context: &str,
) -> Result<(Vec<PreparedPreview>, Vec<PreviewWorkspace>)> {
    let mut prepared = (0..previews.len())
        .map(|_| None)
        .collect::<Vec<Option<PreparedPreview>>>();
    let mut workspaces = Vec::new();

    for include_charts in [false, true] {
        let indices = previews
            .iter()
            .enumerate()
            .filter_map(|(index, preview)| {
                (preview.include_charts == include_charts).then_some(index)
            })
            .collect::<Vec<_>>();
        if indices.is_empty() {
            continue;
        }

        let workspace_dir = build_cache_root.join("tutorials").join(if include_charts {
            "preview-workspace-charts"
        } else {
            "preview-workspace-web"
        });
        fs::create_dir_all(&workspace_dir)?;
        let members = indices
            .iter()
            .map(|index| format!("\"preview-{index:03}\""))
            .collect::<Vec<_>>()
            .join(",\n    ");
        let manifest_path = workspace_dir.join("Cargo.toml");
        write_if_changed(
            &manifest_path,
            format!("[workspace]\nmembers = [\n    {members}\n]\nresolver = \"2\"\n").as_bytes(),
        )?;

        for index in &indices {
            let preview = &mut previews[*index];
            let member_name = format!("preview-{index:03}");
            let package = format!("docs2-tutorial-preview-{index:03}");
            let stage_dir = workspace_dir.join(&member_name);
            // Preview indices are reused as the tutorial set changes. Clear
            // the generated source directory before staging this preview so
            // files removed from a tutorial (for example, an old build.rs)
            // cannot leak into a different preview at the same index.
            if stage_dir.exists() {
                fs::remove_dir_all(&stage_dir)?;
            }
            fs::create_dir_all(&stage_dir)?;
            for (relative, contents) in &preview.files {
                let stage_path = preview_stage_path(relative);
                let destination = stage_dir.join(stage_path);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                write_if_changed(&destination, contents.as_bytes())?;
            }
            let sources = preview
                .files
                .values()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let fission_features = tutorial_fission_features(&sources, include_charts);
            write_if_changed(
                &stage_dir.join("Cargo.toml"),
                tutorial_manifest(
                    &package,
                    workspace_root,
                    &fission_features,
                    preview.files.contains_key("preview/lib.rs"),
                    preview
                        .files
                        .values()
                        .any(|source| source.contains("serde")),
                    preview
                        .files
                        .values()
                        .any(|source| source.contains("serde_yaml")),
                    preview
                        .files
                        .values()
                        .any(|source| source.contains("fission_design_system_codegen")),
                )
                .as_bytes(),
            )?;
            let cache_key = preview_cache_key(&stage_dir, cache_context, &package)?;
            let cache_marker = build_cache_root
                .join("tutorial-preview-cache")
                .join(format!("{cache_key}.fingerprint"));
            prepared[*index] = Some(PreparedPreview {
                package,
                stage_dir,
                cache_marker,
                cache_key,
                cache_context: cache_context.to_owned(),
                outputs: std::mem::take(&mut preview.outputs),
            });
        }
        workspaces.push(PreviewWorkspace {
            manifest_path,
            preview_count: indices.len(),
            include_charts,
            preview_indices: indices,
        });
    }

    let prepared = prepared
        .into_iter()
        .enumerate()
        .map(|(index, preview)| {
            preview.with_context(|| format!("preview {index} was not assigned to a workspace"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((prepared, workspaces))
}

fn tutorial_build_cache_root(project_dir: &Path) -> std::path::PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| project_dir.join("target"))
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

fn compile_preview_workspaces(
    workspaces: &[PreviewWorkspace],
    previews: &[PreparedPreview],
    cargo_target: &Path,
    parallelism: usize,
    wasm_target_rustflags: Option<&str>,
) -> Result<()> {
    for workspace in workspaces {
        let uncached = workspace
            .preview_indices
            .iter()
            .filter_map(|index| previews.get(*index).map(|preview| (*index, preview)))
            .filter_map(
                |(index, preview)| match preview_package_is_cached(preview) {
                    Ok(true) => None,
                    Ok(false) => Some(Ok((index, preview.package.as_str()))),
                    Err(error) => Some(Err(error)),
                },
            )
            .collect::<Result<Vec<_>>>()?;
        if uncached.is_empty() {
            eprintln!(
                "Reusing all {} compiled tutorial app(s) {}.",
                workspace.preview_count,
                if workspace.include_charts {
                    "with charts"
                } else {
                    "without charts"
                }
            );
            continue;
        }
        let feature_label = if workspace.include_charts {
            "with charts"
        } else {
            "without charts"
        };
        eprintln!(
            "Compiling {} uncached tutorial app(s) {feature_label} in parallel (up to {parallelism} Cargo jobs)...",
            uncached.len(),
        );
        let mut command = Command::new("cargo");
        command
            .arg("build")
            .arg("--manifest-path")
            .arg(&workspace.manifest_path)
            .args(["--target", "wasm32-unknown-unknown", "--release", "--jobs"])
            .arg(parallelism.to_string());
        for (_, package) in &uncached {
            command.arg("--package").arg(package);
        }
        let mut command = command.env("CARGO_TARGET_DIR", cargo_target);
        if let Some(flags) = wasm_target_rustflags {
            command = command.env(WEB_WASM_TARGET_RUSTFLAGS, flags);
        }
        let status = command
            .status()
            .context("failed to launch Cargo for tutorial preview apps")?;
        if !status.success() {
            bail!(
                "failed to compile {} tutorial preview app(s) {feature_label} ({status})",
                uncached.len()
            );
        }
        eprintln!(
            "Compiled {} tutorial app(s) {feature_label}.",
            uncached.len()
        );
    }
    Ok(())
}

fn preview_package_is_cached(preview: &PreparedPreview) -> Result<bool> {
    let Some(first) = preview.outputs.first() else {
        return Ok(false);
    };
    let Some(module) = preview_bootstrap_module(&first.path)? else {
        return Ok(false);
    };
    if !preview_package_exists(&first.path, &module) {
        return Ok(false);
    }

    if fs::read_to_string(&preview.cache_marker)
        .is_ok_and(|cached_key| cached_key == preview.cache_key)
    {
        return Ok(true);
    }

    let legacy_package = module.replace('_', "-");
    let legacy_marker = preview
        .cache_marker
        .parent()
        .context("tutorial preview cache marker has no parent")?
        .join(format!("{legacy_package}.fingerprint"));
    let legacy_key = legacy_preview_cache_key(
        &preview.stage_dir,
        &preview.cache_context,
        &preview.package,
        &legacy_package,
    )?;
    if fs::read_to_string(&legacy_marker).is_ok_and(|cached_key| cached_key == legacy_key) {
        fs::write(&preview.cache_marker, &preview.cache_key)?;
        eprintln!(
            "Migrated cached preview package {} to a source-stable cache key.",
            legacy_package
        );
        return Ok(true);
    }
    Ok(false)
}

fn package_previews(
    previews: &[PreparedPreview],
    cargo_target: &Path,
    workers: usize,
    wasm_target_rustflags: Option<&str>,
) -> Result<()> {
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
                        cargo_target,
                        preview,
                        preview_index,
                        previews.len(),
                        worker,
                        wasm_target_rustflags,
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

fn tutorial_web_wasm_simd() -> Result<(bool, Option<String>)> {
    let enabled = match std::env::var(WEB_WASM_SIMD_ENV) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" | "simd" | "simd128" => true,
            "0" | "false" | "no" | "off" | "scalar" => false,
            _ => bail!("{WEB_WASM_SIMD_ENV} must be `simd` (the default) or `scalar`"),
        },
        Err(std::env::VarError::NotPresent) => true,
        Err(error) => return Err(error).context("failed to read Web SIMD setting"),
    };

    let global_rustflags = if let Some(value) = std::env::var_os("CARGO_ENCODED_RUSTFLAGS") {
        Some(("CARGO_ENCODED_RUSTFLAGS", value))
    } else {
        std::env::var_os("RUSTFLAGS").map(|value| ("RUSTFLAGS", value))
    };
    if let Some((variable, value)) = global_rustflags {
        let value = value
            .into_string()
            .map_err(|_| anyhow::anyhow!("{variable} must contain UTF-8 Rust compiler flags"))?;
        let flags = if variable == "CARGO_ENCODED_RUSTFLAGS" {
            value.split('\u{1f}').map(str::to_owned).collect::<Vec<_>>()
        } else {
            shell_words::split(&value).with_context(|| format!("failed to parse {variable}"))?
        };
        match tutorial_target_feature_state(&flags, "simd128") {
            Some(true) if enabled => return Ok((enabled, None)),
            Some(false) if !enabled => return Ok((enabled, None)),
            Some(_) => bail!(
                "{WEB_WASM_SIMD_ENV} conflicts with {variable}; update its `target-feature` flags or remove it"
            ),
            None if enabled => bail!(
                "{variable} overrides Cargo's target-specific Web flags; add `-C target-feature=+simd128` there or set {WEB_WASM_SIMD_ENV}=scalar"
            ),
            None => return Ok((enabled, None)),
        }
    }

    let existing = std::env::var(WEB_WASM_TARGET_RUSTFLAGS).ok();
    let flags = tutorial_update_simd_flags(existing.as_deref().unwrap_or_default(), enabled)?;
    if !flags.is_empty() || existing.is_some() {
        Ok((enabled, Some(flags)))
    } else {
        Ok((enabled, None))
    }
}

fn tutorial_target_feature_state(flags: &[String], feature: &str) -> Option<bool> {
    let mut last_features = None;
    let mut index = 0;
    while index < flags.len() {
        let value = if flags[index] == "-C" {
            index += 1;
            flags
                .get(index)
                .and_then(|argument| argument.strip_prefix("target-feature="))
        } else {
            flags[index].strip_prefix("-Ctarget-feature=")
        };
        if let Some(value) = value {
            last_features = Some(value.to_owned());
        }
        index += 1;
    }
    last_features.and_then(|features| {
        features
            .split(',')
            .filter_map(|entry| {
                let (enabled, name) = match entry.as_bytes().first().copied()? {
                    b'+' => (true, &entry[1..]),
                    b'-' => (false, &entry[1..]),
                    _ => return None,
                };
                (name == feature).then_some(enabled)
            })
            .last()
    })
}

fn tutorial_update_simd_flags(flags: &str, enabled: bool) -> Result<String> {
    let arguments = shell_words::split(flags).context("failed to parse Web target Rust flags")?;
    let mut retained = Vec::with_capacity(arguments.len() + 2);
    let mut last_features = None;
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "-C" {
            if let Some(value) = arguments.get(index + 1) {
                if let Some(features) = value.strip_prefix("target-feature=") {
                    last_features = Some(features.to_owned());
                    index += 2;
                    continue;
                }
            }
        } else if let Some(features) = arguments[index].strip_prefix("-Ctarget-feature=") {
            last_features = Some(features.to_owned());
            index += 1;
            continue;
        }
        retained.push(arguments[index].clone());
        index += 1;
    }

    let mut features = last_features
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .filter(|entry| {
            !matches!(
                entry.strip_prefix('+').or_else(|| entry.strip_prefix('-')),
                Some("simd128")
            )
        })
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if enabled {
        features.push("+simd128".into());
    }
    if !features.is_empty() {
        retained.push("-C".into());
        retained.push(format!("target-feature={}", features.join(",")));
    }
    Ok(retained.join(" "))
}

fn build_preview(
    cargo_target: &Path,
    preview: &PreparedPreview,
    preview_index: usize,
    preview_count: usize,
    worker: usize,
    wasm_target_rustflags: Option<&str>,
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
    let output = &first.path;
    let package_is_current = preview_package_is_cached(preview)?;
    if !package_is_current && output.exists() {
        fs::remove_dir_all(output)
            .with_context(|| format!("failed to replace {}", output.display()))?;
    }
    if package_is_current {
        eprintln!(
            "  Reusing current WebAssembly package for {} / {}",
            first.app, first.step
        );
    } else {
        fs::create_dir_all(output)?;
        compile_preview(
            &first.app,
            &first.step,
            &preview.package,
            &preview.stage_dir,
            output,
            cargo_target,
            wasm_target_rustflags,
        )?;
        if let Some(parent) = preview.cache_marker.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&preview.cache_marker, &preview.cache_key)?;
    }

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

fn preview_package_exists(output: &Path, module: &str) -> bool {
    let package = output.join("pkg");
    let packaged_wasm = package.join(format!("{module}_bg.wasm"));
    let packaged_js = package.join(format!("{module}.js"));
    let bootstrap = output.join("bootstrap.mjs");
    packaged_wasm.is_file() && packaged_js.is_file() && bootstrap.is_file()
}

fn preview_bootstrap_module(output: &Path) -> Result<Option<String>> {
    let bootstrap_path = output.join("bootstrap.mjs");
    let bootstrap = match fs::read_to_string(&bootstrap_path) {
        Ok(bootstrap) => bootstrap,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {}", bootstrap_path.display()))
        }
    };
    let Some((_, module_path)) = bootstrap.split_once("from './pkg/") else {
        return Ok(None);
    };
    let Some((module, _)) = module_path.split_once(".js'") else {
        return Ok(None);
    };
    if module.is_empty()
        || !module
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Ok(None);
    }
    Ok(Some(module.to_owned()))
}

fn preview_cache_context(
    workspace_root: &Path,
    project_dir: &Path,
    wasm_simd: bool,
    wasm_target_rustflags: Option<&str>,
) -> Result<String> {
    let mut hasher = PreviewCacheHasher::default();
    hasher.field(b"fission-tutorial-preview-cache-v2");
    hasher.field(if wasm_simd {
        b"wasm-simd128"
    } else {
        b"wasm-scalar"
    });
    if let Some(flags) = wasm_target_rustflags {
        hasher.field(flags.as_bytes());
    }
    for relative in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo/config.toml",
    ] {
        let path = workspace_root.join(relative);
        if path.is_file() {
            hasher.field(relative.as_bytes());
            hasher.field(&fs::read(&path)?);
        }
    }
    let docs_manifest = project_dir.join("Cargo.toml");
    hasher.field(b"docs2/Cargo.toml");
    hasher.field(&fs::read(docs_manifest)?);

    for (program, args) in [
        ("rustc", vec!["--version"]),
        ("cargo", vec!["--version"]),
        ("wasm-pack", vec!["--version"]),
    ] {
        let output = Command::new(program)
            .args(&args)
            .output()
            .with_context(|| format!("failed to query {program} version for tutorial cache"))?;
        if !output.status.success() {
            bail!("failed to query {program} version for tutorial cache");
        }
        hasher.field(program.as_bytes());
        hasher.field(&output.stdout);
        hasher.field(&output.stderr);
    }

    let dependency_dirs = [
        "crates/authoring/fission",
        "crates/authoring/fission-compose",
        "crates/authoring/fission-compose-macros",
        "crates/authoring/fission-widgets",
        "crates/authoring/fission-macros",
        "crates/authoring/fission-icons",
        "crates/authoring/fission-charts",
        "crates/core/fission-core",
        "crates/core/fission-ir",
        "crates/core/fission-layout",
        "crates/core/fission-text-engine",
        "crates/core/fission-theme",
        "crates/core/fission-i18n",
        "crates/rendering/fission-render",
        "crates/shell/fission-shell-web",
        "crates/tools/fission-diagnostics",
        "crates/tools/fission-design-system-codegen",
    ];
    for relative in dependency_dirs {
        let path = workspace_root.join(relative);
        if path.is_dir() {
            hasher.field(relative.as_bytes());
            hash_directory(&mut hasher, workspace_root, &path)?;
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        let wasm_pack_cache = Path::new(&home).join(".cache/.wasm-pack");
        if let Ok(entries) = fs::read_dir(wasm_pack_cache) {
            let mut versions = entries
                .filter_map(std::result::Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name();
                    name.to_str()
                        .filter(|name| name.starts_with("wasm-opt-"))
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>();
            versions.sort();
            for version in versions {
                hasher.field(version.as_bytes());
            }
        }
    }

    Ok(format!("{:016x}", hasher.finish()))
}

fn preview_cache_key(stage_dir: &Path, context: &str, package: &str) -> Result<String> {
    let mut hasher = PreviewCacheHasher::default();
    hasher.field(context.as_bytes());
    hash_preview_directory(
        &mut hasher,
        stage_dir,
        stage_dir,
        package,
        "tutorial-preview",
    )?;
    Ok(format!("{:016x}", hasher.finish()))
}

fn legacy_preview_cache_key(
    stage_dir: &Path,
    context: &str,
    current_package: &str,
    legacy_package: &str,
) -> Result<String> {
    let mut hasher = PreviewCacheHasher::default();
    hasher.field(context.as_bytes());
    hash_preview_directory(
        &mut hasher,
        stage_dir,
        stage_dir,
        current_package,
        legacy_package,
    )?;
    Ok(format!("{:016x}", hasher.finish()))
}

fn hash_preview_directory(
    hasher: &mut PreviewCacheHasher,
    base: &Path,
    directory: &Path,
    package_name: &str,
    replacement: &str,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            hash_preview_directory(hasher, base, &path, package_name, replacement)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(base)
                .context("tutorial cache input escaped its source directory")?
                .to_string_lossy()
                .replace('\\', "/");
            hasher.field(relative.as_bytes());
            let contents = fs::read(&path)?;
            if relative == "Cargo.toml" {
                let contents = String::from_utf8(contents)
                    .context("tutorial preview Cargo.toml was not UTF-8")?
                    .replace(package_name, replacement);
                hasher.field(contents.as_bytes());
            } else {
                hasher.field(&contents);
            }
        }
    }
    Ok(())
}

fn hash_directory(hasher: &mut PreviewCacheHasher, base: &Path, directory: &Path) -> Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            hash_directory(hasher, base, &path)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(base)
                .context("tutorial cache input escaped its source directory")?
                .to_string_lossy()
                .replace('\\', "/");
            hasher.field(relative.as_bytes());
            hasher.field(&fs::read(path)?);
        }
    }
    Ok(())
}

struct PreviewCacheHasher(u64);

impl Default for PreviewCacheHasher {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl PreviewCacheHasher {
    fn field(&mut self, bytes: &[u8]) {
        for byte in bytes.iter().copied().chain([0xff]) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

fn compile_preview(
    app: &str,
    step: &str,
    package: &str,
    stage_dir: &Path,
    output: &Path,
    cargo_target: &Path,
    wasm_target_rustflags: Option<&str>,
) -> Result<()> {
    let mut command = Command::new("wasm-pack");
    command
        .arg("build")
        .arg(stage_dir)
        .args(["--target", "web", "--release", "--out-dir"])
        .arg(output.join("pkg"))
        .env("CARGO_TARGET_DIR", cargo_target)
        .env("CARGO_BUILD_JOBS", "1");
    if let Some(flags) = wasm_target_rustflags {
        command.env(WEB_WASM_TARGET_RUSTFLAGS, flags);
    }
    let status = command
        .status()
        .context("failed to launch wasm-pack; install it before building goal tutorials")?;
    if !status.success() {
        bail!("tutorial `{app}` step `{step}` failed to compile ({status})");
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

fn preview_stage_path(relative: &str) -> &Path {
    if relative == "preview/lib.rs" {
        // Keep the runnable preview entry point separate from a tutorial's
        // real `src/lib.rs`, which may configure native shells and therefore
        // cannot also be the WASM entry point.
        Path::new("src/preview.rs")
    } else {
        Path::new(relative)
    }
}

fn tutorial_manifest(
    package: &str,
    workspace_root: &Path,
    fission_features: &[&str],
    has_preview_entrypoint: bool,
    include_serde: bool,
    include_serde_yaml: bool,
    include_design_system_codegen: bool,
) -> String {
    let fission = workspace_root.join("crates/authoring/fission");
    let design_system_codegen = workspace_root.join("crates/tools/fission-design-system-codegen");
    let fission_features = fission_features
        .iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let serde_dependency = if include_serde {
        "serde = { version = \"1\", features = [\"derive\"] }\n"
    } else {
        ""
    };
    let serde_yaml_dependency = if include_serde_yaml {
        "serde_yaml = \"0.9\"\n"
    } else {
        ""
    };
    let design_system_codegen_dependency = if include_design_system_codegen {
        format!(
            "\n[build-dependencies]\nfission-design-system-codegen = {{ path = \"{}\" }}\n",
            design_system_codegen.display()
        )
    } else {
        String::new()
    };
    let lib_path = if has_preview_entrypoint {
        "src/preview.rs"
    } else {
        "src/lib.rs"
    };
    format!(
        r#"[package]
name = "{package}"
version = "0.0.0"
edition = "2021"

[lib]
path = "{lib_path}"
crate-type = ["cdylib", "rlib"]

[dependencies]
anyhow = "1"
console_error_panic_hook = "0.1"
fission = {{ path = "{}", default-features = false, features = [{fission_features}] }}
{serde_dependency}{serde_yaml_dependency}wasm-bindgen = "0.2"
{design_system_codegen_dependency}"#,
        fission.display()
    )
}

fn tutorial_fission_features(sources: &[&str], include_charts: bool) -> Vec<&'static str> {
    let uses_any = |markers: &[&str]| {
        sources
            .iter()
            .any(|source| markers.iter().any(|marker| source.contains(marker)))
    };

    let mut features = vec!["web"];
    if include_charts {
        features.push("charts");
    }
    if uses_any(&[
        ".file_system()",
        "FileSystemLocation",
        "PickDirectoryRequest",
    ]) {
        features.push("filesystem");
    }
    if uses_any(&[".store()", "StoreKey<", "StoreBatch"]) {
        features.push("store");
    }
    if uses_any(&[".sql()", "SqlStatement", "SqlMigration", "SqlTransaction"]) {
        features.push("store-sql");
    }
    if uses_any(&["Video::", "Video("]) {
        features.push("video");
    }

    features
}

fn tutorial_html(goal: &str, step: &str, task: &str) -> String {
    let goal = html_escape(goal);
    let step = html_escape(step);
    let task = html_escape(task);
    format!(
        r#"<!doctype html>
<html lang="en-GB"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{step} — {goal}</title><meta name="tutorial-goal" content="{task}"><style>html,body,#fission-web-mount{{width:100%;height:100%;margin:0;overflow:hidden}}body{{background:#f7f6f1}}</style></head>
<body><main id="fission-web-mount" aria-label="App preview for {step}"></main><script>
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fission-tutorial-cache-{label}-{}-{nonce}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn preview_cache_key_does_not_depend_on_workspace_package_number() {
        let stage_dir = temporary_directory("stable-key");
        fs::create_dir_all(stage_dir.join("src")).unwrap();
        fs::write(
            stage_dir.join("Cargo.toml"),
            "[package]\nname = \"docs2-tutorial-preview-007\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
        fs::write(
            stage_dir.join("src/lib.rs"),
            "pub fn answer() -> u8 { 42 }\n",
        )
        .unwrap();
        let first = preview_cache_key(&stage_dir, "context", "docs2-tutorial-preview-007").unwrap();
        fs::write(
            stage_dir.join("Cargo.toml"),
            "[package]\nname = \"docs2-tutorial-preview-108\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
        let shifted =
            preview_cache_key(&stage_dir, "context", "docs2-tutorial-preview-108").unwrap();

        assert_eq!(first, shifted);
        fs::remove_dir_all(stage_dir).unwrap();
    }

    #[test]
    fn preview_entrypoint_does_not_replace_the_tutorial_library() {
        assert_eq!(
            preview_stage_path("preview/lib.rs"),
            Path::new("src/preview.rs")
        );
        assert_eq!(preview_stage_path("src/lib.rs"), Path::new("src/lib.rs"));
    }

    #[test]
    fn tutorial_manifest_enables_only_requested_build_dependencies() {
        let workspace = Path::new("/fission-workspace");
        let features =
            tutorial_fission_features(&["ctx.effects.sql().query(SqlStatement::new(...))"], false);
        let with_codegen =
            tutorial_manifest("tutorial", workspace, &features, true, false, false, true);
        let without_codegen =
            tutorial_manifest("tutorial", workspace, &features, true, false, false, false);
        let source_entrypoint =
            tutorial_manifest("tutorial", workspace, &features, false, false, false, false);

        assert!(with_codegen.contains("path = \"src/preview.rs\""));
        assert!(with_codegen.contains("features = [\"web\", \"store-sql\"]"));
        assert!(with_codegen.contains("[build-dependencies]"));
        assert!(with_codegen.contains("fission-design-system-codegen"));
        assert!(without_codegen.contains("path = \"src/preview.rs\""));
        assert!(!without_codegen.contains("[build-dependencies]"));
        assert!(!without_codegen.contains("fission-design-system-codegen"));
        assert!(source_entrypoint.contains("path = \"src/lib.rs\""));
    }

    #[test]
    fn tutorial_manifest_enables_capability_features_used_by_the_preview() {
        let features = tutorial_fission_features(
            &[
                "ctx.effects.file_system().pick_directory(PickDirectoryRequest { .. })",
                "Video::new(source)",
            ],
            true,
        );

        assert_eq!(features, vec!["web", "charts", "filesystem", "video"]);
    }

    #[test]
    fn cached_numeric_preview_package_migrates_to_the_stable_key() {
        let root = temporary_directory("migration");
        let stage_dir = root.join("stage");
        let output = root.join("output");
        let marker_dir = root.join("cache");
        fs::create_dir_all(stage_dir.join("src")).unwrap();
        fs::create_dir_all(output.join("pkg")).unwrap();
        fs::create_dir_all(&marker_dir).unwrap();

        let legacy_package = "docs2-tutorial-preview-007";
        let current_package = "docs2-tutorial-preview-108";
        let legacy_module = legacy_package.replace('-', "_");
        fs::write(
            stage_dir.join("Cargo.toml"),
            format!("[package]\nname = \"{legacy_package}\"\nversion = \"0.0.0\"\n"),
        )
        .unwrap();
        fs::write(
            stage_dir.join("src/lib.rs"),
            "pub fn answer() -> u8 { 42 }\n",
        )
        .unwrap();
        let mut legacy_hasher = PreviewCacheHasher::default();
        legacy_hasher.field(b"context");
        hash_directory(&mut legacy_hasher, &stage_dir, &stage_dir).unwrap();
        let legacy_key = format!("{:016x}", legacy_hasher.finish());

        fs::write(
            stage_dir.join("Cargo.toml"),
            format!("[package]\nname = \"{current_package}\"\nversion = \"0.0.0\"\n"),
        )
        .unwrap();
        fs::write(
            output.join("bootstrap.mjs"),
            format!("import init from './pkg/{legacy_module}.js';\nawait init();\n"),
        )
        .unwrap();
        fs::write(output.join("pkg").join(format!("{legacy_module}.js")), "").unwrap();
        fs::write(
            output.join("pkg").join(format!("{legacy_module}_bg.wasm")),
            "",
        )
        .unwrap();
        fs::write(
            marker_dir.join(format!("{legacy_package}.fingerprint")),
            legacy_key,
        )
        .unwrap();

        let cache_key = preview_cache_key(&stage_dir, "context", current_package).unwrap();
        let preview = PreparedPreview {
            package: current_package.to_owned(),
            stage_dir: stage_dir.clone(),
            cache_marker: marker_dir.join(format!("{cache_key}.fingerprint")),
            cache_key: cache_key.clone(),
            cache_context: "context".to_owned(),
            outputs: vec![PreviewOutput {
                app: "test-app".to_owned(),
                step: "test-step".to_owned(),
                goal_title: "Test goal".to_owned(),
                step_title: "Test step".to_owned(),
                goal: "Test".to_owned(),
                path: output,
            }],
        };

        assert!(preview_package_is_cached(&preview).unwrap());
        assert_eq!(
            fs::read_to_string(&preview.cache_marker).unwrap(),
            cache_key
        );
        fs::remove_dir_all(root).unwrap();
    }
}
