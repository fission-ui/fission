use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    write_file(&root.join("src/main.rs"), APP_MAIN)?;
    write_file(&root.join("src/app.rs"), APP_RS)?;
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
        Target::Android => platform_readme(
            "Android",
            "Scaffolded target. A generated Fission app can cross-compile for Android after `fission add-target`, but the CLI does not generate Android packaging or launcher files yet.",
            &[
                "Install the Rust target: `rustup target add aarch64-linux-android`.",
                "Set `ANDROID_HOME` and `ANDROID_NDK`.",
                "Set `CC_aarch64_linux_android` and `AR_aarch64_linux_android` from `$ANDROID_NDK/toolchains/llvm/prebuilt/<host>/bin`.",
                "Run `cargo check --target aarch64-linux-android` from the project root to verify the generated app compiles.",
                "Next work: teach `fission add-target` to generate the Android launcher/package files.",
            ],
        ),
        Target::Ios => {
            scaffold_ios_bundle(root, project)?;
            platform_readme(
                "iOS",
                "Runnable simulator target. The CLI generates a simulator app bundle template plus shell scripts that build, install, and launch the Fission app with `simctl`.",
                &[
                    "Install the Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.",
                    "Confirm the simulator SDK path with `xcrun --sdk iphonesimulator --show-sdk-path`.",
                    "Run `./platforms/ios/run-sim.sh` from the project root to build, install, and launch the app on the first available iPhone simulator.",
                    "Set `FISSION_TEST_CONTROL_PORT=<port>` before `run-sim.sh` to expose the in-app test control server on the host.",
                    "Set `IOS_SIM_DEVICE_ID=<udid>` if you want a specific simulator device.",
                ],
            )
        }
        Target::Web => platform_readme(
            "Web",
            "Scaffolded target. `fission-shell-web` is still in progress, so WASM/Web scaffolding is generated but not runnable yet.",
            &[
                "Install the Rust target: `rustup target add wasm32-unknown-unknown`.",
                "Install `wasm-pack` for the eventual shell/example path.",
                "Next work: implement `fission-shell-web` with a wasm entrypoint and WebGPU surface setup.",
            ],
        ),
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

fn write_file(path: &Path, contents: &str) -> Result<()> {
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

    format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nanyhow = \"1\"\nserde = {{ version = \"1\", features = [\"derive\"] }}\n{}",
        project.app.name, deps
    )
}

fn render_project_readme(project: &FissionProject) -> String {
    let mut targets = String::new();
    for target in &project.targets {
        targets.push_str(&format!("- `{}`\n", target.as_str()));
    }
    format!(
        "# {}\n\nGenerated by `fission init`.\n\n## Targets\n\n{}\n## Commands\n\n- `cargo run` -- launch the desktop app\n- `cargo fission add-target web ios android` -- scaffold more targets\n- `cat platforms/<target>/README.md` -- inspect the current prerequisites and status for each target\n\n## Status\n\nDesktop is runnable today. iOS is runnable on the Simulator after `cargo fission add-target ios` via `./platforms/ios/run-sim.sh`. Android still needs first-party launcher/package generation. Web remains scaffold-only until `fission-shell-web` lands.\n",
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

const APP_MAIN: &str = r#"mod app;

use crate::app::{CounterApp, CounterState};
use fission::prelude::*;

fn mobile_app() -> MobileApp<CounterState, CounterApp> {
    MobileApp::new(CounterApp).with_title("Fission App")
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = mobile_app().run_with_android_app(app_handle);
}

#[cfg(target_os = "android")]
fn main() {}

#[cfg(target_os = "ios")]
fn main() -> anyhow::Result<()> {
    mobile_app().run()
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn main() -> anyhow::Result<()> {
    DesktopApp::new(CounterApp).run()
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
        assert!(dir.join("src/app.rs").exists());
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
        assert!(dir.join("platforms/ios/README.md").exists());
        assert!(dir.join("platforms/ios/Info.plist").exists());
        assert!(dir.join("platforms/ios/package-sim.sh").exists());
        assert!(dir.join("platforms/ios/run-sim.sh").exists());
        assert!(dir.join("platforms/android/README.md").exists());
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
