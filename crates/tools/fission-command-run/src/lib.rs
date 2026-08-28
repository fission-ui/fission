pub mod doctor;

use anyhow::{bail, Context, Result};
use fission_command_core::{
    build_linux_native_modules, build_macos_native_modules, build_windows_native_modules,
    embed_and_sign_macos_native_modules, ensure_native_variant_target, ios_executable_name,
    normalized_extension, prepare_web_target, read_desktop_cargo_options,
    read_macos_run_config_for_profile, read_project_config, resolve_app_icon,
    sign_macos_app_if_configured, stage_linux_native_products, stage_project_assets,
    stage_windows_runtime_products, sync_platform_config, test_linux_native_modules,
    test_macos_native_modules, test_windows_native_modules, variant_output_path, FissionProject,
    MacosNativeBundleMode, MacosPackageConfig, NativeVariant, PlatformCapability, Target,
};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, IsTerminal, Read, Seek, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub target: Target,
    pub kind: String,
    pub status: String,
    pub detail: String,
    pub available: bool,
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub project_dir: PathBuf,
    pub target: Option<Target>,
    pub device: Option<String>,
    pub detach: bool,
    pub release: bool,
    pub variant: Option<NativeVariant>,
    pub host: String,
    pub port: u16,
    pub no_open: bool,
    pub headless: bool,
}

#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub project_dir: PathBuf,
    pub target: Option<Target>,
    pub release: bool,
    pub variant: Option<NativeVariant>,
}

#[derive(Clone, Debug)]
pub struct TestOptions {
    pub project_dir: PathBuf,
    pub target: Option<Target>,
    pub headless: bool,
    pub variant: Option<NativeVariant>,
}

#[derive(Clone, Debug)]
pub struct LogOptions {
    pub project_dir: PathBuf,
    pub target: Option<Target>,
    pub device: Option<String>,
    pub follow: bool,
}

#[derive(Clone, Debug)]
pub struct ServeWebOptions {
    pub project_dir: PathBuf,
    pub host: String,
    pub port: u16,
    pub open: bool,
}

pub fn list_devices(project_dir: &Path, json: bool) -> Result<()> {
    let devices = discover_devices(project_dir);
    if json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
        return Ok(());
    }

    println!("Fission devices");
    if devices.is_empty() {
        println!(
            "No runnable devices detected. Run `fission doctor web ios android --project-dir {}`.",
            project_dir.display()
        );
        return Ok(());
    }

    let id_width = devices
        .iter()
        .map(|device| device.id.len())
        .max()
        .unwrap_or(2)
        .max(2);
    let target_width = devices
        .iter()
        .map(|device| device.target.as_str().len())
        .max()
        .unwrap_or(6)
        .max(6);
    println!(
        "{:<id_width$}  {:<target_width$}  {:<16}  {:<12}  {}",
        "ID",
        "TARGET",
        "KIND",
        "STATUS",
        "NAME",
        id_width = id_width,
        target_width = target_width
    );
    for device in devices {
        println!(
            "{:<id_width$}  {:<target_width$}  {:<16}  {:<12}  {}{}",
            device.id,
            device.target.as_str(),
            device.kind,
            device.status,
            device.name,
            if device.detail.is_empty() {
                String::new()
            } else {
                format!(" ({})", device.detail)
            },
            id_width = id_width,
            target_width = target_width
        );
    }
    Ok(())
}

pub fn run_app(options: RunOptions) -> Result<()> {
    let project = read_project_config(&options.project_dir)?;
    let device = select_device(
        &options.project_dir,
        options.target,
        options.device.as_deref(),
    )?;
    ensure_native_variant_target(device.target, options.variant.as_ref())?;
    ensure_target_configured(&project, &options.project_dir, device.target)?;
    sync_target_platform_config(&options.project_dir, &project, device.target)?;

    match device.target {
        Target::Linux | Target::Macos | Target::Terminal | Target::Windows => {
            run_desktop(&project, &options, &device)
        }
        Target::Web => run_web(&options, &device),
        Target::Site => site_serve(
            &options.project_dir,
            options.release,
            options.host,
            options.port,
            !options.no_open,
        ),
        Target::Server => fission_command_server::serve(
            &options.project_dir,
            options.release,
            options.host,
            options.port,
        ),
        Target::Ios => run_ios(&project, &options, &device),
        Target::Android => run_android(&project, &options, &device),
    }
}

pub fn build_app(options: BuildOptions) -> Result<()> {
    let project = read_project_config(&options.project_dir)?;
    let target = options.target.unwrap_or_else(host_desktop_target);
    ensure_native_variant_target(target, options.variant.as_ref())?;
    ensure_target_configured(&project, &options.project_dir, target)?;
    sync_target_platform_config(&options.project_dir, &project, target)?;

    match target {
        Target::Linux => {
            require_desktop_host(target)?;
            build_desktop(
                &options.project_dir,
                options.release,
                target,
                options.variant.as_ref(),
            )?;
            build_linux_native_modules(
                &options.project_dir,
                &project,
                options.variant.as_ref(),
                options.release,
            )?;
            Ok(())
        }
        Target::Windows => {
            require_desktop_host(target)?;
            build_desktop(
                &options.project_dir,
                options.release,
                target,
                options.variant.as_ref(),
            )?;
            build_windows_native_modules(
                &options.project_dir,
                &project,
                options.variant.as_ref(),
                options.release,
            )?;
            Ok(())
        }
        Target::Macos => {
            require_desktop_host(target)?;
            build_desktop(
                &options.project_dir,
                options.release,
                target,
                options.variant.as_ref(),
            )?;
            build_macos_native_modules(
                &options.project_dir,
                &project,
                options.variant.as_ref(),
                options.release,
            )
        }
        Target::Terminal => build_desktop(
            &options.project_dir,
            options.release,
            target,
            options.variant.as_ref(),
        ),
        Target::Web => build_web(&options.project_dir, options.release),
        Target::Site => site_build(&options.project_dir, options.release),
        Target::Server => fission_command_server::build(&options.project_dir, options.release),
        Target::Ios => {
            require_host(Target::Ios)?;
            let script = options.project_dir.join("platforms/ios/package-sim.sh");
            let mut command = command_for_script(&script)?;
            command.current_dir(&options.project_dir);
            if options.release {
                command.env("IOS_SIM_PROFILE", "release");
            }
            run_status(&mut command, "iOS build")
        }
        Target::Android => {
            let apk = package_android(&options.project_dir, options.release)?;
            println!("{}", apk.display());
            Ok(())
        }
    }
}

pub fn test_app(options: TestOptions) -> Result<()> {
    let project = read_project_config(&options.project_dir)?;
    let target = options.target.unwrap_or_else(host_desktop_target);
    ensure_native_variant_target(target, options.variant.as_ref())?;
    ensure_target_configured(&project, &options.project_dir, target)?;
    sync_target_platform_config(&options.project_dir, &project, target)?;

    match target {
        Target::Linux => {
            require_desktop_host(target)?;
            let mut command = Command::new("cargo");
            command.arg("test").current_dir(&options.project_dir);
            run_status(&mut command, "desktop tests")?;
            test_linux_native_modules(&options.project_dir, &project, options.variant.as_ref())
        }
        Target::Windows => {
            require_desktop_host(target)?;
            let mut command = Command::new("cargo");
            command.arg("test").current_dir(&options.project_dir);
            run_status(&mut command, "desktop tests")?;
            test_windows_native_modules(&options.project_dir, &project, options.variant.as_ref())
        }
        Target::Macos => {
            require_desktop_host(target)?;
            let mut command = Command::new("cargo");
            command.arg("test").current_dir(&options.project_dir);
            run_status(&mut command, "desktop tests")?;
            test_macos_native_modules(&options.project_dir, &project, options.variant.as_ref())
        }
        Target::Terminal => {
            let mut command = Command::new("cargo");
            command.arg("test").current_dir(&options.project_dir);
            run_status(&mut command, "terminal tests")
        }
        Target::Web => browser_test_web(&options.project_dir),
        Target::Site => browser_test_site(&options.project_dir),
        Target::Server => browser_test_server(&options.project_dir),
        Target::Ios => {
            require_host(Target::Ios)?;
            run_target_script(
                &options.project_dir,
                "platforms/ios/test-sim.sh",
                |command| {
                    if options.headless {
                        command.env("IOS_SIM_HEADLESS", "1");
                    }
                },
            )
        }
        Target::Android => run_target_script(
            &options.project_dir,
            "platforms/android/test-emulator.sh",
            |command| {
                if options.headless {
                    command.env("ANDROID_EMULATOR_HEADLESS", "1");
                }
            },
        ),
    }
}

fn sync_target_platform_config(
    project_dir: &Path,
    project: &FissionProject,
    target: Target,
) -> Result<()> {
    match target {
        Target::Android | Target::Ios => sync_platform_config(project_dir, project)?,
        Target::Web => prepare_web_target(project_dir, project)?,
        _ => {}
    }
    Ok(())
}

pub fn attach_logs(options: LogOptions) -> Result<()> {
    let project = read_project_config(&options.project_dir)?;
    let device = select_device(
        &options.project_dir,
        options.target,
        options.device.as_deref(),
    )?;
    match device.target {
        Target::Android => attach_android_logs(&project, &device, options.follow),
        Target::Ios => attach_ios_logs(&project, &device, options.follow),
        Target::Web => tail_log_file(
            &detached_log_path(&options.project_dir, "web"),
            options.follow,
        ),
        Target::Site => tail_log_file(
            &detached_log_path(&options.project_dir, Target::Site.as_str()),
            options.follow,
        ),
        Target::Server => tail_log_file(
            &detached_log_path(&options.project_dir, Target::Server.as_str()),
            options.follow,
        ),
        Target::Linux | Target::Macos | Target::Terminal | Target::Windows => tail_log_file(
            &detached_log_path(&options.project_dir, "desktop"),
            options.follow,
        ),
    }
}

pub fn serve_web(options: ServeWebOptions) -> Result<()> {
    fission_command_site::serve_web_app(
        options.project_dir.join("platforms/web"),
        options.host,
        options.port,
        options.open,
    )
}

pub fn site_build(project_dir: &Path, release: bool) -> Result<()> {
    fission_command_site::build(project_dir, release)
}

pub fn site_check(project_dir: &Path, release: bool) -> Result<()> {
    fission_command_site::check(project_dir, release)
}

pub fn site_routes(project_dir: &Path) -> Result<()> {
    fission_command_site::routes(project_dir)
}

pub fn site_serve(
    project_dir: &Path,
    release: bool,
    host: String,
    port: u16,
    open: bool,
) -> Result<()> {
    fission_command_site::serve(project_dir, release, host, port, open)
}

fn browser_test_web(project_dir: &Path) -> Result<()> {
    build_web_for_test(project_dir)?;
    let server = StaticTestServer::start(project_dir.to_path_buf())?;
    let url = format!("{}/platforms/web/", server.base_url());
    let client = fission_test_driver::LiveTestClient::launch_browser(
        fission_test_driver::BrowserTestOptions::new(url).fission_canvas(),
    )?;
    client.pump()?;
    let semantic_nodes = client.get_tree()?.len();
    let report = client
        .browser_report()
        .context("browser test client did not return a readiness report")?;
    println!(
        "Web browser live test passed: renderer={} canvas={}x{} semantic_nodes={}",
        report.renderer.unwrap_or_else(|| "unknown".into()),
        report.width,
        report.height,
        semantic_nodes
    );
    Ok(())
}

fn browser_test_site(project_dir: &Path) -> Result<()> {
    match fission_command_site::build_for_browser_test(project_dir, false)? {
        Some(output_dir) => {
            let server = StaticTestServer::start(output_dir)?;
            let report = fission_test_driver::run_browser_smoke(
                fission_test_driver::BrowserTestOptions::new(format!("{}/", server.base_url())),
            )?;
            println!(
                "Static site browser smoke passed: title=\"{}\" body_text_len={}",
                report.title, report.body_text_len
            );
            Ok(())
        }
        None => {
            println!("Custom static site entry built; run its project-specific browser tests for route coverage.");
            Ok(())
        }
    }
}

fn browser_test_server(project_dir: &Path) -> Result<()> {
    let port = free_local_port()?;
    let mut child = ServerChild::new(fission_command_server::spawn_serve(
        project_dir,
        false,
        "127.0.0.1".to_string(),
        port,
    )?);
    let url = format!("http://127.0.0.1:{port}/");
    wait_for_http(&url, Duration::from_secs(60))?;
    let report = fission_test_driver::run_browser_smoke(
        fission_test_driver::BrowserTestOptions::new(url.clone()),
    )?;
    println!(
        "SSR browser smoke passed: title=\"{}\" body_text_len={}",
        report.title, report.body_text_len
    );
    child.kill();
    Ok(())
}

struct ServerChild {
    child: Option<Child>,
}

impl ServerChild {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ServerChild {
    fn drop(&mut self) {
        self.kill();
    }
}

struct StaticTestServer {
    base_url: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl StaticTestServer {
    fn start(root: PathBuf) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(error) = stream.set_nonblocking(false) {
                            eprintln!("Web test server connection setup failed: {error}");
                            continue;
                        }
                        if let Err(error) = serve_static_test_request(stream, &root) {
                            eprintln!("Web test server request failed: {error}");
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(error) => {
                        eprintln!("Web test server accept failed: {error}");
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        });
        Ok(Self {
            base_url: format!("http://127.0.0.1:{port}"),
            stop,
            handle: Some(handle),
        })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for StaticTestServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = TcpStream::connect(self.base_url.trim_start_matches("http://"));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn serve_static_test_request(mut stream: TcpStream, root: &Path) -> Result<()> {
    let mut buffer = [0u8; 4096];
    let n = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..n]);
    let mut request_parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    let method = request_parts.next().unwrap_or("GET");
    let path = request_parts
        .next()
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    if method == "POST" && path == "/__fission/renderer" {
        stream.write_all(
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )?;
        return Ok(());
    }
    let relative = path.trim_start_matches('/');
    let mut candidate = root.join(relative);
    if path.ends_with('/') || candidate.is_dir() {
        candidate = candidate.join("index.html");
    }
    let (status, content_type, body) = if candidate.is_file() {
        (
            "200 OK",
            static_content_type(&candidate),
            fs::read(&candidate).unwrap_or_default(),
        )
    } else {
        ("404 Not Found", "text/plain", b"not found".to_vec())
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&body)?;
    Ok(())
}

fn static_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

fn free_local_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

fn wait_for_http(url: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match ureq::get(url).call() {
            Ok(response) if response.status() < 500 => return Ok(()),
            Ok(response) => last_error = Some(format!("HTTP {}", response.status())),
            Err(error) => last_error = Some(error.to_string()),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!(
        "timed out waiting for {url}: {}",
        last_error.unwrap_or_else(|| "no response".into())
    )
}

pub fn discover_devices(_project_dir: &Path) -> Vec<Device> {
    let mut devices = Vec::new();
    devices.push(Device {
        id: "desktop".to_string(),
        name: desktop_name().to_string(),
        target: host_desktop_target(),
        kind: "desktop".to_string(),
        status: "available".to_string(),
        detail: current_os_detail(),
        available: true,
    });
    devices.push(Device {
        id: "terminal".to_string(),
        name: "Current terminal".to_string(),
        target: Target::Terminal,
        kind: "terminal".to_string(),
        status: "available".to_string(),
        detail: current_os_detail(),
        available: true,
    });

    devices.push(if let Some(chrome) = detect_chrome() {
        Device {
            id: "chrome".to_string(),
            name: "Chrome/Chromium".to_string(),
            target: Target::Web,
            kind: "browser".to_string(),
            status: "available".to_string(),
            detail: chrome.display().to_string(),
            available: true,
        }
    } else {
        Device {
            id: "web-server".to_string(),
            name: "Local web server".to_string(),
            target: Target::Web,
            kind: "web-server".to_string(),
            status: "available".to_string(),
            detail: "Chrome/Chromium was not auto-detected".to_string(),
            available: true,
        }
    });

    devices.extend(discover_ios_simulators());
    devices.extend(discover_android_devices());
    devices.push(Device {
        id: Target::Site.as_str().to_string(),
        name: "Static site".to_string(),
        target: Target::Site,
        kind: "static-site-server".to_string(),
        status: "available".to_string(),
        detail: "multi-page static output".to_string(),
        available: true,
    });
    devices.push(Device {
        id: Target::Server.as_str().to_string(),
        name: "SSR web app".to_string(),
        target: Target::Server,
        kind: "ssr-server".to_string(),
        status: "available".to_string(),
        detail: "dynamic HTML server".to_string(),
        available: true,
    });
    devices
}

fn select_device(
    project_dir: &Path,
    target: Option<Target>,
    query: Option<&str>,
) -> Result<Device> {
    let devices = discover_devices(project_dir)
        .into_iter()
        .filter(|device| target.map(|target| target == device.target).unwrap_or(true))
        .collect::<Vec<_>>();

    if let Some(query) = query {
        let query_lower = query.to_ascii_lowercase();
        let mut matches = devices
            .iter()
            .filter(|device| {
                device.id.eq_ignore_ascii_case(query)
                    || device.id.to_ascii_lowercase().starts_with(&query_lower)
                    || device.name.eq_ignore_ascii_case(query)
                    || device.name.to_ascii_lowercase().contains(&query_lower)
                    || device.target.as_str() == query_lower
            })
            .cloned()
            .collect::<Vec<_>>();
        if matches.iter().any(|device| !device.available) {
            matches.retain(|device| device.available);
            if matches.is_empty() {
                bail!("device selector `{query}` matched a device that is not currently runnable");
            }
        }
        return match matches.len() {
            0 => bail!(
                "no device matched `{query}`; run `fission devices --project-dir {}`",
                project_dir.display()
            ),
            1 => Ok(matches[0].clone()),
            _ => {
                bail!("device selector `{query}` matched multiple devices; use an exact device id")
            }
        };
    }

    let devices = devices
        .into_iter()
        .filter(|device| device.available)
        .collect::<Vec<_>>();

    if devices.is_empty() {
        bail!(
            "no runnable devices detected; run `fission devices --project-dir {}`",
            project_dir.display()
        );
    }
    if devices.len() == 1 {
        return Ok(devices[0].clone());
    }

    if let Some(device) = preferred_device_for_target(target, &devices) {
        return Ok(device);
    }

    if !io::stdin().is_terminal() {
        print_device_choices(&devices);
        if let Some(target) = target {
            bail!(
                "multiple {} devices are available; pass `--device <id>`",
                target.as_str()
            );
        }
        bail!("multiple devices are available; pass `--device <id>` or `--target <target>`");
    }

    print_device_choices(&devices);
    print!("Select a device: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let index = line
        .trim()
        .parse::<usize>()
        .context("expected a numbered device selection")?;
    if index == 0 || index > devices.len() {
        bail!("device selection {index} is out of range");
    }
    Ok(devices[index - 1].clone())
}

fn preferred_device_for_target(target: Option<Target>, devices: &[Device]) -> Option<Device> {
    match target? {
        Target::Android => {
            let running = devices
                .iter()
                .filter(|device| {
                    matches!(device.kind.as_str(), "android-device" | "android-emulator")
                })
                .cloned()
                .collect::<Vec<_>>();
            if running.len() == 1 {
                return Some(running[0].clone());
            }
            let avds = devices
                .iter()
                .filter(|device| device.kind == "android-avd")
                .cloned()
                .collect::<Vec<_>>();
            if running.is_empty() && avds.len() == 1 {
                return Some(avds[0].clone());
            }
            None
        }
        Target::Ios => {
            let booted = devices
                .iter()
                .filter(|device| device.kind == "ios-simulator" && device.status == "booted")
                .cloned()
                .collect::<Vec<_>>();
            (booted.len() == 1).then(|| booted[0].clone())
        }
        Target::Web
        | Target::Site
        | Target::Server
        | Target::Linux
        | Target::Macos
        | Target::Terminal
        | Target::Windows => None,
    }
}

fn print_device_choices(devices: &[Device]) {
    println!("Available devices:");
    for (idx, device) in devices.iter().enumerate() {
        println!(
            "  {}) {} [{}:{}] - {}",
            idx + 1,
            device.name,
            device.target.as_str(),
            device.id,
            device.status
        );
    }
}

fn ensure_target_configured(
    project: &FissionProject,
    project_dir: &Path,
    target: Target,
) -> Result<()> {
    if !project.targets.contains(&target) {
        bail!(
            "target `{}` is not configured for this app; run `fission add-target {} --project-dir {}`",
            target.as_str(),
            target.as_str(),
            project_dir.display()
        );
    }
    let scaffold = project_dir.join(target.scaffold_relative_path());
    if !scaffold.exists() {
        bail!(
            "target `{}` scaffold is missing at {}; run `fission add-target {} --project-dir {}`",
            target.as_str(),
            scaffold.display(),
            target.as_str(),
            project_dir.display()
        );
    }
    Ok(())
}

fn run_desktop(project: &FissionProject, options: &RunOptions, device: &Device) -> Result<()> {
    if matches!(device.target, Target::Macos) && cfg!(target_os = "macos") {
        let app = package_macos_run_app(
            project,
            &options.project_dir,
            options.variant.as_ref(),
            options.release,
        )?;
        return run_macos_app_bundle(&app, options);
    }
    if matches!(device.target, Target::Linux) && cfg!(target_os = "linux") {
        let app = package_linux_run_app(
            project,
            &options.project_dir,
            options.variant.as_ref(),
            options.release,
        )?;
        let mut command = Command::new(&app.executable);
        command.current_dir(&options.project_dir);
        return run_child(
            command,
            options.detach,
            detached_log_path(&options.project_dir, "desktop"),
        );
    }
    if matches!(device.target, Target::Windows) && cfg!(target_os = "windows") {
        let app = package_windows_run_app(
            project,
            &options.project_dir,
            options.variant.as_ref(),
            options.release,
        )?;
        let mut command = Command::new(&app.executable);
        command.current_dir(&options.project_dir);
        return run_child(
            command,
            options.detach,
            detached_log_path(&options.project_dir, "desktop"),
        );
    }

    let mut command = Command::new("cargo");
    command.arg("run").current_dir(&options.project_dir);
    if options.release {
        command.arg("--release");
    }
    let cargo_options = read_desktop_cargo_options(
        &options.project_dir,
        device.target,
        options.variant.as_ref().map(NativeVariant::as_str),
    )?;
    if cargo_options.no_default_features {
        command.arg("--no-default-features");
    }
    if !cargo_options.features.is_empty() {
        command
            .arg("--features")
            .arg(cargo_options.features.join(","));
    }
    run_child(
        command,
        options.detach,
        detached_log_path(&options.project_dir, "desktop"),
    )
}

fn run_web(options: &RunOptions, _device: &Device) -> Result<()> {
    build_web(&options.project_dir, options.release)?;
    let open = !options.no_open;
    let port = available_web_port(&options.host, options.port)?;
    if options.detach {
        let log_path = detached_log_path(&options.project_dir, "web");
        let log = open_log(&log_path)?;
        let mut command = Command::new(env::current_exe()?);
        command
            .arg("serve-web")
            .arg("--project-dir")
            .arg(&options.project_dir)
            .arg("--host")
            .arg(&options.host)
            .arg("--port")
            .arg(port.to_string());
        if open {
            command.arg("--open");
        }
        let err = log.try_clone()?;
        let child = command
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err))
            .spawn()?;
        println!(
            "Started web server pid {} at {}. Logs: {}",
            child.id(),
            format!("http://{}:{port}/", options.host),
            log_path.display()
        );
        return Ok(());
    }

    fission_command_site::serve_web_app(
        options.project_dir.join("platforms/web"),
        options.host.clone(),
        port,
        open,
    )
}

fn available_web_port(host: &str, requested: u16) -> Result<u16> {
    const SEARCH_LIMIT: u16 = 50;
    let mut first_error = None;
    for offset in 0..=SEARCH_LIMIT {
        let Some(port) = requested.checked_add(offset) else {
            break;
        };
        match TcpListener::bind((host, port)) {
            Ok(listener) => {
                drop(listener);
                if offset > 0 {
                    eprintln!(
                        "Port {host}:{requested} is already in use; using {host}:{port}. Pass `--port {port}` to make this explicit."
                    );
                }
                return Ok(port);
            }
            Err(error) if offset == 0 => {
                first_error = Some(error);
            }
            Err(_) => {}
        }
    }
    if let Some(error) = first_error {
        bail!(
            "failed to find an available web port from {host}:{requested}: first bind failed with {error}"
        );
    }
    bail!("failed to find an available web port from {host}:{requested}");
}

fn run_ios(project: &FissionProject, options: &RunOptions, device: &Device) -> Result<()> {
    require_host(Target::Ios)?;
    let script = options.project_dir.join("platforms/ios/run-sim.sh");
    let mut command = command_for_script(&script)?;
    command.current_dir(&options.project_dir);
    if device.kind == "ios-simulator" {
        command.env("IOS_SIM_DEVICE_ID", &device.id);
    }
    if options.headless || options.no_open {
        command.env("IOS_SIM_HEADLESS", "1");
    }
    if options.release {
        command.env("IOS_SIM_PROFILE", "release");
    }
    run_status(&mut command, "iOS launch")?;
    if !options.detach {
        attach_ios_logs(project, device, true)?;
    }
    Ok(())
}

fn run_android(project: &FissionProject, options: &RunOptions, device: &Device) -> Result<()> {
    if device.kind == "android-avd" {
        let script = options
            .project_dir
            .join("platforms/android/run-emulator.sh");
        let mut command = command_for_script(&script)?;
        command.current_dir(&options.project_dir);
        command.env(
            "ANDROID_AVD_NAME",
            device.id.trim_start_matches("android-avd:"),
        );
        if options.headless || options.no_open {
            command.env("ANDROID_EMULATOR_HEADLESS", "1");
        }
        if options.release {
            command.env("ANDROID_PROFILE", "release");
        }
        run_status(&mut command, "Android emulator launch")?;
        if !options.detach {
            let serial = first_android_serial().unwrap_or_else(|| "emulator-5554".to_string());
            let running = Device {
                id: serial,
                kind: "android-emulator".to_string(),
                ..device.clone()
            };
            attach_android_logs(project, &running, true)?;
        }
        return Ok(());
    }

    let apk = package_android(&options.project_dir, options.release)?;
    let adb = adb_path()?;
    run_status(
        Command::new(&adb)
            .arg("-s")
            .arg(&device.id)
            .arg("install")
            .arg("--no-streaming")
            .arg("-r")
            .arg(&apk),
        "Android install",
    )?;
    run_status(
        Command::new(&adb)
            .arg("-s")
            .arg(&device.id)
            .arg("shell")
            .arg("am")
            .arg("start")
            .arg("-n")
            .arg(format!(
                "{}/rs.fission.runtime.FissionActivity",
                project.app.app_id
            )),
        "Android launch",
    )?;
    if !options.detach {
        attach_android_logs(project, device, true)?;
    }
    Ok(())
}

fn attach_ios_logs(project: &FissionProject, device: &Device, follow: bool) -> Result<()> {
    require_host(Target::Ios)?;
    let executable = ios_executable_name(project);
    let predicate = format!("process == \"{}\"", executable);
    let mut command = Command::new("xcrun");
    command
        .arg("simctl")
        .arg("spawn")
        .arg(&device.id)
        .arg("log")
        .arg(if follow { "stream" } else { "show" })
        .arg("--style")
        .arg("compact")
        .arg("--predicate")
        .arg(predicate);
    println!(
        "Attaching iOS logs for {}. Press Ctrl+C to stop.",
        executable
    );
    run_status(&mut command, "iOS logs")
}

fn attach_android_logs(project: &FissionProject, device: &Device, follow: bool) -> Result<()> {
    let adb = adb_path()?;
    let pid = wait_for_android_pid(
        &adb,
        &device.id,
        &project.app.app_id,
        Duration::from_secs(20),
    )?;
    let mut command = Command::new(&adb);
    command
        .arg("-s")
        .arg(&device.id)
        .arg("logcat")
        .arg("--pid")
        .arg(pid);
    if !follow {
        command.arg("-d");
    }
    println!(
        "Attaching Android logs for {} on {}. Press Ctrl+C to stop.",
        project.app.app_id, device.id
    );
    run_status(&mut command, "Android logs")
}

fn tail_log_file(path: &Path, follow: bool) -> Result<()> {
    if !path.exists() {
        bail!(
            "no detached log file found at {}; run with `--detach` first",
            path.display()
        );
    }
    if !follow {
        print!("{}", fs::read_to_string(path)?);
        return Ok(());
    }

    println!("Following {}. Press Ctrl+C to stop.", path.display());
    let mut offset = 0u64;
    loop {
        let mut file = File::open(path)?;
        file.seek_relative(offset as i64)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        if !buf.is_empty() {
            print!("{}", buf);
            io::stdout().flush()?;
            offset += buf.len() as u64;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn build_web(project_dir: &Path, release: bool) -> Result<()> {
    build_web_with_test_control(project_dir, release, false)
}

fn build_web_for_test(project_dir: &Path) -> Result<()> {
    build_web_with_test_control(project_dir, false, true)
}

fn build_web_with_test_control(
    project_dir: &Path,
    release: bool,
    test_control: bool,
) -> Result<()> {
    let project_dir = fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })?;
    let out_dir = project_dir.join("platforms/web/pkg");
    let mut command = Command::new("wasm-pack");
    command
        .arg("build")
        .arg(&project_dir)
        .arg("--target")
        .arg("web")
        .arg("--out-dir")
        .arg(out_dir);
    command.arg(if release { "--release" } else { "--dev" });
    if test_control {
        command.env("FISSION_WEB_TEST_CONTROL", "1");
    }
    run_status(&mut command, "web build")
}

fn package_android(project_dir: &Path, release: bool) -> Result<PathBuf> {
    let script = project_dir.join("platforms/android/package-apk.sh");
    let mut command = command_for_script(&script)?;
    command.current_dir(project_dir);
    if release {
        command.env("ANDROID_PROFILE", "release");
    }
    let output = command
        .output()
        .context("failed to run Android package script")?;
    if !output.status.success() {
        io::stderr().write_all(&output.stderr).ok();
        bail!("Android package failed with {}", output.status);
    }
    io::stderr().write_all(&output.stderr).ok();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let apk = stdout
        .lines()
        .rev()
        .find(|line| line.trim_end().ends_with(".apk"))
        .map(|line| PathBuf::from(line.trim()))
        .context("Android package script did not print an APK path")?;
    Ok(apk)
}

fn run_child(mut command: Command, detach: bool, log_path: PathBuf) -> Result<()> {
    if detach {
        let log = open_log(&log_path)?;
        let err = log.try_clone()?;
        let child = command
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err))
            .spawn()?;
        println!("Started pid {}. Logs: {}", child.id(), log_path.display());
        return Ok(());
    }
    let status = command.status()?;
    if !status.success() {
        bail!("command exited with {status}");
    }
    Ok(())
}

#[derive(Debug)]
struct DesktopBinary {
    version: String,
    executable_name: String,
    path: PathBuf,
}

#[derive(Debug)]
struct MacosRunApp {
    bundle: PathBuf,
}

#[derive(Debug)]
struct DesktopRunApp {
    executable: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoMetadataPackage>,
    target_directory: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataPackage {
    name: String,
    version: String,
    manifest_path: PathBuf,
    targets: Vec<CargoMetadataTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataTarget {
    name: String,
    kind: Vec<String>,
}

fn build_desktop_binary(
    project_dir: &Path,
    release: bool,
    target: Target,
    variant: Option<&NativeVariant>,
) -> Result<DesktopBinary> {
    let project_dir = fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })?;
    let metadata = cargo_metadata(&project_dir)?;
    let manifest_path = fs::canonicalize(project_dir.join("Cargo.toml")).with_context(|| {
        format!(
            "failed to resolve Cargo manifest at {}",
            project_dir.join("Cargo.toml").display()
        )
    })?;
    let package = desktop_package_for_manifest(&metadata, &manifest_path)?;
    let cargo_options =
        read_desktop_cargo_options(&project_dir, target, variant.map(NativeVariant::as_str))?;
    let executable_name = package
        .targets
        .iter()
        .find(|target| target.kind.iter().any(|kind| kind == "bin") && target.name == package.name)
        .or_else(|| {
            package
                .targets
                .iter()
                .find(|target| target.kind.iter().any(|kind| kind == "bin"))
        })
        .map(|target| target.name.clone())
        .with_context(|| {
            format!(
                "Cargo package `{}` does not define a binary target",
                package.name
            )
        })?;

    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(project_dir.join("Cargo.toml"))
        .arg("--package")
        .arg(&package.name)
        .current_dir(project_dir);
    if release {
        command.arg("--release");
    }
    if cargo_options.no_default_features {
        command.arg("--no-default-features");
    }
    if !cargo_options.features.is_empty() {
        command
            .arg("--features")
            .arg(cargo_options.features.join(","));
    }
    run_status(&mut command, "desktop build")?;

    let profile = if release { "release" } else { "debug" };
    let path = metadata
        .target_directory
        .join(profile)
        .join(platform_executable_name(&executable_name));
    if !path.exists() {
        bail!(
            "desktop build completed but expected binary is missing at {}",
            path.display()
        );
    }

    Ok(DesktopBinary {
        version: package.version.clone(),
        executable_name,
        path,
    })
}

fn desktop_package_for_manifest<'a>(
    metadata: &'a CargoMetadata,
    manifest_path: &Path,
) -> Result<&'a CargoMetadataPackage> {
    metadata
        .packages
        .iter()
        .find(|package| package.manifest_path == manifest_path)
        .or_else(|| {
            metadata
                .packages
                .iter()
                .find(|package| paths_refer_to_same_file(&package.manifest_path, manifest_path))
        })
        .with_context(|| {
            format!(
                "cargo metadata did not include the project package at {}",
                manifest_path.display()
            )
        })
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn cargo_metadata(project_dir: &Path) -> Result<CargoMetadata> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .current_dir(project_dir)
        .output()
        .context("failed to run cargo metadata")?;
    if !output.status.success() {
        io::stderr().write_all(&output.stderr).ok();
        bail!("cargo metadata failed with {}", output.status);
    }
    serde_json::from_slice(&output.stdout).context("failed to parse cargo metadata")
}

fn package_macos_run_app(
    project: &FissionProject,
    project_dir: &Path,
    variant: Option<&NativeVariant>,
    release: bool,
) -> Result<MacosRunApp> {
    let project_dir = fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })?;
    let macos = read_macos_run_config_for_profile(&project_dir, release)?;
    let binary = build_desktop_binary(&project_dir, release, Target::Macos, variant)?;
    let profile = if release { "release" } else { "debug" };
    let app_name = macos_display_name(&project.app.name);
    let app_bundle = variant_output_path(
        macos_run_staging_root(&project_dir, project, profile),
        variant,
    )
    .join(format!("{app_name}.app"));
    let contents = app_bundle.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources_dir = contents.join("Resources");
    if app_bundle.exists() {
        fs::remove_dir_all(&app_bundle).with_context(|| {
            format!(
                "failed to clear previous macOS run bundle at {}",
                app_bundle.display()
            )
        })?;
    }
    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;

    let executable = macos_dir.join(&binary.executable_name);
    fs::copy(&binary.path, &executable).with_context(|| {
        format!(
            "failed to copy {} into macOS app bundle",
            binary.path.display()
        )
    })?;
    if let Some(icon) = resolve_app_icon(&project_dir, Target::Macos)? {
        let extension = normalized_extension(&icon.path)?;
        let destination = resources_dir.join(format!("AppIcon.{extension}"));
        fs::copy(&icon.path, &destination).with_context(|| {
            format!(
                "failed to copy macOS app icon {} to {}",
                icon.path.display(),
                destination.display()
            )
        })?;
    }
    stage_project_assets(&project_dir, &resources_dir)?;

    fs::write(
        contents.join("Info.plist"),
        render_macos_run_info_plist(project, &binary, &app_name, &macos),
    )?;
    fs::write(contents.join("PkgInfo"), "APPL????")?;
    embed_and_sign_macos_native_modules(
        &project_dir,
        &app_bundle,
        project,
        variant,
        &macos,
        MacosNativeBundleMode::Run,
        release,
    )?;
    sign_macos_app_if_configured(&project_dir, &app_bundle, &macos)?;

    Ok(MacosRunApp { bundle: app_bundle })
}

fn macos_run_staging_root(project_dir: &Path, project: &FissionProject, profile: &str) -> PathBuf {
    let mut project_hash = DefaultHasher::new();
    project_dir.hash(&mut project_hash);
    let project_key = format!(
        "{}-{:016x}",
        sanitize_file_stem(&project.app.app_id),
        project_hash.finish()
    );

    env::temp_dir()
        .join("fission")
        .join("run")
        .join("macos")
        .join(project_key)
        .join(profile)
}

fn package_linux_run_app(
    project: &FissionProject,
    project_dir: &Path,
    variant: Option<&NativeVariant>,
    release: bool,
) -> Result<DesktopRunApp> {
    let project_dir = fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })?;
    let binary = build_desktop_binary(&project_dir, release, Target::Linux, variant)?;
    let profile = if release { "release" } else { "debug" };
    let app_root = variant_output_path(
        project_dir.join(".fission/run/linux").join(profile),
        variant,
    )
    .join(sanitize_file_stem(&project.app.name));
    if app_root.exists() {
        fs::remove_dir_all(&app_root).with_context(|| {
            format!(
                "failed to clear previous Linux run bundle at {}",
                app_root.display()
            )
        })?;
    }
    let bin_dir = app_root.join("bin");
    let applications_dir = app_root.join("share/applications");
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&applications_dir)?;

    let executable = bin_dir.join(&binary.executable_name);
    fs::copy(&binary.path, &executable).with_context(|| {
        format!(
            "failed to copy {} into Linux development bundle",
            binary.path.display()
        )
    })?;
    copy_unix_mode(&binary.path, &executable).ok();
    if let Some(icon) = resolve_app_icon(&project_dir, Target::Linux)? {
        let extension = normalized_extension(&icon.path)?;
        let icons_dir = if extension == "svg" {
            app_root.join("share/icons/hicolor/scalable/apps")
        } else {
            app_root.join("share/icons/hicolor/512x512/apps")
        };
        fs::create_dir_all(&icons_dir)?;
        let destination = icons_dir.join(format!("{}.{}", project.app.app_id, extension));
        fs::copy(&icon.path, &destination).with_context(|| {
            format!(
                "failed to copy Linux app icon {} to {}",
                icon.path.display(),
                destination.display()
            )
        })?;
    }
    fs::write(
        applications_dir.join(format!("{}.desktop", project.app.app_id)),
        render_linux_desktop_entry(project, &executable),
    )?;
    let native_products = build_linux_native_modules(&project_dir, project, variant, release)?;
    stage_linux_native_products(&app_root, &native_products)?;
    stage_project_assets(&project_dir, &app_root)?;
    Ok(DesktopRunApp { executable })
}

fn package_windows_run_app(
    project: &FissionProject,
    project_dir: &Path,
    variant: Option<&NativeVariant>,
    release: bool,
) -> Result<DesktopRunApp> {
    let project_dir = fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })?;
    let binary = build_desktop_binary(&project_dir, release, Target::Windows, variant)?;
    let profile = if release { "release" } else { "debug" };
    let app_root = variant_output_path(
        project_dir.join(".fission/run/windows").join(profile),
        variant,
    )
    .join(sanitize_file_stem(&project.app.name));
    if app_root.exists() {
        fs::remove_dir_all(&app_root).with_context(|| {
            format!(
                "failed to clear previous Windows run bundle at {}",
                app_root.display()
            )
        })?;
    }
    fs::create_dir_all(&app_root)?;
    let executable = app_root.join(platform_executable_name(&binary.executable_name));
    fs::copy(&binary.path, &executable).with_context(|| {
        format!(
            "failed to copy {} into Windows development bundle",
            binary.path.display()
        )
    })?;
    if let Some(icon) = resolve_app_icon(&project_dir, Target::Windows)? {
        let extension = normalized_extension(&icon.path)?;
        let destination = app_root.join(format!("app-icon.{extension}"));
        fs::copy(&icon.path, &destination).with_context(|| {
            format!(
                "failed to copy Windows app icon {} to {}",
                icon.path.display(),
                destination.display()
            )
        })?;
    }
    fs::write(
        app_root.join(format!(
            "{}.manifest",
            platform_executable_name(&binary.executable_name)
        )),
        render_windows_development_manifest(project),
    )?;
    let native_products = build_windows_native_modules(&project_dir, project, variant, release)?;
    stage_windows_runtime_products(&app_root, &native_products)?;
    stage_project_assets(&project_dir, &app_root)?;
    Ok(DesktopRunApp { executable })
}

fn run_macos_app_bundle(app: &MacosRunApp, options: &RunOptions) -> Result<()> {
    let log_path = detached_log_path(&options.project_dir, "desktop");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    File::create(&log_path)
        .with_context(|| format!("failed to create log file {}", log_path.display()))?;

    let mut command = Command::new("open");
    command.arg("-n");
    if !options.detach {
        command.arg("-W");
    }
    command
        .arg("--stdout")
        .arg(&log_path)
        .arg("--stderr")
        .arg(&log_path);
    for (key, value) in macos_forwarded_env() {
        command.arg("--env").arg(format!("{key}={value}"));
    }
    command.arg(&app.bundle);

    if options.detach {
        let status = command
            .status()
            .context("failed to launch macOS app bundle")?;
        if !status.success() {
            bail!("macOS app launch failed with {status}");
        }
        println!(
            "Started {}. Logs: {}",
            app.bundle.display(),
            log_path.display()
        );
        Ok(())
    } else {
        println!(
            "Launching {}. Logs: {}",
            app.bundle.display(),
            log_path.display()
        );
        run_status(&mut command, "macOS app")
    }
}

fn macos_forwarded_env() -> Vec<(String, String)> {
    env::vars()
        .filter(|(key, _)| {
            key == "RUST_BACKTRACE" || key.starts_with("FISSION_") || key.starts_with("RUST_LOG")
        })
        .collect()
}

fn render_macos_run_info_plist(
    project: &FissionProject,
    binary: &DesktopBinary,
    app_name: &str,
    macos: &MacosPackageConfig,
) -> String {
    let bundle_id = macos
        .bundle_id
        .as_deref()
        .unwrap_or(project.app.app_id.as_str());
    let minimum_os = macos.minimum_os.as_deref().unwrap_or("13.0");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>{}</string>
  <key>CFBundleName</key>
  <string>{}</string>
  <key>CFBundleDisplayName</key>
  <string>{}</string>
  <key>CFBundleExecutable</key>
  <string>{}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>{}</string>
  <key>CFBundleVersion</key>
  <string>{}</string>
  <key>LSMinimumSystemVersion</key>
  <string>{}</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>NSHighResolutionCapable</key>
  <true/>
{}
{}
</dict>
</plist>
"#,
        escape_xml(bundle_id),
        escape_xml(app_name),
        escape_xml(app_name),
        escape_xml(&binary.executable_name),
        escape_xml(&binary.version),
        escape_xml(&binary.version),
        escape_xml(minimum_os),
        render_macos_run_application_category_entry(macos),
        render_macos_run_capability_plist_entries(project)
    )
}

fn render_macos_run_application_category_entry(macos: &MacosPackageConfig) -> String {
    macos
        .application_category
        .as_deref()
        .filter(|category| !category.trim().is_empty())
        .map(|category| {
            format!(
                "  <key>LSApplicationCategoryType</key>\n  <string>{}</string>",
                escape_xml(category)
            )
        })
        .unwrap_or_default()
}

fn render_macos_run_capability_plist_entries(project: &FissionProject) -> String {
    let mut out = String::new();
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
    {
        out.push_str(
            "  <key>NSBluetoothAlwaysUsageDescription</key>\n  <string>This app uses Bluetooth when you request nearby-device features.</string>\n",
        );
    }
    if project.capabilities.contains(&PlatformCapability::Camera)
        || project
            .capabilities
            .contains(&PlatformCapability::BarcodeScanner)
    {
        out.push_str(
            "  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera when you request camera or barcode features.</string>\n",
        );
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
        || project.capabilities.contains(&PlatformCapability::Wifi)
    {
        out.push_str(
            "  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses location information when you request location-aware or Wi-Fi features.</string>\n",
        );
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
    {
        out.push_str(
            "  <key>NSMicrophoneUsageDescription</key>\n  <string>This app uses the microphone when you request audio capture.</string>\n",
        );
    }
    out
}

fn macos_display_name(name: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in name.chars() {
        match ch {
            '-' | '_' | ' ' => {
                uppercase_next = true;
                if !out.ends_with(' ') && !out.is_empty() {
                    out.push(' ');
                }
            }
            _ if uppercase_next => {
                out.extend(ch.to_uppercase());
                uppercase_next = false;
            }
            _ => out.push(ch),
        }
    }
    out.trim().to_string()
}

fn render_linux_desktop_entry(project: &FissionProject, executable: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec={}\nIcon={}\nStartupNotify=true\nStartupWMClass={}\nCategories=Utility;\n",
        escape_desktop_entry(&macos_display_name(&project.app.name)),
        escape_desktop_entry(&executable.display().to_string()),
        escape_desktop_entry(&project.app.app_id),
        escape_desktop_entry(&project.app.app_id)
    )
}

fn render_windows_development_manifest(project: &FissionProject) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="{}" type="win32"/>
  <description>{}</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#,
        escape_xml(&project.app.app_id),
        escape_xml(&macos_display_name(&project.app.name))
    )
}

fn escape_desktop_entry(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "")
}

fn sanitize_file_stem(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches(['-', '.', '_']);
    if trimmed.is_empty() {
        "app".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(unix)]
fn copy_unix_mode(source: &Path, dest: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(source)?.permissions().mode();
    fs::set_permissions(dest, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn copy_unix_mode(_source: &Path, _dest: &Path) -> Result<()> {
    Ok(())
}

fn platform_executable_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_desktop(
    project_dir: &Path,
    release: bool,
    target: Target,
    variant: Option<&NativeVariant>,
) -> Result<()> {
    build_desktop_binary(project_dir, release, target, variant).map(|_| ())
}

fn run_target_script<F>(project_dir: &Path, relative_script: &str, configure: F) -> Result<()>
where
    F: FnOnce(&mut Command),
{
    let script = project_dir.join(relative_script);
    let mut command = command_for_script(&script)?;
    command.current_dir(project_dir);
    configure(&mut command);
    run_status(&mut command, relative_script)
}

fn run_status(command: &mut Command, label: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run {label}"))?;
    if !status.success() {
        bail!("{label} failed with {status}");
    }
    Ok(())
}

fn command_for_script(script: &Path) -> Result<Command> {
    if !script.exists() {
        bail!("script is missing at {}", script.display());
    }
    let script = fs::canonicalize(script)
        .with_context(|| format!("failed to resolve script {}", script.display()))?;
    if cfg!(windows) {
        if find_in_path("bash").is_none() {
            bail!(
                "running {} on Windows currently requires bash on PATH; install Git for Windows or run the equivalent command in WSL",
                script.display()
            );
        }
        let mut command = Command::new("bash");
        command.arg(script);
        Ok(command)
    } else {
        Ok(Command::new(script))
    }
}

fn detached_log_path(project_dir: &Path, name: &str) -> PathBuf {
    project_dir.join(".fission/run").join(format!("{name}.log"))
}

fn open_log(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open log file {}", path.display()))
}

fn discover_ios_simulators() -> Vec<Device> {
    if !cfg!(target_os = "macos") || find_in_path("xcrun").is_none() {
        return Vec::new();
    }
    let output = match Command::new("xcrun")
        .args(["simctl", "list", "devices", "available", "-j"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(payload) => payload,
        Err(_) => return Vec::new(),
    };
    let mut devices = Vec::new();
    if let Some(groups) = payload.get("devices").and_then(|value| value.as_object()) {
        for (runtime, entries) in groups {
            if !runtime.contains("SimRuntime.iOS") {
                continue;
            }
            let Some(entries) = entries.as_array() else {
                continue;
            };
            for entry in entries {
                let name = entry
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("iOS Simulator");
                if !name.contains("iPhone") {
                    continue;
                }
                let Some(udid) = entry.get("udid").and_then(|value| value.as_str()) else {
                    continue;
                };
                let state = entry
                    .get("state")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                devices.push(Device {
                    id: udid.to_string(),
                    name: name.to_string(),
                    target: Target::Ios,
                    kind: "ios-simulator".to_string(),
                    status: state.to_ascii_lowercase(),
                    detail: runtime
                        .rsplit('.')
                        .next()
                        .unwrap_or(runtime)
                        .replace('-', " "),
                    available: true,
                });
            }
        }
    }
    devices
}

fn discover_android_devices() -> Vec<Device> {
    let mut devices = Vec::new();
    let Ok(adb) = adb_path() else {
        return devices;
    };
    if let Ok(output) = Command::new(&adb).arg("devices").arg("-l").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let Some(serial) = parts.next() else { continue };
                let status = parts.next().unwrap_or("unknown");
                let detail = parts.collect::<Vec<_>>().join(" ");
                devices.push(Device {
                    id: serial.to_string(),
                    name: if serial.starts_with("emulator-") {
                        "Android Emulator"
                    } else {
                        "Android Device"
                    }
                    .to_string(),
                    target: Target::Android,
                    kind: if serial.starts_with("emulator-") {
                        "android-emulator"
                    } else {
                        "android-device"
                    }
                    .to_string(),
                    status: status.to_string(),
                    detail,
                    available: status == "device",
                });
            }
        }
    }

    if let Some(avdmanager) = android_tool("cmdline-tools/latest/bin/avdmanager") {
        if let Ok(output) = Command::new(avdmanager).args(["list", "avd"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let line = line.trim();
                    if let Some(name) = line.strip_prefix("Name:") {
                        let name = name.trim();
                        devices.push(Device {
                            id: format!("android-avd:{name}"),
                            name: name.to_string(),
                            target: Target::Android,
                            kind: "android-avd".to_string(),
                            status: "configured".to_string(),
                            detail: "stopped emulator profile".to_string(),
                            available: true,
                        });
                    }
                }
            }
        }
    }
    devices
}

fn wait_for_android_pid(
    adb: &Path,
    serial: &str,
    app_id: &str,
    timeout: Duration,
) -> Result<String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let output = Command::new(adb)
            .arg("-s")
            .arg(serial)
            .arg("shell")
            .arg("pidof")
            .arg(app_id)
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                let pid = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !pid.is_empty() {
                    return Ok(pid);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    bail!("timed out waiting for Android process `{app_id}` on {serial}")
}

fn first_android_serial() -> Option<String> {
    let adb = adb_path().ok()?;
    let output = Command::new(adb).arg("devices").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?;
            let status = parts.next()?;
            (status == "device").then(|| serial.to_string())
        })
}

fn adb_path() -> Result<PathBuf> {
    android_tool("platform-tools/adb")
        .context("Android adb was not found; run `fission doctor android`")
}

fn android_tool(relative: &str) -> Option<PathBuf> {
    let home = android_home();
    let path = home.join(relative);
    if path.exists() {
        return Some(path);
    }
    let exe = home.join(format!("{relative}.exe"));
    if exe.exists() {
        return Some(exe);
    }
    None
}

fn android_home() -> PathBuf {
    env::var_os("ANDROID_HOME")
        .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(default_android_home)
}

fn default_android_home() -> PathBuf {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        home.join("Library/Android/sdk")
    } else if cfg!(target_os = "windows") {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("Android/Sdk")
    } else {
        home.join("Android/Sdk")
    }
}

fn detect_chrome() -> Option<PathBuf> {
    for var in ["FISSION_CHROME", "CHROME", "CHROME_BIN"] {
        if let Ok(value) = env::var(var) {
            let path = PathBuf::from(value);
            if path.exists() {
                return Some(path);
            }
        }
    }
    let names = if cfg!(target_os = "windows") {
        vec!["chrome.exe", "msedge.exe", "chromium.exe"]
    } else {
        vec!["google-chrome", "chromium", "chromium-browser", "chrome"]
    };
    for name in names {
        if let Some(path) = find_in_path(name) {
            return Some(path);
        }
    }
    for path in platform_chrome_paths() {
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn platform_chrome_paths() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
            PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
        ]
    } else if cfg!(target_os = "windows") {
        let mut paths = Vec::new();
        if let Some(program_files) = env::var_os("PROGRAMFILES") {
            paths.push(PathBuf::from(program_files).join("Google/Chrome/Application/chrome.exe"));
        }
        if let Some(program_files_x86) = env::var_os("PROGRAMFILES(X86)") {
            paths.push(
                PathBuf::from(program_files_x86).join("Google/Chrome/Application/chrome.exe"),
            );
        }
        paths
    } else {
        Vec::new()
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn require_host(target: Target) -> Result<()> {
    match target {
        Target::Ios if !cfg!(target_os = "macos") => {
            bail!("iOS simulator runs require macOS with Xcode")
        }
        _ => Ok(()),
    }
}

fn require_desktop_host(target: Target) -> Result<()> {
    let host = host_desktop_target();
    if target != host {
        bail!(
            "desktop target `{}` cannot be built or run from this host with the current CLI; use `{}` on this machine",
            target.as_str(),
            host.as_str()
        );
    }
    Ok(())
}

fn host_desktop_target() -> Target {
    if cfg!(target_os = "windows") {
        Target::Windows
    } else if cfg!(target_os = "macos") {
        Target::Macos
    } else {
        Target::Linux
    }
}

fn desktop_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows desktop"
    } else if cfg!(target_os = "macos") {
        "macOS desktop"
    } else {
        "Linux desktop"
    }
}

fn current_os_detail() -> String {
    env::consts::OS.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn metadata_package(name: &str, manifest_path: &str) -> CargoMetadataPackage {
        CargoMetadataPackage {
            name: name.into(),
            version: "0.1.0".into(),
            manifest_path: PathBuf::from(manifest_path),
            targets: Vec::new(),
        }
    }

    fn project() -> FissionProject {
        FissionProject {
            app: fission_command_core::AppConfig {
                name: "field-inspector".into(),
                app_id: "com.fission.examples.fieldinspector".into(),
                splash: None,
            },
            targets: BTreeSet::new(),
            capabilities: BTreeSet::new(),
            native: Default::default(),
        }
    }

    #[test]
    fn browser_test_server_serves_javascript_modules() {
        assert_eq!(
            static_content_type(Path::new("platforms/web/bootstrap.mjs")),
            "text/javascript; charset=utf-8"
        );
    }

    #[test]
    fn browser_test_server_serves_multiple_resources() {
        let root = env::temp_dir().join(format!(
            "fission-web-test-server-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create test root");
        fs::write(root.join("index.html"), "ready").expect("write index");
        fs::write(root.join("bootstrap.mjs"), "export const ready = true;").expect("write module");

        let server = StaticTestServer::start(root.clone()).expect("start test server");
        let index = ureq::get(&format!("{}/", server.base_url()))
            .call()
            .expect("fetch index");
        assert_eq!(index.into_string().expect("read index"), "ready");
        let module = ureq::get(&format!("{}/bootstrap.mjs", server.base_url()))
            .call()
            .expect("fetch module");
        assert_eq!(
            module.header("content-type"),
            Some("text/javascript; charset=utf-8")
        );
        assert_eq!(
            module.into_string().expect("read module"),
            "export const ready = true;"
        );

        drop(server);
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn browser_test_server_accepts_renderer_diagnostics() {
        let root = env::temp_dir().join(format!(
            "fission-web-test-server-diagnostics-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create test root");

        let server = StaticTestServer::start(root.clone()).expect("start test server");
        let response = ureq::post(&format!("{}/__fission/renderer", server.base_url()))
            .send_string(r#"{"active":"canvas2d-software"}"#)
            .expect("post renderer diagnostics");
        assert_eq!(response.status(), 204);

        drop(server);
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn linux_development_entry_uses_app_identity() {
        let entry = render_linux_desktop_entry(&project(), Path::new("/tmp/field-inspector"));
        assert!(entry.contains("Name=Field Inspector"));
        assert!(entry.contains("Icon=com.fission.examples.fieldinspector"));
        assert!(entry.contains("StartupWMClass=com.fission.examples.fieldinspector"));
    }

    #[test]
    fn windows_development_manifest_uses_app_identity() {
        let manifest = render_windows_development_manifest(&project());
        assert!(manifest.contains("com.fission.examples.fieldinspector"));
        assert!(manifest.contains("PerMonitorV2"));
        assert!(manifest.contains("asInvoker"));
    }

    #[test]
    fn macos_development_plist_uses_package_identity() {
        let binary = DesktopBinary {
            version: "1.2.3".into(),
            executable_name: "field-inspector".into(),
            path: PathBuf::from("/tmp/field-inspector"),
        };
        let config = MacosPackageConfig {
            bundle_id: Some("com.example.packaged".into()),
            minimum_os: Some("14.0".into()),
            ..Default::default()
        };

        let plist = render_macos_run_info_plist(&project(), &binary, "Field Inspector", &config);

        assert!(plist.contains("<string>com.example.packaged</string>"));
        assert!(plist.contains("<key>LSMinimumSystemVersion</key>\n  <string>14.0</string>"));
    }

    #[test]
    fn macos_run_bundle_is_staged_outside_the_project_volume() {
        let project_dir = Path::new("/Volumes/My Shared Files/projects/example");

        let staging = macos_run_staging_root(project_dir, &project(), "debug");

        assert!(staging.starts_with(env::temp_dir()));
        assert!(!staging.starts_with(project_dir));
        assert!(staging.ends_with("debug"));
        assert!(staging
            .to_string_lossy()
            .contains("com.fission.examples.fieldinspector"));
    }

    #[test]
    fn desktop_package_selection_uses_nested_project_manifest() {
        let metadata = CargoMetadata {
            packages: vec![
                metadata_package("workspace-first", "/workspace/crates/first/Cargo.toml"),
                metadata_package("desktop", "/workspace/crates/desktop/Cargo.toml"),
            ],
            target_directory: PathBuf::from("/workspace/target"),
        };

        let package = desktop_package_for_manifest(
            &metadata,
            Path::new("/workspace/crates/desktop/Cargo.toml"),
        )
        .expect("desktop package should be selected");

        assert_eq!(package.name, "desktop");
    }

    #[test]
    fn desktop_package_selection_does_not_fall_back_to_first_workspace_member() {
        let metadata = CargoMetadata {
            packages: vec![metadata_package(
                "workspace-first",
                "/workspace/crates/first/Cargo.toml",
            )],
            target_directory: PathBuf::from("/workspace/target"),
        };

        let error = desktop_package_for_manifest(
            &metadata,
            Path::new("/workspace/crates/desktop/Cargo.toml"),
        )
        .expect_err("an unmatched project manifest must fail");

        assert!(error.to_string().contains("crates/desktop/Cargo.toml"));
    }
}
