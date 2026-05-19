use crate::{ios_executable_name, read_project_config, FissionProject, Target};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, IsTerminal, Read, Seek, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Device {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) target: Target,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) available: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct RunOptions {
    pub(crate) project_dir: PathBuf,
    pub(crate) target: Option<Target>,
    pub(crate) device: Option<String>,
    pub(crate) detach: bool,
    pub(crate) release: bool,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) no_open: bool,
    pub(crate) headless: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct BuildOptions {
    pub(crate) project_dir: PathBuf,
    pub(crate) target: Option<Target>,
    pub(crate) release: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct TestOptions {
    pub(crate) project_dir: PathBuf,
    pub(crate) target: Option<Target>,
    pub(crate) headless: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct LogOptions {
    pub(crate) project_dir: PathBuf,
    pub(crate) target: Option<Target>,
    pub(crate) device: Option<String>,
    pub(crate) follow: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ServeWebOptions {
    pub(crate) project_dir: PathBuf,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) open: bool,
}

pub(crate) fn list_devices(project_dir: &Path, json: bool) -> Result<()> {
    let devices = discover_devices(project_dir);
    if json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
        return Ok(());
    }

    println!("Fission devices");
    if devices.is_empty() {
        println!("No runnable devices detected. Run `cargo fission doctor web ios android --project-dir {}`.", project_dir.display());
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

pub(crate) fn run_app(options: RunOptions) -> Result<()> {
    let project = read_project_config(&options.project_dir)?;
    let device = select_device(
        &options.project_dir,
        options.target,
        options.device.as_deref(),
    )?;
    ensure_target_configured(&project, &options.project_dir, device.target)?;

    match device.target {
        Target::Linux | Target::Macos | Target::Windows => run_desktop(&options, &device),
        Target::Web => run_web(&options, &device),
        Target::Ios => run_ios(&project, &options, &device),
        Target::Android => run_android(&project, &options, &device),
    }
}

pub(crate) fn build_app(options: BuildOptions) -> Result<()> {
    let project = read_project_config(&options.project_dir)?;
    let target = options.target.unwrap_or_else(host_desktop_target);
    ensure_target_configured(&project, &options.project_dir, target)?;

    match target {
        Target::Linux | Target::Macos | Target::Windows => {
            require_desktop_host(target)?;
            build_desktop(&options.project_dir, options.release)
        }
        Target::Web => build_web(&options.project_dir, options.release),
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

pub(crate) fn test_app(options: TestOptions) -> Result<()> {
    let project = read_project_config(&options.project_dir)?;
    let target = options.target.unwrap_or_else(host_desktop_target);
    ensure_target_configured(&project, &options.project_dir, target)?;

    match target {
        Target::Linux | Target::Macos | Target::Windows => {
            require_desktop_host(target)?;
            let mut command = Command::new("cargo");
            command.arg("test").current_dir(&options.project_dir);
            run_status(&mut command, "desktop tests")
        }
        Target::Web => run_target_script(
            &options.project_dir,
            "platforms/web/test-browser.sh",
            |_| {},
        ),
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

pub(crate) fn attach_logs(options: LogOptions) -> Result<()> {
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
        Target::Linux | Target::Macos | Target::Windows => tail_log_file(
            &detached_log_path(&options.project_dir, "desktop"),
            options.follow,
        ),
    }
}

pub(crate) fn serve_web(options: ServeWebOptions) -> Result<()> {
    serve_static(
        options.project_dir,
        options.host,
        options.port,
        options.open,
    )
}

fn discover_devices(_project_dir: &Path) -> Vec<Device> {
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
                "no device matched `{query}`; run `cargo fission devices --project-dir {}`",
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
            "no runnable devices detected; run `cargo fission devices --project-dir {}`",
            project_dir.display()
        );
    }
    if devices.len() == 1 {
        return Ok(devices[0].clone());
    }

    if !io::stdin().is_terminal() {
        print_device_choices(&devices);
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
            "target `{}` is not configured for this app; run `cargo fission add-target {} --project-dir {}`",
            target.as_str(),
            target.as_str(),
            project_dir.display()
        );
    }
    let scaffold = project_dir.join(target.scaffold_relative_path());
    if !scaffold.exists() {
        bail!(
            "target `{}` scaffold is missing at {}; run `cargo fission add-target {} --project-dir {}`",
            target.as_str(),
            scaffold.display(),
            target.as_str(),
            project_dir.display()
        );
    }
    Ok(())
}

fn run_desktop(options: &RunOptions, _device: &Device) -> Result<()> {
    let mut command = Command::new("cargo");
    command.arg("run").current_dir(&options.project_dir);
    if options.release {
        command.arg("--release");
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
            .arg(options.port.to_string());
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
            web_url(&options.host, options.port),
            log_path.display()
        );
        return Ok(());
    }

    serve_static(
        options.project_dir.clone(),
        options.host.clone(),
        options.port,
        open,
    )
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
            .arg(format!("{}/android.app.NativeActivity", project.app.app_id)),
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

fn build_desktop(project_dir: &Path, release: bool) -> Result<()> {
    let mut command = Command::new("cargo");
    command.arg("build").current_dir(project_dir);
    if release {
        command.arg("--release");
    }
    run_status(&mut command, "desktop build")
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

fn serve_static(root: PathBuf, host: String, port: u16, open: bool) -> Result<()> {
    let listener = TcpListener::bind((host.as_str(), port))
        .with_context(|| format!("failed to bind {}:{}", host, port))?;
    let url = web_url(&host, port);
    println!("Serving {} at {}", root.display(), url);
    println!("Press Ctrl+C to stop.");
    if open {
        let _ = open_url(&url);
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_http_request(stream, &root) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle_http_request(mut stream: TcpStream, root: &Path) -> Result<()> {
    let mut reader = io::BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    let response = static_response(root, path)?;
    stream.write_all(&response)?;
    Ok(())
}

fn static_response(root: &Path, request_path: &str) -> Result<Vec<u8>> {
    let mut relative = request_path.trim_start_matches('/').to_string();
    if relative.is_empty() {
        relative = "platforms/web/".to_string();
    }
    if relative.ends_with('/') {
        relative.push_str("index.html");
    }
    let path = sanitize_static_path(root, &relative)?;
    if !path.exists() || !path.is_file() {
        println!("GET {} 404", request_path);
        return Ok(http_response(404, "text/plain", b"not found"));
    }
    let body = fs::read(&path)?;
    let content_type = content_type(&path);
    println!("GET {} 200", request_path);
    Ok(http_response(200, content_type, &body))
}

fn sanitize_static_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = PathBuf::from(root);
    for part in relative.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\\') {
            bail!("invalid static path `{relative}`");
        }
        path.push(part);
    }
    Ok(path)
}

fn http_response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(OsStr::to_str).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "css" => "text/css; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn web_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}/platforms/web/")
}

fn open_url(url: &str) -> Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(url);
        cmd
    } else if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd
    } else {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };
    command.spawn()?;
    Ok(())
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
        .context("Android adb was not found; run `cargo fission doctor android`")
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
