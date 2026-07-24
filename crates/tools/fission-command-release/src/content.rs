use super::*;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use fission_test_driver::{Selector, SelectorQuery};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize, Default)]
struct ContentToml {
    release: Option<ReleaseContentRoot>,
    #[serde(default)]
    releases: Vec<ReleaseEntryContent>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseContentRoot {
    active_release: Option<String>,
    #[serde(default)]
    default_locales: Vec<String>,
    screenshots: Option<ScreenshotConfig>,
    assets: Option<ProviderAssets>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseEntryContent {
    id: Option<String>,
    version: Option<String>,
    #[serde(default)]
    locales: Vec<String>,
    metadata: Option<String>,
    release_notes: Option<String>,
    review: Option<String>,
    privacy: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ScreenshotConfig {
    raw_dir: Option<String>,
    rendered_dir: Option<String>,
    #[serde(default)]
    scenarios: Vec<ScreenshotScenario>,
}

#[derive(Debug, Deserialize, Default)]
struct ScreenshotScenario {
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    targets: Vec<String>,
    script: Option<String>,
    command: Option<String>,
    test_port: Option<u16>,
    timeout_ms: Option<u64>,
    wait_for: Option<String>,
    #[serde(default)]
    steps: Vec<ScreenshotStep>,
}

#[derive(Debug, Deserialize, Default)]
struct ScreenshotStep {
    cmd: String,
    selector: Option<String>,
    text: Option<String>,
    key: Option<String>,
    value: Option<String>,
    modifiers: Option<u8>,
    ms: Option<u64>,
    x: Option<f32>,
    y: Option<f32>,
    dx: Option<f32>,
    dy: Option<f32>,
    width: Option<u32>,
    height: Option<u32>,
    name: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ProviderAssets {
    app_store: Option<AppStoreAssets>,
    play_store: Option<PlayStoreAssets>,
    microsoft_store: Option<MicrosoftStoreAssets>,
}

#[derive(Debug, Deserialize, Default)]
struct AppStoreAssets {
    screenshot_sets_dir: Option<String>,
    app_previews_dir: Option<String>,
    #[serde(default)]
    review_attachments: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct PlayStoreAssets {
    screenshot_sets_dir: Option<String>,
    preview_video_dir: Option<String>,
    feature_graphic: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MicrosoftStoreAssets {
    screenshot_sets_dir: Option<String>,
    trailers_dir: Option<String>,
    logo_dir: Option<String>,
}

#[derive(Debug, Serialize)]
struct RenderManifest {
    schema_version: u32,
    created_at_unix_seconds: u64,
    provider: String,
    source_dir: String,
    output_dir: String,
    assets: Vec<RenderedAsset>,
}

#[derive(Debug, Serialize)]
struct RenderedAsset {
    kind: String,
    source: String,
    output: String,
    sha256: String,
    size_bytes: u64,
    width: Option<u32>,
    height: Option<u32>,
}

pub(crate) fn validate_release_content_model(
    project_dir: &Path,
    provider: Option<DistributionProvider>,
) -> LifecycleReport {
    let mut report = base_report("release-content.validate", provider, None);
    report.checks.push(path_check(
        "release_content.root_exists",
        project_dir.join("release-content"),
        "release-content directory exists",
    ));
    let config = match load_content_config(project_dir) {
        Ok(config) => {
            report.checks.push(ok_check(
                "release_content.config_parses",
                "fission.toml release content config parses",
            ));
            config
        }
        Err(error) => {
            report.checks.push(failed_check(
                "release_content.config_parses",
                error.to_string(),
            ));
            finalize_status(&mut report);
            return report;
        }
    };
    validate_screenshots(project_dir, &config, provider, &mut report.checks);
    validate_provider_assets(project_dir, &config, provider, &mut report.checks);
    finalize_status(&mut report);
    report
}

pub(super) fn capture_release_content(
    project_dir: &Path,
    target: Target,
    set: &str,
) -> Result<LifecycleReport> {
    let config = load_content_config(project_dir)?;
    let mut report = base_report("release-content.capture", None, Some(target));
    let screenshots = config
        .release
        .as_ref()
        .and_then(|release| release.screenshots.as_ref())
        .context("release.screenshots must be configured before capture")?;
    let raw_dir = project_dir.join(
        screenshots
            .raw_dir
            .as_deref()
            .unwrap_or("release-content/screenshots/raw"),
    );
    fs::create_dir_all(&raw_dir)?;
    let scenarios = screenshots
        .scenarios
        .iter()
        .filter(|scenario| scenario.targets.iter().any(|item| item == target.as_str()))
        .collect::<Vec<_>>();
    if scenarios.is_empty() {
        report.checks.push(failed_check(
            "release_content.capture.scenarios_available",
            format!(
                "no screenshot scenarios target {} for set {set}",
                target.as_str()
            ),
        ));
        finalize_status(&mut report);
        return Ok(report);
    }
    for scenario in scenarios {
        capture_scenario(
            project_dir,
            &raw_dir,
            target,
            set,
            scenario,
            &mut report.checks,
        )?;
    }
    finalize_status(&mut report);
    Ok(report)
}

pub(super) fn render_release_content(
    project_dir: &Path,
    provider: DistributionProvider,
) -> Result<LifecycleReport> {
    let config = load_content_config(project_dir)?;
    let mut report = base_report("release-content.render", Some(provider), None);
    let screenshots = config
        .release
        .as_ref()
        .and_then(|release| release.screenshots.as_ref())
        .context("release.screenshots must be configured before render")?;
    let raw_dir = project_dir.join(
        screenshots
            .raw_dir
            .as_deref()
            .unwrap_or("release-content/screenshots/raw"),
    );
    let rendered_root = project_dir.join(
        screenshots
            .rendered_dir
            .as_deref()
            .unwrap_or("release-content/screenshots/rendered"),
    );
    let output_dir = rendered_root.join(provider.as_str());
    fs::create_dir_all(&output_dir)?;
    let mut assets = Vec::new();
    collect_render_assets(&raw_dir, &raw_dir, &output_dir, &mut assets)?;
    let manifest = RenderManifest {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.as_str().to_string(),
        source_dir: raw_dir.display().to_string(),
        output_dir: output_dir.display().to_string(),
        assets,
    };
    let manifest_path = output_dir.join("release-content-manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    report.checks.push(LifecycleCheck {
        id: "release_content.render.manifest_written".to_string(),
        status: "passed".to_string(),
        summary: "render manifest was written".to_string(),
        details: Some(manifest_path.display().to_string()),
        remediation: Vec::new(),
    });
    report.checks.push(LifecycleCheck {
        id: "release_content.render.assets_present".to_string(),
        status: if manifest.assets.is_empty() {
            "missing"
        } else {
            "passed"
        }
        .to_string(),
        summary: "rendered release assets exist".to_string(),
        details: Some(format!("{} assets", manifest.assets.len())),
        remediation: vec![
            "Run release-content capture or add raw screenshots/videos before rendering."
                .to_string(),
        ],
    });
    finalize_status(&mut report);
    Ok(report)
}

pub(crate) fn materialize_release_content_manifest(
    project_dir: &Path,
    provider: DistributionProvider,
) -> Result<Option<PathBuf>> {
    let config = load_content_config(project_dir)?;
    let Some(release) = config.release.as_ref() else {
        return Ok(None);
    };
    let content_root = project_dir.join("release-content");
    if !content_root.exists() {
        return Ok(None);
    }
    fs::create_dir_all(&content_root)?;
    let active_release = release.active_release.as_deref();
    let selected_releases = selected_release_content_entries(&config, active_release);
    let mut referenced_files = Vec::new();
    for entry in &selected_releases {
        for relative in [
            entry.metadata.as_deref(),
            entry.release_notes.as_deref(),
            entry.review.as_deref(),
            entry.privacy.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            collect_referenced_content_files(project_dir, relative, &mut referenced_files)?;
        }
    }
    let rendered_manifest_path = project_dir
        .join(
            release
                .screenshots
                .as_ref()
                .and_then(|screenshots| screenshots.rendered_dir.as_deref())
                .unwrap_or("release-content/screenshots/rendered"),
        )
        .join(provider.as_str())
        .join("release-content-manifest.json");
    let rendered_manifest = if rendered_manifest_path.exists() {
        let value: Value = serde_json::from_slice(&fs::read(&rendered_manifest_path)?)?;
        Some(json!({
            "path": display_project_path(project_dir, &rendered_manifest_path),
            "sha256": sha256_file(&rendered_manifest_path)?,
            "asset_count": value.get("assets").and_then(Value::as_array).map(Vec::len).unwrap_or_default(),
            "manifest": value,
        }))
    } else {
        None
    };
    let fission_toml = project_dir.join("fission.toml");
    let manifest = json!({
        "schema_version": 1,
        "created_at_unix_seconds": now_unix_seconds(),
        "provider": provider.as_str(),
        "active_release": active_release,
        "default_locales": &release.default_locales,
        "releases": selected_releases.iter().map(release_entry_manifest).collect::<Vec<_>>(),
        "source_config": {
            "path": display_project_path(project_dir, &fission_toml),
            "sha256": sha256_file(&fission_toml)?,
        },
        "referenced_files": referenced_files,
        "rendered_screenshots": rendered_manifest,
    });
    let path = content_root.join("content-manifest.json");
    fs::write(&path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(Some(path))
}

fn selected_release_content_entries<'a>(
    config: &'a ContentToml,
    active_release: Option<&str>,
) -> Vec<&'a ReleaseEntryContent> {
    if let Some(active_release) = active_release {
        let entries = config
            .releases
            .iter()
            .filter(|entry| entry.id.as_deref() == Some(active_release))
            .collect::<Vec<_>>();
        if !entries.is_empty() {
            return entries;
        }
    }
    config.releases.iter().collect()
}

fn release_entry_manifest(entry: &&ReleaseEntryContent) -> Value {
    json!({
        "id": entry.id.as_deref(),
        "version": entry.version.as_deref(),
        "locales": &entry.locales,
        "metadata": entry.metadata.as_deref(),
        "release_notes": entry.release_notes.as_deref(),
        "review": entry.review.as_deref(),
        "privacy": entry.privacy.as_deref(),
    })
}

fn collect_referenced_content_files(
    project_dir: &Path,
    relative: &str,
    files: &mut Vec<Value>,
) -> Result<()> {
    let path = project_dir.join(relative);
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        let mut children = Vec::new();
        collect_file_paths(&path, &mut children)?;
        children.sort();
        for child in children {
            files.push(content_file_entry(project_dir, &child)?);
        }
    } else {
        files.push(content_file_entry(project_dir, &path)?);
    }
    Ok(())
}

fn collect_file_paths(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_file_paths(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn content_file_entry(project_dir: &Path, path: &Path) -> Result<Value> {
    Ok(json!({
        "path": display_project_path(project_dir, path),
        "sha256": sha256_file(path)?,
        "size_bytes": fs::metadata(path).map(|metadata| metadata.len()).unwrap_or_default(),
    }))
}

fn display_project_path(project_dir: &Path, path: &Path) -> String {
    path.strip_prefix(project_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn capture_scenario(
    project_dir: &Path,
    raw_dir: &Path,
    target: Target,
    set: &str,
    scenario: &ScreenshotScenario,
    checks: &mut Vec<LifecycleCheck>,
) -> Result<()> {
    let id = scenario.id.as_deref().unwrap_or("scenario");
    checks.push(required_text_check(
        &format!("release_content.capture.{id}.id"),
        scenario.id.as_deref(),
        "scenario id is set",
    ));
    checks.push(required_text_check(
        &format!("release_content.capture.{id}.name"),
        scenario.name.as_deref(),
        "scenario name is set",
    ));
    checks.push(required_text_check(
        &format!("release_content.capture.{id}.wait_for"),
        scenario.wait_for.as_deref(),
        "scenario wait selector is set",
    ));
    if scenario.script.is_none() && scenario.command.is_none() {
        checks.push(failed_check(
            &format!("release_content.capture.{id}.driver"),
            "scenario script or command is missing".to_string(),
        ));
        return Ok(());
    };
    if let Some(script) = scenario.script.as_deref() {
        let script_path = project_dir.join(script);
        checks.push(path_check(
            &format!("release_content.capture.{id}.script_exists"),
            script_path.clone(),
            "scenario script exists",
        ));
        if !script_path.exists() {
            return Ok(());
        }
        return match script_path.extension().and_then(|value| value.to_str()) {
            Some("sh") => run_capture_script(
                "bash",
                &[script_path.to_string_lossy().as_ref()],
                project_dir,
                raw_dir,
                target,
                set,
                id,
                checks,
            ),
            Some("ps1") => run_capture_script(
                "pwsh",
                &["-File", script_path.to_string_lossy().as_ref()],
                project_dir,
                raw_dir,
                target,
                set,
                id,
                checks,
            ),
            _ => {
                let receipt = raw_dir.join(format!("{set}-{id}-capture-plan.json"));
                let body = serde_json::json!({
                    "schema_version": 1,
                    "target": target.as_str(),
                    "set": set,
                    "scenario": id,
                    "script": script_path,
                    "wait_for": scenario.wait_for,
                    "status": "planned",
                    "note": "Non-shell scenario files are validated and recorded; execution is handled by the Fission platform test runner."
                });
                fs::write(&receipt, serde_json::to_vec_pretty(&body)?)?;
                checks.push(ok_check(
                    &format!("release_content.capture.{id}.plan_written"),
                    receipt.display().to_string(),
                ));
                Ok(())
            }
        };
    }
    run_test_control_capture(project_dir, raw_dir, target, set, id, scenario, checks)
}

fn run_capture_script(
    program: &str,
    args: &[&str],
    project_dir: &Path,
    raw_dir: &Path,
    target: Target,
    set: &str,
    id: &str,
    checks: &mut Vec<LifecycleCheck>,
) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .current_dir(project_dir)
        .env("FISSION_CAPTURE_OUTPUT", raw_dir)
        .env("FISSION_CAPTURE_TARGET", target.as_str())
        .env("FISSION_CAPTURE_SET", set)
        .env("FISSION_CAPTURE_SCENARIO", id)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to run capture script through {program}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    checks.push(LifecycleCheck {
        id: format!("release_content.capture.{id}.script_ran"),
        status: if output.status.success() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        summary: "capture script completed".to_string(),
        details: Some(format!(
            "stdout: {}; stderr: {}",
            stdout.trim(),
            stderr.trim()
        )),
        remediation: vec![
            "Fix the scenario script or run it manually with the printed environment variables."
                .to_string(),
        ],
    });
    Ok(())
}

fn run_test_control_capture(
    project_dir: &Path,
    raw_dir: &Path,
    target: Target,
    set: &str,
    id: &str,
    scenario: &ScreenshotScenario,
    checks: &mut Vec<LifecycleCheck>,
) -> Result<()> {
    let command = scenario
        .command
        .as_deref()
        .context("scenario command is missing")?;
    let port = scenario.test_port.unwrap_or_else(free_loopback_port);
    let timeout = Duration::from_millis(scenario.timeout_ms.unwrap_or(20_000));
    let stdout_path = raw_dir.join(format!("{set}-{id}-stdout.log"));
    let stderr_path = raw_dir.join(format!("{set}-{id}-stderr.log"));
    let mut child = spawn_capture_command(
        project_dir,
        command,
        target,
        set,
        id,
        port,
        &stdout_path,
        &stderr_path,
    )?;
    let result = run_test_control_steps(raw_dir, set, id, scenario, port, timeout, checks);
    terminate_capture_process(&mut child);
    if let Err(error) = result {
        let receipt = write_capture_failure_receipt(
            raw_dir,
            target,
            set,
            id,
            scenario,
            &stdout_path,
            &stderr_path,
            &error.to_string(),
        )?;
        checks.push(failed_check(
            &format!("release_content.capture.{id}.test_control_failed"),
            format!("{}; receipt: {}", error, receipt.display()),
        ));
    }
    checks.push(LifecycleCheck {
        id: format!("release_content.capture.{id}.logs"),
        status: "passed".to_string(),
        summary: "capture command logs were recorded".to_string(),
        details: Some(format!(
            "stdout: {}; stderr: {}",
            stdout_path.display(),
            stderr_path.display()
        )),
        remediation: Vec::new(),
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn spawn_capture_command(
    project_dir: &Path,
    command: &str,
    target: Target,
    set: &str,
    id: &str,
    port: u16,
    stdout_path: &Path,
    stderr_path: &Path,
) -> Result<Child> {
    let stdout = fs::File::create(stdout_path)?;
    let stderr = fs::File::create(stderr_path)?;
    let mut cmd = shell_command(command);
    cmd.current_dir(project_dir)
        .env("FISSION_TEST_CONTROL_PORT", port.to_string())
        .env("FISSION_CAPTURE_TARGET", target.as_str())
        .env("FISSION_CAPTURE_SET", set)
        .env("FISSION_CAPTURE_SCENARIO", id)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    cmd.spawn()
        .with_context(|| format!("failed to spawn capture command `{command}`"))
}

fn run_test_control_steps(
    raw_dir: &Path,
    set: &str,
    id: &str,
    scenario: &ScreenshotScenario,
    port: u16,
    timeout: Duration,
    checks: &mut Vec<LifecycleCheck>,
) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("cargo-fission-release-content/0.1")
        .build()?;
    wait_for_test_control(&client, port, timeout)?;
    checks.push(ok_check(
        &format!("release_content.capture.{id}.test_control_ready"),
        format!("http://127.0.0.1:{port}"),
    ));
    let result = (|| -> Result<()> {
        if let Some(wait_for) = scenario.wait_for.as_deref() {
            let payload = wait_for_payload(wait_for, timeout)?;
            send_test_command(&client, port, &payload)?;
            checks.push(ok_check(
                &format!("release_content.capture.{id}.wait_for"),
                wait_for.to_string(),
            ));
        }
        let mut saw_screenshot = false;
        for (index, step) in scenario.steps.iter().enumerate() {
            let response =
                send_test_command(&client, port, &step_payload(step, raw_dir, set, id)?)?;
            if step.cmd == "screenshot" || step.cmd == "capture_screenshot" {
                write_screenshot_response(raw_dir, set, id, index, step, &response)?;
                saw_screenshot = true;
            }
            checks.push(ok_check(
                &format!("release_content.capture.{id}.step.{index}"),
                step.cmd.clone(),
            ));
        }
        if !saw_screenshot {
            let response = send_test_command(&client, port, &json!({"cmd": "CaptureScreenshot"}))?;
            write_screenshot_response(
                raw_dir,
                set,
                id,
                scenario.steps.len(),
                &ScreenshotStep {
                    cmd: "capture_screenshot".to_string(),
                    name: Some("final".to_string()),
                    ..Default::default()
                },
                &response,
            )?;
        }
        Ok(())
    })();
    let _ = send_test_command(&client, port, &json!({"cmd": "Quit"}));
    result
}

fn wait_for_test_control(client: &Client, port: u16, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    let url = format!("http://127.0.0.1:{port}/health");
    loop {
        if client
            .get(&url)
            .send()
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        if start.elapsed() > timeout {
            bail!("timed out waiting for Fission test control server at {url}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn send_test_command(client: &Client, port: u16, payload: &serde_json::Value) -> Result<Value> {
    let response = client
        .post(format!("http://127.0.0.1:{port}/cmd"))
        .json(payload)
        .send()
        .with_context(|| format!("failed to send test command {payload}"))?;
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("test command failed with {status}: {text}");
    }
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse test command response: {text}"))?;
    if value.get("status").and_then(Value::as_str) == Some("Error") {
        bail!(
            "test command returned error: {}",
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        );
    }
    Ok(value)
}

fn step_payload(step: &ScreenshotStep, raw_dir: &Path, set: &str, id: &str) -> Result<Value> {
    match step.cmd.as_str() {
        "tap_text" => Ok(json!({"cmd": "TapText", "text": required_step_text(step, "text")?})),
        "tap_selector" => Ok(json!({
            "cmd": "TapSelector",
            "query": required_step_selector(step)?
        })),
        "activate_selector" => Ok(json!({
            "cmd": "ActivateSelector",
            "query": required_step_selector(step)?
        })),
        "focus_selector" => Ok(json!({
            "cmd": "FocusSelector",
            "query": required_step_selector(step)?
        })),
        "scroll_into_view" => Ok(json!({
            "cmd": "ScrollIntoView",
            "query": required_step_selector(step)?
        })),
        "fill_text" => Ok(json!({
            "cmd": "FillText",
            "query": required_step_selector(step)?,
            "text": required_step_text(step, "text")?
        })),
        "clear_text" => Ok(json!({
            "cmd": "ClearText",
            "query": required_step_selector(step)?
        })),
        "toggle" => Ok(json!({
            "cmd": "Toggle",
            "query": required_step_selector(step)?
        })),
        "select_option" => Ok(json!({
            "cmd": "SelectOption",
            "query": required_step_selector(step)?
        })),
        "wait_for_selector" => Ok(json!({
            "cmd": "WaitForSelector",
            "query": required_step_selector(step)?,
            "timeout_ms": step.ms.unwrap_or(5_000)
        })),
        "wait_for_visible" => Ok(json!({
            "cmd": "WaitForVisible",
            "query": required_step_selector(step)?,
            "timeout_ms": step.ms.unwrap_or(5_000)
        })),
        "wait_for_enabled" => Ok(json!({
            "cmd": "WaitForEnabled",
            "query": required_step_selector(step)?,
            "timeout_ms": step.ms.unwrap_or(5_000)
        })),
        "wait_for_value" => Ok(json!({
            "cmd": "WaitForValue",
            "query": required_step_selector(step)?,
            "value": step.value.as_deref().or(step.text.as_deref()).context("wait_for_value step requires value or text")?,
            "timeout_ms": step.ms.unwrap_or(5_000)
        })),
        "wait_for_text" => Ok(json!({
            "cmd": "WaitForText",
            "text": required_step_text(step, "text")?,
            "timeout_ms": step.ms.unwrap_or(5_000)
        })),
        "type_text" => Ok(json!({"cmd": "TypeText", "text": required_step_text(step, "text")?})),
        "press_key" => Ok(json!({
            "cmd": "PressKey",
            "key": required_step_text(step, "key")?,
            "modifiers": step.modifiers.unwrap_or(0)
        })),
        "tap" => Ok(json!({
            "cmd": "Tap",
            "x": required_step_f32(step.x, "x")?,
            "y": required_step_f32(step.y, "y")?
        })),
        "scroll" => Ok(json!({
            "cmd": "Scroll",
            "x": step.x.unwrap_or(0.0),
            "y": step.y.unwrap_or(0.0),
            "dx": step.dx.unwrap_or(0.0),
            "dy": step.dy.unwrap_or(0.0)
        })),
        "wait" => Ok(json!({"cmd": "Wait", "ms": step.ms.unwrap_or(250)})),
        "pump" => Ok(json!({"cmd": "Pump"})),
        "resize" => Ok(json!({
            "cmd": "SimulateResize",
            "width": step.width.context("resize step requires width")?,
            "height": step.height.context("resize step requires height")?
        })),
        "screenshot" | "capture_screenshot" => {
            let _ = screenshot_output_path(raw_dir, set, id, 0, step);
            Ok(json!({"cmd": "CaptureScreenshot"}))
        }
        other => bail!("unsupported screenshot scenario step `{other}`"),
    }
}

fn wait_for_payload(wait_for: &str, timeout: Duration) -> Result<Value> {
    if let Some(text) = wait_for.strip_prefix("text:") {
        return Ok(json!({
            "cmd": "WaitForText",
            "text": text,
            "timeout_ms": timeout.as_millis().min(u64::MAX as u128) as u64,
        }));
    }
    Ok(json!({
        "cmd": "WaitForVisible",
        "query": selector_query_payload(wait_for)?,
        "timeout_ms": timeout.as_millis().min(u64::MAX as u128) as u64,
    }))
}

fn required_step_selector(step: &ScreenshotStep) -> Result<Value> {
    selector_query_payload(
        step.selector
            .as_deref()
            .context("selector step requires selector")?,
    )
}

fn selector_query_payload(selector: &str) -> Result<Value> {
    let (selector, index) = split_selector_index(selector);
    let value = selector.trim();
    if value.is_empty() {
        bail!("selector cannot be empty");
    }
    let mut query = SelectorQuery::new(selector_payload(value)?);
    query.index = index;
    serde_json::to_value(query).context("failed to serialize LiveTest selector query")
}

fn split_selector_index(selector: &str) -> (&str, Option<usize>) {
    let Some((base, suffix)) = selector.rsplit_once('#') else {
        return (selector, None);
    };
    match suffix.parse::<usize>() {
        Ok(index) => (base, Some(index)),
        Err(_) => (selector, None),
    }
}

fn selector_payload(selector: &str) -> Result<Selector> {
    if let Some(value) = selector.strip_prefix("semantic:") {
        return Ok(Selector::semantic_identifier(value));
    }
    if let Some(value) = selector
        .strip_prefix("test_id:")
        .or_else(|| selector.strip_prefix("test-id:"))
    {
        return Ok(Selector::test_id(value));
    }
    if let Some(value) = selector
        .strip_prefix("widget_id:")
        .or_else(|| selector.strip_prefix("widget:"))
    {
        return Ok(Selector::widget_id(value));
    }
    if let Some(value) = selector
        .strip_prefix("accessibility:")
        .or_else(|| selector.strip_prefix("a11y:"))
    {
        return Ok(Selector::accessibility_identifier(value));
    }
    if let Some(value) = selector.strip_prefix("label:") {
        return Ok(Selector::label(value));
    }
    if let Some(value) = selector.strip_prefix("role:") {
        let (role, label) = value
            .split_once(':')
            .context("role selector must be role:<role>:<label>")?;
        return Ok(Selector::role_label(role, label));
    }
    Ok(Selector::semantic_identifier(selector))
}

fn write_screenshot_response(
    raw_dir: &Path,
    set: &str,
    id: &str,
    index: usize,
    step: &ScreenshotStep,
    response: &Value,
) -> Result<()> {
    let payload = response
        .get("png_base64")
        .and_then(Value::as_str)
        .context("CaptureScreenshot response did not include png_base64")?;
    let bytes = STANDARD
        .decode(payload)
        .context("CaptureScreenshot response had invalid base64")?;
    let path = screenshot_output_path(raw_dir, set, id, index, step);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn screenshot_output_path(
    raw_dir: &Path,
    set: &str,
    id: &str,
    index: usize,
    step: &ScreenshotStep,
) -> std::path::PathBuf {
    if let Some(path) = step
        .path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return raw_dir.join(path);
    }
    let name = step
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{index:02}"));
    raw_dir.join(format!("{set}-{id}-{name}.png"))
}

fn required_step_text<'a>(step: &'a ScreenshotStep, field: &str) -> Result<&'a str> {
    match field {
        "text" => step.text.as_deref().context("step requires text"),
        "key" => step.key.as_deref().context("step requires key"),
        _ => bail!("unknown step text field {field}"),
    }
}

fn required_step_f32(value: Option<f32>, field: &str) -> Result<f32> {
    value.with_context(|| format!("step requires {field}"))
}

fn free_loopback_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .ok()
        .and_then(|listener| listener.local_addr().ok())
        .map(|addr| addr.port())
        .unwrap_or(19_900)
}

fn shell_command(command: &str) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    }
}

fn terminate_capture_process(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn write_capture_failure_receipt(
    raw_dir: &Path,
    target: Target,
    set: &str,
    id: &str,
    scenario: &ScreenshotScenario,
    stdout_path: &Path,
    stderr_path: &Path,
    error: &str,
) -> Result<std::path::PathBuf> {
    let receipt = raw_dir.join(format!("{set}-{id}-capture-failure.json"));
    let body = json!({
        "schema_version": 1,
        "created_at_unix_seconds": now_unix_seconds(),
        "target": target.as_str(),
        "set": set,
        "scenario": {
            "id": scenario.id.as_deref(),
            "name": scenario.name.as_deref(),
            "wait_for": scenario.wait_for.as_deref(),
            "command": scenario.command.as_deref(),
            "test_port": scenario.test_port,
            "timeout_ms": scenario.timeout_ms,
            "step_count": scenario.steps.len(),
        },
        "stdout": stdout_path.display().to_string(),
        "stderr": stderr_path.display().to_string(),
        "error": error,
    });
    fs::write(&receipt, serde_json::to_vec_pretty(&body)?)?;
    Ok(receipt)
}

fn validate_screenshots(
    project_dir: &Path,
    config: &ContentToml,
    provider: Option<DistributionProvider>,
    checks: &mut Vec<LifecycleCheck>,
) {
    let screenshots = config
        .release
        .as_ref()
        .and_then(|release| release.screenshots.as_ref());
    let Some(screenshots) = screenshots else {
        checks.push(failed_check(
            "release_content.screenshots_configured",
            "[release.screenshots] is missing".to_string(),
        ));
        return;
    };
    let raw_dir = project_dir.join(
        screenshots
            .raw_dir
            .as_deref()
            .unwrap_or("release-content/screenshots/raw"),
    );
    let rendered_dir = project_dir.join(
        screenshots
            .rendered_dir
            .as_deref()
            .unwrap_or("release-content/screenshots/rendered"),
    );
    checks.push(path_check(
        "release_content.screenshots.raw_dir_exists",
        raw_dir,
        "raw screenshot directory exists",
    ));
    checks.push(path_check(
        "release_content.screenshots.rendered_dir_exists",
        rendered_dir.clone(),
        "rendered screenshot directory exists",
    ));
    checks.push(LifecycleCheck {
        id: "release_content.screenshots.scenarios_configured".to_string(),
        status: if screenshots.scenarios.is_empty() {
            "missing"
        } else {
            "passed"
        }
        .to_string(),
        summary: "screenshot scenarios are configured".to_string(),
        details: Some(format!("{} scenarios", screenshots.scenarios.len())),
        remediation: vec![
            "Add [[release.screenshots.scenarios]] entries with id, targets, script, and wait_for."
                .to_string(),
        ],
    });
    for scenario in &screenshots.scenarios {
        let id = scenario.id.as_deref().unwrap_or("scenario");
        if let Some(script) = scenario.script.as_deref() {
            checks.push(path_check(
                &format!("release_content.screenshots.{id}.script_exists"),
                project_dir.join(script),
                "scenario script exists",
            ));
        }
    }
    if let Some(provider) = provider {
        let provider_dir = rendered_dir.join(provider.as_str());
        let count = count_assets(&provider_dir).unwrap_or(0);
        checks.push(LifecycleCheck {
            id: format!("release_content.{}.rendered_assets", provider.as_str()),
            status: if count > 0 { "passed" } else { "missing" }.to_string(),
            summary: "provider rendered release assets exist".to_string(),
            details: Some(format!("{} assets in {}", count, provider_dir.display())),
            remediation: vec![format!(
                "Run `fission release-content render --provider {}` after capture.",
                provider.as_str()
            )],
        });
        validate_required_provider_assets(project_dir, config, provider, &provider_dir, checks);
        validate_rendered_asset_rules(provider, &provider_dir, checks);
    }
}

fn validate_required_provider_assets(
    project_dir: &Path,
    config: &ContentToml,
    provider: DistributionProvider,
    rendered_provider_dir: &Path,
    checks: &mut Vec<LifecycleCheck>,
) {
    let rendered_count = count_assets(rendered_provider_dir).unwrap_or(0);
    let configured_screenshot_count =
        configured_provider_screenshot_count(project_dir, config, provider).unwrap_or(0);
    let total_screenshots = rendered_count + configured_screenshot_count;
    let (min_screenshots, _) = provider_screenshot_count(provider);
    checks.push(LifecycleCheck {
        id: format!("release_content.{}.required_assets", provider.as_str()),
        status: if total_screenshots >= min_screenshots {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: "provider-required screenshot assets exist".to_string(),
        details: Some(format!(
            "{total_screenshots} screenshot asset(s), minimum {min_screenshots}"
        )),
        remediation: vec![format!(
            "Render release-content screenshots for {} or configure the provider screenshot directory.",
            provider.as_str()
        )],
    });

    if provider == DistributionProvider::MicrosoftStore {
        let logos = configured_microsoft_logo_count(project_dir, config).unwrap_or(0);
        checks.push(LifecycleCheck {
            id: "release_content.microsoft_store.required_logos".to_string(),
            status: if logos > 0 { "passed" } else { "missing" }.to_string(),
            summary: "Microsoft Store logo assets exist".to_string(),
            details: Some(format!("{logos} logo asset(s)")),
            remediation: vec![
                "Configure release.assets.microsoft_store.logo_dir with generated Store logo assets."
                    .to_string(),
            ],
        });
    }
}

fn validate_provider_assets(
    project_dir: &Path,
    config: &ContentToml,
    provider: Option<DistributionProvider>,
    checks: &mut Vec<LifecycleCheck>,
) {
    let assets = config
        .release
        .as_ref()
        .and_then(|release| release.assets.as_ref());
    match provider {
        Some(DistributionProvider::PlayStore) => {
            if let Some(play) = assets.and_then(|assets| assets.play_store.as_ref()) {
                check_optional_path(
                    project_dir,
                    "release_content.play_store.feature_graphic",
                    play.feature_graphic.as_deref(),
                    "Play Store feature graphic exists",
                    checks,
                );
                check_optional_path(
                    project_dir,
                    "release_content.play_store.screenshot_sets_dir",
                    play.screenshot_sets_dir.as_deref(),
                    "Play Store screenshot set directory exists",
                    checks,
                );
                check_optional_path(
                    project_dir,
                    "release_content.play_store.preview_video_dir",
                    play.preview_video_dir.as_deref(),
                    "Play Store preview video directory exists",
                    checks,
                );
            }
        }
        Some(DistributionProvider::AppStore) => {
            if let Some(app) = assets.and_then(|assets| assets.app_store.as_ref()) {
                check_optional_path(
                    project_dir,
                    "release_content.app_store.screenshot_sets_dir",
                    app.screenshot_sets_dir.as_deref(),
                    "App Store screenshot set directory exists",
                    checks,
                );
                check_optional_path(
                    project_dir,
                    "release_content.app_store.app_previews_dir",
                    app.app_previews_dir.as_deref(),
                    "App Store preview video directory exists",
                    checks,
                );
                if let Some(dir) = app
                    .app_previews_dir
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                {
                    validate_app_store_preview_assets(&project_dir.join(dir), checks);
                }
                for path in &app.review_attachments {
                    checks.push(path_check(
                        "release_content.app_store.review_attachment",
                        project_dir.join(path),
                        "App Review attachment exists",
                    ));
                }
            }
        }
        Some(DistributionProvider::MicrosoftStore) => {
            if let Some(ms) = assets.and_then(|assets| assets.microsoft_store.as_ref()) {
                check_optional_path(
                    project_dir,
                    "release_content.microsoft_store.screenshot_sets_dir",
                    ms.screenshot_sets_dir.as_deref(),
                    "Microsoft Store screenshot directory exists",
                    checks,
                );
                check_optional_path(
                    project_dir,
                    "release_content.microsoft_store.trailers_dir",
                    ms.trailers_dir.as_deref(),
                    "Microsoft Store trailers directory exists",
                    checks,
                );
                check_optional_path(
                    project_dir,
                    "release_content.microsoft_store.logo_dir",
                    ms.logo_dir.as_deref(),
                    "Microsoft Store logo directory exists",
                    checks,
                );
            }
        }
        _ => {}
    }
}

fn configured_provider_screenshot_count(
    project_dir: &Path,
    config: &ContentToml,
    provider: DistributionProvider,
) -> Result<usize> {
    let assets = config
        .release
        .as_ref()
        .and_then(|release| release.assets.as_ref());
    let dir = match provider {
        DistributionProvider::PlayStore => assets
            .and_then(|assets| assets.play_store.as_ref())
            .and_then(|assets| assets.screenshot_sets_dir.as_deref()),
        DistributionProvider::AppStore => assets
            .and_then(|assets| assets.app_store.as_ref())
            .and_then(|assets| assets.screenshot_sets_dir.as_deref()),
        DistributionProvider::MicrosoftStore => assets
            .and_then(|assets| assets.microsoft_store.as_ref())
            .and_then(|assets| assets.screenshot_sets_dir.as_deref()),
        _ => None,
    };
    match dir.filter(|value| !value.trim().is_empty()) {
        Some(dir) => count_assets(&project_dir.join(dir)),
        None => Ok(0),
    }
}

fn configured_microsoft_logo_count(project_dir: &Path, config: &ContentToml) -> Result<usize> {
    let dir = config
        .release
        .as_ref()
        .and_then(|release| release.assets.as_ref())
        .and_then(|assets| assets.microsoft_store.as_ref())
        .and_then(|assets| assets.logo_dir.as_deref());
    match dir.filter(|value| !value.trim().is_empty()) {
        Some(dir) => count_assets(&project_dir.join(dir)),
        None => Ok(0),
    }
}

fn collect_render_assets(
    root: &Path,
    current: &Path,
    output_root: &Path,
    assets: &mut Vec<RenderedAsset>,
) -> Result<()> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_render_assets(root, &path, output_root, assets)?;
            continue;
        }
        if !is_release_asset(&path) {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let dest = output_root.join(relative);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&path, &dest)?;
        let size = fs::metadata(&dest)?.len();
        let sha256 = sha256_file(&dest)?;
        let kind = asset_kind(&dest);
        let dimensions = if kind == "image" {
            image_dimensions(&dest).ok().flatten()
        } else {
            None
        };
        assets.push(RenderedAsset {
            kind: kind.to_string(),
            source: path.display().to_string(),
            output: dest.display().to_string(),
            sha256,
            size_bytes: size,
            width: dimensions.map(|(width, _)| width),
            height: dimensions.map(|(_, height)| height),
        });
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let len = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if len == 0 {
            break;
        }
        hasher.update(&buffer[..len]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn validate_rendered_asset_rules(
    provider: DistributionProvider,
    provider_dir: &Path,
    checks: &mut Vec<LifecycleCheck>,
) {
    let Ok(files) = rendered_asset_files(provider_dir) else {
        return;
    };
    let image_files = files
        .iter()
        .filter(|path| asset_kind(path) == "image")
        .collect::<Vec<_>>();
    let video_files = files
        .iter()
        .filter(|path| asset_kind(path) == "video")
        .collect::<Vec<_>>();
    let (min_images, max_images) = provider_screenshot_count(provider);
    checks.push(LifecycleCheck {
        id: format!("release_content.{}.screenshot_count", provider.as_str()),
        status: if image_files.len() >= min_images && image_files.len() <= max_images {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        summary: "provider screenshot count is within supported bounds".to_string(),
        details: Some(format!(
            "{} screenshots, expected {}..={}",
            image_files.len(),
            min_images,
            max_images
        )),
        remediation: vec![format!(
            "Render a provider screenshot set with between {min_images} and {max_images} images."
        )],
    });
    for path in image_files {
        validate_image_asset(provider, path, checks);
    }
    for path in video_files {
        validate_video_asset(provider, path, checks);
    }
}

fn rendered_asset_files(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    collect_rendered_asset_files(root, &mut files)?;
    Ok(files)
}

fn collect_rendered_asset_files(root: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_rendered_asset_files(&path, files)?;
        } else if is_release_asset(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn validate_image_asset(
    provider: DistributionProvider,
    path: &Path,
    checks: &mut Vec<LifecycleCheck>,
) {
    let id_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let allowed = provider_image_extensions(provider);
    checks.push(LifecycleCheck {
        id: format!(
            "release_content.{}.image.{id_stem}.format",
            provider.as_str()
        ),
        status: if allowed.contains(&ext.as_str()) {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        summary: "image format is accepted by the provider".to_string(),
        details: Some(path.display().to_string()),
        remediation: vec![format!("Use one of: {}.", allowed.join(", "))],
    });
    let size = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let max_bytes = provider_max_image_bytes(provider);
    checks.push(LifecycleCheck {
        id: format!("release_content.{}.image.{id_stem}.size", provider.as_str()),
        status: if size > 0 && size <= max_bytes {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        summary: "image file size is accepted by the provider".to_string(),
        details: Some(format!("{size} bytes; max {max_bytes} bytes")),
        remediation: vec![
            "Re-render the image at an accepted resolution/compression level.".to_string(),
        ],
    });
    match image_dimensions(path) {
        Ok(Some((width, height))) => {
            let valid = provider_dimension_check(provider, width, height);
            checks.push(LifecycleCheck {
                id: format!(
                    "release_content.{}.image.{id_stem}.dimensions",
                    provider.as_str()
                ),
                status: if valid { "passed" } else { "failed" }.to_string(),
                summary: "image dimensions are accepted by the provider".to_string(),
                details: Some(format!("{width}x{height}")),
                remediation: vec![
                    "Capture/render the screenshot at a provider-supported device size."
                        .to_string(),
                ],
            });
        }
        Ok(None) | Err(_) => checks.push(LifecycleCheck {
            id: format!(
                "release_content.{}.image.{id_stem}.dimensions",
                provider.as_str()
            ),
            status: "failed".to_string(),
            summary: "image dimensions can be read".to_string(),
            details: Some(path.display().to_string()),
            remediation: vec![
                "Replace the file with a valid PNG/JPEG/WebP screenshot asset.".to_string(),
            ],
        }),
    }
    if provider == DistributionProvider::AppStore {
        validate_app_store_display_type(path, id_stem, checks);
    }
}

fn validate_app_store_display_type(path: &Path, id_stem: &str, checks: &mut Vec<LifecycleCheck>) {
    let display_type = app_store_screenshot_display_type(path);
    checks.push(LifecycleCheck {
        id: format!("release_content.app-store.image.{id_stem}.display_type"),
        status: if display_type.is_some() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        summary: "App Store screenshot display type is explicit".to_string(),
        details: display_type
            .map(str::to_string)
            .or_else(|| Some(path.display().to_string())),
        remediation: vec![
            "Place each App Store screenshot under a display type directory such as APP_IPHONE_67, APP_IPHONE_65, APP_IPAD_PRO_3GEN_129, APP_IPAD_PRO_3GEN_11, or APP_DESKTOP.".to_string(),
        ],
    });
}

fn app_store_screenshot_display_type(path: &Path) -> Option<&'static str> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .find_map(app_store_screenshot_display_type_segment)
}

fn app_store_screenshot_display_type_segment(segment: &str) -> Option<&'static str> {
    match segment
        .to_ascii_uppercase()
        .replace(['-', '.', ' '], "_")
        .as_str()
    {
        "APP_IPHONE_67" | "IPHONE_67" | "IPHONE_6_7" => Some("APP_IPHONE_67"),
        "APP_IPHONE_65" | "IPHONE_65" | "IPHONE_6_5" => Some("APP_IPHONE_65"),
        "APP_IPHONE_61" | "IPHONE_61" | "IPHONE_6_1" => Some("APP_IPHONE_61"),
        "APP_IPHONE_58" | "IPHONE_58" | "IPHONE_5_8" => Some("APP_IPHONE_58"),
        "APP_IPAD_PRO_3GEN_129" | "IPAD_129" | "IPAD_12_9" | "TABLET_129" | "TABLET_12_9" => {
            Some("APP_IPAD_PRO_3GEN_129")
        }
        "APP_IPAD_PRO_3GEN_11" | "IPAD_11" | "TABLET_11" => Some("APP_IPAD_PRO_3GEN_11"),
        "APP_DESKTOP" | "MAC" | "DESKTOP" => Some("APP_DESKTOP"),
        _ => None,
    }
}

fn app_store_preview_type(path: &Path) -> Option<&'static str> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .find_map(app_store_preview_type_segment)
}

fn app_store_preview_type_segment(segment: &str) -> Option<&'static str> {
    match segment
        .to_ascii_uppercase()
        .replace(['-', '.', ' '], "_")
        .as_str()
    {
        "APP_IPHONE_67" | "IPHONE_67" | "IPHONE_6_7" => Some("IPHONE_67"),
        "APP_IPHONE_65" | "IPHONE_65" | "IPHONE_6_5" => Some("IPHONE_65"),
        "APP_IPHONE_61" | "IPHONE_61" | "IPHONE_6_1" => Some("IPHONE_61"),
        "APP_IPHONE_58" | "IPHONE_58" | "IPHONE_5_8" => Some("IPHONE_58"),
        "APP_IPAD_PRO_3GEN_129"
        | "IPAD_PRO_3GEN_129"
        | "IPAD_129"
        | "IPAD_12_9"
        | "TABLET_129"
        | "TABLET_12_9" => Some("IPAD_PRO_3GEN_129"),
        "APP_IPAD_PRO_3GEN_11" | "IPAD_PRO_3GEN_11" | "IPAD_11" | "TABLET_11" => {
            Some("IPAD_PRO_3GEN_11")
        }
        "APP_DESKTOP" | "DESKTOP" | "MAC" => Some("DESKTOP"),
        _ => None,
    }
}

fn validate_video_asset(
    provider: DistributionProvider,
    path: &Path,
    checks: &mut Vec<LifecycleCheck>,
) {
    let id_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("video");
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let allowed = provider_video_extensions(provider);
    checks.push(LifecycleCheck {
        id: format!(
            "release_content.{}.video.{id_stem}.format",
            provider.as_str()
        ),
        status: if allowed.contains(&ext.as_str()) {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        summary: "video format is accepted by the provider".to_string(),
        details: Some(path.display().to_string()),
        remediation: vec![format!("Use one of: {}.", allowed.join(", "))],
    });
}

fn validate_app_store_preview_assets(root: &Path, checks: &mut Vec<LifecycleCheck>) {
    let Ok(files) = rendered_asset_files(root) else {
        return;
    };
    for path in files.iter().filter(|path| asset_kind(path) == "video") {
        validate_video_asset(DistributionProvider::AppStore, path, checks);
        validate_app_store_preview_type(path, checks);
    }
}

fn validate_app_store_preview_type(path: &Path, checks: &mut Vec<LifecycleCheck>) {
    let id_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("preview");
    let preview_type = app_store_preview_type(path);
    checks.push(LifecycleCheck {
        id: format!("release_content.app-store.video.{id_stem}.preview_type"),
        status: if preview_type.is_some() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        summary: "App Store preview type is explicit".to_string(),
        details: preview_type
            .map(str::to_string)
            .or_else(|| Some(path.display().to_string())),
        remediation: vec![
            "Place each App Store preview under a preview type directory such as IPHONE_67, IPHONE_65, IPAD_PRO_3GEN_129, IPAD_PRO_3GEN_11, or DESKTOP.".to_string(),
        ],
    });
}

fn provider_screenshot_count(provider: DistributionProvider) -> (usize, usize) {
    match provider {
        DistributionProvider::AppStore => (1, 10),
        DistributionProvider::PlayStore => (2, 8),
        DistributionProvider::MicrosoftStore => (1, 10),
        _ => (1, usize::MAX),
    }
}

fn provider_image_extensions(provider: DistributionProvider) -> &'static [&'static str] {
    match provider {
        DistributionProvider::PlayStore => &["png", "jpg", "jpeg", "webp"],
        DistributionProvider::AppStore => &["png", "jpg", "jpeg"],
        DistributionProvider::MicrosoftStore => &["png", "jpg", "jpeg"],
        _ => &["png", "jpg", "jpeg", "webp"],
    }
}

fn provider_video_extensions(provider: DistributionProvider) -> &'static [&'static str] {
    match provider {
        DistributionProvider::AppStore => &["mov", "m4v", "mp4"],
        DistributionProvider::MicrosoftStore => &["mp4"],
        _ => &["mp4"],
    }
}

fn provider_max_image_bytes(provider: DistributionProvider) -> u64 {
    match provider {
        DistributionProvider::PlayStore => 8 * 1024 * 1024,
        DistributionProvider::AppStore => 10 * 1024 * 1024,
        DistributionProvider::MicrosoftStore => 50 * 1024 * 1024,
        _ => 10 * 1024 * 1024,
    }
}

fn provider_dimension_check(provider: DistributionProvider, width: u32, height: u32) -> bool {
    match provider {
        DistributionProvider::PlayStore => {
            let min = width.min(height);
            let max = width.max(height);
            min >= 320 && max <= 3840 && max <= min * 2
        }
        DistributionProvider::AppStore => width >= 320 && height >= 320,
        DistributionProvider::MicrosoftStore => width >= 1366 && height >= 768,
        _ => width > 0 && height > 0,
    }
}

fn check_optional_path(
    project_dir: &Path,
    id: &str,
    value: Option<&str>,
    summary: &str,
    checks: &mut Vec<LifecycleCheck>,
) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        checks.push(path_check(id, project_dir.join(value), summary));
    } else {
        checks.push(LifecycleCheck {
            id: id.to_string(),
            status: "missing".to_string(),
            summary: summary.to_string(),
            details: None,
            remediation: vec!["Configure the provider asset path in fission.toml.".to_string()],
        });
    }
}

fn count_assets(path: &Path) -> Result<usize> {
    let mut count = 0;
    if !path.exists() {
        return Ok(0);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            count += count_assets(&path)?;
        } else if is_release_asset(&path) {
            count += 1;
        }
    }
    Ok(count)
}

fn is_release_asset(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "mp4" | "mov" | "m4v")
    )
}

fn asset_kind(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "mov" | "m4v" => "video",
        _ => "image",
    }
}

fn image_dimensions(path: &Path) -> Result<Option<(u32, u32)>> {
    let bytes = fs::read(path)?;
    if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        return Ok(Some((width, height)));
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok(webp_dimensions(&bytes));
    }
    if bytes.len() >= 4 && bytes[0] == 0xff && bytes[1] == 0xd8 {
        return Ok(jpeg_dimensions(&bytes));
    }
    Ok(None)
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut index = 2usize;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        let marker = bytes[index];
        index += 1;
        if matches!(marker, 0xd8 | 0xd9 | 0x01) {
            continue;
        }
        if index + 2 > bytes.len() {
            return None;
        }
        let len = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
        if len < 2 || index + len > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) && len >= 7
        {
            let height = u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as u32;
            return Some((width, height));
        }
        index += len;
    }
    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    match bytes.get(12..16)? {
        b"VP8X" if bytes.len() >= 30 => {
            let width = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let height = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            Some((width, height))
        }
        b"VP8 " if bytes.len() >= 30 => {
            let width = u16::from_le_bytes([bytes[26], bytes[27]]) as u32 & 0x3fff;
            let height = u16::from_le_bytes([bytes[28], bytes[29]]) as u32 & 0x3fff;
            Some((width, height))
        }
        b"VP8L" if bytes.len() >= 25 => {
            let b0 = bytes[21] as u32;
            let b1 = bytes[22] as u32;
            let b2 = bytes[23] as u32;
            let b3 = bytes[24] as u32;
            let width = 1 + (((b1 & 0x3f) << 8) | b0);
            let height = 1 + (((b3 & 0x0f) << 10) | (b2 << 2) | ((b1 & 0xc0) >> 6));
            Some((width, height))
        }
        _ => None,
    }
}

fn required_text_check(id: &str, value: Option<&str>, summary: &str) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if value.is_some_and(|value| !value.trim().is_empty()) {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: summary.to_string(),
        details: value.map(str::to_string),
        remediation: vec!["Set the missing scenario field in fission.toml.".to_string()],
    }
}

fn load_content_config(project_dir: &Path) -> Result<ContentToml> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
