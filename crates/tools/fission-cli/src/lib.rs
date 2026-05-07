use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_APP_ICON_PNG: &[u8] = include_bytes!("../../../../docs/fission_logo.png");

#[derive(Parser, Debug)]
#[command(
    name = "fission",
    version,
    about = "Scaffold and manage Fission applications"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create a new Fission application.
    Init {
        /// Directory to create.
        path: PathBuf,
        /// Crate/package name override.
        #[arg(long)]
        name: Option<String>,
        /// Application identifier used by mobile targets.
        #[arg(long)]
        app_id: Option<String>,
        /// Optional local Fission checkout to use as a path dependency.
        #[arg(long)]
        local_path: Option<PathBuf>,
    },
    /// Add one or more platform targets to an existing Fission app.
    AddTarget {
        #[arg(value_enum)]
        targets: Vec<Target>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Target {
    Android,
    Ios,
    Linux,
    Macos,
    Web,
    Windows,
}

impl Target {
    fn as_str(self) -> &'static str {
        match self {
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Web => "web",
            Self::Windows => "windows",
        }
    }

    fn scaffold_relative_path(self) -> &'static str {
        match self {
            Self::Android => "platforms/android/README.md",
            Self::Ios => "platforms/ios/README.md",
            Self::Linux => "platforms/linux/README.md",
            Self::Macos => "platforms/macos/README.md",
            Self::Web => "platforms/web/README.md",
            Self::Windows => "platforms/windows/README.md",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FissionProject {
    app: AppConfig,
    targets: BTreeSet<Target>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    name: String,
    app_id: String,
}

pub fn run<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let mut argv: Vec<std::ffi::OsString> = args.into_iter().map(Into::into).collect();
    if let Some(bin) = argv.first() {
        if let Some(name) = Path::new(bin).file_name().and_then(|value| value.to_str()) {
            if name == "cargo-fission" {
                argv[0] = std::ffi::OsString::from("cargo fission");
                if argv.get(1).and_then(|value| value.to_str()) == Some("fission") {
                    argv.remove(1);
                }
            }
        }
    }
    let cli = Cli::parse_from(argv);
    match cli.command {
        Command::Init {
            path,
            name,
            app_id,
            local_path,
        } => init_project(&path, name, app_id, local_path),
        Command::AddTarget {
            targets,
            project_dir,
        } => add_targets(&project_dir, &targets),
    }
}

fn init_project(
    root: &Path,
    name: Option<String>,
    app_id: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<()> {
    if root.exists() && root.read_dir()?.next().is_some() {
        bail!(
            "refusing to initialize into a non-empty directory: {}",
            root.display()
        );
    }
    fs::create_dir_all(root.join("src"))?;

    let project_name = name.unwrap_or_else(|| {
        root.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("fission-app")
            .to_string()
    });
    let normalized_name = normalize_crate_name(&project_name);
    let project = FissionProject {
        app: AppConfig {
            name: normalized_name.clone(),
            app_id: app_id
                .unwrap_or_else(|| format!("com.example.{}", normalized_name.replace('-', "_"))),
        },
        targets: [Target::Windows, Target::Macos, Target::Linux]
            .into_iter()
            .collect(),
    };

    write_file(
        &root.join("Cargo.toml"),
        &render_cargo_toml(&project, local_path.as_deref()),
    )?;
    write_file(
        &root.join("src/main.rs"),
        &render_app_main(project.app.name.as_str()),
    )?;
    write_file(&root.join("src/lib.rs"), APP_LIB)?;
    write_file(&root.join("src/app.rs"), APP_RS)?;
    write_binary_file(&root.join("assets/app-icon.png"), DEFAULT_APP_ICON_PNG)?;
    write_file(&root.join("README.md"), &render_project_readme(&project))?;
    write_file(&root.join(".gitignore"), "target/\nplatforms/*/build/\n")?;
    write_project_config(root, &project)?;

    for target in &project.targets {
        scaffold_target(root, &project, *target)?;
    }

    Ok(())
}

fn add_targets(project_dir: &Path, targets: &[Target]) -> Result<()> {
    if targets.is_empty() {
        bail!("no targets provided");
    }
    let mut project = read_project_config(project_dir)?;
    for target in targets {
        project.targets.insert(*target);
        scaffold_target(project_dir, &project, *target)?;
    }
    write_project_config(project_dir, &project)?;
    write_file(
        &project_dir.join("README.md"),
        &render_project_readme(&project),
    )?;
    Ok(())
}

fn write_project_config(root: &Path, project: &FissionProject) -> Result<()> {
    let data = toml::to_string_pretty(project)?;
    write_file(&root.join("fission.toml"), &(data + "\n"))
}

fn read_project_config(root: &Path) -> Result<FissionProject> {
    let path = root.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn scaffold_target(root: &Path, project: &FissionProject, target: Target) -> Result<()> {
    let relative = Path::new(target.scaffold_relative_path());
    let text = match target {
        Target::Android => {
            scaffold_android_bundle(root, project)?;
            platform_readme(
                "Android",
                "Runnable emulator target. The CLI generates a NativeActivity manifest plus shell scripts that build, install, and launch the Fission app on an Android emulator.",
                &[
                    "Install the Rust target: `rustup target add aarch64-linux-android`.",
                    "Set `ANDROID_HOME` and `ANDROID_NDK`.",
                    "Run `./platforms/android/run-emulator.sh` from the project root to build, package, install, and launch the app on the configured emulator.",
                    "Override `ANDROID_AVD_NAME`, `ANDROID_SYSTEM_IMAGE`, or `ANDROID_HOME` if your local SDK setup differs.",
                    "Set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs, or `ANDROID_EMULATOR_RESTART=1` to relaunch a hidden emulator visibly.",
                    "The generated package uses `assets/app-icon.png` as its default launcher icon.",
                    "Set `FISSION_TEST_CONTROL_PORT=<host-port>` before `run-emulator.sh`; the script forwards it to the fixed in-app device port.",
                ],
            )
        }
        Target::Ios => {
            scaffold_ios_bundle(root, project)?;
            platform_readme(
                "iOS",
                "Simulator scaffolding target. The CLI generates a simulator app bundle template plus shell scripts that build, install, and launch the Fission app with `simctl`, but the current Vello path still renders a black frame on CoreSimulator.",
                &[
                    "Install the Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.",
                    "Confirm the simulator SDK path with `xcrun --sdk iphonesimulator --show-sdk-path`.",
                    "Run `./platforms/ios/run-sim.sh` from the project root to build, install, and launch the app on the first available iPhone simulator.",
                    "Current blocker: CoreSimulator's Metal device does not expose `DownlevelFlags(INDIRECT_EXECUTION)`, so the app launches but only renders a black frame inside `wgpu` / Vello.",
                    "The generated bundle uses `assets/app-icon.png` as its default app icon.",
                    "Set `FISSION_TEST_CONTROL_PORT=<port>` before `run-sim.sh` to expose the in-app test control server on the host.",
                    "Set `IOS_SIM_DEVICE_ID=<udid>` if you want a specific simulator device.",
                ],
            )
        }
        Target::Web => {
            scaffold_web_bundle(root, project)?;
            platform_readme(
                "Web",
                "Runnable browser target. The CLI generates a WASM host page plus helper scripts that build the app with `wasm-pack` and serve it locally.",
                &[
                    "Install the Rust target: `rustup target add wasm32-unknown-unknown`.",
                    "Install `wasm-pack` once: `cargo install wasm-pack`.",
                    "Run `./platforms/web/run-browser.sh` from the project root to build the wasm package and serve the app locally.",
                    "Set `FISSION_WEB_PORT=<port>` or `FISSION_WEB_HOST=<host>` if the default `127.0.0.1:8123` does not suit your machine.",
                    "Set `FISSION_WEB_OPEN=1` if you want the helper script to open a browser tab automatically.",
                    "The generated page uses `assets/app-icon.png` as its default favicon/app icon seed.",
                ],
            )
        }
        Target::Linux | Target::Macos | Target::Windows => platform_readme(
            match target {
                Target::Linux => "Linux",
                Target::Macos => "macOS",
                Target::Windows => "Windows",
                _ => unreachable!(),
            },
            "Runnable target. Desktop platforms share the default `src/main.rs` entrypoint through `DesktopApp`.",
            &[
                "Run `cargo run` from the project root to launch the desktop app.",
                "This target uses the default Vello desktop shell path.",
            ],
        ),
    };
    write_file(&root.join(relative), &text)
}

fn scaffold_ios_bundle(root: &Path, project: &FissionProject) -> Result<()> {
    let executable = ios_executable_name(project);
    let bundle_name = ios_bundle_name(project);
    let plist = render_ios_plist(project, &executable);
    let package_script = render_ios_package_script(project, &bundle_name, &executable);
    let run_script = render_ios_run_script(project);

    write_file(&root.join("platforms/ios/Info.plist"), &plist)?;
    write_file(&root.join("platforms/ios/package-sim.sh"), &package_script)?;
    write_file(&root.join("platforms/ios/run-sim.sh"), &run_script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let package_path = root.join("platforms/ios/package-sim.sh");
        let run_path = root.join("platforms/ios/run-sim.sh");
        fs::set_permissions(&package_path, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&run_path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn scaffold_android_bundle(root: &Path, project: &FissionProject) -> Result<()> {
    let manifest = render_android_manifest(project);
    let package_script = render_android_package_script(project);
    let run_script = render_android_run_script(project);

    write_file(
        &root.join("platforms/android/AndroidManifest.xml"),
        &manifest,
    )?;
    write_file(
        &root.join("platforms/android/package-apk.sh"),
        &package_script,
    )?;
    write_file(&root.join("platforms/android/run-emulator.sh"), &run_script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let package_path = root.join("platforms/android/package-apk.sh");
        let run_path = root.join("platforms/android/run-emulator.sh");
        fs::set_permissions(&package_path, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&run_path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn scaffold_web_bundle(root: &Path, project: &FissionProject) -> Result<()> {
    let index_html = render_web_index(project);
    let bootstrap = render_web_bootstrap(project);
    let build_script = render_web_build_script();
    let run_script = render_web_run_script(project);

    write_file(&root.join("platforms/web/index.html"), &index_html)?;
    write_file(&root.join("platforms/web/bootstrap.mjs"), &bootstrap)?;
    write_file(&root.join("platforms/web/build-wasm.sh"), &build_script)?;
    write_file(&root.join("platforms/web/run-browser.sh"), &run_script)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/web/build-wasm.sh",
            "platforms/web/run-browser.sh",
        ] {
            let path = root.join(relative);
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms)?;
        }
    }

    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn write_binary_file(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn render_cargo_toml(project: &FissionProject, local_path: Option<&Path>) -> String {
    let deps = if let Some(root) = local_path {
        let fission_path = root.join("crates/authoring/fission");
        let fission_core_path = root.join("crates/core/fission-core");
        let fission_macros_path = root.join("crates/authoring/fission-macros");
        format!(
            "fission = {{ path = {:?} }}\nfission-core = {{ path = {:?} }}\nfission-macros = {{ path = {:?} }}\nlazy_static = \"1\"\n",
            fission_path.to_string_lossy().to_string(),
            fission_core_path.to_string_lossy().to_string(),
            fission_macros_path.to_string_lossy().to_string(),
        )
    } else {
        format!(
            "fission = \"{}\"\nfission-core = \"{}\"\nfission-macros = \"{}\"\nlazy_static = \"1\"\n",
            CURRENT_VERSION, CURRENT_VERSION, CURRENT_VERSION
        )
    };
    let lib_name = project.app.name.replace('-', "_");

    format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{}\"\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nanyhow = \"1\"\nserde = {{ version = \"1\", features = [\"derive\"] }}\nconsole_error_panic_hook = \"0.1\"\n{}\n[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nwasm-bindgen = \"0.2\"\n",
        project.app.name, lib_name, deps
    )
}

fn render_project_readme(project: &FissionProject) -> String {
    let mut targets = String::new();
    for target in &project.targets {
        targets.push_str(&format!("- `{}`\n", target.as_str()));
    }
    format!(
        "# {}\n\nGenerated by `fission init`.\n\n## Targets\n\n{}\n## Commands\n\n- `cargo run` -- launch the desktop app\n- `cargo fission add-target web ios android` -- scaffold more targets\n- `cat platforms/<target>/README.md` -- inspect the current prerequisites and status for each target\n\n## Assets\n\n- `assets/app-icon.png` is the default app icon seed copied from Fission's `docs/fission_logo.png`\n\n## Status\n\nDesktop is runnable today. iOS is runnable on the simulator after `cargo fission add-target ios` via `./platforms/ios/run-sim.sh`. Android is runnable on the emulator after `cargo fission add-target android` via `./platforms/android/run-emulator.sh`. Web is runnable in the browser after `cargo fission add-target web` via `./platforms/web/run-browser.sh`.\n",
        project.app.name, targets
    )
}

fn platform_readme(title: &str, summary: &str, bullets: &[&str]) -> String {
    let mut out = format!("# {} target\n\n{}\n", title, summary);
    for bullet in bullets {
        out.push_str(&format!("\n- {}", bullet));
    }
    out.push('\n');
    out
}

fn normalize_crate_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' => ch,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn ios_executable_name(project: &FissionProject) -> String {
    project.app.name.replace('-', "_")
}

fn ios_bundle_name(project: &FissionProject) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in project.app.name.chars() {
        match ch {
            '-' | '_' | ' ' => uppercase_next = true,
            _ if uppercase_next => {
                out.extend(ch.to_uppercase());
                uppercase_next = false;
            }
            _ => out.push(ch),
        }
    }
    if out.is_empty() {
        "FissionApp".to_string()
    } else {
        out
    }
}

fn android_library_name(project: &FissionProject) -> String {
    project.app.name.replace('-', "_")
}

fn render_ios_plist(project: &FissionProject, executable: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>{display_name}</string>
  <key>CFBundleExecutable</key>
  <string>{executable}</string>
  <key>CFBundleIdentifier</key>
  <string>{bundle_id}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>{display_name}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>LSRequiresIPhoneOS</key>
  <true/>
  <key>MinimumOSVersion</key>
  <string>18.0</string>
  <key>UIDeviceFamily</key>
  <array>
    <integer>1</integer>
    <integer>2</integer>
  </array>
</dict>
</plist>
"#,
        display_name = ios_bundle_name(project),
        executable = executable,
        bundle_id = project.app.app_id,
    )
}

fn render_ios_package_script(
    project: &FissionProject,
    bundle_name: &str,
    executable: &str,
) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${{IOS_SIM_TARGET:-aarch64-apple-ios-sim}}"
PROFILE="${{IOS_SIM_PROFILE:-debug}}"
PACKAGE_NAME="{package_name}"
BUNDLE_NAME="{bundle_name}.app"
EXECUTABLE_NAME="{executable}"
BUILD_DIR="$SCRIPT_DIR/build/$PROFILE"
BUNDLE_DIR="$BUILD_DIR/$BUNDLE_NAME"

BUILD_ARGS=(build --manifest-path "$PROJECT_DIR/Cargo.toml" --target "$TARGET" --package "$PACKAGE_NAME")
ARTIFACT_DIR=debug
if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
  ARTIFACT_DIR=release
fi

cargo "${{BUILD_ARGS[@]}}"
TARGET_DIR=$(python3 - <<'PY' "$PROJECT_DIR/Cargo.toml"
import json
import subprocess
import sys

manifest = sys.argv[1]
metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--manifest-path", manifest, "--format-version", "1", "--no-deps"]
    )
)
print(metadata["target_directory"])
PY
)

mkdir -p "$BUNDLE_DIR"
cp "$TARGET_DIR/$TARGET/$ARTIFACT_DIR/$PACKAGE_NAME" "$BUNDLE_DIR/$EXECUTABLE_NAME"
cp "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist"
cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/AppIcon.png"
printf '%s\n' "$BUNDLE_DIR"
"#,
        package_name = project.app.name,
        bundle_name = bundle_name,
        executable = executable,
    )
}

fn render_ios_run_script(project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
BUNDLE_DIR=$("$SCRIPT_DIR/package-sim.sh")
DEVICE_ID="${{IOS_SIM_DEVICE_ID:-}}"

if [[ -z "$DEVICE_ID" ]]; then
  DEVICE_ID=$(python3 - <<'PY'
import json
import subprocess
payload = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "devices", "available", "-j"]))
for runtime, devices in payload["devices"].items():
    if not runtime.startswith("com.apple.CoreSimulator.SimRuntime.iOS-"):
        continue
    for device in devices:
        if device.get("isAvailable") and "iPhone" in device["name"]:
            print(device["udid"])
            raise SystemExit(0)
raise SystemExit("no available iPhone simulator found")
PY
)
fi

xcrun simctl boot "$DEVICE_ID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$DEVICE_ID" -b
xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR"

if [[ -n "${{FISSION_TEST_CONTROL_PORT:-}}" ]]; then
  SIMCTL_CHILD_FISSION_TEST_CONTROL_PORT="${{FISSION_TEST_CONTROL_PORT}}" \
    xcrun simctl launch --terminate-running-process "$DEVICE_ID" "{bundle_id}"
else
  xcrun simctl launch --terminate-running-process "$DEVICE_ID" "{bundle_id}"
fi
"#,
        bundle_id = project.app.app_id,
    )
}

fn render_android_manifest(project: &FissionProject) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{app_id}">

    <uses-permission android:name="android.permission.INTERNET" />

    <uses-sdk
        android:minSdkVersion="24"
        android:targetSdkVersion="32" />

    <application
        android:debuggable="true"
        android:extractNativeLibs="true"
        android:hasCode="false"
        android:icon="@drawable/app_icon"
        android:label="{label}">
        <activity
            android:name="android.app.NativeActivity"
            android:configChanges="orientation|keyboardHidden|screenSize|screenLayout|smallestScreenSize|uiMode|density"
            android:exported="true"
            android:launchMode="singleTask">
            <meta-data
                android:name="android.app.lib_name"
                android:value="{lib_name}" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>

</manifest>
"#,
        app_id = project.app.app_id,
        label = ios_bundle_name(project),
        lib_name = android_library_name(project),
    )
}

fn render_android_package_script(project: &FissionProject) -> String {
    let lib_name = android_library_name(project);
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
TARGET="${{ANDROID_TARGET_TRIPLE:-aarch64-linux-android}}"
PACKAGE_NAME="{package_name}"
LIB_NAME="{lib_name}"
PROFILE="${{ANDROID_PROFILE:-debug}}"
ANDROID_HOME="${{ANDROID_HOME:-$HOME/Library/Android/sdk}}"
ANDROID_NDK="${{ANDROID_NDK:-$ANDROID_HOME/ndk/24.0.8215888}}"
ANDROID_TOOLCHAIN="${{ANDROID_TOOLCHAIN:-$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin}}"
CC_aarch64_linux_android="${{CC_aarch64_linux_android:-$ANDROID_TOOLCHAIN/aarch64-linux-android24-clang}}"
AR_aarch64_linux_android="${{AR_aarch64_linux_android:-$ANDROID_TOOLCHAIN/llvm-ar}}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${{CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER:-$CC_aarch64_linux_android}}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="${{CARGO_TARGET_AARCH64_LINUX_ANDROID_AR:-$AR_aarch64_linux_android}}"
export ANDROID_HOME ANDROID_NDK ANDROID_TOOLCHAIN CC_aarch64_linux_android AR_aarch64_linux_android
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER CARGO_TARGET_AARCH64_LINUX_ANDROID_AR

BUILD_TOOLS=$(find "$ANDROID_HOME/build-tools" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -1)
ANDROID_JAR=$(find "$ANDROID_HOME/platforms" -maxdepth 2 -path '*/android.jar' | sort -V | tail -1)
AAPT="$BUILD_TOOLS/aapt"
ZIPALIGN="$BUILD_TOOLS/zipalign"
APKSIGNER="$BUILD_TOOLS/apksigner"

BUILD_ARGS=(build --manifest-path "$PROJECT_DIR/Cargo.toml" --lib --target "$TARGET" --package "$PACKAGE_NAME")
ARTIFACT_DIR=debug
if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
  ARTIFACT_DIR=release
fi

cargo "${{BUILD_ARGS[@]}}"
TARGET_DIR=$(python3 - <<'PY' "$PROJECT_DIR/Cargo.toml"
import json
import subprocess
import sys

manifest = sys.argv[1]
metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--manifest-path", manifest, "--format-version", "1", "--no-deps"]
    )
)
print(metadata["target_directory"])
PY
)

SO_PATH="$TARGET_DIR/$TARGET/$ARTIFACT_DIR/lib$LIB_NAME.so"
BUILD_DIR="$SCRIPT_DIR/build/$PROFILE"
APK_ROOT="$BUILD_DIR/apk-root"
UNALIGNED_APK="$BUILD_DIR/$PACKAGE_NAME-unaligned.apk"
ALIGNED_APK="$BUILD_DIR/$PACKAGE_NAME-aligned.apk"
SIGNED_APK="$BUILD_DIR/$PACKAGE_NAME.apk"
KEYSTORE="${{ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}}"

rm -rf "$APK_ROOT"
mkdir -p "$APK_ROOT/lib/arm64-v8a" "$APK_ROOT/res/drawable-nodpi" "$BUILD_DIR"
cp "$SO_PATH" "$APK_ROOT/lib/arm64-v8a/lib$LIB_NAME.so"
cp "$PROJECT_DIR/assets/app-icon.png" "$APK_ROOT/res/drawable-nodpi/app_icon.png"

"$AAPT" package -f -F "$UNALIGNED_APK" -M "$SCRIPT_DIR/AndroidManifest.xml" -S "$APK_ROOT/res" -I "$ANDROID_JAR"
(cd "$APK_ROOT" && zip -qr "$UNALIGNED_APK" lib)
"$ZIPALIGN" -f 4 "$UNALIGNED_APK" "$ALIGNED_APK"

if [[ ! -f "$KEYSTORE" ]]; then
  mkdir -p "$(dirname "$KEYSTORE")"
  keytool -genkeypair -v \
    -keystore "$KEYSTORE" \
    -storepass android \
    -alias androiddebugkey \
    -keypass android \
    -dname "CN=Android Debug,O=Android,C=US" \
    -keyalg RSA \
    -keysize 2048 \
    -validity 10000 >/dev/null 2>&1
fi

"$APKSIGNER" sign \
  --ks "$KEYSTORE" \
  --ks-pass pass:android \
  --key-pass pass:android \
  --out "$SIGNED_APK" \
  "$ALIGNED_APK"

printf '%s\n' "$SIGNED_APK"
"#,
        package_name = project.app.name,
        lib_name = lib_name,
    )
}

fn render_android_run_script(project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
ANDROID_HOME="${{ANDROID_HOME:-$HOME/Library/Android/sdk}}"
ADB="$ANDROID_HOME/platform-tools/adb"
EMULATOR_BIN="$ANDROID_HOME/emulator/emulator"
AVDMANAGER="${{ANDROID_AVDMANAGER:-$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager}}"
AVD_NAME="${{ANDROID_AVD_NAME:-FissionApi32Arm64}}"
SYSTEM_IMAGE="${{ANDROID_SYSTEM_IMAGE:-system-images;android-32;google_apis;arm64-v8a}}"
DEVICE_PORT="${{ANDROID_TEST_CONTROL_DEVICE_PORT:-48761}}"
HOST_PORT="${{FISSION_TEST_CONTROL_PORT:-48761}}"
HEADLESS="${{ANDROID_EMULATOR_HEADLESS:-0}}"
RESTART_EMULATOR="${{ANDROID_EMULATOR_RESTART:-0}}"

if ! "$AVDMANAGER" list avd | grep -q "Name: $AVD_NAME"; then
  echo "no" | "$AVDMANAGER" create avd -n "$AVD_NAME" -k "$SYSTEM_IMAGE" --abi "google_apis/arm64-v8a" --device "pixel_5"
fi

RUNNING_EMULATOR=$("$ADB" devices | awk '/^emulator-.*device$/ {{ print $1; exit }}')
if [[ -n "$RUNNING_EMULATOR" && "$RESTART_EMULATOR" == "1" ]]; then
  "$ADB" -s "$RUNNING_EMULATOR" emu kill >/dev/null || true
  until ! "$ADB" devices | grep -q '^emulator-'; do
    sleep 1
  done
  RUNNING_EMULATOR=""
fi

if [[ -z "$RUNNING_EMULATOR" ]]; then
  EMULATOR_ARGS=(-avd "$AVD_NAME" -gpu "${{ANDROID_EMULATOR_GPU:-swiftshader_indirect}}" -no-audio)
  if [[ "$HEADLESS" == "1" ]]; then
    EMULATOR_ARGS+=(-no-window)
  fi
  printf 'Launching emulator %s (%s)\n' "$AVD_NAME" "$([[ "$HEADLESS" == "1" ]] && echo headless || echo visible)"
  "$EMULATOR_BIN" "${{EMULATOR_ARGS[@]}}" >/tmp/fission-android-emulator.log 2>&1 &
  "$ADB" wait-for-device
  until "$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' | grep -q '^1$'; do
    sleep 1
  done
else
  printf 'Using existing emulator %s\n' "$RUNNING_EMULATOR"
  if [[ "$HEADLESS" != "1" ]]; then
    printf 'If the window is not visible, restart with ANDROID_EMULATOR_RESTART=1 to relaunch a visible emulator.\n'
  fi
fi

APK=$("$SCRIPT_DIR/package-apk.sh")
"$ADB" install -r "$APK"
"$ADB" forward "tcp:$HOST_PORT" "tcp:$DEVICE_PORT"
"$ADB" shell am start -n {app_id}/android.app.NativeActivity >/dev/null
printf 'APK=%s\n' "$APK"
"#,
        app_id = project.app.app_id,
    )
}

fn render_web_index(project: &FissionProject) -> String {
    let title = ios_bundle_name(project);
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <link rel="icon" type="image/png" href="../../assets/app-icon.png" />
    <style>
      :root {{
        color-scheme: dark;
        background: #14171f;
      }}
      html, body {{
        margin: 0;
        min-height: 100%;
        background: radial-gradient(circle at top, #243043 0%, #14171f 55%);
        color: #f6fbff;
        font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      }}
      body {{
        display: grid;
        grid-template-rows: auto 1fr;
      }}
      header {{
        padding: 16px 20px 0;
      }}
      h1 {{
        margin: 0;
        font-size: 14px;
        letter-spacing: 0.14em;
        text-transform: uppercase;
        color: #9acdbd;
      }}
      p {{
        margin: 6px 0 0;
        color: #b8c2d1;
        font-size: 14px;
      }}
      canvas {{
        display: block;
        width: 100vw;
        height: calc(100vh - 64px);
      }}
    </style>
  </head>
  <body>
    <header>
      <h1>{title}</h1>
      <p>Generated by `fission add-target web`.</p>
    </header>
    <script type="module" src="./bootstrap.mjs"></script>
  </body>
</html>
"#,
        title = title,
    )
}

fn render_web_bootstrap(project: &FissionProject) -> String {
    let module_name = project.app.name.replace('-', "_");
    format!(
        "import init from \"./pkg/{}.js\";\n\nawait init();\n",
        module_name
    )
}

fn render_web_build_script() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
PROFILE="${FISSION_WEB_PROFILE:-dev}"
BUILD_ARGS=(build "$PROJECT_DIR" --target web --out-dir "$SCRIPT_DIR/pkg")

if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
else
  BUILD_ARGS+=(--dev)
fi

wasm-pack "${BUILD_ARGS[@]}"
"#
    .to_string()
}

fn render_web_run_script(_project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${{FISSION_WEB_HOST:-127.0.0.1}}"
PORT="${{FISSION_WEB_PORT:-8123}}"
URL="http://${{HOST}}:${{PORT}}/platforms/web/"

"$SCRIPT_DIR/build-wasm.sh"

printf 'Serving %s\n' "$URL"
printf 'Press Ctrl+C to stop the local server.\n'
if [[ "${{FISSION_WEB_OPEN:-0}}" == "1" ]]; then
  open "$URL"
fi

cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST"
"#
    )
}

fn render_app_main(package_name: &str) -> String {
    let lib_name = package_name.replace('-', "_");
    format!(
        r#"#[cfg(target_os = "android")]
fn main() {{}}

#[cfg(target_arch = "wasm32")]
fn main() {{}}

#[cfg(target_os = "ios")]
fn main() -> anyhow::Result<()> {{
    {lib_name}::run_mobile()
}}

#[cfg(not(any(target_arch = "wasm32", target_os = "ios", target_os = "android")))]
fn main() -> anyhow::Result<()> {{
    {lib_name}::run_desktop()
}}
"#
    )
}

const APP_LIB: &str = r#"pub mod app;

use crate::app::{CounterApp, CounterState};
use fission::prelude::*;

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<CounterState, CounterApp> {
    let app = MobileApp::new(CounterApp).with_title("Fission App");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<CounterState, CounterApp> {
    WebApp::new(CounterApp).with_title("Fission App")
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::new(CounterApp).run()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn run_mobile() -> anyhow::Result<()> {
    mobile_app().run()
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = mobile_app().run_with_android_app(app_handle);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
"#;

const APP_RS: &str = r#"use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterState {
    pub count: i32,
}

impl AppState for CounterState {}

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Increment;

fn on_increment(state: &mut CounterState, _action: Increment, _ctx: &mut ReducerContext<CounterState>) {
    state.count += 1;
}

pub struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let increment = ctx.bind(Increment, on_increment as Handler<CounterState, Increment>);

        Column {
            gap: Some(16.0),
            children: vec![
                Text::new(format!("Count: {}", view.state.count)).size(28.0).into_node(),
                Button {
                    on_press: Some(increment),
                    child: Some(Box::new(Text::new("Increment").into_node())),
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node()
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fission-cli-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn init_creates_project_files() {
        let dir = unique_dir("init");
        run([
            "fission",
            "init",
            dir.to_str().unwrap(),
            "--name",
            "hello-fission",
        ])
        .unwrap();

        assert!(dir.join("Cargo.toml").exists());
        assert!(dir.join("src/main.rs").exists());
        assert!(dir.join("src/lib.rs").exists());
        assert!(dir.join("src/app.rs").exists());
        assert!(dir.join("assets/app-icon.png").exists());
        assert!(dir.join("fission.toml").exists());
        assert!(dir.join("platforms/windows/README.md").exists());
        assert!(dir.join("platforms/macos/README.md").exists());
        assert!(dir.join("platforms/linux/README.md").exists());
    }

    #[test]
    fn add_target_updates_manifest_and_scaffold() {
        let dir = unique_dir("targets");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        run([
            "fission",
            "add-target",
            "web",
            "ios",
            "android",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let project = read_project_config(&dir).unwrap();
        assert!(project.targets.contains(&Target::Web));
        assert!(project.targets.contains(&Target::Ios));
        assert!(project.targets.contains(&Target::Android));
        assert!(dir.join("platforms/web/README.md").exists());
        assert!(dir.join("platforms/web/index.html").exists());
        assert!(dir.join("platforms/web/bootstrap.mjs").exists());
        assert!(dir.join("platforms/web/build-wasm.sh").exists());
        assert!(dir.join("platforms/web/run-browser.sh").exists());
        assert!(dir.join("platforms/ios/README.md").exists());
        assert!(dir.join("platforms/ios/Info.plist").exists());
        assert!(dir.join("platforms/ios/package-sim.sh").exists());
        assert!(dir.join("platforms/ios/run-sim.sh").exists());
        assert!(dir.join("platforms/android/README.md").exists());
        assert!(dir.join("platforms/android/AndroidManifest.xml").exists());
        assert!(dir.join("platforms/android/package-apk.sh").exists());
        assert!(dir.join("platforms/android/run-emulator.sh").exists());
        assert!(
            std::fs::read_to_string(dir.join("platforms/android/AndroidManifest.xml"))
                .unwrap()
                .contains("android:icon=\"@drawable/app_icon\"")
        );
        assert!(
            std::fs::read_to_string(dir.join("platforms/ios/package-sim.sh"))
                .unwrap()
                .contains("AppIcon.png")
        );
        assert!(
            std::fs::read_to_string(dir.join("platforms/web/index.html"))
                .unwrap()
                .contains("../../assets/app-icon.png")
        );
    }

    #[test]
    fn cargo_fission_alias_accepts_prefixed_subcommand() {
        let dir = unique_dir("cargo-fission");
        run([
            "cargo-fission",
            "fission",
            "init",
            dir.to_str().unwrap(),
            "--name",
            "cargo-fission-demo",
        ])
        .unwrap();

        assert!(dir.join("Cargo.toml").exists());
        assert!(dir.join("fission.toml").exists());
    }
}
