//! Attached browser preview lifecycle. JSON output is one event per line.

mod readiness;

use crate::{BuildOptions, WebCargoOptions};
use anyhow::{bail, Context, Result};
use fission_command_core::{read_project_config, Target};
use fission_command_process::{in_owned_process_tree, run_captured, CleanupError, ProcessSession};
use fission_command_site::preview::{normalize_mount, PreviewServer};
use serde::Serialize;
use std::{
    io::{self, BufRead, Read, Write},
    path::{Component, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;

#[derive(Clone, Debug)]
pub struct PreviewOptions {
    pub project_dir: PathBuf,
    pub target: Target,
    pub release: bool,
    pub web_cargo: WebCargoOptions,
    pub host: String,
    pub port: u16,
    pub mount: String,
    pub entry: String,
    pub startup_timeout: Duration,
    pub live_test: bool,
    pub json: bool,
    pub stdin_control: bool,
    pub open: bool,
}

#[derive(Serialize)]
struct Event<'a> {
    schema: &'static str,
    session: &'a str,
    owner_pid: u32,
    stage: &'a str,
    target: Target,
    mount: &'a str,
    url: &'a Option<String>,
    readiness: &'static str,
    live_test_ready: bool,
    detail: serde_json::Value,
}

struct Events {
    options: PreviewOptions,
    session: String,
    stage: &'static str,
    url: Option<String>,
    live_test_ready: bool,
    silent: bool,
}

impl Events {
    fn emit(&mut self, stage: &'static str, detail: serde_json::Value) -> Result<()> {
        self.stage = stage;
        if self.silent {
            return Ok(());
        }
        if self.options.json {
            let event = Event {
                schema: "fission.preview.v1",
                session: &self.session,
                owner_pid: std::process::id(),
                stage,
                target: self.options.target,
                mount: &self.options.mount,
                url: &self.url,
                readiness: if stage == "ready" && self.live_test_ready {
                    "live_test"
                } else if stage == "ready" {
                    "local_assets"
                } else {
                    "unverified"
                },
                live_test_ready: stage == "ready" && self.live_test_ready,
                detail,
            };
            let mut stdout = io::stdout().lock();
            serde_json::to_writer(&mut stdout, &event)?;
            writeln!(stdout)?;
            stdout.flush()?;
        } else {
            eprintln!("Preview {stage}: {detail}");
        }
        Ok(())
    }
}

/// Builds, binds, validates, and serves until Ctrl+C/SIGTERM or an opt-in stdin
/// stop request. No detached process, persisted PID, or control HTTP service.
pub fn run(options: PreviewOptions) -> Result<()> {
    let session = ProcessSession::new()?;
    let stop = Arc::new(AtomicBool::new(false));
    if options.stdin_control {
        read_stop_requests(stop.clone());
    }
    let session_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let mut events = Events {
        options,
        session: session_id,
        stage: "validating",
        url: None,
        live_test_ready: false,
        silent: false,
    };
    let mut cancelled = || {
        if session.interrupted() {
            stop.store(true, Ordering::Release);
        }
        stop.load(Ordering::Acquire)
    };
    let result = lifecycle(&mut events, &mut cancelled, |_, _, server, cancelled| {
        while !cancelled() {
            server.check_running()?;
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    });
    match result {
        Ok(()) => events.emit(
            "stopped",
            serde_json::json!({"reason": "stop_requested", "owned_resources_released": true}),
        ),
        Err(ref error)
            if stop.load(Ordering::Acquire) && error.downcast_ref::<CleanupError>().is_none() =>
        {
            events.emit(
                "stopped",
                serde_json::json!({"reason": "cancelled", "owned_resources_released": true}),
            )
        }
        Err(error) => {
            let released = error.downcast_ref::<CleanupError>().is_none();
            let failed_stage = events.stage;
            let retry: Vec<_> = std::iter::once("fission".to_string())
                .chain(
                    std::env::args_os()
                        .skip(1)
                        .map(|arg| arg.to_string_lossy().into_owned()),
                )
                .collect();
            events.emit("failed", serde_json::json!({
                "failed_stage": failed_stage, "message": format!("{error:#}"),
                "retry_argv": retry, "recovery": if released { "Correct the reported configuration, asset, tool, or build error and retry. For a bind failure use --port 0 or another port. This session has stopped its owned resources; no PID or port cleanup command is needed." } else { "Owned worker cleanup could not be confirmed. Inspect the reported OS error and this session's process tree before retrying; do not terminate unrelated listeners by port." },
                "owned_resources_released": released,
            }))?;
            Err(error)
        }
    }
}

/// Runs a finite operation using the same freshly built, verified, owned preview.
/// The callback cannot retain the server; cleanup happens on every return path.
pub(crate) fn with_preview(
    options: PreviewOptions,
    cancelled: &mut impl FnMut() -> bool,
    operation: impl FnOnce(
        &std::path::Path,
        &Url,
        &mut PreviewServer,
        &mut dyn FnMut() -> bool,
    ) -> Result<()>,
) -> Result<()> {
    let mut events = Events {
        options,
        session: String::new(),
        stage: "validating",
        url: None,
        live_test_ready: false,
        silent: true,
    };
    lifecycle(&mut events, cancelled, operation)
        .with_context(|| format!("review preview stage {}", events.stage))
}

fn lifecycle(
    events: &mut Events,
    cancelled: &mut impl FnMut() -> bool,
    operation: impl FnOnce(
        &std::path::Path,
        &Url,
        &mut PreviewServer,
        &mut dyn FnMut() -> bool,
    ) -> Result<()>,
) -> Result<()> {
    events.emit("validating", serde_json::json!({}))?;
    if !matches!(events.options.target, Target::Web | Target::Site) {
        bail!("preview supports only --target web or --target static-site");
    }
    if events.options.live_test && events.options.target != Target::Web {
        bail!("--live-test requires --target web; static previews have no test-control service");
    }
    if events.options.startup_timeout.is_zero()
        || events.options.startup_timeout > Duration::from_secs(3600)
    {
        bail!("startup timeout must be between 1 and 3600 seconds");
    }
    events.options.mount = normalize_mount(&events.options.mount)?;
    let entry_path = PathBuf::from(&events.options.entry);
    if entry_path.is_absolute()
        || !entry_path
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
        || entry_path.extension().is_none_or(|ext| ext != "html")
        || events
            .options
            .entry
            .bytes()
            .any(|b| !b.is_ascii_alphanumeric() && !b"/-_.~".contains(&b))
    {
        bail!("invalid preview entry; use a relative HTML path such as index.html or about/index.html");
    }
    events.options.project_dir = events
        .options
        .project_dir
        .canonicalize()
        .context("preview project directory does not exist")?;
    let project = read_project_config(&events.options.project_dir)?;
    crate::ensure_target_configured(&project, &events.options.project_dir, events.options.target)?;
    crate::ensure_web_cargo_feature_target(
        events.options.target,
        &events.options.web_cargo.features,
        events.options.web_cargo.no_default_features,
    )?;
    let root = match events.options.target {
        Target::Web => events.options.project_dir.join("platforms/web"),
        _ => fission_command_site::output_dir(&events.options.project_dir)?,
    };
    let deadline = Instant::now() + events.options.startup_timeout;
    events.emit(
        "building",
        serde_json::json!({"timeout_seconds": events.options.startup_timeout.as_secs()}),
    )?;
    let mut build = Command::new(std::env::current_exe()?);
    build
        .arg("preview-build")
        .arg("--target")
        .arg(events.options.target.as_str())
        .arg("--project-dir")
        .arg(&events.options.project_dir);
    if events.options.release {
        build.arg("--release");
    }
    if events.options.web_cargo.no_default_features {
        build.arg("--no-default-features");
    }
    if !events.options.web_cargo.features.is_empty() {
        build
            .arg("--features")
            .arg(events.options.web_cargo.features.join(","));
    }
    if events.options.live_test {
        build.env("FISSION_WEB_TEST_CONTROL", "1");
    } else {
        build.env_remove("FISSION_WEB_TEST_CONTROL");
    }
    let output = run_captured(
        &mut build,
        "preview build",
        remaining(deadline)?,
        &mut *cancelled,
    )?;
    if !events.options.json {
        eprint!("{output}");
    }
    if cancelled() {
        bail!("preview cancelled");
    }
    let entry = root.join(&events.options.entry);
    if !entry.is_file() {
        bail!("expected preview entry {} is missing after build; check the configured output directory and --entry", entry.display());
    }
    events.emit("serving", serde_json::json!({"output_dir": root}))?;
    let mut server = PreviewServer::start(
        root.clone(),
        &events.options.host,
        events.options.port,
        &events.options.mount,
        events.options.target == Target::Web,
    )?;
    let result = (|| {
        let mut address = server.address();
        if address.ip().is_unspecified() {
            address.set_ip(if address.is_ipv4() {
                std::net::Ipv4Addr::LOCALHOST.into()
            } else {
                std::net::Ipv6Addr::LOCALHOST.into()
            });
        }
        let base = Url::parse(&format!("http://{address}{}", events.options.mount))?;
        let url = if events.options.entry == "index.html" {
            base.clone()
        } else {
            base.join(&events.options.entry)?
        };
        events.url = Some(url.to_string());
        events.emit("verifying", serde_json::json!({}))?;
        let assets = readiness::verify(
            &root,
            &events.options.entry,
            &base,
            events.options.target == Target::Web,
            deadline,
            &mut *cancelled,
        )?;
        server.check_running()?;
        if events.options.live_test && !events.silent {
            events.emit("verifying_live_test", serde_json::json!({}))?;
            let mut probe = Command::new(std::env::current_exe()?);
            probe
                .arg("preview-probe")
                .arg("--url")
                .arg(url.as_str())
                .arg("--timeout-seconds")
                .arg(remaining(deadline)?.as_secs().max(1).to_string());
            run_captured(
                &mut probe,
                "preview LiveTest readiness",
                remaining(deadline)?,
                &mut *cancelled,
            )?;
            events.live_test_ready = true;
        }
        if cancelled() {
            bail!("preview cancelled");
        }
        let signals: &[&str] = if cfg!(unix) {
            &["SIGINT", "SIGTERM"]
        } else {
            &["Ctrl+C"]
        };
        events.emit("ready", serde_json::json!({"verified_local_assets": assets, "stop": {"stdin": if events.options.stdin_control { Some("stop\n") } else { None }, "signals": signals}}))?;
        if events.options.open {
            if let Err(error) = fission_command_site::open_url(url.as_str()) {
                eprintln!("Preview is ready; could not open browser: {error}");
            }
        }
        operation(&root, &base, &mut server, cancelled)
    })();
    server
        .stop()
        .context("failed to release owned preview server")?;
    result
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .context("preview startup timed out; retry with a larger --startup-timeout-seconds")
}

fn read_stop_requests(stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        loop {
            let mut line = String::new();
            match input.by_ref().take(256).read_line(&mut line) {
                Ok(0) | Err(_) => {
                    stop.store(true, Ordering::Release);
                    break;
                }
                Ok(_) if line.trim() == "stop" => {
                    stop.store(true, Ordering::Release);
                    break;
                }
                Ok(_) => {}
            }
        }
    });
}

/// Internal build worker; descendants inherit the parent's owned process tree.
pub fn build_worker(options: BuildOptions, web: WebCargoOptions) -> Result<()> {
    if !matches!(options.target, Some(Target::Web | Target::Site)) {
        bail!("preview build supports only web and static-site targets");
    }
    in_owned_process_tree(|| crate::build_app_with_web_cargo_options(options, web))
}

/// Internal opt-in probe. Readiness requires a running renderer and test bridge,
/// plus successful pump/tree commands, not merely an installed JS property.
pub fn probe_worker(url: String, timeout: Duration) -> Result<()> {
    let mut options = fission_test_driver::BrowserTestOptions::new(url).fission_canvas();
    options.timeout_ms = timeout.as_millis().min(u64::MAX as u128) as u64;
    let client = fission_test_driver::LiveTestClient::launch_browser(options)?;
    client.pump()?;
    client.get_tree()?;
    Ok(())
}
