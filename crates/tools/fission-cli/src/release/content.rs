use super::*;
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize, Default)]
struct ContentToml {
    release: Option<ReleaseContentRoot>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseContentRoot {
    screenshots: Option<ScreenshotConfig>,
    assets: Option<ProviderAssets>,
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
    text: Option<String>,
    key: Option<String>,
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

pub(super) fn validate_release_content_model(
    project_dir: &Path,
    provider: Option<publish::DistributionProvider>,
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
    provider: publish::DistributionProvider,
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
        .user_agent("fission-cli-release-content/0.1")
        .build()?;
    wait_for_test_control(&client, port, timeout)?;
    checks.push(ok_check(
        &format!("release_content.capture.{id}.test_control_ready"),
        format!("http://127.0.0.1:{port}"),
    ));
    let mut saw_screenshot = false;
    for (index, step) in scenario.steps.iter().enumerate() {
        let response = send_test_command(&client, port, &step_payload(step, raw_dir, set, id)?)?;
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
    let _ = send_test_command(&client, port, &json!({"cmd": "Quit"}));
    Ok(())
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
    provider: Option<publish::DistributionProvider>,
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
        validate_rendered_asset_rules(provider, &provider_dir, checks);
    }
}

fn validate_provider_assets(
    project_dir: &Path,
    config: &ContentToml,
    provider: Option<publish::DistributionProvider>,
    checks: &mut Vec<LifecycleCheck>,
) {
    let assets = config
        .release
        .as_ref()
        .and_then(|release| release.assets.as_ref());
    match provider {
        Some(publish::DistributionProvider::PlayStore) => {
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
        Some(publish::DistributionProvider::AppStore) => {
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
                for path in &app.review_attachments {
                    checks.push(path_check(
                        "release_content.app_store.review_attachment",
                        project_dir.join(path),
                        "App Review attachment exists",
                    ));
                }
            }
        }
        Some(publish::DistributionProvider::MicrosoftStore) => {
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
        let dimensions = image_dimensions(&dest).ok().flatten();
        assets.push(RenderedAsset {
            kind: asset_kind(&dest).to_string(),
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
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
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
    provider: publish::DistributionProvider,
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
    provider: publish::DistributionProvider,
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
}

fn validate_video_asset(
    provider: publish::DistributionProvider,
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

fn provider_screenshot_count(provider: publish::DistributionProvider) -> (usize, usize) {
    match provider {
        publish::DistributionProvider::AppStore => (1, 10),
        publish::DistributionProvider::PlayStore => (2, 8),
        publish::DistributionProvider::MicrosoftStore => (1, 10),
        _ => (1, usize::MAX),
    }
}

fn provider_image_extensions(provider: publish::DistributionProvider) -> &'static [&'static str] {
    match provider {
        publish::DistributionProvider::PlayStore => &["png", "jpg", "jpeg", "webp"],
        publish::DistributionProvider::AppStore => &["png", "jpg", "jpeg"],
        publish::DistributionProvider::MicrosoftStore => &["png", "jpg", "jpeg"],
        _ => &["png", "jpg", "jpeg", "webp"],
    }
}

fn provider_video_extensions(provider: publish::DistributionProvider) -> &'static [&'static str] {
    match provider {
        publish::DistributionProvider::AppStore => &["mov", "m4v", "mp4"],
        publish::DistributionProvider::MicrosoftStore => &["mp4"],
        _ => &["mp4"],
    }
}

fn provider_max_image_bytes(provider: publish::DistributionProvider) -> u64 {
    match provider {
        publish::DistributionProvider::PlayStore => 8 * 1024 * 1024,
        publish::DistributionProvider::AppStore => 10 * 1024 * 1024,
        publish::DistributionProvider::MicrosoftStore => 50 * 1024 * 1024,
        _ => 10 * 1024 * 1024,
    }
}

fn provider_dimension_check(
    provider: publish::DistributionProvider,
    width: u32,
    height: u32,
) -> bool {
    match provider {
        publish::DistributionProvider::PlayStore => {
            let min = width.min(height);
            let max = width.max(height);
            min >= 320 && max <= 3840 && max <= min * 2
        }
        publish::DistributionProvider::AppStore => width >= 320 && height >= 320,
        publish::DistributionProvider::MicrosoftStore => width >= 1366 && height >= 768,
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
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fission-release-content-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_content_project(dir: &Path) {
        fs::create_dir_all(dir.join("release-content/screenshots/raw/en-US")).unwrap();
        fs::write(
            dir.join("release-content/screenshots/raw/en-US/home.png"),
            b"png",
        )
        .unwrap();
        fs::create_dir_all(dir.join("tests/release_screenshots")).unwrap();
        fs::write(
            dir.join("tests/release_screenshots/home.toml"),
            "wait = true\n",
        )
        .unwrap();
        fs::write(
            dir.join("fission.toml"),
            r#"[app]
name = "content-demo"
app_id = "com.example.content_demo"

[release.screenshots]
raw_dir = "release-content/screenshots/raw"
rendered_dir = "release-content/screenshots/rendered"

[[release.screenshots.scenarios]]
id = "home"
name = "Home"
targets = ["web"]
script = "tests/release_screenshots/home.toml"
wait_for = "semantic:home"

[release.assets.play_store]
screenshot_sets_dir = "release-content/screenshots/rendered/play-store"
feature_graphic = "release-content/screenshots/raw/en-US/home.png"
"#,
        )
        .unwrap();
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes
    }

    #[test]
    fn render_release_content_copies_raw_assets_and_writes_manifest() {
        let dir = unique_dir("render");
        write_content_project(&dir);
        let report =
            render_release_content(&dir, publish::DistributionProvider::PlayStore).unwrap();
        assert_ne!(report.status, "blocked");
        assert!(dir
            .join("release-content/screenshots/rendered/play-store/en-US/home.png")
            .exists());
        let manifest = dir
            .join("release-content/screenshots/rendered/play-store/release-content-manifest.json");
        assert!(manifest.exists());
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
        let sha = manifest["assets"][0]["sha256"].as_str().unwrap();
        assert_eq!(sha.len(), 64);
    }

    #[test]
    fn image_dimensions_reads_png_header() {
        let dir = unique_dir("png-dimensions");
        let path = dir.join("screen.png");
        fs::write(&path, png_header(1440, 2560)).unwrap();
        assert_eq!(image_dimensions(&path).unwrap(), Some((1440, 2560)));
    }

    #[test]
    fn provider_asset_validation_reports_dimensions() {
        let dir = unique_dir("asset-rules");
        let provider_dir = dir.join("release-content/screenshots/rendered/play-store/en-US");
        fs::create_dir_all(&provider_dir).unwrap();
        fs::write(provider_dir.join("one.png"), png_header(1440, 2560)).unwrap();
        fs::write(provider_dir.join("two.png"), png_header(1440, 2560)).unwrap();
        let mut checks = Vec::new();
        validate_rendered_asset_rules(
            publish::DistributionProvider::PlayStore,
            &dir.join("release-content/screenshots/rendered/play-store"),
            &mut checks,
        );
        assert!(checks.iter().any(|check| {
            check.id == "release_content.play-store.screenshot_count" && check.status == "passed"
        }));
        assert!(checks
            .iter()
            .any(|check| { check.id.ends_with(".dimensions") && check.status == "passed" }));
    }

    #[test]
    fn screenshot_step_payload_uses_test_control_protocol() {
        let step = ScreenshotStep {
            cmd: "tap_text".to_string(),
            text: Some("Save".to_string()),
            ..Default::default()
        };
        let payload = step_payload(&step, Path::new("/tmp"), "store", "save").unwrap();
        assert_eq!(payload["cmd"], "TapText");
        assert_eq!(payload["text"], "Save");
    }
}
