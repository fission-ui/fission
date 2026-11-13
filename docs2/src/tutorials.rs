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
use std::time::Instant;

mod cache;
mod dependencies;
mod packaging;
mod static_site;

const TUTORIAL_BUILD_PARALLELISM_ENV: &str = "FISSION_TUTORIAL_BUILD_PARALLELISM";
const WEB_WASM_SIMD_ENV: &str = "FISSION_WEB_WASM_SIMD";
const WEB_WASM_TARGET_RUSTFLAGS: &str = "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS";
const DEFAULT_TUTORIAL_BUILD_PARALLELISM: usize = 2;
const MAX_TUTORIAL_BUILD_PARALLELISM: usize = 64;
const PACKAGE_PARALLELISM_ENV: &str = "FISSION_TUTORIAL_PACKAGE_PARALLELISM";
const WASM_OPT_ENV: &str = "FISSION_TUTORIAL_WASM_OPT";

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
    cache_dir: std::path::PathBuf,
    cache_key: String,
    package_cached: bool,
    sqlite_assets: Option<std::path::PathBuf>,
    outputs: Vec<PreviewOutput>,
}

struct PreviewWorkspace {
    manifest_path: std::path::PathBuf,
    preview_count: usize,
    include_charts: bool,
    preview_indices: Vec<usize>,
}

/// Build the real HTML checkpoints for one website tutorial during authoring QA.
pub fn check_static_goal(project_dir: &Path, app: &str) -> Result<()> {
    let documents = load_goals(project_dir.join("content/goals"))?;
    let document = documents
        .iter()
        .find(|doc| doc.meta.app == app)
        .with_context(|| format!("unknown tutorial {app}"))?;
    let workspace = project_dir
        .parent()
        .context("docs2 has no workspace parent")?;
    let target = std::path::PathBuf::from(
        std::env::var_os("CARGO_TARGET_DIR")
            .context("set the shared CARGO_TARGET_DIR before building tutorials")?,
    );
    let cache_context = static_cache_context(workspace, project_dir)?;
    let mut files = BTreeMap::new();
    for step in &document.steps {
        files.extend(step.files.clone());
        if !static_site::is_static(&files) {
            bail!(
                "{app} / {} does not define a complete Static site project",
                step.id
            );
        }
        static_site::build(
            workspace,
            &target,
            PreviewBuild {
                files: files.clone(),
                include_charts: false,
                outputs: vec![PreviewOutput {
                    app: app.to_owned(),
                    step: step.id.clone(),
                    goal_title: document.meta.title.clone(),
                    step_title: step.title.clone(),
                    goal: document.meta.goal.clone(),
                    path: project_dir
                        .join("generated-assets/tutorials")
                        .join(app)
                        .join(&step.id),
                }],
            },
            &cache_context,
        )?;
    }
    Ok(())
}

pub fn build_all(project_dir: &Path) -> Result<()> {
    build_selected(project_dir, None)
}

/// Package one final checkpoint for focused interaction QA without rebuilding every lesson.
pub fn build_goal_preview(project_dir: &Path, app: &str) -> Result<()> {
    build_selected(project_dir, Some(app))
}

fn build_selected(project_dir: &Path, selected_app: Option<&str>) -> Result<()> {
    let started = Instant::now();
    let content_dir = project_dir.join("content/goals");
    let mut documents = load_goals(&content_dir)?;
    if let Some(app) = selected_app {
        documents.retain(|document| document.meta.app == app);
        if documents.is_empty() {
            bail!("unknown tutorial app: {app}");
        }
    }
    let parallelism = tutorial_build_parallelism()?;
    let packaging_parallelism = build_parallelism(PACKAGE_PARALLELISM_ENV, 2)?;
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
            if selected_app.is_some()
                && document.steps.last().is_some_and(|last| last.id != step.id)
            {
                continue;
            }
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
                        .any(|source| source.contains("fission::charts")),
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
    eprintln!(
        "Extracted {} distinct tutorial checkpoints in {:.2?}.",
        previews.len(),
        started.elapsed()
    );
    let workspace_root = project_dir
        .parent()
        .context("docs2 must live below the Fission workspace")?;
    let build_cache_root = tutorial_build_cache_root(project_dir);
    let (static_previews, previews): (Vec<_>, Vec<_>) = previews
        .into_iter()
        .partition(|preview| static_site::is_static(&preview.files));
    if !static_previews.is_empty() {
        let context = static_cache_context(workspace_root, project_dir)?;
        for preview in static_previews {
            static_site::build(workspace_root, &build_cache_root, preview, &context)?;
        }
    }
    if previews.is_empty() {
        return Ok(());
    }
    let preparation_started = Instant::now();
    let tools = packaging::PackagingTools::discover(workspace_root, tutorial_wasm_opt()?)?;
    let cache_context = format!(
        "{}\n{}\n{}",
        preview_cache_context(
            workspace_root,
            project_dir,
            wasm_simd,
            wasm_target_rustflags.as_deref(),
        )?,
        tools.binding_context(),
        cache::source_identity(&BTreeMap::from([(
            "packaging.rs".into(),
            fs::read_to_string(project_dir.join("src/tutorials/packaging.rs"))?
        )]))
    );
    let (previews, workspaces) =
        prepare_preview_workspaces(workspace_root, &build_cache_root, previews, &cache_context)?;
    eprintln!(
        "Prepared {} Web projects and validated their dependency/cache inputs in {:.2?}.",
        previews.len(),
        preparation_started.elapsed()
    );
    let cargo_target = build_cache_root.join("tutorial-wasm");
    compile_preview_workspaces(
        &workspaces,
        &previews,
        &cargo_target,
        parallelism,
        wasm_target_rustflags.as_deref(),
    )?;
    let workers = packaging_parallelism.min(previews.len());
    let hits = previews
        .iter()
        .filter(|preview| preview.package_cached)
        .count();
    eprintln!(
        "Packaging {} unique Web preview(s): {hits} cache hits, {} misses; {workers} workers ({PACKAGE_PARALLELISM_ENV}). wasm-opt: {}.",
        previews.len(), previews.len() - hits, if tools.optimization_context().is_some() { "enabled" } else { "disabled" },
    );
    let packaging_started = Instant::now();
    package_previews(&previews, &cargo_target, workers, &tools)?;
    eprintln!(
        "Tutorial packaging completed in {:.2?}; all tutorials ready in {:.2?}.",
        packaging_started.elapsed(),
        started.elapsed()
    );
    Ok(())
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

    // Compile like-feature packages together, so cached and fresh subsets cannot
    // change Fission's enabled capabilities through workspace feature unification.
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, preview) in previews.iter().enumerate() {
        let sources = preview
            .files
            .values()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let features = tutorial_fission_features(&sources, preview.include_charts);
        groups.entry(features.join("-")).or_default().push(index);
    }
    let bindgen_version = packaging::required_bindgen_version(workspace_root)?;
    for (features, indices) in groups {
        let include_charts = previews[indices[0]].include_charts;
        if indices.is_empty() {
            continue;
        }

        let workspace_dir = build_cache_root
            .join("tutorials")
            .join(format!("preview-{features}"));
        fs::create_dir_all(&workspace_dir)?;
        let members = indices
            .iter()
            .map(|index| {
                format!(
                    "\"preview-{}\"",
                    cache::source_identity(&previews[*index].files)
                )
            })
            .collect::<Vec<_>>()
            .join(",\n    ");
        let manifest_path = workspace_dir.join("Cargo.toml");
        write_if_changed(
            &manifest_path,
            format!("[workspace]\nmembers = [\n    {members}\n]\nresolver = \"2\"\n").as_bytes(),
        )?;
        cache::seed_lock(
            &workspace_dir,
            &fs::read(workspace_root.join("Cargo.lock"))?,
        )?;

        for index in &indices {
            let preview = &mut previews[*index];
            let identity = cache::source_identity(&preview.files);
            let member_name = format!("preview-{identity}");
            let package = format!("docs2-tutorial-preview-{identity}");
            let stage_dir = workspace_dir.join(&member_name);
            let sources = preview
                .files
                .values()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let fission_features = tutorial_fission_features(&sources, include_charts);
            let manifest = tutorial_manifest(
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
                preview
                    .files
                    .values()
                    .any(|source| source.contains("web_sys::")),
                preview
                    .files
                    .values()
                    .any(|source| source.contains("serde_json::")),
            )
            .replace(
                "wasm-bindgen = \"0.2\"",
                &format!("wasm-bindgen = \"={bindgen_version}\""),
            );
            let mut files = preview
                .files
                .iter()
                .map(|(path, source)| {
                    (
                        preview_stage_path(path).to_string_lossy().into_owned(),
                        source.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            files.insert("Cargo.toml".into(), manifest);
            cache::sync_sources(&stage_dir, &files)?;
            let cache_key = preview_cache_key(&stage_dir, cache_context, &package)?;
            let cache_dir = build_cache_root
                .join("tutorial-preview-cache")
                .join("release")
                .join(&cache_key);
            prepared[*index] = Some(PreparedPreview {
                package,
                cache_dir,
                cache_key,
                package_cached: false,
                sqlite_assets: fission_features
                    .contains(&"store-sqlite-web")
                    .then(|| workspace_root.join("crates/core/fission-store-sqlite/assets/web")),
                outputs: std::mem::take(&mut preview.outputs),
            });
        }
        let dependency_contexts = dependencies::contexts(&manifest_path)?;
        for index in &indices {
            let preview = prepared[*index].as_mut().unwrap();
            let stage = workspace_dir.join(format!(
                "preview-{}",
                cache::source_identity(&previews[*index].files)
            ));
            let dependency_context = dependency_contexts
                .get(&preview.package)
                .context("tutorial has no resolved dependencies")?;
            preview.cache_key = preview_cache_key(
                &stage,
                &format!("{cache_context}\n{dependency_context}"),
                &preview.package,
            )?;
            preview.cache_dir = build_cache_root
                .join("tutorial-preview-cache/release")
                .join(&preview.cache_key);
            // Validate the complete package once per build, not once during
            // compilation, again for counters, and a third time in packaging.
            preview.package_cached = preview_package_is_cached(preview)?;
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
        let started = Instant::now();
        let uncached = workspace
            .preview_indices
            .iter()
            .filter_map(|index| previews.get(*index).map(|preview| (*index, preview)))
            .filter(|(_, preview)| !preview.package_cached)
            .map(|(index, preview)| (index, preview.package.as_str()))
            .collect::<Vec<_>>();
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
            .args([
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "--jobs",
            ])
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
            "Compiled {} tutorial app(s) {feature_label} in {:.2?}.",
            uncached.len(),
            started.elapsed()
        );
    }
    Ok(())
}

fn preview_package_is_cached(preview: &PreparedPreview) -> Result<bool> {
    cache::package_is_current(
        &preview.cache_dir,
        &preview.cache_key,
        &packaged_module(preview),
    )
}

fn packaged_module(preview: &PreparedPreview) -> String {
    // Rust package identity stays stable; browser assets change whenever any
    // compiled input changes, so an old cached glue file cannot load a new ABI.
    format!("preview_{}", preview.cache_key)
}

fn package_previews(
    previews: &[PreparedPreview],
    cargo_target: &Path,
    workers: usize,
    tools: &packaging::PackagingTools,
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
                        tools,
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
    build_parallelism(
        TUTORIAL_BUILD_PARALLELISM_ENV,
        DEFAULT_TUTORIAL_BUILD_PARALLELISM,
    )
}

fn build_parallelism(variable: &str, default: usize) -> Result<usize> {
    let value = match std::env::var(variable) {
        Ok(value) => value
            .parse::<usize>()
            .with_context(|| format!("{variable} must be an integer"))?,
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => return Err(error).with_context(|| format!("failed to read {variable}")),
    };
    if !(1..=MAX_TUTORIAL_BUILD_PARALLELISM).contains(&value) {
        bail!("{variable} must be between 1 and {MAX_TUTORIAL_BUILD_PARALLELISM}");
    }
    Ok(value)
}

fn tutorial_wasm_opt() -> Result<bool> {
    match std::env::var(WASM_OPT_ENV) {
        Err(std::env::VarError::NotPresent) => Ok(false),
        Ok(value) => match value.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => bail!("{WASM_OPT_ENV} must be true or false"),
        },
        Err(error) => Err(error.into()),
    }
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
    tools: &packaging::PackagingTools,
) -> Result<()> {
    let started = Instant::now();
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
    let module = packaged_module(preview);
    let package_is_current = preview.package_cached;
    if package_is_current {
        eprintln!(
            "  Reusing current WebAssembly package for {} / {}",
            first.app, first.step
        );
    } else {
        if preview.cache_dir.exists() {
            fs::remove_dir_all(&preview.cache_dir)?;
        }
        let input = cargo_target
            .join("wasm32-unknown-unknown/release")
            .join(format!("{}.wasm", preview.package.replace('-', "_")));
        tools.package(&input, &preview.cache_dir, &module)?;
        cache::mark_complete(&preview.cache_dir, &preview.cache_key)?;
    }
    let mut published_package = preview.cache_dir.clone();
    if let Some(context) = tools.optimization_context() {
        let mut hasher = PreviewCacheHasher::default();
        hasher.field(preview.cache_key.as_bytes());
        hasher.field(context.as_bytes());
        let key = format!("{:016x}", hasher.finish());
        let optimized = preview
            .cache_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("optimized")
            .join(&key);
        if !cache::package_is_current(&optimized, &key, &module)? {
            if optimized.exists() {
                fs::remove_dir_all(&optimized)?;
            }
            let before = fs::metadata(preview.cache_dir.join(format!("{module}_bg.wasm")))?.len();
            let opt_started = Instant::now();
            tools.optimize_package(&preview.cache_dir, &optimized, &module)?;
            cache::mark_complete(&optimized, &key)?;
            let after = fs::metadata(optimized.join(format!("{module}_bg.wasm")))?.len();
            eprintln!(
                "  wasm-opt {} / {}: {before} -> {after} bytes in {:.2?}",
                first.app,
                first.step,
                opt_started.elapsed()
            );
        } else {
            eprintln!(
                "  Reusing optimized package: {} / {}",
                first.app, first.step
            );
        }
        published_package = optimized;
    }
    let version = published_package
        .file_name()
        .and_then(|name| name.to_str())
        .context("package has no version")?;
    for step in &preview.outputs {
        if step.path.exists() {
            fs::remove_dir_all(&step.path)?;
        }
        copy_dir(&published_package, &step.path.join("pkg"))?;
        fs::remove_file(step.path.join("pkg/.complete"))?;
        fs::write(
            step.path.join("bootstrap.mjs"),
            format!("import init from './pkg/{module}.js?v={version}';\nawait init({{ module_or_path: new URL('./pkg/{module}_bg.wasm?v={version}', import.meta.url) }});\n"),
        )?;
        if let Some(assets) = &preview.sqlite_assets {
            let module = preview_bootstrap_module(&step.path)?
                .context("SQLite preview bootstrap has no packaged module")?;
            install_sqlite_preview(&step.path, assets, &step.app, &module, version)?;
        }
        fs::write(
            step.path.join("index.html"),
            tutorial_html(&step.goal_title, &step.step_title, &step.goal, version),
        )?;
        eprintln!("  Ready: {} / {}", step.app, step.step);
    }
    eprintln!("  Preview packaged/published in {:.2?}", started.elapsed());
    Ok(())
}

fn install_sqlite_preview(
    output: &Path,
    assets: &Path,
    app: &str,
    module: &str,
    version: &str,
) -> Result<()> {
    let destination = output.join(format!("sqlite-{version}"));
    fs::create_dir_all(&destination)?;
    for name in [
        "sqlite3.mjs",
        "sqlite3.wasm",
        "sqlite3-opfs-async-proxy.js",
        "fission-sqlite.mjs",
        "fission-sqlite-worker.mjs",
        "NOTICE.txt",
    ] {
        fs::copy(assets.join(name), destination.join(name))
            .with_context(|| format!("failed to stage SQLite preview asset {name}"))?;
    }
    // A tutorial shares its database across checkpoints, not across unrelated
    // tutorials or generated package numbers. No mock provider is installed.
    let application_id = serde_json::to_string(&format!("rs.fission.docs2.{app}"))?;
    fs::write(
        output.join("bootstrap.mjs"),
        format!(
            "import init from './pkg/{module}.js?v={version}';\n\
             import {{ installFissionSqlite }} from './sqlite-{version}/fission-sqlite.mjs';\n\
             if (!globalThis.crossOriginIsolated) {{\n\
                 const mount = document.getElementById('fission-web-mount');\n\
                 mount.setAttribute('role', 'status');\n\
                 mount.style.cssText = 'box-sizing:border-box;max-width:48rem;padding:24px;font:16px/1.6 system-ui,sans-serif;overflow:auto';\n\
                 mount.textContent = 'This preview uses SQLite and cannot run here because cross-origin isolation is unavailable. Both the documentation page and embedded app need Cross-Origin-Opener-Policy: same-origin and Cross-Origin-Embedder-Policy: require-corp. Run the tutorial’s Web app locally, or use a host configured with those headers. This preview has not opened the database.';\n\
             }} else {{\n\
             installFissionSqlite({{ appId: {application_id} }});\n\
             await init({{ module_or_path: new URL('./pkg/{module}_bg.wasm?v={version}', import.meta.url) }});\n\
             }}\n"
        ),
    )?;
    Ok(())
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
    let Some((module, _)) = module_path.split_once(".js") else {
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
    hasher.field(b"fission-tutorial-preview-cache-v3;cargo-release;lib-only");
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
    // Generator behavior is an input too, but editorial content is not.
    hasher.field(&fs::read(project_dir.join("src/tutorials.rs"))?);
    for module in ["cache.rs", "static_site.rs", "dependencies.rs"] {
        hasher.field(&fs::read(project_dir.join("src/tutorials").join(module))?);
    }
    for name in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"] {
        hasher.field(name.as_bytes());
        if let Some(value) = std::env::var_os(name) {
            hasher.field(value.to_string_lossy().as_bytes());
        }
    }
    hasher.field(
        profile_overrides(std::env::vars_os().map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        }))
        .as_bytes(),
    );

    if let Some(directory) = std::env::var_os("FISSION_MATERIAL_ICONS_DIR") {
        hasher.field(b"FISSION_MATERIAL_ICONS_DIR");
        hasher.field(directory.to_string_lossy().as_bytes());
        let directory = Path::new(&directory);
        hash_directory(&mut hasher, directory, directory)?;
    }

    for (program, args) in [("rustc", vec!["-vV"]), ("cargo", vec!["--version"])] {
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

    Ok(format!("{:016x}", hasher.finish()))
}

fn profile_overrides(values: impl IntoIterator<Item = (String, String)>) -> String {
    // Include every profile override, not a hand-maintained subset: overflow
    // checks and debug assertions can change application behavior too. Static
    // HTML builders use dev; Wasm and its host build scripts use release.
    let settings = values
        .into_iter()
        .filter(|(name, _)| name.starts_with("CARGO_PROFILE_"))
        .collect::<BTreeMap<_, _>>();
    let mut hasher = PreviewCacheHasher::default();
    for (name, value) in settings {
        hasher.field(name.as_bytes());
        hasher.field(value.as_bytes());
    }
    format!("{:016x}", hasher.finish())
}

fn static_cache_context(workspace_root: &Path, project_dir: &Path) -> Result<String> {
    let mut hasher = PreviewCacheHasher::default();
    hasher.field(preview_cache_context(workspace_root, project_dir, false, None)?.as_bytes());
    let dependency_dirs = [
        "crates/authoring/fission",
        "crates/authoring/fission-compose",
        "crates/authoring/fission-compose-macros",
        "crates/authoring/fission-widgets",
        "crates/authoring/fission-macros",
        "crates/authoring/fission-icons",
        "crates/authoring/fission-charts",
        "crates/core/fission-core",
        "crates/core/fission-semantics",
        "crates/core/fission-ir",
        "crates/core/fission-layout",
        "crates/core/fission-text-engine",
        "crates/core/fission-theme",
        "crates/core/fission-i18n",
        "crates/core/fission-store",
        "crates/core/fission-store-sqlite",
        "crates/rendering/fission-render",
        "crates/rendering/fission-render-vello",
        "crates/shell/fission-shell",
        "crates/shell/fission-shell-winit",
        "crates/shell/fission-shell-web",
        "crates/tools/fission-test-driver",
        "crates/tools/fission-diagnostics",
        "crates/tools/fission-design-system-codegen",
        "third_party/vello/Cargo.toml",
        "third_party/vello/vello_common",
        "third_party/vello/vello_cpu",
        "third_party/vello/vello_gpu",
        "third_party/vello/vello_gpu_shaders",
        "third_party/vello/glifo",
    ];
    for relative in dependency_dirs {
        let path = workspace_root.join(relative);
        if path.is_dir() {
            hasher.field(relative.as_bytes());
            hash_directory(&mut hasher, workspace_root, &path)?;
        } else if path.is_file() {
            hasher.field(relative.as_bytes());
            hasher.field(&fs::read(path)?);
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
    include_browser_location: bool,
    include_serde_json: bool,
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
    let serde_json_dependency = if include_serde_json {
        "serde_json = \"1\"\n"
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
    let browser_dependency = if include_browser_location {
        "web-sys = { version = \"0.3\", features = [\"Window\", \"Location\"] }\n"
    } else {
        ""
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
{serde_dependency}{serde_yaml_dependency}{serde_json_dependency}wasm-bindgen = "0.2"
{browser_dependency}{design_system_codegen_dependency}"#,
        fission.display()
    )
}

/// Features shared by preview builds and the authored-source audit tool.
pub fn tutorial_fission_features(sources: &[&str], include_charts: bool) -> Vec<&'static str> {
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
    if features.contains(&"store") || features.contains(&"store-sql") {
        features.push("store-sqlite-web");
    }
    if uses_any(&["Video::", "Video("]) {
        features.push("video");
    }

    features
}

fn tutorial_html(goal: &str, step: &str, task: &str, version: &str) -> String {
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
</script><script type="module" src="./bootstrap.mjs?v={version}"></script></body></html>"#
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

    #[test]
    fn all_profile_overrides_are_cache_inputs_and_environment_order_is_not() {
        let base = Vec::<(String, String)>::new();
        for name in [
            "CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS",
            "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS",
            "CARGO_PROFILE_DEV_DEBUG_ASSERTIONS",
            "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_OPT_LEVEL",
        ] {
            assert_ne!(
                profile_overrides(base.clone()),
                profile_overrides([(name.to_owned(), "true".into())])
            );
        }
        let first = [
            (
                "CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS".into(),
                "true".into(),
            ),
            ("CARGO_PROFILE_DEV_OPT_LEVEL".into(), "1".into()),
        ];
        assert_eq!(
            profile_overrides(first.clone()),
            profile_overrides(first.into_iter().rev())
        );
        assert_eq!(
            profile_overrides(base),
            profile_overrides([("UNRELATED_VALUE".into(), "other".into())])
        );
    }

    pub(super) fn temporary_directory(label: &str) -> std::path::PathBuf {
        static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target"));
        target.join("test-fixtures/tutorial-cache").join(format!(
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
        let with_codegen = tutorial_manifest(
            "tutorial", workspace, &features, true, false, false, true, false, false,
        );
        let without_codegen = tutorial_manifest(
            "tutorial", workspace, &features, true, false, false, false, false, false,
        );
        let source_entrypoint = tutorial_manifest(
            "tutorial", workspace, &features, false, false, false, false, false, false,
        );

        assert!(with_codegen.contains("path = \"src/preview.rs\""));
        assert!(with_codegen.contains("features = [\"web\", \"store-sql\", \"store-sqlite-web\"]"));
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
    fn store_previews_enable_the_real_web_provider() {
        let features = tutorial_fission_features(&["ctx.effects.store().get(key)"], false);
        assert_eq!(features, vec!["web", "store", "store-sqlite-web"]);
    }

    #[test]
    fn browser_location_dependency_is_conditional() {
        let workspace = Path::new("/fission-workspace");
        let with_browser = tutorial_manifest(
            "tutorial",
            workspace,
            &["web"],
            true,
            false,
            false,
            false,
            true,
            true,
        );
        let without_browser = tutorial_manifest(
            "tutorial",
            workspace,
            &["web"],
            true,
            false,
            false,
            false,
            false,
            false,
        );
        assert!(with_browser.contains("web-sys"));
        assert!(with_browser.contains("\"Location\""));
        assert!(!without_browser.contains("web-sys"));
        assert!(with_browser.contains("serde_json"));
        assert!(!without_browser.contains("serde_json"));
    }

    #[test]
    fn sqlite_preview_stages_official_assets_and_stable_app_namespace() {
        let root = temporary_directory("sqlite-preview");
        let assets = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("crates/core/fission-store-sqlite/assets/web");
        install_sqlite_preview(&root, &assets, "reading-list", "preview_007", "v1").unwrap();
        let first = fs::read_to_string(root.join("bootstrap.mjs")).unwrap();
        assert!(first.contains("rs.fission.docs2.reading-list"));
        assert!(first.contains("if (!globalThis.crossOriginIsolated)"));
        assert!(first.contains("This preview has not opened the database."));
        assert!(
            first.find("installFissionSqlite({").unwrap() < first.find("await init({").unwrap()
        );
        assert_eq!(
            preview_bootstrap_module(&root).unwrap().as_deref(),
            Some("preview_007")
        );
        assert_eq!(
            fs::read(root.join("sqlite-v1/NOTICE.txt")).unwrap(),
            fs::read(assets.join("NOTICE.txt")).unwrap()
        );
        assert!(root.join("sqlite-v1/sqlite3.wasm").is_file());
        assert!(root.join("sqlite-v1/fission-sqlite-worker.mjs").is_file());
        install_sqlite_preview(&root, &assets, "reading-list", "preview_108", "v2").unwrap();
        let shifted = fs::read_to_string(root.join("bootstrap.mjs")).unwrap();
        assert!(shifted.contains("rs.fission.docs2.reading-list"));
        assert!(!shifted.contains("preview_007"));
        assert!(shifted.contains("preview_108.js?v=v2"));
        assert!(shifted.contains("preview_108_bg.wasm?v=v2"));
        assert!(shifted.contains("sqlite-v2/fission-sqlite.mjs"));
        assert!(tutorial_html("Goal", "Step", "Task", "v2").contains("bootstrap.mjs?v=v2"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restored_package_does_not_need_generated_output_or_cargo_artifacts() {
        let root = temporary_directory("restored-package");
        let stage_dir = root.join("stage");
        let output = root.join("output");
        let cache_dir = root.join("cache");
        fs::create_dir_all(stage_dir.join("src")).unwrap();
        fs::create_dir_all(&cache_dir).unwrap();
        let package = "docs2-tutorial-preview-stable";
        let cache_key = "source-toolchain-profile";
        let module = format!("preview_{cache_key}");
        fs::write(cache_dir.join(format!("{module}.js")), "js").unwrap();
        fs::write(cache_dir.join(format!("{module}_bg.wasm")), "wasm").unwrap();
        cache::mark_complete(&cache_dir, cache_key).unwrap();
        let preview = PreparedPreview {
            package: package.to_owned(),
            cache_dir,
            cache_key: cache_key.to_owned(),
            package_cached: true,
            sqlite_assets: None,
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
        assert!(!preview.outputs[0].path.exists());
        fs::write(
            preview.cache_dir.join(format!("{module}_bg.wasm")),
            "corrupt",
        )
        .unwrap();
        assert!(!preview_package_is_cached(&preview).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
