use anyhow::Result;
use clap::Parser;
use std::path::Path;

mod cli;

#[cfg(test)]
use fission_command_core::{read_project_config, Target};

use cli::{Cli, Command, ServerCommand, SiteCommand};

pub fn run<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let mut argv: Vec<std::ffi::OsString> = args.into_iter().map(Into::into).collect();
    if let Some(bin) = argv.first() {
        if let Some(name) = Path::new(bin).file_name().and_then(|value| value.to_str()) {
            if name == "cargo-fission" {
                argv[0] = std::ffi::OsString::from("fission");
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
        } => fission_command_core::init_project(&path, name, app_id, local_path),
        Command::AddTarget {
            targets,
            project_dir,
        } => fission_command_core::add_targets(&project_dir, &targets),
        Command::AddCapability {
            capabilities,
            project_dir,
        } => fission_command_core::add_capabilities(&project_dir, &capabilities),
        Command::Doctor {
            targets,
            project_dir,
            strict,
        } => fission_command_run::doctor::run_doctor(&project_dir, &targets, strict),
        Command::Devices { project_dir, json } => {
            fission_command_run::list_devices(&project_dir, json)
        }
        Command::Run {
            target,
            device,
            project_dir,
            detach,
            release,
            host,
            port,
            no_open,
            headless,
        } => fission_command_run::run_app(fission_command_run::RunOptions {
            project_dir,
            target,
            device,
            detach,
            release,
            host,
            port,
            no_open,
            headless,
        }),
        Command::Build {
            target,
            project_dir,
            release,
        } => fission_command_run::build_app(fission_command_run::BuildOptions {
            project_dir,
            target,
            release,
        }),
        Command::Test {
            target,
            project_dir,
            headless,
        } => fission_command_run::test_app(fission_command_run::TestOptions {
            project_dir,
            target,
            headless,
        }),
        Command::Site { command } => match command {
            SiteCommand::Build {
                project_dir,
                release,
            } => fission_command_site::build(&project_dir, release),
            SiteCommand::Check {
                project_dir,
                release,
            } => fission_command_site::check(&project_dir, release),
            SiteCommand::Serve {
                project_dir,
                host,
                port,
                release,
                no_open,
            } => fission_command_site::serve(&project_dir, release, host, port, !no_open),
            SiteCommand::Routes { project_dir } => fission_command_site::routes(&project_dir),
        },
        Command::Server { command } => match command {
            ServerCommand::Build {
                project_dir,
                release,
            } => fission_command_server::build(&project_dir, release),
            ServerCommand::Check {
                project_dir,
                release,
            } => fission_command_server::check(&project_dir, release),
            ServerCommand::Serve {
                project_dir,
                host,
                port,
                release,
            } => fission_command_server::serve(&project_dir, release, host, port),
            ServerCommand::Routes { project_dir } => fission_command_server::routes(&project_dir),
            ServerCommand::Artifacts {
                project_dir,
                release,
                no_compile,
            } => fission_command_server::artifacts(&project_dir, release, !no_compile),
        },
        Command::Package {
            target,
            format,
            project_dir,
            release,
            json,
        } => fission_command_package::package(fission_command_package::PackageOptions {
            project_dir,
            target,
            format,
            release,
            json,
        }),
        Command::Distribute {
            action,
            provider,
            artifact,
            site,
            deploy,
            track,
            dry_run,
            yes,
            project_dir,
            json,
        } => fission_command_package::distribute(fission_command_package::DistributeOptions {
            project_dir,
            provider,
            action: action.unwrap_or(fission_command_package::DistributeAction::Publish),
            artifact,
            site,
            deploy,
            track,
            dry_run,
            yes,
            json,
        }),
        Command::Publish {
            provider,
            artifact,
            site,
            deploy,
            track,
            dry_run,
            yes,
            project_dir,
            json,
        } => fission_command_package::distribute(fission_command_package::DistributeOptions {
            project_dir,
            provider,
            action: fission_command_package::DistributeAction::Publish,
            artifact,
            site,
            deploy,
            track,
            dry_run,
            yes,
            json,
        }),
        Command::Readiness {
            kind,
            target,
            format,
            provider,
            artifact,
            site,
            track,
            project_dir,
            json,
        } => fission_command_package::readiness(fission_command_package::ReadinessOptions {
            project_dir,
            kind,
            target,
            format,
            provider,
            artifact,
            site,
            track,
            json,
        }),
        Command::ReleaseConfig { command } => fission_command_release::release_config(command),
        Command::ReleaseContent { command } => fission_command_release::release_content(command),
        Command::Beta { command } => fission_command_release::beta(command),
        Command::Signing { command } => fission_command_release::signing(command),
        Command::Reviews { command } => fission_command_release::reviews(command),
        Command::ReleaseWorkflow { command } => fission_command_release::release_workflow(command),
        Command::Auth { command } => fission_command_release::auth(command),
        Command::Logs {
            target,
            device,
            project_dir,
            follow,
        } => fission_command_run::attach_logs(fission_command_run::LogOptions {
            project_dir,
            target,
            device,
            follow,
        }),
        Command::Ui {
            project_dir,
            screenshot,
            exit_after_render,
            width,
            height,
        } => fission_command_ui::run_ui(fission_command_ui::UiOptions {
            project_dir,
            screenshot,
            exit_after_render,
            width,
            height,
        }),
        Command::ServeWeb {
            project_dir,
            host,
            port,
            open,
        } => fission_command_run::serve_web(fission_command_run::ServeWebOptions {
            project_dir,
            host,
            port,
            open,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, process::Command};

    fn unique_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cargo-fission-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[cfg(unix)]
    fn write_executable(path: impl AsRef<std::path::Path>, contents: &str) {
        use std::os::unix::fs::PermissionsExt;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    fn path_with_fake_bin(fake_bin: &std::path::Path) -> String {
        let existing = std::env::var("PATH").unwrap_or_default();
        format!("{}:{existing}", fake_bin.display())
    }

    #[cfg(unix)]
    fn write_fake_cargo(fake_bin: &std::path::Path) {
        write_executable(
            fake_bin.join("cargo"),
            r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "metadata" ]]; then
  printf '%s\n' "$FAKE_TARGET_DIR"
  exit 0
fi
if [[ "${1:-}" == "build" ]]; then
  target=""
  package=""
  profile="debug"
  args=("$@")
  for ((i = 0; i < ${#args[@]}; i++)); do
    case "${args[$i]}" in
      --target) target="${args[$((i + 1))]}" ;;
      --package) package="${args[$((i + 1))]}" ;;
      --release) profile="release" ;;
    esac
  done
  if [[ -z "$target" || -z "$package" ]]; then
    printf 'fake cargo expected --target and --package\n' >&2
    exit 2
  fi
  artifact_dir="$FAKE_TARGET_DIR/$target/$profile"
  mkdir -p "$artifact_dir"
  lib_name="${package//-/_}"
  printf 'fake native library\n' > "$artifact_dir/lib${lib_name}.so"
  printf '#!/usr/bin/env sh\nexit 0\n' > "$artifact_dir/$package"
  chmod +x "$artifact_dir/$package"
  exit 0
fi
printf 'unsupported fake cargo invocation: %s\n' "$*" >&2
exit 2
"#,
        );
    }

    #[cfg(unix)]
    fn write_fake_python3(fake_bin: &std::path::Path) {
        write_executable(
            fake_bin.join("python3"),
            r#"#!/usr/bin/env bash
set -euo pipefail
script=$(cat)
if [[ "$script" == *"import plistlib"* ]]; then
  printf 'plistlib must not be used by generated mobile package scripts\n' >&2
  exit 44
fi
if [[ "$script" == *"cargo\", \"metadata\""* || "$script" == *"metadata = json.loads"* ]]; then
  printf '%s\n' "$FAKE_TARGET_DIR"
  exit 0
fi
if [[ "$script" == *"android:minSdkVersion"* ]]; then
  if [[ "${1:-}" == "-" ]]; then
    shift
  fi
  source="$1"
  dest="$2"
  min_api="$3"
  target_api="$4"
  has_code=false
  if [[ -f "$(dirname "$dest")/apk-root/classes.dex" ]]; then
    has_code=true
  fi
  sed -E \
    -e "s/android:minSdkVersion=\"[0-9]+\"/android:minSdkVersion=\"$min_api\"/" \
    -e "s/android:targetSdkVersion=\"[0-9]+\"/android:targetSdkVersion=\"$target_api\"/" \
    -e "s/android:hasCode=\"(true|false)\"/android:hasCode=\"$has_code\"/" \
    "$source" > "$dest"
  exit 0
fi
printf 'unsupported fake python3 script\n%s\n' "$script" >&2
exit 2
"#,
        );
    }

    #[cfg(unix)]
    fn write_fake_ios_tools(fake_bin: &std::path::Path) {
        write_executable(
            fake_bin.join("xcrun"),
            r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--find" && "${2:-}" == "plutil" ]]; then
  printf '%s/plutil\n' "$FISSION_FAKE_BIN"
  exit 0
fi
if [[ "${1:-}" == "--find" && "${2:-}" == "ibtool" ]]; then
  printf '%s/ibtool\n' "$FISSION_FAKE_BIN"
  exit 0
fi
printf 'unsupported fake xcrun invocation: %s\n' "$*" >&2
exit 2
"#,
        );
        write_executable(
            fake_bin.join("plutil"),
            r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" != "-replace" || "${3:-}" != "-string" ]]; then
  printf 'unsupported fake plutil invocation: %s\n' "$*" >&2
  exit 2
fi
key="$2"
value="$4"
file="$5"
tmp="$(mktemp)"
awk -v key="$key" -v value="$value" '
  replace_next {
    print "  <string>" value "</string>"
    replace_next = 0
    next
  }
  {
    print
    if ($0 ~ "<key>" key "</key>") {
      replace_next = 1
    }
  }
' "$file" > "$tmp"
mv "$tmp" "$file"
"#,
        );
        write_executable(
            fake_bin.join("ibtool"),
            r#"#!/usr/bin/env bash
set -euo pipefail
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  if [[ "${args[$i]}" == "--compile" ]]; then
    mkdir -p "${args[$((i + 1))]}"
    exit 0
  fi
done
printf 'fake ibtool missing --compile\n' >&2
exit 2
"#,
        );
    }

    #[cfg(unix)]
    fn write_fake_android_tools(android_home: &std::path::Path, fake_bin: &std::path::Path) {
        let build_tools = android_home.join("build-tools/35.0.0");
        let ndk_bin = android_home.join("ndk/27.0.0/toolchains/llvm/prebuilt/linux-x86_64/bin");
        fs::create_dir_all(android_home.join("platforms/android-35")).unwrap();
        fs::write(android_home.join("platforms/android-35/android.jar"), "").unwrap();
        fs::create_dir_all(&build_tools).unwrap();
        fs::create_dir_all(&ndk_bin).unwrap();
        fs::write(ndk_bin.join("aarch64-linux-android24-clang"), "").unwrap();
        fs::write(ndk_bin.join("llvm-ar"), "").unwrap();

        write_executable(
            build_tools.join("aapt"),
            r#"#!/usr/bin/env bash
set -euo pipefail
out=""
manifest=""
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  case "${args[$i]}" in
    -F) out="${args[$((i + 1))]}" ;;
    -M) manifest="${args[$((i + 1))]}" ;;
  esac
done
if [[ -z "$out" || -z "$manifest" ]]; then
  printf 'fake aapt missing -F or -M\n' >&2
  exit 2
fi
mkdir -p "$(dirname "$out")"
{
  printf 'AAPT_MANIFEST_BEGIN\n'
  cat "$manifest"
  printf '\nAAPT_MANIFEST_END\n'
} > "$out"
"#,
        );
        write_executable(
            build_tools.join("zipalign"),
            r#"#!/usr/bin/env bash
set -euo pipefail
args=("$@")
count=${#args[@]}
input="${args[$((count - 2))]}"
output="${args[$((count - 1))]}"
cp "$input" "$output"
"#,
        );
        write_executable(
            build_tools.join("apksigner"),
            r#"#!/usr/bin/env bash
set -euo pipefail
out=""
input=""
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  if [[ "${args[$i]}" == "--out" ]]; then
    out="${args[$((i + 1))]}"
  fi
done
input="${args[$((${#args[@]} - 1))]}"
if [[ -z "$out" ]]; then
  printf 'fake apksigner missing --out\n' >&2
  exit 2
fi
cp "$input" "$out"
"#,
        );
        write_executable(
            fake_bin.join("zip"),
            r#"#!/usr/bin/env bash
set -euo pipefail
archive=""
entries=()
for arg in "$@"; do
  if [[ "$arg" == -* ]]; then
    continue
  fi
  if [[ -z "$archive" ]]; then
    archive="$arg"
  else
    entries+=("$arg")
  fi
done
if [[ -z "$archive" ]]; then
  printf 'fake zip missing archive\n' >&2
  exit 2
fi
for entry in "${entries[@]}"; do
  if [[ -d "$entry" ]]; then
    while IFS= read -r file; do
      printf 'APK_ENTRY=%s\n' "$file" >> "$archive"
    done < <(find "$entry" -type f | sort)
  elif [[ -f "$entry" ]]; then
    printf 'APK_ENTRY=%s\n' "$entry" >> "$archive"
  fi
done
"#,
        );
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
        let readme = std::fs::read_to_string(dir.join("README.md")).unwrap();
        assert!(readme.contains("fission devices --project-dir ."));
        assert!(readme.contains("fission run --project-dir ."));
        assert!(readme.contains("fission logs --target <target>"));
        assert!(readme.contains("fission build --target <target>"));
        assert!(readme.contains("fission test --target <target>"));
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("default-features = false"));
        assert!(manifest.contains("features = [\"desktop\"]"));
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
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("default-features = false"));
        assert!(manifest.contains("features = [\"desktop\", \"web\", \"android\", \"ios\"]"));
        assert!(dir.join("platforms/web/README.md").exists());
        assert!(dir.join("platforms/web/index.html").exists());
        assert!(dir.join("platforms/web/bootstrap.mjs").exists());
        assert!(dir.join("platforms/web/build-wasm.sh").exists());
        assert!(dir.join("platforms/web/run-browser.sh").exists());
        assert!(dir.join("platforms/web/test-browser.sh").exists());
        assert!(dir.join("platforms/ios/README.md").exists());
        assert!(dir.join("platforms/ios/Info.plist").exists());
        assert!(dir.join("platforms/ios/LaunchScreen.storyboard").exists());
        assert!(dir.join("platforms/ios/package-sim.sh").exists());
        assert!(dir.join("platforms/ios/run-sim.sh").exists());
        assert!(dir.join("platforms/ios/test-sim.sh").exists());
        assert!(dir.join("platforms/android/README.md").exists());
        assert!(dir.join("platforms/android/AndroidManifest.xml").exists());
        assert!(dir.join("platforms/android/res/values/colors.xml").exists());
        assert!(dir.join("platforms/android/res/values/styles.xml").exists());
        assert!(dir
            .join("platforms/android/res/drawable/fission_splash_background.xml")
            .exists());
        assert!(dir.join("platforms/android/package-apk.sh").exists());
        assert!(dir.join("platforms/android/run-emulator.sh").exists());
        assert!(dir.join("platforms/android/test-emulator.sh").exists());
        let android_manifest =
            std::fs::read_to_string(dir.join("platforms/android/AndroidManifest.xml")).unwrap();
        assert!(android_manifest.contains("android:icon=\"@drawable/app_icon\""));
        assert!(android_manifest.contains("android:hasCode=\"false\""));
        assert!(android_manifest.contains("android:targetSdkVersion=\"35\""));
        assert!(android_manifest.contains("android:theme=\"@style/FissionLaunchTheme\""));
        let android_styles =
            std::fs::read_to_string(dir.join("platforms/android/res/values/styles.xml")).unwrap();
        assert!(android_styles.contains("android:windowBackground"));
        assert!(android_styles.contains("android:windowSplashScreenAnimatedIcon"));
        let android_package_script =
            std::fs::read_to_string(dir.join("platforms/android/package-apk.sh")).unwrap();
        assert!(android_package_script.contains("detect_android_toolchain"));
        assert!(android_package_script
            .contains("darwin-aarch64 darwin-x86_64 linux-x86_64 windows-x86_64"));
        assert!(android_package_script.contains(
            "ANDROID_MIN_API_LEVEL=\"${ANDROID_MIN_API_LEVEL:-${ANDROID_API_LEVEL:-24}}\""
        ));
        assert!(android_package_script.contains("ANDROID_TARGET_API_LEVEL="));
        assert!(
            android_package_script.contains("aarch64-linux-android${ANDROID_MIN_API_LEVEL}-clang")
        );
        assert!(android_package_script.contains("BUILD_MANIFEST"));
        assert!(android_package_script.contains("android:targetSdkVersion=\"{target_api}\""));
        assert!(android_package_script.contains("import pathlib"));
        assert!(android_package_script.contains("with_name(\"apk-root\")"));
        assert!(android_package_script.contains("android:hasCode="));
        assert!(android_package_script.contains("cp -R \"$SCRIPT_DIR/res/.\""));
        assert!(android_package_script.contains("fission_splash_image.png"));
        assert!(android_package_script.contains("APP_ICONS"));
        assert!(android_package_script.contains("res/drawable-nodpi/app_icon.*"));
        let android_run_script =
            std::fs::read_to_string(dir.join("platforms/android/run-emulator.sh")).unwrap();
        assert!(android_run_script.contains("ANDROID_EMULATOR_API_LEVEL"));
        assert!(android_run_script.contains("fission doctor android"));
        assert!(android_run_script.contains("wait_for_android_boot()"));
        assert!(android_run_script.contains("cmd package list packages"));
        assert!(android_run_script.contains("ADB_INSTALL_FLAGS:---no-streaming -r"));
        assert!(!android_run_script.contains("wait_for_android_boot() {\n  wait_for_android_boot"));
        assert!(!android_run_script.contains("  wait_for_android_boot\n  wait_for_android_boot"));
        assert!(
            std::fs::read_to_string(dir.join("platforms/android/README.md"))
                .unwrap()
                .contains("fission run --target android")
        );
        let android_test_script =
            std::fs::read_to_string(dir.join("platforms/android/test-emulator.sh")).unwrap();
        assert!(android_test_script.contains("/health"));
        let ios_package_script =
            std::fs::read_to_string(dir.join("platforms/ios/package-sim.sh")).unwrap();
        assert!(ios_package_script.contains("TARGET=\"${IOS_SIM_TARGET:-aarch64-apple-ios-sim}\""));
        assert!(ios_package_script.contains("PROFILE=\"${IOS_SIM_PROFILE:-debug}\""));
        assert!(ios_package_script.contains("BUNDLE_ID=\"${IOS_BUNDLE_ID:-com.example."));
        assert!(ios_package_script.contains("DISPLAY_NAME=\"${IOS_DISPLAY_NAME:-"));
        assert!(ios_package_script.contains("EXECUTABLE_NAME=\"${IOS_EXECUTABLE_NAME:-"));
        assert!(ios_package_script.contains("xcrun --find plutil"));
        assert!(ios_package_script.contains("-replace CFBundleIdentifier -string"));
        assert!(!ios_package_script.contains("import plistlib"));
        assert!(ios_package_script.contains("PkgInfo"));
        assert!(ios_package_script.contains("PLATFORM_APP_ICONS"));
        assert!(ios_package_script.contains("AppIcon.png"));
        assert!(ios_package_script.contains("LaunchScreen.storyboard"));
        assert!(ios_package_script.contains("ibtool"));
        assert!(ios_package_script.contains("LaunchScreen.storyboardc"));
        assert!(ios_package_script.contains("SplashImage.png"));
        let ios_plist = std::fs::read_to_string(dir.join("platforms/ios/Info.plist")).unwrap();
        assert!(ios_plist.contains("UILaunchStoryboardName"));
        let ios_run_script = std::fs::read_to_string(dir.join("platforms/ios/run-sim.sh")).unwrap();
        assert!(ios_run_script.contains("BUNDLE_ID=\"${IOS_BUNDLE_ID:-com.example."));
        assert!(ios_run_script.contains("IOS_SIM_UNINSTALL_BEFORE_INSTALL"));
        assert!(ios_run_script.contains(
            "xcrun simctl launch --terminate-running-process \"$DEVICE_ID\" \"$BUNDLE_ID\""
        ));
        assert!(std::fs::read_to_string(dir.join("platforms/ios/README.md"))
            .unwrap()
            .contains("fission run --target ios"));
        assert!(
            std::fs::read_to_string(dir.join("platforms/ios/test-sim.sh"))
                .unwrap()
                .contains("/health")
        );
        assert!(
            std::fs::read_to_string(dir.join("platforms/web/index.html"))
                .unwrap()
                .contains("../../assets/app-icon.png")
        );
        let web_index = std::fs::read_to_string(dir.join("platforms/web/index.html")).unwrap();
        assert!(web_index.contains("id=\"fission-web-mount\""));
        assert!(web_index.contains("height: 100vh"));
        assert!(web_index.contains("outline: none"));
        assert!(web_index.contains("touch-action: none"));
        assert!(!web_index.contains("Generated by"));
        let web_test_script =
            std::fs::read_to_string(dir.join("platforms/web/test-browser.sh")).unwrap();
        assert!(web_test_script.contains("--remote-debugging-port=\"$CDP_PORT\""));
        assert!(web_test_script.contains("/json/list"));
        assert!(std::fs::read_to_string(dir.join("platforms/web/README.md"))
            .unwrap()
            .contains("fission run --target web"));
    }

    #[test]
    fn init_hardens_existing_android_native_only_scaffold() {
        let dir = unique_dir("android-hardening");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        run([
            "fission",
            "add-target",
            "android",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let manifest_path = dir.join("platforms/android/AndroidManifest.xml");
        let package_path = dir.join("platforms/android/package-apk.sh");
        fs::write(
            &manifest_path,
            fs::read_to_string(&manifest_path)
                .unwrap()
                .replace("android:hasCode=\"false\"", "android:hasCode=\"true\""),
        )
        .unwrap();
        fs::write(
            &package_path,
            fs::read_to_string(&package_path)
                .unwrap()
                .replace("import pathlib\n", "")
                .replace(
                    r#"has_code = "true" if pathlib.Path(dest).with_name("apk-root").joinpath("classes.dex").exists() else "false"
manifest = re.sub(r'android:hasCode="(?:true|false)"', f'android:hasCode="{has_code}"', manifest)
"#,
                    "",
                ),
        )
        .unwrap();

        run(["fission", "init", dir.to_str().unwrap()]).unwrap();

        let manifest = fs::read_to_string(manifest_path).unwrap();
        assert!(manifest.contains("android:hasCode=\"false\""));
        let package_script = fs::read_to_string(package_path).unwrap();
        assert!(package_script.contains("import pathlib"));
        assert!(package_script.contains("with_name(\"apk-root\")"));
        assert!(package_script.contains("android:hasCode="));
    }

    #[test]
    fn init_hardens_existing_ios_package_script_without_replacing_user_files() {
        let dir = unique_dir("ios-package-hardening");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        run([
            "fission",
            "add-target",
            "ios",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();
        let script_path = dir.join("platforms/ios/package-sim.sh");
        let mut script = fs::read_to_string(&script_path).unwrap();
        script = script.replace(
            r#"cp "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist"
PLUTIL=$(xcrun --find plutil 2>/dev/null || command -v plutil || true)
if [[ -z "$PLUTIL" ]]; then
  printf 'plutil not found. Install Xcode command line tools to package the iOS simulator app.\n' >&2
  exit 1
fi
"$PLUTIL" -replace CFBundleIdentifier -string "$BUNDLE_ID" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleDisplayName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleExecutable -string "$EXECUTABLE_NAME" "$BUNDLE_DIR/Info.plist"
"#,
            r#"python3 - <<'PY' "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist" "$BUNDLE_ID" "$DISPLAY_NAME" "$EXECUTABLE_NAME"
import plistlib
import sys

source, dest, bundle_id, display_name, executable_name = sys.argv[1:]
with open(source, "rb") as handle:
    plist = plistlib.load(handle)
plist["CFBundleIdentifier"] = bundle_id
plist["CFBundleDisplayName"] = display_name
plist["CFBundleName"] = display_name
plist["CFBundleExecutable"] = executable_name
with open(dest, "wb") as handle:
    plistlib.dump(plist, handle, sort_keys=False)
PY
"#,
        );
        fs::write(&script_path, script).unwrap();

        run(["fission", "init", dir.to_str().unwrap()]).unwrap();

        let hardened = fs::read_to_string(script_path).unwrap();
        assert!(hardened.contains("xcrun --find plutil"));
        assert!(hardened.contains("-replace CFBundleExecutable -string"));
        assert!(!hardened.contains("import plistlib"));
    }

    #[cfg(unix)]
    #[test]
    fn ios_package_script_executes_without_python_plistlib() {
        let dir = unique_dir("ios-package-e2e");
        run([
            "fission",
            "init",
            dir.to_str().unwrap(),
            "--name",
            "ios-package-e2e",
            "--app-id",
            "com.example.iospackagee2e",
        ])
        .unwrap();
        run([
            "fission",
            "add-target",
            "ios",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let fake_bin = dir.join("fake-bin");
        let fake_target = dir.join("fake-target");
        fs::create_dir_all(&fake_bin).unwrap();
        fs::create_dir_all(&fake_target).unwrap();
        write_fake_cargo(&fake_bin);
        write_fake_python3(&fake_bin);
        write_fake_ios_tools(&fake_bin);

        let output = Command::new("bash")
            .arg("platforms/ios/package-sim.sh")
            .current_dir(&dir)
            .env("PATH", path_with_fake_bin(&fake_bin))
            .env("FAKE_TARGET_DIR", &fake_target)
            .env("FISSION_FAKE_BIN", &fake_bin)
            .env("IOS_BUNDLE_ID", "com.example.overridden")
            .env("IOS_DISPLAY_NAME", "Overridden iOS")
            .env("IOS_EXECUTABLE_NAME", "OverriddenExecutable")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "package-sim.sh failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let bundle_dir = String::from_utf8(output.stdout).unwrap();
        let bundle_dir = PathBuf::from(bundle_dir.trim());
        assert!(bundle_dir.join("OverriddenExecutable").exists());
        assert!(bundle_dir.join("LaunchScreen.storyboardc").exists());
        let plist = fs::read_to_string(bundle_dir.join("Info.plist")).unwrap();
        assert!(plist.contains("<string>com.example.overridden</string>"));
        assert!(plist.contains("<string>Overridden iOS</string>"));
        assert!(plist.contains("<string>OverriddenExecutable</string>"));
    }

    #[cfg(unix)]
    #[test]
    fn android_package_script_builds_native_only_apk_metadata() {
        let dir = unique_dir("android-package-e2e");
        run([
            "fission",
            "init",
            dir.to_str().unwrap(),
            "--name",
            "android-package-e2e",
            "--app-id",
            "com.example.androidpackagee2e",
        ])
        .unwrap();
        run([
            "fission",
            "add-target",
            "android",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let fake_bin = dir.join("fake-bin");
        let fake_target = dir.join("fake-target");
        let android_home = dir.join("fake-android-sdk");
        fs::create_dir_all(&fake_bin).unwrap();
        fs::create_dir_all(&fake_target).unwrap();
        write_fake_cargo(&fake_bin);
        write_fake_python3(&fake_bin);
        write_fake_android_tools(&android_home, &fake_bin);
        let keystore = dir.join("debug.keystore");
        fs::write(&keystore, "fake debug keystore").unwrap();

        let output = Command::new("bash")
            .arg("platforms/android/package-apk.sh")
            .current_dir(&dir)
            .env("PATH", path_with_fake_bin(&fake_bin))
            .env("FAKE_TARGET_DIR", &fake_target)
            .env("ANDROID_HOME", &android_home)
            .env("ANDROID_DEBUG_KEYSTORE", &keystore)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "package-apk.sh failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let apk = String::from_utf8(output.stdout).unwrap();
        let apk = PathBuf::from(apk.trim());
        let apk_payload = fs::read_to_string(apk).unwrap();
        assert!(apk_payload.contains("android:hasCode=\"false\""));
        assert!(apk_payload.contains("android:minSdkVersion=\"24\""));
        assert!(apk_payload.contains("android:targetSdkVersion=\"35\""));
        assert!(apk_payload.contains("APK_ENTRY=lib/arm64-v8a/libandroid_package_e2e.so"));
        assert!(!apk_payload.contains("APK_ENTRY=classes.dex"));
    }

    #[test]
    fn init_existing_project_enables_features_for_detected_mobile_targets() {
        let dir = unique_dir("init-mobile-features");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("platforms/android")).unwrap();
        fs::create_dir_all(dir.join("platforms/ios")).unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            r#"[package]
name = "existing-mobile"
version = "0.1.0"
edition = "2021"

[dependencies]
fission = { version = "0.4.1", default-features = false, features = ["desktop"] }
"#,
        )
        .unwrap();

        run(["fission", "init", dir.to_str().unwrap()]).unwrap();

        let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("default-features = false"));
        assert!(manifest.contains(r#"features = ["desktop", "android", "ios"]"#));
    }

    #[test]
    fn add_target_updates_multiline_fission_dependency_features() {
        let dir = unique_dir("multiline-features");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            r#"[package]
name = "multiline-features"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"

[dependencies.fission]
version = "0.4.1"
default-features = true
features = ["desktop"]
"#,
        )
        .unwrap();

        run([
            "fission",
            "add-target",
            "android",
            "ios",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("[dependencies.fission]"));
        assert!(manifest.contains("default-features = false"));
        assert!(manifest.contains(r#"features = ["desktop", "android", "ios"]"#));
    }

    #[test]
    fn custom_icon_config_is_preserved_and_applied_to_mobile_scaffolds() {
        let dir = unique_dir("custom-icons");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        fs::copy(
            dir.join("assets/app-icon.png"),
            dir.join("assets/shared-icon.png"),
        )
        .unwrap();
        fs::copy(
            dir.join("assets/app-icon.png"),
            dir.join("assets/android-icon.png"),
        )
        .unwrap();
        fs::copy(
            dir.join("assets/app-icon.png"),
            dir.join("assets/ios-icon.png"),
        )
        .unwrap();
        let mut manifest = fs::read_to_string(dir.join("fission.toml")).unwrap();
        manifest.push_str(
            r##"

[package.icons]
mode = "mixed"
source = "assets/shared-icon.png"
safe_zone = 0.72
allow_upscale = false

[package.icons.android]
source = "assets/android-icon.png"

[package.icons.ios]
source = "assets/ios-icon.png"
"##,
        );
        fs::write(dir.join("fission.toml"), manifest).unwrap();

        run([
            "fission",
            "add-target",
            "ios",
            "android",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let manifest = fs::read_to_string(dir.join("fission.toml")).unwrap();
        assert!(manifest.contains("[package.icons]"));
        assert!(manifest.contains("source = \"assets/shared-icon.png\""));
        assert!(manifest.contains("[package.icons.android]"));
        assert!(manifest.contains("[package.icons.ios]"));
        assert!(dir
            .join("platforms/android/res/drawable-nodpi/app_icon.png")
            .exists());
        assert!(dir.join("platforms/ios/AppIcon.png").exists());
        let android_manifest =
            fs::read_to_string(dir.join("platforms/android/AndroidManifest.xml")).unwrap();
        assert!(android_manifest.contains("android:icon=\"@drawable/app_icon\""));
        let android_package =
            fs::read_to_string(dir.join("platforms/android/package-apk.sh")).unwrap();
        assert!(android_package.contains("APP_ICONS"));
        let ios_package = fs::read_to_string(dir.join("platforms/ios/package-sim.sh")).unwrap();
        assert!(ios_package.contains("PLATFORM_APP_ICONS"));
    }

    #[test]
    fn custom_splash_config_updates_mobile_platform_files() {
        let dir = unique_dir("custom-splash");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        fs::copy(
            dir.join("assets/app-icon.png"),
            dir.join("assets/custom-splash.png"),
        )
        .unwrap();
        let mut manifest = fs::read_to_string(dir.join("fission.toml")).unwrap();
        manifest.push_str(
            r##"

[app.splash]
background_color = "#123456"
image = "assets/custom-splash.png"
resize_mode = "cover"
android_animation_duration_ms = 650
"##,
        );
        fs::write(dir.join("fission.toml"), manifest).unwrap();

        run([
            "fission",
            "add-target",
            "ios",
            "android",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let android_colors =
            fs::read_to_string(dir.join("platforms/android/res/values/colors.xml")).unwrap();
        assert!(android_colors.contains("#123456"));
        let android_styles =
            fs::read_to_string(dir.join("platforms/android/res/values/styles.xml")).unwrap();
        assert!(android_styles.contains("650"));
        assert!(dir
            .join("platforms/android/res/drawable-nodpi/fission_splash_image.png")
            .exists());
        let storyboard =
            fs::read_to_string(dir.join("platforms/ios/LaunchScreen.storyboard")).unwrap();
        assert!(storyboard.contains("scaleAspectFill"));
        assert!(storyboard.contains("red=\"0.070588\""));
        assert!(dir.join("platforms/ios/SplashImage.png").exists());
    }

    #[test]
    fn add_capability_updates_project_and_platform_config() {
        let dir = unique_dir("capability");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        run([
            "fission",
            "add-target",
            "ios",
            "android",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();
        run([
            "fission",
            "add-capability",
            "nfc",
            "biometric",
            "bluetooth",
            "barcode-scanner",
            "camera",
            "geolocation",
            "haptics",
            "microphone",
            "volume-control",
            "wifi",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        let project = read_project_config(&dir).unwrap();
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Nfc));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Biometric));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Bluetooth));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::BarcodeScanner));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Camera));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Geolocation));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Haptics));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Microphone));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::VolumeControl));
        assert!(project
            .capabilities
            .contains(&fission_command_core::PlatformCapability::Wifi));

        let android_manifest =
            std::fs::read_to_string(dir.join("platforms/android/AndroidManifest.xml")).unwrap();
        assert!(android_manifest.contains("android.permission.NFC"));
        assert!(android_manifest.contains("android.hardware.nfc"));
        assert!(android_manifest.contains("android.permission.USE_BIOMETRIC"));
        assert!(android_manifest.contains("android.permission.BLUETOOTH_SCAN"));
        assert!(android_manifest.contains("android.permission.BLUETOOTH_CONNECT"));
        assert!(android_manifest.contains("android.hardware.bluetooth_le"));
        assert!(android_manifest.contains("android.permission.CAMERA"));
        assert!(android_manifest.contains("android.hardware.camera.flash"));
        assert!(android_manifest.contains("android.permission.ACCESS_FINE_LOCATION"));
        assert!(android_manifest.contains("android.permission.VIBRATE"));
        assert!(android_manifest.contains("android.permission.RECORD_AUDIO"));
        assert!(android_manifest.contains("android.permission.MODIFY_AUDIO_SETTINGS"));
        assert!(android_manifest.contains("android.permission.NEARBY_WIFI_DEVICES"));
        assert!(android_manifest.contains("android.permission.ACCESS_WIFI_STATE"));
        assert!(dir
            .join("platforms/android/java/rs/fission/runtime/FissionAndroidCapabilities.java")
            .exists());

        let ios_info = std::fs::read_to_string(dir.join("platforms/ios/Info.plist")).unwrap();
        assert!(ios_info.contains("NFCReaderUsageDescription"));
        assert!(ios_info.contains("NSFaceIDUsageDescription"));
        assert!(ios_info.contains("NSBluetoothAlwaysUsageDescription"));
        assert!(ios_info.contains("NSCameraUsageDescription"));
        assert!(ios_info.contains("NSLocationWhenInUseUsageDescription"));
        assert!(ios_info.contains("NSMicrophoneUsageDescription"));
        let ios_entitlements =
            std::fs::read_to_string(dir.join("platforms/ios/Entitlements.plist")).unwrap();
        assert!(ios_entitlements.contains("com.apple.developer.nfc.readersession.formats"));
        assert!(ios_entitlements.contains("com.apple.developer.networking.wifi-info"));
    }

    #[test]
    fn init_existing_project_preserves_user_files_and_detects_targets() {
        let dir = unique_dir("existing");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("platforms/web")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"existing-web\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn existing() {}\n").unwrap();
        fs::write(dir.join("README.md"), "# keep me\n").unwrap();
        fs::write(
            dir.join("platforms/web/index.html"),
            "<!doctype html><title>keep me</title>\n",
        )
        .unwrap();

        run(["fission", "init", dir.to_str().unwrap()]).unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("README.md")).unwrap(),
            "# keep me\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("src/main.rs")).unwrap(),
            "fn main() {}\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("src/lib.rs")).unwrap(),
            "pub fn existing() {}\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("platforms/web/index.html")).unwrap(),
            "<!doctype html><title>keep me</title>\n"
        );

        let project = read_project_config(&dir).unwrap();
        assert_eq!(project.app.name, "existing-web");
        assert!(project.targets.contains(&Target::Web));
        assert!(project.targets.contains(&Target::Macos));
        assert!(project.targets.contains(&Target::Linux));
        assert!(project.targets.contains(&Target::Windows));
        assert!(dir.join("platforms/web/README.md").exists());
        assert!(dir.join("platforms/web/bootstrap.mjs").exists());
        assert!(dir.join("assets/app-icon.png").exists());
    }

    #[test]
    fn init_existing_project_is_idempotent() {
        let dir = unique_dir("idempotent");
        run(["fission", "init", dir.to_str().unwrap(), "--name", "idem"]).unwrap();
        let manifest = fs::read_to_string(dir.join("fission.toml")).unwrap();
        let main = fs::read_to_string(dir.join("src/main.rs")).unwrap();

        run(["fission", "init", dir.to_str().unwrap()]).unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("fission.toml")).unwrap(),
            manifest
        );
        assert_eq!(fs::read_to_string(dir.join("src/main.rs")).unwrap(), main);
    }

    #[test]
    fn add_target_preserves_existing_target_files() {
        let dir = unique_dir("preserve-target");
        run([
            "fission",
            "init",
            dir.to_str().unwrap(),
            "--name",
            "preserve-target",
        ])
        .unwrap();
        fs::create_dir_all(dir.join("platforms/web")).unwrap();
        fs::write(
            dir.join("platforms/web/index.html"),
            "<!doctype html><title>custom</title>\n",
        )
        .unwrap();
        fs::write(dir.join("README.md"), "# custom readme\n").unwrap();

        run([
            "fission",
            "add-target",
            "web",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("platforms/web/index.html")).unwrap(),
            "<!doctype html><title>custom</title>\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("README.md")).unwrap(),
            "# custom readme\n"
        );
        assert!(dir.join("platforms/web/README.md").exists());
        assert!(dir.join("platforms/web/bootstrap.mjs").exists());
        let project = read_project_config(&dir).unwrap();
        assert!(project.targets.contains(&Target::Web));
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

    #[test]
    fn doctor_command_runs_in_non_strict_mode() {
        let dir = unique_dir("doctor");
        run(["fission", "init", dir.to_str().unwrap()]).unwrap();
        run([
            "fission",
            "doctor",
            "web",
            "--project-dir",
            dir.to_str().unwrap(),
        ])
        .unwrap();
    }
}
