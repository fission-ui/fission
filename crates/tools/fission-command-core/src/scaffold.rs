use super::*;

pub(super) fn scaffold_target_with_policy(
    root: &Path,
    project: &FissionProject,
    target: Target,
    write_policy: WritePolicy,
) -> Result<()> {
    let relative = Path::new(target.scaffold_relative_path());
    let text = match target {
        Target::Android => {
            scaffold_android_bundle(root, project, write_policy)?;
            platform_readme(
                "Android",
                "Runnable emulator target. The CLI generates a Gradle Android project shell plus scripts that build, install, and launch the Fission app on an Android emulator.",
                &[
                    "Install the Rust target: `rustup target add aarch64-linux-android`.",
                    "Run `fission doctor android --project-dir .` to check SDK, NDK, emulator, and Rust target setup.",
                    "Run `fission devices --project-dir .` to list connected Android devices and configured emulators.",
                    "Run `fission run --target android --project-dir .` to build, install, launch, and attach to logs.",
                    "Run `fission run --target android --device <adb-serial> --project-dir .` to launch on a specific device.",
                    "Run `fission test --target android --project-dir .` for an emulator launch plus test-control health check.",
                    "Run `./platforms/android/run-emulator.sh` from the project root to build, package, install, and launch the app on the configured emulator.",
                    "Run `fission package --target android --format aab --release --project-dir .` or `./platforms/android/package-aab.sh` to create the signed Play Store app bundle.",
                    "Override `ANDROID_HOME`, `ANDROID_NDK`, `ANDROID_MIN_API_LEVEL`, `ANDROID_TARGET_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` if your local SDK setup differs.",
                    "Set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs, or `ANDROID_EMULATOR_RESTART=1` to relaunch a hidden emulator visibly.",
                    "The generated package uses `assets/app-icon.png` as its default launcher icon.",
                    "Configure `[app.splash]` in `fission.toml` to generate the native Android launch theme, splash background, static image, and optional Android animated drawable.",
                    "Run `fission add-capability nfc --project-dir .` to add NFC manifest permission and feature declarations.",
                    "Run `fission add-capability notifications --project-dir .` to add Android notification permission for API 33 and newer.",
                    "Run `fission add-capability biometric --project-dir .` to add biometric manifest permissions.",
                    "Run `fission add-capability passkeys --project-dir .` to record passkey/WebAuthn use. Android passkeys also require Digital Asset Links and host Credential Manager integration for production sign-in.",
                    "Run `fission add-capability bluetooth --project-dir .` to add Bluetooth permissions and optional hardware feature declarations.",
                    "Run `fission add-capability barcode-scanner --project-dir .` to add camera permission for barcode scanning.",
                    "Run `fission add-capability camera --project-dir .` to add camera permission and optional camera/flash hardware feature declarations.",
                    "Run `fission add-capability geolocation --project-dir .` to add location permissions.",
                    "Run `fission add-capability haptics --project-dir .` to add the vibration permission.",
                    "Run `fission add-capability microphone --project-dir .` to add audio recording permission.",
                    "Run `fission add-capability volume-control --project-dir .` to add Android audio settings permission.",
                    "Run `fission add-capability wifi --project-dir .` to add Wi-Fi permissions and optional hardware feature declarations.",
                    "Set `FISSION_TEST_CONTROL_PORT=<host-port>` before `run-emulator.sh`; the script forwards it to the fixed in-app device port.",
                ],
            )
        }
        Target::Ios => {
            scaffold_ios_bundle(root, project, write_policy)?;
            platform_readme(
                "iOS",
                "Simulator target. The CLI generates a simulator app bundle template plus shell scripts that build, install, launch, and smoke-test the Fission app with `simctl`.",
                &[
                    "Install the Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.",
                    "Run `fission doctor ios --project-dir .` to check Xcode, simulator, and Rust target setup.",
                    "Confirm the simulator SDK path with `xcrun --sdk iphonesimulator --show-sdk-path`.",
                    "Run `fission devices --project-dir .` to list available iOS simulators.",
                    "Run `fission run --target ios --project-dir .` to build, install, launch, and attach to simulator logs.",
                    "Run `fission run --target ios --device <simulator-udid> --project-dir .` to launch on a specific simulator.",
                    "Run `fission test --target ios --project-dir .` for a simulator launch plus test-control health check.",
                    "Run `./platforms/ios/run-sim.sh` from the project root to build, install, and launch the app on the first available iPhone simulator.",
                    "Run `fission package --target ios --format ipa --release --project-dir .` or `./platforms/ios/package-ipa.sh` to create a signed IPA when IOS_SIGNING_IDENTITY is configured.",
                    "The generated bundle uses `assets/app-icon.png` as its default app icon.",
                    "Configure `[app.splash]` in `fission.toml` to generate the native iOS launch storyboard and splash image copied into the simulator bundle.",
                    "Run `fission add-capability nfc --project-dir .` to add the NFC usage description and entitlements file.",
                    "Run `fission add-capability notifications --project-dir .` to record local-notification use. iOS prompts at runtime and does not require an Info.plist usage key for local notifications.",
                    "Run `fission add-capability biometric --project-dir .` to add the Face ID usage description.",
                    "Run `fission add-capability passkeys --project-dir .` to record passkey/WebAuthn use. iOS production passkeys require associated domains such as `webcredentials:example.com` in the app entitlements.",
                    "Run `fission add-capability bluetooth --project-dir .` to add the Bluetooth usage description.",
                    "Run `fission add-capability barcode-scanner --project-dir .` to add the camera usage description for barcode scanning.",
                    "Run `fission add-capability camera --project-dir .` to add the camera usage description.",
                    "Run `fission add-capability geolocation --project-dir .` to add the location usage description.",
                    "Run `fission add-capability microphone --project-dir .` to add the microphone usage description.",
                    "Run `fission add-capability wifi --project-dir .` to add Wi-Fi entitlements and the location usage description required by current-network information APIs.",
                    "Volume control does not require an iOS Info.plist key in the generated scaffold.",
                    "Haptics do not require an iOS Info.plist key in the generated scaffold.",
                    "Set `FISSION_TEST_CONTROL_PORT=<port>` before `run-sim.sh` to expose the in-app test control server on the host.",
                    "Set `IOS_SIM_DEVICE_ID=<udid>` if you want a specific simulator device.",
                    "Set `IOS_SIM_HEADLESS=1` for CI or background-only simulator runs; otherwise the script opens Simulator visibly.",
                ],
            )
        }
        Target::Web => {
            scaffold_web_bundle(root, project, write_policy)?;
            platform_readme(
                "Web",
                "Runnable browser target. The CLI generates a WASM host page plus helper scripts that build the app with `wasm-pack` and serve it locally.",
                &[
                    "Install the Rust target: `rustup target add wasm32-unknown-unknown`.",
                    "Install `wasm-pack` once: `cargo install wasm-pack`.",
                    "Install Node.js 22+ so the smoke test can inspect Chrome/Chromium CDP runtime and console output.",
                    "Run `fission doctor web --project-dir .` to check wasm-pack, generated JavaScript glue, Chrome/Chromium, and Rust target setup.",
                    "Run `fission devices --project-dir .` to confirm Chrome/Chromium detection.",
                    "Run `fission run --target web --project-dir .` to build, serve, open, and attach to the local server.",
                    "Run `fission run --target web --detach --project-dir .` to keep the local server running in the background.",
                    "Run `fission test --target web --project-dir .` for a headless Chrome/Chromium CDP smoke test.",
                    "Run `./platforms/web/run-browser.sh` from the project root to build the wasm package and serve the app locally.",
                    "Set `FISSION_WEB_PORT=<port>` or `FISSION_WEB_HOST=<host>` if the default `127.0.0.1:8123` does not suit your machine.",
                    "Set `FISSION_WEB_OPEN=1` if you want the helper script to open a browser tab automatically.",
                    "The generated page uses `assets/app-icon.png` as its default favicon/app icon seed.",
                ],
            )
        }
        Target::Server => platform_readme(
            "SSR",
            "Server-rendered Fission target. The CLI runs the app through the server shell for dynamic HTML, revalidated pages, server jobs, signed actions, worker artifacts, and focused browser islands.",
            &[
                "Configure `[server].entry` in `fission.toml` so the CLI can invoke the server app.",
                "Run `fission server check --project-dir .` to render all declared server routes.",
                "Run `fission server serve --project-dir .` to serve the app locally.",
                "Run `fission server artifacts --project-dir .` to generate browser worker and island WASM shims.",
                "Run `fission package --target ssr --format docker-image --release --project-dir .` to package the server app as an OCI/Docker image.",
            ],
        ),
        Target::Site => {
            write_file_with_policy(
                &root.join("content/getting-started.md"),
                "---\ntitle: Site content\ndescription: Static site content rendered by the Fission static site shell.\n---\n\n# Site content\n\nAdd Markdown files under `content/`. `fission site build` renders them through real Fission widgets, lowers the nodes to Core IR, and emits static HTML.\n",
                write_policy,
            )?;
            platform_readme(
                "Static site",
                "Static multi-page website target. The site shell renders Markdown content through real Fission widgets, lowers nodes to Core IR, and emits semantic static HTML.",
                &[
                    "Add Markdown or MDX content under `content/`.",
                    "Run `fission site routes --project-dir .` to list generated routes.",
                    "Run `fission site build --project-dir .` to render HTML into `target/fission/site`.",
                    "Run `fission site serve --project-dir .` to build and serve the generated site locally.",
                    "Run `fission package --target static-site --format static --release --project-dir .` to package the generated site.",
                    "Unsupported interactive widgets fail during the static render instead of silently falling back to JavaScript.",
                ],
            )
        }
        Target::Terminal => platform_readme(
            "Terminal",
            "Terminal target. The CLI treats this as a terminal-shell app using the project's normal Rust entrypoint and terminal-shell feature.",
            &[
                "Use `fission::terminal::TerminalApp` or a target-aware app entrypoint for terminal rendering.",
                "Run `fission run --target terminal --project-dir .` to execute the app in the current terminal.",
                "Run `fission test --target terminal --project-dir .` for Rust tests until terminal-shell package formats are defined by the terminal-shell RFC.",
                "This target enables the `terminal-shell` Fission feature but does not imply native desktop, web, or mobile shells.",
            ],
        ),
        Target::Windows => {
            scaffold_windows_bundle(root, project, write_policy)?;
            platform_readme(
                "Windows",
                "Runnable desktop target with release packaging scaffolds for EXE, MSI, and MSIX distribution.",
                &[
                    "Run `fission run --project-dir .` from the project root to launch the desktop app and attach output.",
                    "Run `fission build --project-dir . --release` for a release desktop build.",
                    "Run `fission package --target windows --format exe --release --project-dir .` to copy the signed release executable into a package artifact.",
                    "Run `fission package --target windows --format msix --release --project-dir .` or `./platforms/windows/package-msix.ps1` to create an MSIX package with `makeappx`.",
                    "Run `fission package --target windows --format msi --release --project-dir .` or `./platforms/windows/package-msi.ps1` to create an MSI package with WiX.",
                    "Set `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_BASE64`, or `WINDOWS_CERTIFICATE_THUMBPRINT` plus `WINDOWS_CERTIFICATE_PASSWORD` where needed; never commit certificate files or passwords.",
                    "Edit `[package.windows]` in `fission.toml` for Store package identity, publisher identity, package version, and installer preference.",
                    "For an unpackaged NSIS app, build the architecture-matched shortcut helper with `./platforms/windows/build-shortcut-aumid-helper.ps1 -Architecture x64` (or `arm64`) and include `platforms/windows/fission-shortcut-aumid.nsh`. Embed the helper once, then apply one stable AppUserModelID to every Start Menu shortcut after `CreateShortCut`.",
                    "Pass that exact AppUserModelID to `DesktopApp::with_windows_app_user_model_id`; package identity remains authoritative for MSIX, so the explicit value is only the unpackaged fallback.",
                    "Sign the compiled shortcut helper before embedding it in a signed installer. The helper deliberately fails installation if it cannot persist the shortcut identity.",
                    "The generated MSIX manifest stages the desktop executable as a full-trust Windows app and copies `assets/app-icon.png` into the package asset set by default.",
                ],
            )
        }
        Target::Linux | Target::Macos => platform_readme(
            match target {
                Target::Linux => "Linux",
                Target::Macos => "macOS",
                _ => unreachable!(),
            },
            "Runnable target. Desktop platforms share the default `src/main.rs` entrypoint through `DesktopApp`.",
            &[
                "Run `fission run --project-dir .` from the project root to launch the desktop app and attach output.",
                "Run `fission build --project-dir . --release` for a release desktop build.",
                "Run `fission test --project-dir .` for the app crate's Rust tests.",
                "This target uses the default Vello desktop shell path.",
            ],
        ),
    };
    write_file_with_policy(&root.join(relative), &text, write_policy)
}

pub(super) fn scaffold_ios_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let executable = ios_executable_name(project);
    let bundle_name = ios_bundle_name(project);
    let plist = render_ios_plist(project, &executable);
    let package_script = render_ios_package_script(project, &bundle_name, &executable);
    let ipa_script = render_ios_ipa_package_script(project);
    let run_script = render_ios_run_script(project);
    let test_script = render_ios_test_script();

    write_file_with_policy(&root.join("platforms/ios/Info.plist"), &plist, write_policy)?;
    write_file_with_policy(
        &root.join("platforms/ios/Package.swift"),
        &render_ios_host_package(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/Sources/FissionHost/FissionNativeCapabilities.swift"),
        render_ios_host_native_capabilities_swift(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/NativeModules/README.md"),
        IOS_NATIVE_MODULES_README,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/NativeModules/Package.swift"),
        &render_ios_native_modules_package(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join(
            "platforms/ios/NativeModules/Sources/FissionNativeModules/FissionNativeCapabilities.swift",
        ),
        render_ios_native_capabilities_swift(),
        write_policy,
    )?;
    sync_ios_native_module_sources(root, project)?;
    if project.capabilities.contains(&PlatformCapability::Nfc)
        || project.capabilities.contains(&PlatformCapability::Wifi)
    {
        write_file_with_policy(
            &root.join("platforms/ios/Entitlements.plist"),
            &render_ios_entitlements_plist(project),
            write_policy,
        )?;
    }
    write_file_with_policy(
        &root.join("platforms/ios/package-sim.sh"),
        &package_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/package-ipa.sh"),
        &ipa_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/run-sim.sh"),
        &run_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/ios/test-sim.sh"),
        &test_script,
        write_policy,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/ios/package-sim.sh",
            "platforms/ios/package-ipa.sh",
            "platforms/ios/run-sim.sh",
            "platforms/ios/test-sim.sh",
        ] {
            let path = root.join(relative);
            if path.exists() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    Ok(())
}

pub(super) fn scaffold_android_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let manifest = render_android_manifest(project);
    let package_script = render_android_package_script(project);
    let package_aab_script = render_android_aab_package_script(project);
    let run_script = render_android_run_script(project);
    let test_script = render_android_test_script();

    write_file_with_policy(
        &root.join("platforms/android/settings.gradle.kts"),
        &render_android_settings_gradle(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/build.gradle.kts"),
        &render_android_root_build_gradle(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/gradle.properties"),
        render_android_gradle_properties(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/app/build.gradle.kts"),
        &render_android_app_build_gradle(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/native-modules.gradle"),
        &render_android_native_modules_gradle(project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/AndroidManifest.xml"),
        &manifest,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/package-apk.sh"),
        &package_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/package-aab.sh"),
        &package_aab_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/run-emulator.sh"),
        &run_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/test-emulator.sh"),
        &test_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/java/rs/fission/runtime/FissionActivity.java"),
        render_android_activity_java(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/native-modules/README.md"),
        ANDROID_NATIVE_MODULES_README,
        write_policy,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/android/package-apk.sh",
            "platforms/android/package-aab.sh",
            "platforms/android/run-emulator.sh",
            "platforms/android/test-emulator.sh",
        ] {
            let path = root.join(relative);
            if path.exists() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    Ok(())
}

pub(super) fn scaffold_windows_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let executable = windows_executable_name(root, project);
    write_file_with_policy(
        &root.join("platforms/windows/Package.appxmanifest"),
        &render_windows_appx_manifest(project, &executable),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/package-msix.ps1"),
        &render_windows_msix_package_script(project, &executable),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/package-msi.ps1"),
        &render_windows_msi_package_script(project, &executable),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/shortcut-aumid-helper.cpp"),
        render_windows_shortcut_aumid_helper_source(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/build-shortcut-aumid-helper.ps1"),
        render_windows_shortcut_aumid_helper_build_script(),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/windows/fission-shortcut-aumid.nsh"),
        render_windows_shortcut_aumid_nsis_include(),
        write_policy,
    )?;
    Ok(())
}

pub(super) fn windows_executable_name(root: &Path, project: &FissionProject) -> String {
    let stem = cargo_package_name(root).unwrap_or_else(|| sanitize_file_stem(&project.app.name));
    format!("{stem}.exe")
}

pub(super) fn windows_identity_name(project: &FissionProject) -> String {
    let mut out = project
        .app
        .app_id
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' => ch,
            '_' => '.',
            _ => '.',
        })
        .collect::<String>();
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    out = out.trim_matches(['.', '-']).to_string();
    if out.is_empty() {
        "Fission.App".to_string()
    } else {
        out
    }
}

pub(super) fn windows_publisher_name() -> &'static str {
    "CN=Fission Developer"
}

pub(super) fn render_windows_appx_manifest(project: &FissionProject, executable: &str) -> String {
    let display_name = escape_xml_attribute(&project.app.name);
    let identity_name = escape_xml_attribute(&windows_identity_name(project));
    let publisher = escape_xml_attribute(windows_publisher_name());
    let install_dir = escape_xml_attribute(&sanitize_file_stem(&project.app.name));
    let executable = escape_xml_attribute(executable);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap rescap">
  <Identity Name="{identity_name}" Publisher="{publisher}" Version="0.1.0.1" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>{display_name}</DisplayName>
    <PublisherDisplayName>Fission Developer</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="App" Executable="VFS\ProgramFilesX64\{install_dir}\{executable}" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="{display_name}" Description="{display_name}" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"#
    )
}

pub(super) fn render_windows_msix_package_script(
    project: &FissionProject,
    executable: &str,
) -> String {
    let app_name = sanitize_file_stem(&project.app.name);
    let package_name = windows_identity_name(project);
    let template = r#"$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$Profile = if ($env:WINDOWS_PROFILE) { $env:WINDOWS_PROFILE } else { "debug" }
$CargoProfileArg = if ($Profile -eq "release") { @("--release") } else { @() }
$ExecutableName = if ($env:WINDOWS_EXECUTABLE_NAME) { $env:WINDOWS_EXECUTABLE_NAME } else { "__EXECUTABLE__" }
$BinaryPath = if ($env:WINDOWS_BINARY) { $env:WINDOWS_BINARY } else { Join-Path $ProjectDir "target\$Profile\$ExecutableName" }
$OutRoot = Join-Path $ProjectDir "target\fission\windows\msix"
$LayoutDir = Join-Path $OutRoot "layout"
$AppDir = Join-Path $LayoutDir "VFS\ProgramFilesX64\__APP_NAME__"
$AssetsDir = Join-Path $LayoutDir "Assets"
$MsixPath = Join-Path $OutRoot "__PACKAGE_NAME__-$Profile.msix"

if (-not $env:WINDOWS_BINARY) {
  cargo build @CargoProfileArg --manifest-path (Join-Path $ProjectDir "Cargo.toml")
}
if (-not (Test-Path $BinaryPath)) {
  throw "Windows executable was not found at $BinaryPath. Set WINDOWS_BINARY or WINDOWS_EXECUTABLE_NAME if the crate name changed."
}
$MakeAppx = Get-Command makeappx -ErrorAction SilentlyContinue
if (-not $MakeAppx) {
  throw "makeappx was not found. Install Windows SDK MSIX packaging tools and ensure makeappx is on PATH."
}

Remove-Item -Recurse -Force $LayoutDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $AppDir, $AssetsDir | Out-Null
Copy-Item $BinaryPath (Join-Path $AppDir $ExecutableName) -Force
if ($env:FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST) {
  $NativeManifest = Get-Content -Raw $env:FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST | ConvertFrom-Json
  foreach ($Product in $NativeManifest.products) {
    if ($Product.kind -eq "driver-package") {
      throw "MSIX native product manifest must not contain driver package $($Product.name)."
    }
    $NativeDestination = Join-Path $AppDir $Product.destination
    $NativeParent = Split-Path -Parent $NativeDestination
    New-Item -ItemType Directory -Force $NativeParent | Out-Null
    if (Test-Path $Product.source -PathType Container) {
      Copy-Item $Product.source $NativeDestination -Recurse -Force
    } else {
      Copy-Item $Product.source $NativeDestination -Force
    }
  }
}
Copy-Item (Join-Path $ScriptDir "Package.appxmanifest") (Join-Path $LayoutDir "AppxManifest.xml") -Force

$IconSource = if ($env:WINDOWS_APP_ICON) { $env:WINDOWS_APP_ICON } else { Join-Path $ProjectDir "assets\app-icon.png" }
if (Test-Path $IconSource) {
  Copy-Item $IconSource (Join-Path $AssetsDir "StoreLogo.png") -Force
  Copy-Item $IconSource (Join-Path $AssetsDir "Square44x44Logo.png") -Force
  Copy-Item $IconSource (Join-Path $AssetsDir "Square150x150Logo.png") -Force
}

& $MakeAppx.Source pack /d $LayoutDir /p $MsixPath /overwrite | Out-Host

$Certificate = $env:WINDOWS_CERTIFICATE
$TempCertificate = $null
try {
  if (-not $Certificate -and $env:WINDOWS_CERTIFICATE_BASE64) {
    $TempCertificate = Join-Path ([System.IO.Path]::GetTempPath()) ("fission-windows-cert-" + [System.Guid]::NewGuid().ToString() + ".pfx")
    [System.IO.File]::WriteAllBytes($TempCertificate, [System.Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
    $Certificate = $TempCertificate
  }
  $Thumbprint = $env:WINDOWS_CERTIFICATE_THUMBPRINT
  if ($Certificate -or $Thumbprint) {
    $SignTool = Get-Command signtool -ErrorAction SilentlyContinue
    if (-not $SignTool) {
      throw "signtool was not found. Install Windows SDK signing tools or set WINDOWS_SKIP_SIGNING=1 for unsigned local packages."
    }
    $SignArgs = @("sign", "/fd", "SHA256")
    if ($Certificate) {
      $SignArgs += @("/f", $Certificate)
      if ($env:WINDOWS_CERTIFICATE_PASSWORD) { $SignArgs += @("/p", $env:WINDOWS_CERTIFICATE_PASSWORD) }
    } else {
      $SignArgs += @("/sha1", $Thumbprint)
    }
    $SignArgs += $MsixPath
    & $SignTool.Source @SignArgs | Out-Host
  } elseif ($Profile -eq "release" -and $env:WINDOWS_SKIP_SIGNING -ne "1") {
    throw "Release MSIX packaging requires WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT from a secure secret source. Set WINDOWS_SKIP_SIGNING=1 only for local unsigned validation."
  }
} finally {
  if ($TempCertificate) { Remove-Item -Force $TempCertificate -ErrorAction SilentlyContinue }
}

Write-Output $MsixPath
"#;
    template
        .replace("__APP_NAME__", &app_name)
        .replace("__PACKAGE_NAME__", &package_name)
        .replace("__EXECUTABLE__", executable)
}

pub(super) fn render_windows_msi_package_script(
    project: &FissionProject,
    executable: &str,
) -> String {
    let app_name = sanitize_file_stem(&project.app.name);
    let display_name = project.app.name.clone();
    let upgrade_code = deterministic_guid(&project.app.app_id);
    let manufacturer = "Fission Developer";
    let template = r#"$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$Profile = if ($env:WINDOWS_PROFILE) { $env:WINDOWS_PROFILE } else { "debug" }
$CargoProfileArg = if ($Profile -eq "release") { @("--release") } else { @() }
$ExecutableName = if ($env:WINDOWS_EXECUTABLE_NAME) { $env:WINDOWS_EXECUTABLE_NAME } else { "__EXECUTABLE__" }
$BinaryPath = if ($env:WINDOWS_BINARY) { $env:WINDOWS_BINARY } else { Join-Path $ProjectDir "target\$Profile\$ExecutableName" }
$OutRoot = Join-Path $ProjectDir "target\fission\windows\msi"
$MsiPath = Join-Path $OutRoot "__APP_NAME__-$Profile.msi"
$Version = if ($env:WINDOWS_MSI_VERSION) { $env:WINDOWS_MSI_VERSION } else { "0.1.0" }
$UpgradeCode = if ($env:WINDOWS_MSI_UPGRADE_CODE) { $env:WINDOWS_MSI_UPGRADE_CODE } else { "__UPGRADE_CODE__" }

if (-not $env:WINDOWS_BINARY) {
  cargo build @CargoProfileArg --manifest-path (Join-Path $ProjectDir "Cargo.toml")
}
if (-not (Test-Path $BinaryPath)) {
  throw "Windows executable was not found at $BinaryPath. Set WINDOWS_BINARY or WINDOWS_EXECUTABLE_NAME if the crate name changed."
}
New-Item -ItemType Directory -Force $OutRoot | Out-Null

$Wix = Get-Command wix -ErrorAction SilentlyContinue
$Candle = Get-Command candle -ErrorAction SilentlyContinue
$Light = Get-Command light -ErrorAction SilentlyContinue
if ($Wix) {
  $WxsPath = Join-Path $OutRoot "package.wxs"
  @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="__DISPLAY_NAME__" Manufacturer="__MANUFACTURER__" Version="$Version" UpgradeCode="$UpgradeCode" Scope="perMachine">
    <MajorUpgrade DowngradeErrorMessage="A newer version of __DISPLAY_NAME__ is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <StandardDirectory Id="ProgramFiles6432Folder">
      <Directory Id="INSTALLFOLDER" Name="__APP_NAME__">
        <Component Id="MainExecutable" Guid="*">
          <File Id="AppExe" Source="$BinaryPath" KeyPath="yes" />
        </Component>
      </Directory>
    </StandardDirectory>
    <Feature Id="MainFeature" Title="__DISPLAY_NAME__" Level="1">
      <ComponentRef Id="MainExecutable" />
    </Feature>
  </Package>
</Wix>
"@ | Set-Content -Encoding UTF8 $WxsPath
  & $Wix.Source build $WxsPath -o $MsiPath | Out-Host
} elseif ($Candle -and $Light) {
  $WxsPath = Join-Path $OutRoot "package-wix3.wxs"
  $WixObj = Join-Path $OutRoot "package.wixobj"
  @"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="__DISPLAY_NAME__" Language="1033" Version="$Version" Manufacturer="__MANUFACTURER__" UpgradeCode="$UpgradeCode">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of __DISPLAY_NAME__ is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLFOLDER" Name="__APP_NAME__">
          <Component Id="MainExecutable" Guid="*">
            <File Id="AppExe" Source="$BinaryPath" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="MainFeature" Title="__DISPLAY_NAME__" Level="1">
      <ComponentRef Id="MainExecutable" />
    </Feature>
  </Product>
</Wix>
"@ | Set-Content -Encoding UTF8 $WxsPath
  & $Candle.Source -nologo -arch x64 -out $WixObj $WxsPath | Out-Host
  & $Light.Source -nologo -out $MsiPath $WixObj | Out-Host
} else {
  throw "WiX was not found. Install WiX Toolset (`wix`) or WiX 3 (`candle` and `light`) to package an MSI."
}

$Certificate = $env:WINDOWS_CERTIFICATE
$TempCertificate = $null
try {
  if (-not $Certificate -and $env:WINDOWS_CERTIFICATE_BASE64) {
    $TempCertificate = Join-Path ([System.IO.Path]::GetTempPath()) ("fission-windows-cert-" + [System.Guid]::NewGuid().ToString() + ".pfx")
    [System.IO.File]::WriteAllBytes($TempCertificate, [System.Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
    $Certificate = $TempCertificate
  }
  $Thumbprint = $env:WINDOWS_CERTIFICATE_THUMBPRINT
  if ($Certificate -or $Thumbprint) {
    $SignTool = Get-Command signtool -ErrorAction SilentlyContinue
    if (-not $SignTool) {
      throw "signtool was not found. Install Windows SDK signing tools or set WINDOWS_SKIP_SIGNING=1 for unsigned local packages."
    }
    $SignArgs = @("sign", "/fd", "SHA256")
    if ($Certificate) {
      $SignArgs += @("/f", $Certificate)
      if ($env:WINDOWS_CERTIFICATE_PASSWORD) { $SignArgs += @("/p", $env:WINDOWS_CERTIFICATE_PASSWORD) }
    } else {
      $SignArgs += @("/sha1", $Thumbprint)
    }
    $SignArgs += $MsiPath
    & $SignTool.Source @SignArgs | Out-Host
  } elseif ($Profile -eq "release" -and $env:WINDOWS_SKIP_SIGNING -ne "1") {
    throw "Release MSI packaging requires WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT from a secure secret source. Set WINDOWS_SKIP_SIGNING=1 only for local unsigned validation."
  }
} finally {
  if ($TempCertificate) { Remove-Item -Force $TempCertificate -ErrorAction SilentlyContinue }
}

Write-Output $MsiPath
"#;
    template
        .replace("__APP_NAME__", &app_name)
        .replace("__DISPLAY_NAME__", &display_name)
        .replace("__MANUFACTURER__", manufacturer)
        .replace("__UPGRADE_CODE__", &upgrade_code)
        .replace("__EXECUTABLE__", executable)
}

pub(super) fn render_windows_shortcut_aumid_helper_source() -> &'static str {
    r#"#include <windows.h>

#include <cwchar>
#include <cwctype>
#include <cstdio>

#include <propkey.h>
#include <propvarutil.h>
#include <shobjidl.h>
#include <wrl/client.h>

namespace {

using Microsoft::WRL::ComPtr;

bool IsValidAppUserModelId(const wchar_t* app_user_model_id) {
  if (app_user_model_id == nullptr) {
    return false;
  }

  const size_t length = std::wcslen(app_user_model_id);
  if (length == 0 || length > 128) {
    return false;
  }

  for (size_t index = 0; index < length; ++index) {
    if (std::iswspace(app_user_model_id[index]) != 0) {
      return false;
    }
  }

  return true;
}

int ReportFailure(const wchar_t* operation, HRESULT result) {
  std::fwprintf(
      stderr,
      L"%ls failed (HRESULT 0x%08lX).\n",
      operation,
      static_cast<unsigned long>(result));
  return 1;
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
  if (argc != 3) {
    std::fwprintf(
        stderr,
        L"Usage: fission-shortcut-aumid.exe <shortcut.lnk> <app-user-model-id>\n");
    return 2;
  }

  const wchar_t* shortcut_path = argv[1];
  const wchar_t* app_user_model_id = argv[2];
  if (!IsValidAppUserModelId(app_user_model_id)) {
    std::fwprintf(
        stderr,
        L"The AppUserModelID must contain 1-128 UTF-16 code units and no whitespace.\n");
    return 3;
  }

  const HRESULT initialize_result =
      CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  const bool should_uninitialize = SUCCEEDED(initialize_result);
  if (FAILED(initialize_result) && initialize_result != RPC_E_CHANGED_MODE) {
    return ReportFailure(L"CoInitializeEx", initialize_result);
  }

  int exit_code = 0;
  ComPtr<IShellLinkW> shell_link;
  HRESULT result = CoCreateInstance(
      CLSID_ShellLink,
      nullptr,
      CLSCTX_INPROC_SERVER,
      IID_PPV_ARGS(&shell_link));
  if (FAILED(result)) {
    exit_code = ReportFailure(L"CoCreateInstance(CLSID_ShellLink)", result);
    goto finish;
  }

  {
    ComPtr<IPersistFile> persist_file;
    result = shell_link.As(&persist_file);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"QueryInterface(IPersistFile)", result);
      goto finish;
    }

    result = persist_file->Load(shortcut_path, STGM_READWRITE);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPersistFile::Load", result);
      goto finish;
    }

    ComPtr<IPropertyStore> property_store;
    result = shell_link.As(&property_store);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"QueryInterface(IPropertyStore)", result);
      goto finish;
    }

    PROPVARIANT app_id_value;
    PropVariantInit(&app_id_value);
    result = InitPropVariantFromString(app_user_model_id, &app_id_value);
    if (SUCCEEDED(result)) {
      result = property_store->SetValue(PKEY_AppUserModel_ID, app_id_value);
    }
    if (SUCCEEDED(result)) {
      result = property_store->Commit();
    }
    PropVariantClear(&app_id_value);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPropertyStore::SetValue/Commit", result);
      goto finish;
    }

    result = persist_file->Save(shortcut_path, TRUE);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPersistFile::Save", result);
      goto finish;
    }
  }

finish:
  if (should_uninitialize) {
    CoUninitialize();
  }
  return exit_code;
}
"#
}

pub(super) fn render_windows_shortcut_aumid_helper_build_script() -> &'static str {
    r#"[CmdletBinding()]
param(
  [ValidateSet("x64", "arm64")]
  [string] $Architecture = $(if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" })
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$SourcePath = Join-Path $ScriptDir "shortcut-aumid-helper.cpp"
$OutputDirectory = Join-Path $ProjectDir "target\fission\windows\shortcut-aumid\$Architecture"
$OutputPath = Join-Path $OutputDirectory "fission-shortcut-aumid.exe"
$ObjectPath = Join-Path $OutputDirectory "fission-shortcut-aumid.obj"

if (-not (Test-Path $SourcePath -PathType Leaf)) {
  throw "The shortcut AUMID helper source was not found at $SourcePath."
}

$VsWhere = Get-Command vswhere.exe -ErrorAction SilentlyContinue
if (-not $VsWhere -and ${env:ProgramFiles(x86)}) {
  $BundledVsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $BundledVsWhere -PathType Leaf) {
    $VsWhere = Get-Item $BundledVsWhere
  }
}
if (-not $VsWhere) {
  throw "vswhere.exe was not found. Install Visual Studio Build Tools with the target C++ toolchain."
}
$VsWherePath = if ($VsWhere -is [System.IO.FileInfo]) {
  $VsWhere.FullName
} else {
  $VsWhere.Source
}

$RequiredComponent = if ($Architecture -eq "arm64") {
  "Microsoft.VisualStudio.Component.VC.Tools.ARM64"
} else {
  "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
}
$Installation = & $VsWherePath -latest -products * -requires $RequiredComponent -property installationPath
if (-not $Installation) {
  throw "Visual Studio Build Tools with component $RequiredComponent were not found for $Architecture."
}
$VsDevCmd = Join-Path $Installation "Common7\Tools\VsDevCmd.bat"
if (-not (Test-Path $VsDevCmd -PathType Leaf)) {
  throw "VsDevCmd.bat was not found at $VsDevCmd."
}

$DeveloperCommand = "call `"$VsDevCmd`" -no_logo -arch=$Architecture -host_arch=amd64 && set"
$EnvironmentLines = & $env:ComSpec /d /c $DeveloperCommand
if ($LASTEXITCODE -ne 0) {
  throw "Visual Studio failed to initialize the $Architecture C++ build environment."
}
foreach ($Line in $EnvironmentLines) {
  $Separator = $Line.IndexOf("=")
  if ($Separator -gt 0) {
    $Name = $Line.Substring(0, $Separator)
    $Value = $Line.Substring($Separator + 1)
    [Environment]::SetEnvironmentVariable($Name, $Value, "Process")
  }
}

$Compiler = Get-Command cl.exe -ErrorAction SilentlyContinue
if (-not $Compiler) {
  throw "cl.exe was not available after initializing the $Architecture C++ build environment."
}

New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$CompileArguments = @(
  "/nologo",
  "/EHsc",
  "/MT",
  "/DUNICODE",
  "/D_UNICODE",
  "/Fo$ObjectPath",
  "/Fe$OutputPath",
  $SourcePath,
  "/link",
  "ole32.lib",
  "shell32.lib",
  "propsys.lib"
)
& $Compiler.Source @CompileArguments | Out-Host
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $OutputPath -PathType Leaf)) {
  throw "The $Architecture shortcut AUMID helper build failed."
}

Write-Output $OutputPath
"#
}

pub(super) fn render_windows_shortcut_aumid_nsis_include() -> &'static str {
    r#"!ifndef FISSION_SHORTCUT_AUMID_NSH
!define FISSION_SHORTCUT_AUMID_NSH

!include "LogicLib.nsh"

; Embed the architecture-matched helper once in an installer section.
!macro FissionEmbedShortcutAppUserModelIdHelper HELPER_PATH
  InitPluginsDir
  File "/oname=$PLUGINSDIR\fission-shortcut-aumid.exe" "${HELPER_PATH}"
!macroend

; Apply the same stable AppUserModelID passed to
; WinitApp::with_windows_app_user_model_id or
; DesktopApp::with_windows_app_user_model_id. Call this after CreateShortCut.
!macro FissionSetShortcutAppUserModelId SHORTCUT_PATH APP_USER_MODEL_ID
  Push $0
  Push $1
  nsExec::ExecToStack /TIMEOUT=30000 '"$PLUGINSDIR\fission-shortcut-aumid.exe" "${SHORTCUT_PATH}" "${APP_USER_MODEL_ID}"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    DetailPrint "Failed to apply AppUserModelID to ${SHORTCUT_PATH}: exit=$0 output=$1"
    MessageBox MB_ICONSTOP|MB_OK "Windows notification identity setup failed. The installation cannot continue."
    Pop $1
    Pop $0
    SetErrors
    Abort
  ${EndIf}
  Pop $1
  Pop $0
!macroend

!endif
"#
}

pub(super) fn sanitize_file_stem(value: &str) -> String {
    let stem = value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_string();
    if stem.is_empty() {
        "app".to_string()
    } else {
        stem
    }
}

pub(super) fn deterministic_guid(value: &str) -> String {
    fn fnv64(seed: u64, value: &str) -> u64 {
        let mut hash = seed;
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
    let left = fnv64(0xcbf29ce484222325, value);
    let right = fnv64(0x84222325cbf29ce4, value);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&left.to_be_bytes());
    bytes[8..].copy_from_slice(&right.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

pub(super) fn scaffold_web_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let index_html = render_web_index(project);
    let bootstrap = render_web_bootstrap(project);
    let build_script = render_web_build_script();
    let run_script = render_web_run_script(project);
    let test_script = render_web_test_script(project);

    write_file_with_policy(
        &root.join("platforms/web/index.html"),
        &index_html,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/bootstrap.mjs"),
        &bootstrap,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/build-wasm.sh"),
        &build_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/run-browser.sh"),
        &run_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/web/test-browser.sh"),
        &test_script,
        write_policy,
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/web/build-wasm.sh",
            "platforms/web/run-browser.sh",
            "platforms/web/test-browser.sh",
        ] {
            let path = root.join(relative);
            if path.exists() {
                let mut perms = fs::metadata(&path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(path, perms)?;
            }
        }
    }

    Ok(())
}

pub(super) fn write_generated_app_agents(project_root: &Path) -> Result<()> {
    let repo_root = find_git_root(project_root).unwrap_or_else(|| project_root.to_path_buf());
    let root_agents = repo_root.join("AGENTS.md");
    if let Some(existing) = read_optional_string(&root_agents)? {
        if is_generated_app_agents(&existing) {
            return write_file_with_policy(
                &root_agents,
                GENERATED_APP_AGENTS_MD,
                WritePolicy::Overwrite,
            );
        }

        let fission_agents = repo_root.join("AGENTS.fission.md");
        let write_policy = read_optional_string(&fission_agents)?
            .filter(|existing| is_generated_app_agents(existing))
            .map(|_| WritePolicy::Overwrite)
            .unwrap_or(WritePolicy::PreserveExisting);

        return write_file_with_policy(&fission_agents, GENERATED_APP_AGENTS_MD, write_policy);
    }

    write_file_with_policy(
        &root_agents,
        GENERATED_APP_AGENTS_MD,
        WritePolicy::Overwrite,
    )
}

pub(super) fn read_optional_string(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub(super) fn is_generated_app_agents(contents: &str) -> bool {
    contents.contains(GENERATED_APP_AGENTS_MARKER)
        || contents == GENERATED_APP_AGENTS_MD
        || (contents.contains("# Fission App Guidelines")
            && contents.contains(
                "These instructions apply when building or reviewing a Fission-based app",
            )
            && contents.contains("## Source-Grounded Work")
            && contents.contains("## Validation"))
}

pub(super) fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = fs::canonicalize(start).ok()?;
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub(crate) fn write_file(path: &Path, contents: &str) -> Result<()> {
    write_file_with_policy(path, contents, WritePolicy::Overwrite)
}

pub(super) fn write_file_with_policy(
    path: &Path,
    contents: &str,
    write_policy: WritePolicy,
) -> Result<()> {
    if write_policy == WritePolicy::PreserveExisting && path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn write_binary_file_with_policy(
    path: &Path,
    contents: &[u8],
    write_policy: WritePolicy,
) -> Result<()> {
    if write_policy == WritePolicy::PreserveExisting && path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}
