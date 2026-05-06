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
    write_file(&root.join("src/main.rs"), DESKTOP_MAIN)?;
    write_file(&root.join("src/app.rs"), APP_RS)?;
    write_file(&root.join("README.md"), &render_project_readme(&project))?;
    write_file(&root.join(".gitignore"), "target/\n")?;
    write_project_config(root, &project)?;

    for target in &project.targets {
        scaffold_target(root, *target)?;
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
        scaffold_target(project_dir, *target)?;
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

fn scaffold_target(root: &Path, target: Target) -> Result<()> {
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
        Target::Ios => platform_readme(
            "iOS",
            "Scaffolded target. A generated Fission app can cross-compile for iOS after `fission add-target`, but the CLI does not generate an Xcode launcher yet.",
            &[
                "Install the Rust target: `rustup target add aarch64-apple-ios`.",
                "Confirm the iPhoneOS SDK path with `xcrun --sdk iphoneos --show-sdk-path`.",
                "Run `cargo check --target aarch64-apple-ios` from the project root to verify the generated app compiles.",
                "Next work: teach `fission add-target` to generate the iOS launcher/package files.",
            ],
        ),
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
        "# {}\n\nGenerated by `fission init`.\n\n## Targets\n\n{}\n## Commands\n\n- `cargo run` -- launch the desktop app\n- `cargo fission add-target web ios android` -- scaffold more targets\n- `cat platforms/<target>/README.md` -- inspect the current prerequisites and status for each target\n\n## Status\n\nDesktop is runnable today. iOS and Android generated apps can cross-compile after `fission add-target`, but still need first-party launcher/package generation. Web remains scaffold-only until `fission-shell-web` lands.\n",
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

const DESKTOP_MAIN: &str = r#"mod app;

use crate::app::{CounterApp, CounterState};
use fission::prelude::*;

fn main() -> anyhow::Result<()> {
    let _ = std::any::TypeId::of::<CounterState>();
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
