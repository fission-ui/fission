use super::*;

pub(super) fn discover_ios_simulators() -> Vec<Device> {
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

pub(super) fn discover_android_devices() -> Vec<Device> {
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

pub(super) fn wait_for_android_pid(
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

pub(super) fn first_android_serial() -> Option<String> {
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

pub(super) fn adb_path() -> Result<PathBuf> {
    android_tool("platform-tools/adb")
        .context("Android adb was not found; run `fission doctor android`")
}

pub(super) fn android_tool(relative: &str) -> Option<PathBuf> {
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

pub(super) fn android_home() -> PathBuf {
    env::var_os("ANDROID_HOME")
        .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(default_android_home)
}

pub(super) fn default_android_home() -> PathBuf {
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

pub(super) fn detect_chrome() -> Option<PathBuf> {
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

pub(super) fn platform_chrome_paths() -> Vec<PathBuf> {
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

pub(super) fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub(super) fn require_host(target: Target) -> Result<()> {
    match target {
        Target::Ios if !cfg!(target_os = "macos") => {
            bail!("iOS simulator runs require macOS with Xcode")
        }
        _ => Ok(()),
    }
}

pub(super) fn require_desktop_host(target: Target) -> Result<()> {
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

pub(super) fn host_desktop_target() -> Target {
    if cfg!(target_os = "windows") {
        Target::Windows
    } else if cfg!(target_os = "macos") {
        Target::Macos
    } else {
        Target::Linux
    }
}

pub(super) fn desktop_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows desktop"
    } else if cfg!(target_os = "macos") {
        "macOS desktop"
    } else {
        "Linux desktop"
    }
}

pub(super) fn current_os_detail() -> String {
    env::consts::OS.to_string()
}
