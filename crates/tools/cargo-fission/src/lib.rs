use anyhow::Result;
use clap::Parser;
use std::path::Path;

mod cli;

#[cfg(test)]
use fission_command_core::{read_project_config, Target};

use cli::{Cli, Command, SiteCommand};

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
    use std::{fs, path::PathBuf};

    fn unique_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cargo-fission-{}-{}", name, std::process::id()));
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
        assert!(dir.join("platforms/ios/package-sim.sh").exists());
        assert!(dir.join("platforms/ios/run-sim.sh").exists());
        assert!(dir.join("platforms/ios/test-sim.sh").exists());
        assert!(dir.join("platforms/android/README.md").exists());
        assert!(dir.join("platforms/android/AndroidManifest.xml").exists());
        assert!(dir.join("platforms/android/package-apk.sh").exists());
        assert!(dir.join("platforms/android/run-emulator.sh").exists());
        assert!(dir.join("platforms/android/test-emulator.sh").exists());
        let android_manifest =
            std::fs::read_to_string(dir.join("platforms/android/AndroidManifest.xml")).unwrap();
        assert!(android_manifest.contains("android:icon=\"@drawable/app_icon\""));
        assert!(android_manifest.contains("android:targetSdkVersion=\"35\""));
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
        let android_run_script =
            std::fs::read_to_string(dir.join("platforms/android/run-emulator.sh")).unwrap();
        assert!(android_run_script.contains("ANDROID_EMULATOR_API_LEVEL"));
        assert!(android_run_script.contains("fission doctor android"));
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
        assert!(ios_package_script.contains("plistlib.load"));
        assert!(ios_package_script.contains("PkgInfo"));
        assert!(ios_package_script.contains("AppIcon.png"));
        let ios_run_script = std::fs::read_to_string(dir.join("platforms/ios/run-sim.sh")).unwrap();
        assert!(ios_run_script.contains("BUNDLE_ID=\"${IOS_BUNDLE_ID:-com.example."));
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
