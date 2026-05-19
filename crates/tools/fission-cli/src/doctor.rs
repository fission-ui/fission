use crate::Target;
use anyhow::{bail, Result};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    Ok,
    Warn,
    Error,
}

#[derive(Debug)]
struct Check {
    area: &'static str,
    name: String,
    status: Status,
    detail: String,
    suggestion: Option<String>,
}

impl Check {
    fn ok(area: &'static str, name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            area,
            name: name.into(),
            status: Status::Ok,
            detail: detail.into(),
            suggestion: None,
        }
    }

    fn warn(
        area: &'static str,
        name: impl Into<String>,
        detail: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            area,
            name: name.into(),
            status: Status::Warn,
            detail: detail.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    fn error(
        area: &'static str,
        name: impl Into<String>,
        detail: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            area,
            name: name.into(),
            status: Status::Error,
            detail: detail.into(),
            suggestion: Some(suggestion.into()),
        }
    }
}

pub(crate) fn run_doctor(project_dir: &Path, targets: &[Target], strict: bool) -> Result<()> {
    let targets = normalized_targets(targets);
    let mut checks = Vec::new();

    checks.extend(check_project(project_dir, &targets));
    checks.extend(check_rust_targets(&targets));

    if targets.contains(&Target::Web) {
        checks.extend(check_web());
    }
    if targets.contains(&Target::Android) {
        checks.extend(check_android());
    }
    if targets.contains(&Target::Ios) {
        checks.extend(check_ios());
    }

    print_checks(project_dir, &targets, &checks);

    if strict && checks.iter().any(|check| check.status == Status::Error) {
        bail!("fission doctor found required toolchain errors");
    }

    Ok(())
}

fn normalized_targets(targets: &[Target]) -> Vec<Target> {
    if targets.is_empty() {
        return vec![Target::Web, Target::Ios, Target::Android];
    }

    targets.iter().copied().collect()
}

fn check_project(project_dir: &Path, targets: &[Target]) -> Vec<Check> {
    let mut checks = Vec::new();
    let fission_toml = project_dir.join("fission.toml");
    if fission_toml.exists() {
        checks.push(Check::ok(
            "project",
            "fission.toml",
            format!("found {}", fission_toml.display()),
        ));
    } else {
        checks.push(Check::warn(
            "project",
            "fission.toml",
            format!("{} does not exist", fission_toml.display()),
            "run doctor from a Fission app root, or pass --project-dir <path>",
        ));
    }

    for target in targets {
        let path = project_dir.join(target.scaffold_relative_path());
        if path.exists() {
            checks.push(Check::ok(
                "project",
                format!("{} scaffold", target.as_str()),
                format!("found {}", path.display()),
            ));
        } else {
            checks.push(Check::warn(
                "project",
                format!("{} scaffold", target.as_str()),
                format!("{} does not exist", path.display()),
                format!(
                    "run `cargo fission add-target {} --project-dir {}`",
                    target.as_str(),
                    project_dir.display()
                ),
            ));
        }
    }

    checks
}

fn check_rust_targets(targets: &[Target]) -> Vec<Check> {
    let mut checks = Vec::new();
    let Some(rustup) = find_in_path("rustup") else {
        checks.push(Check::error(
            "rust",
            "rustup",
            "rustup is not on PATH",
            "install Rust from https://rustup.rs/ and restart your shell",
        ));
        return checks;
    };

    checks.push(Check::ok(
        "rust",
        "rustup",
        format!("found {}", rustup.display()),
    ));

    let installed = command_stdout(&rustup, ["target", "list", "--installed"])
        .unwrap_or_else(|_| String::new());
    for rust_target in required_rust_targets(targets) {
        if installed.lines().any(|line| line.trim() == rust_target) {
            checks.push(Check::ok(
                "rust",
                rust_target,
                "target is installed".to_string(),
            ));
        } else {
            checks.push(Check::error(
                "rust",
                rust_target,
                "target is not installed".to_string(),
                format!("run `rustup target add {}`", rust_target),
            ));
        }
    }

    checks
}

fn required_rust_targets(targets: &[Target]) -> Vec<&'static str> {
    let mut required = Vec::new();
    if targets.contains(&Target::Web) {
        required.push("wasm32-unknown-unknown");
    }
    if targets.contains(&Target::Android) {
        required.push("aarch64-linux-android");
    }
    if targets.contains(&Target::Ios) {
        required.push("aarch64-apple-ios");
        required.push("aarch64-apple-ios-sim");
    }
    required
}

fn check_web() -> Vec<Check> {
    let mut checks = Vec::new();
    check_command(
        &mut checks,
        "web",
        "wasm-pack",
        "install with `cargo install wasm-pack`",
    );
    check_command(
        &mut checks,
        "web",
        "python3",
        "install Python 3 so generated web scripts can serve the app locally",
    );
    check_node_websocket(&mut checks);

    match detect_chrome() {
        Some(path) => checks.push(Check::ok(
            "web",
            "Chrome/Chromium",
            format!("found {}", path.display()),
        )),
        None => checks.push(Check::warn(
            "web",
            "Chrome/Chromium",
            "no Chrome-compatible browser was detected for headless CDP smoke tests",
            "install Chrome/Chromium, or set FISSION_CHROME=/path/to/chrome",
        )),
    }

    checks
}

fn check_node_websocket(checks: &mut Vec<Check>) {
    let Some(node) = find_in_path("node") else {
        checks.push(Check::error(
            "web",
            "Node.js",
            "node is not on PATH",
            "install Node 22+ so generated browser smoke tests can inspect Chrome CDP console/runtime errors",
        ));
        return;
    };

    let supports_websocket = Command::new(&node)
        .args([
            "-e",
            "process.exit(typeof WebSocket === 'function' ? 0 : 1)",
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if supports_websocket {
        checks.push(Check::ok(
            "web",
            "Node.js WebSocket",
            format!("found {}", node.display()),
        ));
    } else {
        checks.push(Check::error(
            "web",
            "Node.js WebSocket",
            format!(
                "{} does not expose the built-in WebSocket client",
                node.display()
            ),
            "install Node 22+ for Chrome CDP smoke tests",
        ));
    }
}

fn check_android() -> Vec<Check> {
    let mut checks = Vec::new();
    let android_home = detect_android_home();
    if android_home.is_dir() {
        checks.push(Check::ok(
            "android",
            "ANDROID_HOME",
            format!("using {}", android_home.display()),
        ));
    } else {
        checks.push(Check::error(
            "android",
            "ANDROID_HOME",
            format!("{} does not exist", android_home.display()),
            "install Android Studio/SDK, or set ANDROID_HOME to your SDK path",
        ));
        return checks;
    }

    check_file(
        &mut checks,
        "android",
        "adb",
        android_home.join("platform-tools/adb"),
        "install Android platform-tools with sdkmanager",
    );
    check_file(
        &mut checks,
        "android",
        "emulator",
        android_home.join("emulator/emulator"),
        "install the Android emulator package with sdkmanager",
    );
    check_file(
        &mut checks,
        "android",
        "sdkmanager",
        android_home.join("cmdline-tools/latest/bin/sdkmanager"),
        "install Android SDK command-line tools",
    );
    check_file(
        &mut checks,
        "android",
        "avdmanager",
        android_home.join("cmdline-tools/latest/bin/avdmanager"),
        "install Android SDK command-line tools",
    );

    let platforms_dir = android_home.join("platforms");
    match latest_android_api(&platforms_dir) {
        Some(api) => checks.push(Check::ok(
            "android",
            "Android platform",
            format!("latest installed API level is {}", api),
        )),
        None => checks.push(Check::error(
            "android",
            "Android platform",
            format!("no platforms were found under {}", platforms_dir.display()),
            "install one with `sdkmanager \"platforms;android-35\"` or newer",
        )),
    }

    match latest_android_system_image_api(&android_home) {
        Some(api) => checks.push(Check::ok(
            "android",
            "Android emulator image",
            format!("latest installed google_apis arm64 image is API {}", api),
        )),
        None => checks.push(Check::warn(
            "android",
            "Android emulator image",
            format!(
                "no google_apis arm64 emulator image found under {}",
                android_home.join("system-images").display()
            ),
            "install one with `sdkmanager \"system-images;android-35;google_apis;arm64-v8a\"` or set ANDROID_SYSTEM_IMAGE",
        )),
    }

    let build_tools_dir = android_home.join("build-tools");
    match latest_child_dir(&build_tools_dir) {
        Some(path) => checks.push(Check::ok(
            "android",
            "build-tools",
            format!("using {}", path.display()),
        )),
        None => checks.push(Check::error(
            "android",
            "build-tools",
            format!(
                "no build-tools were found under {}",
                build_tools_dir.display()
            ),
            "install build-tools with `sdkmanager \"build-tools;35.0.0\"` or newer",
        )),
    }

    let ndk = detect_android_ndk(&android_home);
    if ndk.is_dir() {
        checks.push(Check::ok(
            "android",
            "Android NDK",
            format!("using {}", ndk.display()),
        ));
    } else {
        checks.push(Check::error(
            "android",
            "Android NDK",
            format!("{} does not exist", ndk.display()),
            "install the NDK with sdkmanager, or set ANDROID_NDK to an installed NDK path",
        ));
        return checks;
    }

    match detect_android_toolchain(&ndk) {
        Some(toolchain) => {
            checks.push(Check::ok(
                "android",
                "NDK LLVM toolchain",
                format!("using {}", toolchain.display()),
            ));
            let min_api = env::var("ANDROID_MIN_API_LEVEL").unwrap_or_else(|_| "24".to_string());
            let clang = toolchain.join(format!("aarch64-linux-android{}-clang", min_api));
            if clang.exists() {
                checks.push(Check::ok(
                    "android",
                    "aarch64 clang",
                    format!("found {}", clang.display()),
                ));
            } else {
                checks.push(Check::error(
                    "android",
                    "aarch64 clang",
                    format!("{} does not exist", clang.display()),
                    "set ANDROID_MIN_API_LEVEL to an API supported by the installed NDK",
                ));
            }
        }
        None => checks.push(Check::error(
            "android",
            "NDK LLVM toolchain",
            format!(
                "no prebuilt LLVM toolchain found under {}",
                ndk.join("toolchains/llvm/prebuilt").display()
            ),
            "set ANDROID_TOOLCHAIN to the NDK LLVM bin directory",
        )),
    }

    checks
}

fn check_ios() -> Vec<Check> {
    let mut checks = Vec::new();
    if !cfg!(target_os = "macos") {
        checks.push(Check::error(
            "ios",
            "macOS host",
            "iOS builds require macOS with Xcode",
            "run iOS packaging on macOS",
        ));
        return checks;
    }

    check_command(
        &mut checks,
        "ios",
        "xcrun",
        "install Xcode and run `xcode-select --install` if command-line tools are missing",
    );

    match command_stdout("xcrun", ["--sdk", "iphonesimulator", "--show-sdk-path"]) {
        Ok(path) if !path.trim().is_empty() => checks.push(Check::ok(
            "ios",
            "iPhoneSimulator SDK",
            path.trim().to_string(),
        )),
        _ => checks.push(Check::error(
            "ios",
            "iPhoneSimulator SDK",
            "xcrun could not resolve the iPhoneSimulator SDK",
            "install Xcode and open it once to finish component installation",
        )),
    }

    match command_stdout("xcrun", ["simctl", "list", "devices", "available", "-j"]) {
        Ok(output) if output.contains("iPhone") => checks.push(Check::ok(
            "ios",
            "iPhone simulator",
            "at least one available iPhone simulator is installed",
        )),
        Ok(_) => checks.push(Check::error(
            "ios",
            "iPhone simulator",
            "simctl did not report any available iPhone simulator",
            "install an iOS simulator runtime from Xcode Settings > Platforms",
        )),
        Err(_) => checks.push(Check::error(
            "ios",
            "simctl",
            "xcrun simctl is not available",
            "install Xcode and ensure xcrun is on PATH",
        )),
    }

    checks
}

fn check_command(checks: &mut Vec<Check>, area: &'static str, name: &str, suggestion: &str) {
    match find_in_path(name) {
        Some(path) => checks.push(Check::ok(area, name, format!("found {}", path.display()))),
        None => checks.push(Check::error(
            area,
            name,
            format!("{} is not on PATH", name),
            suggestion,
        )),
    }
}

fn check_file(
    checks: &mut Vec<Check>,
    area: &'static str,
    name: &str,
    path: PathBuf,
    suggestion: &str,
) {
    if path.exists() {
        checks.push(Check::ok(area, name, format!("found {}", path.display())));
    } else {
        checks.push(Check::error(
            area,
            name,
            format!("{} does not exist", path.display()),
            suggestion,
        ));
    }
}

fn print_checks(project_dir: &Path, targets: &[Target], checks: &[Check]) {
    println!("Fission doctor");
    println!("Project: {}", project_dir.display());
    println!(
        "Targets: {}",
        targets
            .iter()
            .map(|target| target.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();

    for check in checks {
        let marker = match check.status {
            Status::Ok => "[ok]",
            Status::Warn => "[warn]",
            Status::Error => "[error]",
        };
        println!(
            "{} {} - {}: {}",
            marker, check.area, check.name, check.detail
        );
        if let Some(suggestion) = &check.suggestion {
            println!("       suggestion: {}", suggestion);
        }
    }

    let ok = checks
        .iter()
        .filter(|check| check.status == Status::Ok)
        .count();
    let warn = checks
        .iter()
        .filter(|check| check.status == Status::Warn)
        .count();
    let error = checks
        .iter()
        .filter(|check| check.status == Status::Error)
        .count();
    println!();
    println!("Summary: {} ok, {} warnings, {} errors", ok, warn, error);
}

fn command_stdout<I, S>(program: impl AsRef<OsStr>, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        bail!("command exited with {}", output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let explicit = Path::new(name);
    if explicit.components().count() > 1 && explicit.exists() {
        return Some(explicit.to_path_buf());
    }

    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{}.exe", name));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
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

    for name in ["google-chrome", "chromium", "chromium-browser", "chrome"] {
        if let Some(path) = find_in_path(name) {
            return Some(path);
        }
    }

    for path in [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ] {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn detect_android_home() -> PathBuf {
    if let Ok(value) = env::var("ANDROID_HOME").or_else(|_| env::var("ANDROID_SDK_ROOT")) {
        return PathBuf::from(value);
    }
    default_android_home()
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

fn detect_android_ndk(android_home: &Path) -> PathBuf {
    if let Ok(value) = env::var("ANDROID_NDK").or_else(|_| env::var("ANDROID_NDK_HOME")) {
        return PathBuf::from(value);
    }
    latest_child_dir(&android_home.join("ndk")).unwrap_or_else(|| android_home.join("ndk"))
}

fn detect_android_toolchain(ndk: &Path) -> Option<PathBuf> {
    if let Ok(value) = env::var("ANDROID_TOOLCHAIN") {
        let path = PathBuf::from(value);
        if path.exists() {
            return Some(path);
        }
    }

    let prebuilt = ndk.join("toolchains/llvm/prebuilt");
    for host in android_host_candidates() {
        let path = prebuilt.join(host).join("bin");
        if path.exists() {
            return Some(path);
        }
    }

    latest_child_dir(&prebuilt).map(|path| path.join("bin"))
}

fn android_host_candidates() -> Vec<&'static str> {
    if cfg!(target_os = "macos") {
        vec!["darwin-aarch64", "darwin-x86_64"]
    } else if cfg!(target_os = "windows") {
        vec!["windows-x86_64"]
    } else {
        vec!["linux-x86_64"]
    }
}

fn latest_android_api(platforms_dir: &Path) -> Option<u32> {
    let mut apis = Vec::new();
    for entry in fs::read_dir(platforms_dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(value) = name.strip_prefix("android-") {
            if let Ok(api) = value.parse::<u32>() {
                apis.push(api);
            }
        }
    }
    apis.into_iter().max()
}

fn latest_android_system_image_api(android_home: &Path) -> Option<u32> {
    let root = android_home.join("system-images");
    let mut apis = Vec::new();
    for entry in fs::read_dir(&root).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(value) = name.strip_prefix("android-") else {
            continue;
        };
        let Ok(api) = value.parse::<u32>() else {
            continue;
        };
        if entry.path().join("google_apis/arm64-v8a").is_dir() {
            apis.push(api);
        }
    }
    apis.into_iter().max()
}

fn latest_child_dir(path: &Path) -> Option<PathBuf> {
    let mut dirs = fs::read_dir(path)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort_by(|left, right| {
        version_key(left)
            .cmp(&version_key(right))
            .then_with(|| left.cmp(right))
    });
    dirs.pop()
}

fn version_key(path: &Path) -> Vec<u32> {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u32>().ok())
        .collect()
}
