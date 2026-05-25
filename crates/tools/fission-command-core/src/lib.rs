use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_APP_ICON_PNG: &[u8] = include_bytes!("../../../../docs/fission_logo.png");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Android,
    Ios,
    Linux,
    Macos,
    Site,
    Web,
    Windows,
}

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Site => "site",
            Self::Web => "web",
            Self::Windows => "windows",
        }
    }

    pub fn scaffold_relative_path(self) -> &'static str {
        match self {
            Self::Android => "platforms/android/README.md",
            Self::Ios => "platforms/ios/README.md",
            Self::Linux => "platforms/linux/README.md",
            Self::Macos => "platforms/macos/README.md",
            Self::Site => "platforms/site/README.md",
            Self::Web => "platforms/web/README.md",
            Self::Windows => "platforms/windows/README.md",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DistributionProvider {
    #[value(name = "app-store")]
    AppStore,
    #[value(name = "github-pages")]
    GithubPages,
    #[value(name = "github-releases")]
    GithubReleases,
    #[value(name = "cloudflare-pages")]
    CloudflarePages,
    Dropbox,
    #[value(name = "google-drive")]
    GoogleDrive,
    #[value(name = "microsoft-store")]
    MicrosoftStore,
    Netlify,
    #[value(name = "onedrive")]
    OneDrive,
    #[value(name = "play-store")]
    PlayStore,
    S3,
}

impl DistributionProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppStore => "app-store",
            Self::GithubPages => "github-pages",
            Self::GithubReleases => "github-releases",
            Self::CloudflarePages => "cloudflare-pages",
            Self::Dropbox => "dropbox",
            Self::GoogleDrive => "google-drive",
            Self::MicrosoftStore => "microsoft-store",
            Self::Netlify => "netlify",
            Self::OneDrive => "onedrive",
            Self::PlayStore => "play-store",
            Self::S3 => "s3",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FissionProject {
    pub app: AppConfig,
    pub targets: BTreeSet<Target>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub app_id: String,
}

#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: Option<CargoPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WritePolicy {
    Overwrite,
    PreserveExisting,
}

pub fn init_project(
    root: &Path,
    name: Option<String>,
    app_id: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<()> {
    let existing_project = root.exists() && root.read_dir()?.next().is_some();
    fs::create_dir_all(root.join("src"))?;

    let write_policy = if existing_project {
        WritePolicy::PreserveExisting
    } else {
        WritePolicy::Overwrite
    };
    let project = initial_project_config(root, name, app_id)?;

    write_file_with_policy(
        &root.join("Cargo.toml"),
        &render_cargo_toml(&project, local_path.as_deref()),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("src/main.rs"),
        &render_app_main(project.app.name.as_str()),
        write_policy,
    )?;
    write_file_with_policy(&root.join("src/lib.rs"), APP_LIB, write_policy)?;
    write_file_with_policy(&root.join("src/app.rs"), APP_RS, write_policy)?;
    write_binary_file_with_policy(
        &root.join("assets/app-icon.png"),
        DEFAULT_APP_ICON_PNG,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("README.md"),
        &render_project_readme(&project),
        write_policy,
    )?;
    write_file_with_policy(
        &root.join(".gitignore"),
        "target/\nplatforms/*/build/\n",
        write_policy,
    )?;
    write_project_config(root, &project)?;

    let targets = project.targets.iter().copied().collect::<Vec<_>>();
    for target in targets {
        scaffold_target_with_policy(root, &project, target, write_policy)?;
    }

    Ok(())
}

fn initial_project_config(
    root: &Path,
    name: Option<String>,
    app_id: Option<String>,
) -> Result<FissionProject> {
    let existing = if root.join("fission.toml").exists() {
        Some(read_project_config(root)?)
    } else {
        None
    };
    let cargo_name = cargo_package_name(root);
    if let (Some(requested), Some(cargo_name)) = (&name, &cargo_name) {
        let requested = normalize_crate_name(requested);
        let cargo_name = normalize_crate_name(cargo_name);
        if requested != cargo_name {
            bail!(
                "refusing to set app name `{requested}` for existing Cargo package `{cargo_name}`; rename the package in Cargo.toml first or omit --name"
            );
        }
    }
    let project_name = cargo_name
        .or(name)
        .or_else(|| existing.as_ref().map(|project| project.app.name.clone()))
        .unwrap_or_else(|| {
            root.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("fission-app")
                .to_string()
        });
    let normalized_name = normalize_crate_name(&project_name);

    let mut targets = existing
        .as_ref()
        .map(|project| project.targets.clone())
        .unwrap_or_default();
    targets.extend(detect_project_targets(root));
    if targets.is_empty() {
        targets.extend([Target::Windows, Target::Macos, Target::Linux]);
    }

    Ok(FissionProject {
        app: AppConfig {
            name: normalized_name.clone(),
            app_id: app_id
                .or_else(|| existing.map(|project| project.app.app_id))
                .unwrap_or_else(|| format!("com.example.{}", normalized_name.replace('-', "_"))),
        },
        targets,
    })
}

pub fn cargo_package_name(root: &Path) -> Option<String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let manifest: CargoManifest = toml::from_str(&manifest).ok()?;
    manifest.package.map(|package| package.name)
}

fn detect_project_targets(root: &Path) -> BTreeSet<Target> {
    let mut targets = BTreeSet::new();
    if root.join("src/main.rs").exists() || root.join("src/lib.rs").exists() {
        targets.extend([Target::Windows, Target::Macos, Target::Linux]);
    }
    for (target, relative) in [
        (Target::Android, "platforms/android"),
        (Target::Ios, "platforms/ios"),
        (Target::Linux, "platforms/linux"),
        (Target::Macos, "platforms/macos"),
        (Target::Site, "content"),
        (Target::Web, "platforms/web"),
        (Target::Windows, "platforms/windows"),
    ] {
        if root.join(relative).exists() {
            targets.insert(target);
        }
    }
    targets
}

pub fn add_targets(project_dir: &Path, targets: &[Target]) -> Result<()> {
    if targets.is_empty() {
        bail!("no targets provided");
    }
    let mut project = read_project_config(project_dir)?;
    for target in targets {
        let target_exists =
            project.targets.contains(target) || target_scaffold_dir_exists(project_dir, *target);
        project.targets.insert(*target);
        let write_policy = if target_exists {
            WritePolicy::PreserveExisting
        } else {
            WritePolicy::Overwrite
        };
        scaffold_target_with_policy(project_dir, &project, *target, write_policy)?;
    }
    write_project_config(project_dir, &project)?;
    update_cargo_fission_features(project_dir, &project)?;
    write_file_with_policy(
        &project_dir.join("README.md"),
        &render_project_readme(&project),
        WritePolicy::PreserveExisting,
    )?;
    Ok(())
}

fn target_scaffold_dir_exists(project_dir: &Path, target: Target) -> bool {
    Path::new(target.scaffold_relative_path())
        .parent()
        .is_some_and(|relative| project_dir.join(relative).exists())
}

fn write_project_config(root: &Path, project: &FissionProject) -> Result<()> {
    let data = toml::to_string_pretty(project)?;
    write_file(&root.join("fission.toml"), &(data + "\n"))
}

pub fn read_project_config(root: &Path) -> Result<FissionProject> {
    let path = root.join("fission.toml");
    let data = fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read {}; run `fission init {}` to register this project without overwriting existing files",
            path.display(),
            root.display()
        )
    })?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn update_cargo_fission_features(root: &Path, project: &FissionProject) -> Result<()> {
    let path = root.join("Cargo.toml");
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let feature_list = render_fission_feature_list(&project.targets);
    let mut changed = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(updated) = update_inline_fission_dependency(line, &feature_list) {
            changed |= updated != line;
            out.push(updated);
        } else {
            out.push(line.to_string());
        }
    }
    if changed {
        fs::write(&path, out.join("\n") + "\n")
            .with_context(|| format!("failed to update {}", path.display()))?;
    }
    Ok(())
}

fn update_inline_fission_dependency(line: &str, feature_list: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("fission =") {
        return None;
    }
    let indent = &line[..line.len() - trimmed.len()];
    let value = trimmed.strip_prefix("fission =")?.trim();
    if value.starts_with('"') {
        return Some(format!(
            "{indent}fission = {{ version = {value}, default-features = false, features = [{feature_list}] }}"
        ));
    }
    if !(value.starts_with('{') && value.ends_with('}')) {
        return None;
    }
    let inner = value
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))?
        .trim();
    let mut fields = split_top_level_fields(inner)
        .into_iter()
        .filter(|field| {
            let key = field
                .split_once('=')
                .map(|(key, _)| key.trim())
                .unwrap_or_default();
            key != "default-features" && key != "features"
        })
        .collect::<Vec<_>>();
    fields.push("default-features = false".to_string());
    fields.push(format!("features = [{feature_list}]"));
    Some(format!("{indent}fission = {{ {} }}", fields.join(", ")))
}

fn split_top_level_fields(input: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut bracket_depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in input.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ',' if bracket_depth == 0 => {
                let field = input[start..index].trim();
                if !field.is_empty() {
                    fields.push(field.to_string());
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let field = input[start..].trim();
    if !field.is_empty() {
        fields.push(field.to_string());
    }
    fields
}

fn scaffold_target_with_policy(
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
                "Runnable emulator target. The CLI generates a NativeActivity manifest plus shell scripts that build, install, and launch the Fission app on an Android emulator.",
                &[
                    "Install the Rust target: `rustup target add aarch64-linux-android`.",
                    "Run `fission doctor android --project-dir .` to check SDK, NDK, emulator, and Rust target setup.",
                    "Run `fission devices --project-dir .` to list connected Android devices and configured emulators.",
                    "Run `fission run --target android --project-dir .` to build, install, launch, and attach to logs.",
                    "Run `fission run --target android --device <adb-serial> --project-dir .` to launch on a specific device.",
                    "Run `fission test --target android --project-dir .` for an emulator launch plus test-control health check.",
                    "Run `./platforms/android/run-emulator.sh` from the project root to build, package, install, and launch the app on the configured emulator.",
                    "Override `ANDROID_HOME`, `ANDROID_NDK`, `ANDROID_MIN_API_LEVEL`, `ANDROID_TARGET_API_LEVEL`, `ANDROID_AVD_NAME`, or `ANDROID_SYSTEM_IMAGE` if your local SDK setup differs.",
                    "Set `ANDROID_EMULATOR_HEADLESS=1` for background/CI runs, or `ANDROID_EMULATOR_RESTART=1` to relaunch a hidden emulator visibly.",
                    "The generated package uses `assets/app-icon.png` as its default launcher icon.",
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
                    "The generated bundle uses `assets/app-icon.png` as its default app icon.",
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
                    "Run `fission doctor web --project-dir .` to check wasm-pack, Node.js, Chrome/Chromium, and Rust target setup.",
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
                    "Unsupported interactive widgets fail during the static render instead of silently falling back to JavaScript.",
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
                "Run `fission run --project-dir .` from the project root to launch the desktop app and attach output.",
                "Run `fission build --project-dir . --release` for a release desktop build.",
                "Run `fission test --project-dir .` for the app crate's Rust tests.",
                "This target uses the default Vello desktop shell path.",
            ],
        ),
    };
    write_file_with_policy(&root.join(relative), &text, write_policy)
}

fn scaffold_ios_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let executable = ios_executable_name(project);
    let bundle_name = ios_bundle_name(project);
    let plist = render_ios_plist(project, &executable);
    let package_script = render_ios_package_script(project, &bundle_name, &executable);
    let run_script = render_ios_run_script(project);
    let test_script = render_ios_test_script();

    write_file_with_policy(&root.join("platforms/ios/Info.plist"), &plist, write_policy)?;
    write_file_with_policy(
        &root.join("platforms/ios/package-sim.sh"),
        &package_script,
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

fn scaffold_android_bundle(
    root: &Path,
    project: &FissionProject,
    write_policy: WritePolicy,
) -> Result<()> {
    let manifest = render_android_manifest(project);
    let package_script = render_android_package_script(project);
    let run_script = render_android_run_script(project);
    let test_script = render_android_test_script();

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
        &root.join("platforms/android/run-emulator.sh"),
        &run_script,
        write_policy,
    )?;
    write_file_with_policy(
        &root.join("platforms/android/test-emulator.sh"),
        &test_script,
        write_policy,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for relative in [
            "platforms/android/package-apk.sh",
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

fn scaffold_web_bundle(
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

fn write_file(path: &Path, contents: &str) -> Result<()> {
    write_file_with_policy(path, contents, WritePolicy::Overwrite)
}

fn write_file_with_policy(path: &Path, contents: &str, write_policy: WritePolicy) -> Result<()> {
    if write_policy == WritePolicy::PreserveExisting && path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn write_binary_file_with_policy(
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

fn render_cargo_toml(project: &FissionProject, local_path: Option<&Path>) -> String {
    let feature_list = render_fission_feature_list(&project.targets);
    let deps = if let Some(root) = local_path {
        let fission_path = root.join("crates/authoring/fission");
        format!(
            "fission = {{ path = {:?}, default-features = false, features = [{}] }}\n",
            fission_path.to_string_lossy().to_string(),
            feature_list
        )
    } else {
        format!(
            "fission = {{ version = \"{}\", default-features = false, features = [{}] }}\n",
            CURRENT_VERSION, feature_list
        )
    };
    let lib_name = project.app.name.replace('-', "_");

    format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{}\"\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nanyhow = \"1\"\nserde = {{ version = \"1\", features = [\"derive\"] }}\n{}\n[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nconsole_error_panic_hook = \"0.1\"\nwasm-bindgen = \"0.2\"\n",
        project.app.name, lib_name, deps
    )
}

fn render_fission_feature_list(targets: &BTreeSet<Target>) -> String {
    fission_features_for_targets(targets)
        .into_iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn fission_features_for_targets(targets: &BTreeSet<Target>) -> Vec<&'static str> {
    let mut features = Vec::new();
    if targets
        .iter()
        .any(|target| matches!(target, Target::Linux | Target::Macos | Target::Windows))
    {
        features.push("desktop");
    }
    if targets.contains(&Target::Web) {
        features.push("web");
    }
    if targets.contains(&Target::Android) {
        features.push("android");
    }
    if targets.contains(&Target::Ios) {
        features.push("ios");
    }
    if targets.contains(&Target::Site) {
        features.push("site");
    }
    features
}

fn render_project_readme(project: &FissionProject) -> String {
    let mut targets = String::new();
    for target in &project.targets {
        targets.push_str(&format!("- `{}`\n", target.as_str()));
    }
    format!(
        "# {}\n\nGenerated by `fission init`.\n\n## Targets\n\n{}\n## Commands\n\n- `fission doctor --project-dir .` -- check local SDKs, browsers, emulators, and Rust targets\n- `fission devices --project-dir .` -- list runnable desktop, browser, simulator, emulator, and device targets\n- `fission run --project-dir .` -- launch the desktop app and attach to output\n- `fission run --target web --project-dir .` -- launch the web app and attach to the local server\n- `fission run --target ios --project-dir .` -- build, install, launch, and attach to simulator logs\n- `fission run --target android --project-dir .` -- build, install, launch, and attach to Android logs\n- `fission run --target <target> --device <id> --detach --project-dir .` -- launch without attaching\n- `fission logs --target <target> --device <id> --project-dir . --follow` -- attach later where supported\n- `fission build --target <target> --project-dir . --release` -- build a target without launching it\n- `fission test --target <target> --project-dir .` -- run the generated platform smoke test\n- `fission add-target web ios android --project-dir .` -- scaffold more targets\n- `cat platforms/<target>/README.md` -- inspect target-specific prerequisites and environment variables\n\n## Assets\n\n- `assets/app-icon.png` is the default app icon seed copied from Fission's `docs/fission_logo.png`\n\n## Status\n\nDesktop, web, iOS simulator, and Android emulator workflows are runnable through `fission run`. The platform scripts remain checked in so CI and advanced users can call the lower-level build, run, and smoke-test steps directly when needed.\n",
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

pub fn ios_executable_name(project: &FissionProject) -> String {
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
BUNDLE_ID="${{IOS_BUNDLE_ID:-{bundle_id}}}"
DISPLAY_NAME="${{IOS_DISPLAY_NAME:-{bundle_name}}}"
EXECUTABLE_NAME="${{IOS_EXECUTABLE_NAME:-{executable}}}"
BUNDLE_NAME="${{IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}}"
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

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"
cp "$TARGET_DIR/$TARGET/$ARTIFACT_DIR/$PACKAGE_NAME" "$BUNDLE_DIR/$EXECUTABLE_NAME"
chmod +x "$BUNDLE_DIR/$EXECUTABLE_NAME"
python3 - <<'PY' "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist" "$BUNDLE_ID" "$DISPLAY_NAME" "$EXECUTABLE_NAME"
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
cp "$PROJECT_DIR/assets/app-icon.png" "$BUNDLE_DIR/AppIcon.png"
printf 'APPL????' > "$BUNDLE_DIR/PkgInfo"
printf '%s\n' "$BUNDLE_DIR"
"#,
        package_name = project.app.name,
        bundle_id = project.app.app_id,
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
BUNDLE_ID="${{IOS_BUNDLE_ID:-{bundle_id}}}"
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

if [[ "${{IOS_SIM_HEADLESS:-0}}" != "1" ]] && command -v open >/dev/null 2>&1; then
  open -a Simulator --args -CurrentDeviceUDID "$DEVICE_ID" >/dev/null 2>&1 \
    || open -a Simulator >/dev/null 2>&1 \
    || true
fi

xcrun simctl boot "$DEVICE_ID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$DEVICE_ID" -b
xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR"

if [[ -n "${{FISSION_TEST_CONTROL_PORT:-}}" ]]; then
  SIMCTL_CHILD_FISSION_TEST_CONTROL_PORT="${{FISSION_TEST_CONTROL_PORT}}" \
    xcrun simctl launch --terminate-running-process "$DEVICE_ID" "$BUNDLE_ID"
else
  xcrun simctl launch --terminate-running-process "$DEVICE_ID" "$BUNDLE_ID"
fi
"#,
        bundle_id = project.app.app_id,
    )
}

fn render_ios_test_script() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
export FISSION_TEST_CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-48711}"

"$SCRIPT_DIR/run-sim.sh"

python3 - <<'PY' "$FISSION_TEST_CONTROL_PORT"
import sys
import time
import urllib.request

port = sys.argv[1]
url = f"http://127.0.0.1:{port}/health"
deadline = time.time() + 90
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            body = response.read().decode("utf-8", "replace")
        if response.status == 200 and '"status":"ok"' in body:
            print(f"iOS simulator test control is healthy on {url}")
            raise SystemExit(0)
    except Exception as error:
        last_error = error
    time.sleep(1)
raise SystemExit(f"iOS simulator test control did not become healthy on {url}: {last_error}")
PY
"#
    .to_string()
}

fn render_android_manifest(project: &FissionProject) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{app_id}">

    <uses-permission android:name="android.permission.INTERNET" />

    <uses-sdk
        android:minSdkVersion="24"
        android:targetSdkVersion="35" />

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
ANDROID_HOME="${{ANDROID_HOME:-${{ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}}}"
ANDROID_MIN_API_LEVEL="${{ANDROID_MIN_API_LEVEL:-${{ANDROID_API_LEVEL:-24}}}}"

find_android_ndk() {{
  if [[ -n "${{ANDROID_NDK:-}}" ]]; then
    printf '%s\n' "$ANDROID_NDK"
    return
  fi
  local ndk_root="$ANDROID_HOME/ndk"
  if [[ ! -d "$ndk_root" ]]; then
    printf 'Android NDK not found. Set ANDROID_NDK or install one under %s.\n' "$ndk_root" >&2
    return 1
  fi
  local ndk
  ndk=$(find "$ndk_root" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -1)
  if [[ -z "$ndk" ]]; then
    printf 'Android NDK not found. Set ANDROID_NDK or install one under %s.\n' "$ndk_root" >&2
    return 1
  fi
  printf '%s\n' "$ndk"
}}

detect_android_toolchain() {{
  local prebuilt_root="$ANDROID_NDK/toolchains/llvm/prebuilt"
  local host
  for host in darwin-aarch64 darwin-x86_64 linux-x86_64 windows-x86_64; do
    if [[ -d "$prebuilt_root/$host/bin" ]]; then
      printf '%s\n' "$prebuilt_root/$host/bin"
      return
    fi
  done
  local fallback
  fallback=$(find "$prebuilt_root" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort | head -1 || true)
  if [[ -n "$fallback" && -d "$fallback/bin" ]]; then
    printf '%s\n' "$fallback/bin"
    return
  fi
  printf 'No Android NDK LLVM prebuilt toolchain found under %s. Expected a prebuilt host directory such as darwin-x86_64 or linux-x86_64.\n' "$prebuilt_root" >&2
  return 1
}}

detect_latest_android_api() {{
  find "$ANDROID_HOME/platforms" -maxdepth 1 -type d -name 'android-*' 2>/dev/null \
    | sed 's#.*android-##' \
    | sort -n \
    | tail -1
}}

detect_build_tools_dir() {{
  if [[ -n "${{ANDROID_BUILD_TOOLS:-}}" ]]; then
    if [[ -d "$ANDROID_BUILD_TOOLS" ]]; then
      printf '%s\n' "$ANDROID_BUILD_TOOLS"
      return
    fi
    if [[ -d "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS" ]]; then
      printf '%s\n' "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS"
      return
    fi
  fi
  find "$ANDROID_HOME/build-tools" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort -V | tail -1
}}

ANDROID_TARGET_API_LEVEL="${{ANDROID_TARGET_API_LEVEL:-$(detect_latest_android_api)}}"
if [[ -z "$ANDROID_TARGET_API_LEVEL" ]]; then
  printf 'No Android platform found under %s/platforms. Install one with sdkmanager "platforms;android-35" or newer.\n' "$ANDROID_HOME" >&2
  exit 1
fi

ANDROID_NDK=$(find_android_ndk)
ANDROID_TOOLCHAIN="${{ANDROID_TOOLCHAIN:-$(detect_android_toolchain)}}"
CC_aarch64_linux_android="${{CC_aarch64_linux_android:-$ANDROID_TOOLCHAIN/aarch64-linux-android${{ANDROID_MIN_API_LEVEL}}-clang}}"
AR_aarch64_linux_android="${{AR_aarch64_linux_android:-$ANDROID_TOOLCHAIN/llvm-ar}}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${{CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER:-$CC_aarch64_linux_android}}"
CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="${{CARGO_TARGET_AARCH64_LINUX_ANDROID_AR:-$AR_aarch64_linux_android}}"
export ANDROID_HOME ANDROID_NDK ANDROID_MIN_API_LEVEL ANDROID_TARGET_API_LEVEL ANDROID_TOOLCHAIN CC_aarch64_linux_android AR_aarch64_linux_android
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER CARGO_TARGET_AARCH64_LINUX_ANDROID_AR

BUILD_TOOLS=$(detect_build_tools_dir)
if [[ -z "$BUILD_TOOLS" || ! -d "$BUILD_TOOLS" ]]; then
  printf 'Android build-tools not found. Install them with sdkmanager "build-tools;35.0.0" or set ANDROID_BUILD_TOOLS.\n' >&2
  exit 1
fi
ANDROID_JAR="$ANDROID_HOME/platforms/android-$ANDROID_TARGET_API_LEVEL/android.jar"
if [[ ! -f "$ANDROID_JAR" ]]; then
  printf 'Android platform android-%s not found. Install it with sdkmanager "platforms;android-%s" or set ANDROID_TARGET_API_LEVEL.\n' "$ANDROID_TARGET_API_LEVEL" "$ANDROID_TARGET_API_LEVEL" >&2
  exit 1
fi
AAPT="$BUILD_TOOLS/aapt"
ZIPALIGN="$BUILD_TOOLS/zipalign"
APKSIGNER="$BUILD_TOOLS/apksigner"
for tool in "$AAPT" "$ZIPALIGN" "$APKSIGNER"; do
  if [[ ! -x "$tool" ]]; then
    printf 'Required Android build tool is missing or not executable: %s\n' "$tool" >&2
    exit 1
  fi
done

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

BUILD_MANIFEST="$BUILD_DIR/AndroidManifest.xml"
python3 - <<'PY' "$SCRIPT_DIR/AndroidManifest.xml" "$BUILD_MANIFEST" "$ANDROID_MIN_API_LEVEL" "$ANDROID_TARGET_API_LEVEL"
import re
import sys

source, dest, min_api, target_api = sys.argv[1:]
manifest = open(source, encoding="utf-8").read()
manifest = re.sub(r'android:minSdkVersion="\d+"', f'android:minSdkVersion="{{min_api}}"', manifest)
manifest = re.sub(r'android:targetSdkVersion="\d+"', f'android:targetSdkVersion="{{target_api}}"', manifest)
open(dest, "w", encoding="utf-8").write(manifest)
PY

"$AAPT" package -f -F "$UNALIGNED_APK" -M "$BUILD_MANIFEST" -S "$APK_ROOT/res" -I "$ANDROID_JAR"
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
ANDROID_HOME="${{ANDROID_HOME:-${{ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}}}"
ADB="$ANDROID_HOME/platform-tools/adb"
EMULATOR_BIN="$ANDROID_HOME/emulator/emulator"
AVDMANAGER="${{ANDROID_AVDMANAGER:-$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager}}"

detect_latest_emulator_api() {{
  find "$ANDROID_HOME/system-images" -path '*/google_apis/arm64-v8a' -type d 2>/dev/null \
    | sed -n 's#.*system-images/android-\([0-9][0-9]*\)/google_apis/arm64-v8a#\1#p' \
    | sort -n \
    | tail -1
}}

android_system_image_path() {{
  local image="$1"
  image="${{image#system-images;}}"
  printf '%s/system-images/%s\n' "$ANDROID_HOME" "${{image//;/\/}}"
}}

ANDROID_EMULATOR_API_LEVEL="${{ANDROID_EMULATOR_API_LEVEL:-$(detect_latest_emulator_api)}}"
if [[ -z "$ANDROID_EMULATOR_API_LEVEL" ]]; then
  printf 'No Android arm64 google_apis emulator image found under %s/system-images.\nInstall one with sdkmanager "system-images;android-35;google_apis;arm64-v8a" or set ANDROID_SYSTEM_IMAGE.\n' "$ANDROID_HOME" >&2
  exit 1
fi
AVD_NAME="${{ANDROID_AVD_NAME:-FissionApi${{ANDROID_EMULATOR_API_LEVEL}}Arm64}}"
SYSTEM_IMAGE="${{ANDROID_SYSTEM_IMAGE:-system-images;android-${{ANDROID_EMULATOR_API_LEVEL}};google_apis;arm64-v8a}}"
DEVICE_PORT="${{ANDROID_TEST_CONTROL_DEVICE_PORT:-48761}}"
HOST_PORT="${{FISSION_TEST_CONTROL_PORT:-48761}}"
HEADLESS="${{ANDROID_EMULATOR_HEADLESS:-0}}"
RESTART_EMULATOR="${{ANDROID_EMULATOR_RESTART:-0}}"

for tool in "$ADB" "$EMULATOR_BIN" "$AVDMANAGER"; do
  if [[ ! -x "$tool" ]]; then
    printf 'Required Android tool is missing or not executable: %s\nRun `fission doctor android --project-dir .` for setup help.\n' "$tool" >&2
    exit 1
  fi
done

if ! "$AVDMANAGER" list avd | grep -q "Name: $AVD_NAME"; then
  if [[ ! -d "$(android_system_image_path "$SYSTEM_IMAGE")" ]]; then
    printf 'Android system image is not installed: %s\nInstall it with sdkmanager "%s" or set ANDROID_SYSTEM_IMAGE.\n' "$SYSTEM_IMAGE" "$SYSTEM_IMAGE" >&2
    exit 1
  fi
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

fn render_android_test_script() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
export FISSION_TEST_CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-48761}"

"$SCRIPT_DIR/run-emulator.sh"

python3 - <<'PY' "$FISSION_TEST_CONTROL_PORT"
import sys
import time
import urllib.request

port = sys.argv[1]
url = f"http://127.0.0.1:{port}/health"
deadline = time.time() + 90
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            body = response.read().decode("utf-8", "replace")
        if response.status == 200 and '"status":"ok"' in body:
            print(f"Android emulator test control is healthy on {url}")
            raise SystemExit(0)
    except Exception as error:
        last_error = error
    time.sleep(1)
raise SystemExit(f"Android emulator test control did not become healthy on {url}: {last_error}")
PY
"#
    .to_string()
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
        width: 100%;
        height: 100%;
        overflow: hidden;
        overscroll-behavior: none;
        background: #14171f;
      }}
      body, #fission-web-mount {{
        width: 100vw;
        height: 100vh;
      }}
      canvas {{
        display: block;
        width: 100vw;
        height: 100vh;
        border: 0;
        outline: none;
        user-select: none;
        -webkit-user-drag: none;
        touch-action: none;
        -webkit-tap-highlight-color: transparent;
      }}
      canvas:focus, canvas:focus-visible {{
        outline: none;
      }}
    </style>
  </head>
  <body>
    <main id="fission-web-mount" aria-label="{title}"></main>
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
  if command -v open >/dev/null 2>&1; then
    open "$URL"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$URL"
  elif command -v cmd.exe >/dev/null 2>&1; then
    cmd.exe /C start "$URL"
  else
    printf 'No browser opener found. Open %s manually.\n' "$URL"
  fi
fi

cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST"
"#
    )
}

fn render_web_test_script(_project: &FissionProject) -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${FISSION_WEB_HOST:-127.0.0.1}"
PORT="${FISSION_WEB_PORT:-8123}"
CDP_PORT="${FISSION_WEB_CDP_PORT:-9222}"
URL="http://${HOST}:${PORT}/platforms/web/"
PROFILE_DIR="$SCRIPT_DIR/build/chrome-profile"

require_node_websocket() {
  if ! command -v node >/dev/null 2>&1; then
    printf 'Node.js was not found. Install Node 22+ so the generated browser smoke test can inspect Chrome CDP console/runtime errors.\n' >&2
    exit 1
  fi
  if ! node -e 'process.exit(typeof WebSocket === "function" ? 0 : 1)' >/dev/null 2>&1; then
    printf 'Node.js is available but does not expose the built-in WebSocket client. Install Node 22+ for Chrome CDP smoke tests.\n' >&2
    exit 1
  fi
}

detect_chrome() {
  if [[ -n "${FISSION_CHROME:-}" && -x "$FISSION_CHROME" ]]; then
    printf '%s\n' "$FISSION_CHROME"
    return
  fi
  local candidate
  for candidate in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "/Applications/Chromium.app/Contents/MacOS/Chromium" \
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  for candidate in google-chrome chromium chromium-browser chrome; do
    if command -v "$candidate" >/dev/null 2>&1; then
      command -v "$candidate"
      return
    fi
  done
  return 1
}

require_node_websocket
"$SCRIPT_DIR/build-wasm.sh"

mkdir -p "$SCRIPT_DIR/build"
cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST" >"$SCRIPT_DIR/build/web-server.log" 2>&1 &
SERVER_PID=$!

cleanup() {
  if [[ -n "${CHROME_PID:-}" ]]; then
    kill "$CHROME_PID" >/dev/null 2>&1 || true
  fi
  kill "$SERVER_PID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

printf 'Running transient web smoke test at %s\n' "$URL"
printf 'The local server is stopped automatically when this script exits.\n'

python3 - <<'PY' "$URL"
import sys
import time
import urllib.request

url = sys.argv[1]
deadline = time.time() + 30
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            if response.status == 200:
                raise SystemExit(0)
    except Exception as error:
        last_error = error
    time.sleep(0.5)
raise SystemExit(f"web server did not serve {url}: {last_error}")
PY

CHROME=$(detect_chrome) || {
  printf 'Chrome/Chromium was not found. Set FISSION_CHROME=/path/to/chrome or run `fission doctor web --project-dir .`.\n' >&2
  exit 1
}

rm -rf "$PROFILE_DIR"
"$CHROME" \
  --headless=new \
  --no-first-run \
  --no-default-browser-check \
  --remote-debugging-port="$CDP_PORT" \
  --user-data-dir="$PROFILE_DIR" \
  "$URL" >"$SCRIPT_DIR/build/chrome.log" 2>&1 &
CHROME_PID=$!

CDP_PORT="$CDP_PORT" FISSION_WEB_URL="$URL" node <<'NODE'
const cdpPort = process.env.CDP_PORT;
const expectedUrl = process.env.FISSION_WEB_URL;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitForTarget() {
  const deadline = Date.now() + 60_000;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      const targets = await response.json();
      const target = targets.find((entry) => entry.type === 'page' && entry.url.startsWith(expectedUrl));
      if (target?.webSocketDebuggerUrl) {
        return target.webSocketDebuggerUrl;
      }
    } catch (error) {
      lastError = error;
    }
    await sleep(250);
  }
  throw new Error(`Chrome CDP target did not become ready for ${expectedUrl}: ${lastError?.message ?? lastError}`);
}

class CdpClient {
  constructor(url) {
    this.url = url;
    this.ws = null;
    this.nextId = 1;
    this.pending = new Map();
    this.errors = [];
  }

  async open() {
    await new Promise((resolve, reject) => {
      const ws = new WebSocket(this.url);
      this.ws = ws;
      ws.addEventListener('open', resolve, { once: true });
      ws.addEventListener('error', (event) => reject(new Error(`CDP websocket error: ${event.message ?? 'unknown error'}`)), { once: true });
      ws.addEventListener('message', (event) => this.onMessage(event.data));
      ws.addEventListener('close', () => {
        for (const { reject: rejectPending } of this.pending.values()) {
          rejectPending(new Error('CDP websocket closed'));
        }
        this.pending.clear();
      });
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const message = { id, method, params };
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP command timed out: ${method}`));
      }, 10_000);
      this.pending.set(id, { resolve, reject, timeout, method });
      this.ws.send(JSON.stringify(message));
    });
  }

  onMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id) {
      const pending = this.pending.get(message.id);
      if (!pending) return;
      clearTimeout(pending.timeout);
      this.pending.delete(message.id);
      if (message.error) {
        pending.reject(new Error(`${pending.method}: ${message.error.message}`));
      } else {
        pending.resolve(message.result ?? {});
      }
      return;
    }

    if (message.method === 'Runtime.exceptionThrown') {
      this.errors.push(formatException(message.params?.exceptionDetails));
    } else if (message.method === 'Runtime.consoleAPICalled') {
      const type = message.params?.type;
      if (type === 'error' || type === 'assert') {
        this.errors.push(`console.${type}: ${(message.params?.args ?? []).map(formatRemoteObject).join(' ')}`);
      }
    } else if (message.method === 'Log.entryAdded') {
      const entry = message.params?.entry;
      if (entry?.level === 'error') {
        this.errors.push(`browser log error: ${entry.text}${entry.url ? ` (${entry.url}:${entry.lineNumber ?? 0})` : ''}`);
      }
    }
  }

  close() {
    this.ws?.close();
  }
}

function formatRemoteObject(value) {
  if (!value) return '<missing>';
  if (Object.prototype.hasOwnProperty.call(value, 'value')) return JSON.stringify(value.value);
  return value.description ?? value.unserializableValue ?? value.type ?? '<unknown>';
}

function formatException(details) {
  if (!details) return 'runtime exception: <missing details>';
  const exception = details.exception?.description ?? details.exception?.value ?? details.text ?? 'unknown exception';
  const location = details.url ? ` at ${details.url}:${details.lineNumber ?? 0}:${details.columnNumber ?? 0}` : '';
  return `runtime exception: ${exception}${location}`;
}

function errorBlock(errors) {
  return errors.slice(0, 10).map((error, index) => `${index + 1}. ${error}`).join('\n');
}

async function readCanvas(client) {
  const expression = `(() => {
    const canvas = document.querySelector('canvas');
    if (!canvas) return { ready: false, reason: 'no canvas element' };
    const rect = canvas.getBoundingClientRect();
    return {
      ready: rect.width > 0 && rect.height > 0,
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      gpu: typeof navigator.gpu !== 'undefined',
      title: document.title,
    };
  })()`;
  const result = await client.send('Runtime.evaluate', { expression, returnByValue: true });
  if (result.exceptionDetails) {
    throw new Error(formatException(result.exceptionDetails));
  }
  return result.result?.value ?? { ready: false, reason: 'evaluation returned no value' };
}

async function main() {
  const wsUrl = await waitForTarget();
  const client = new CdpClient(wsUrl);
  await client.open();
  try {
    await Promise.all([
      client.send('Runtime.enable'),
      client.send('Log.enable'),
      client.send('Page.enable'),
    ]);

    const deadline = Date.now() + 60_000;
    let readySince = null;
    let lastCanvas = null;
    while (Date.now() < deadline) {
      if (client.errors.length > 0) {
        throw new Error(`browser reported runtime/console errors:\n${errorBlock(client.errors)}`);
      }
      lastCanvas = await readCanvas(client);
      if (lastCanvas.ready) {
        readySince ??= Date.now();
        if (Date.now() - readySince >= 1_500) {
          console.log(`Web app rendered canvas ${lastCanvas.width}x${lastCanvas.height}; no runtime console errors observed.`);
          return;
        }
      } else {
        readySince = null;
      }
      await sleep(250);
    }
    throw new Error(`web app did not render a non-empty canvas. Last canvas state: ${JSON.stringify(lastCanvas)}`);
  } finally {
    client.close();
  }
}

main().catch((error) => {
  console.error(error.stack ?? error.message ?? String(error));
  process.exit(1);
});
NODE
"#
    .to_string()
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

use crate::app::CounterApp;
use fission::prelude::*;

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<crate::app::CounterState, CounterApp> {
    let app = MobileApp::new(CounterApp).with_title("Fission App");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<crate::app::CounterState, CounterApp> {
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

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
}

impl AppState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

pub struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let increment = with_reducer!(ctx, Increment, on_increment);

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
