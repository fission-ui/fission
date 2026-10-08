use clap::{Parser, Subcommand};
use fission_command_core::{DistributionProvider, NativeVariant, PlatformCapability, Target};
use fission_command_package as package;
use fission_command_release as release;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "fission",
    version,
    about = "Scaffold and manage Fission applications"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    #[command(hide = true)]
    TestVisualCase {
        #[arg(long)]
        url: String,
        #[arg(long)]
        mount_url: String,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long)]
        timeout_ms: u64,
        #[arg(long)]
        screenshot: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Build and verify a browser preview, serving until stopped (Ctrl+C).
    Preview(PreviewArgs),
    #[command(hide = true)]
    PreviewBuild {
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        project_dir: PathBuf,
        #[arg(long)]
        release: bool,
        #[arg(long, value_delimiter = ',')]
        features: Vec<String>,
        #[arg(long)]
        no_default_features: bool,
    },
    #[command(hide = true)]
    PreviewProbe {
        #[arg(long)]
        url: String,
        #[arg(long)]
        timeout_seconds: u64,
    },
    /// List alpha scene, game, asset, and physics features.
    Features,
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
    /// Add one or more host capabilities and update platform config where possible.
    AddCapability {
        #[arg(value_enum)]
        capabilities: Vec<PlatformCapability>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },
    /// Check local toolchains and SDKs needed by Fission targets.
    Doctor {
        /// Targets to check; defaults to web, iOS, and Android.
        #[arg(value_enum)]
        targets: Vec<Target>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Exit with a non-zero status when required checks fail.
        #[arg(long)]
        strict: bool,
    },
    /// List runnable desktop, browser, simulator, emulator, and device targets.
    Devices {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Build and run the app on a selected device, attaching logs by default.
    Run {
        /// Restrict device selection to one target.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Device id or exact/prefix device name from `fission devices`.
        #[arg(long)]
        device: Option<String>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Start the app and return instead of attaching logs/process output.
        #[arg(long)]
        detach: bool,
        /// Build in release mode.
        #[arg(long)]
        release: bool,
        /// Cargo features to activate for a Web build. Comma-separated or repeatable.
        #[arg(long, value_delimiter = ',')]
        features: Vec<String>,
        /// Do not activate default Cargo features for a Web build.
        #[arg(long)]
        no_default_features: bool,
        /// Select native modules belonging to this desktop variant.
        #[arg(long)]
        variant: Option<NativeVariant>,
        /// Host for the local web server.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port for the local web server.
        #[arg(long, default_value_t = 8123)]
        port: u16,
        /// Do not open a browser, Simulator, or emulator UI where supported.
        #[arg(long)]
        no_open: bool,
        /// Prefer headless simulator/emulator execution where supported.
        #[arg(long)]
        headless: bool,
    },
    /// Build a configured target without launching it.
    Build {
        /// Target to build; defaults to the host desktop target.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Build in release mode.
        #[arg(long)]
        release: bool,
        /// Cargo features to activate for a Web build. Comma-separated or repeatable.
        #[arg(long, value_delimiter = ',')]
        features: Vec<String>,
        /// Do not activate default Cargo features for a Web build.
        #[arg(long)]
        no_default_features: bool,
        /// Select native modules belonging to this desktop variant.
        #[arg(long)]
        variant: Option<NativeVariant>,
    },
    /// Run target smoke tests, or qualify browser captures with --visual-review.
    Test {
        /// Target to test; defaults to the host desktop target.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Prefer headless simulator/emulator execution where supported.
        #[arg(long)]
        headless: bool,
        /// Cargo features to activate for a Web build. Comma-separated or repeatable.
        #[arg(long, value_delimiter = ',')]
        features: Vec<String>,
        /// Do not activate default Cargo features for a Web build.
        #[arg(long)]
        no_default_features: bool,
        /// Select native modules belonging to this desktop variant.
        #[arg(long)]
        variant: Option<NativeVariant>,
        #[command(flatten)]
        visual: VisualReviewArgs,
    },
    /// Build, check, serve, or list routes for a static Fission site.
    Site {
        #[command(subcommand)]
        command: SiteCommand,
    },
    /// Build, check, serve, or list routes for a server-rendered Fission web app.
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
    /// Package a build output into a distributable artifact.
    Package {
        /// Target to package.
        #[arg(long, value_enum)]
        target: Target,
        /// Package format.
        #[arg(long, value_enum)]
        format: package::PackageFormat,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Build/package in release mode.
        #[arg(long)]
        release: bool,
        /// Select native modules belonging to this desktop variant.
        #[arg(long)]
        variant: Option<NativeVariant>,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Publish a packaged artifact to a configured distribution provider.
    Distribute {
        /// Lifecycle action; defaults to publish.
        #[arg(value_enum)]
        action: Option<package::DistributeAction>,
        /// Distribution provider.
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        /// Artifact manifest emitted by `fission package`.
        #[arg(long)]
        artifact: Option<PathBuf>,
        /// Named distribution site/profile from fission.toml.
        #[arg(long, default_value = "production")]
        site: String,
        /// Deployment id used by promote/rollback/status operations.
        #[arg(long)]
        deploy: Option<String>,
        /// Provider track/channel/group, such as internal, testflight, or production.
        #[arg(long)]
        track: Option<String>,
        /// Locale to include in this distribution decision. Can be repeated.
        #[arg(long = "locale")]
        locales: Vec<String>,
        /// Show what would happen without mutating provider state.
        #[arg(long)]
        dry_run: bool,
        /// Confirm overwrites or provider-side setup changes.
        #[arg(long)]
        yes: bool,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run the shared package/publish workflow, or open an interactive publish flow.
    Publish {
        /// Distribution provider.
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        /// Target to build/package before publishing. Defaults from the provider.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Package format to build before publishing. Defaults from the target/provider.
        #[arg(long, value_enum)]
        format: Option<package::PackageFormat>,
        /// Artifact manifest emitted by `fission package`. When omitted, the workflow packages the provider default target/format first.
        #[arg(long)]
        artifact: Option<PathBuf>,
        /// Named distribution site/profile from fission.toml.
        #[arg(long, default_value = "production")]
        site: String,
        /// Provider deployment id, release tag, or image tag override where supported.
        #[arg(long)]
        deploy: Option<String>,
        /// Provider track/channel/group, such as internal, testflight, or production.
        #[arg(long)]
        track: Option<String>,
        /// Locale to include in this publish decision. Can be repeated.
        #[arg(long = "locale")]
        locales: Vec<String>,
        /// Open the guided publish flow as a windowed app when available.
        #[arg(long)]
        app: bool,
        /// Use the line-oriented guided CLI flow instead of the Fission TUI.
        #[arg(long)]
        guided: bool,
        /// Show what would happen without mutating provider state.
        #[arg(long)]
        dry_run: bool,
        /// Replace provider release metadata even when no fresh release-config lock is present.
        #[arg(long)]
        overwrite_remote: bool,
        /// Confirm non-interactive package and publish mutations.
        #[arg(long)]
        yes: bool,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Emit machine-readable JSON for the publish workflow.
        #[arg(long)]
        json: bool,
    },
    /// Run package, distribution, or release readiness checks.
    Readiness {
        /// Readiness area to check.
        #[arg(value_enum)]
        kind: package::ReadinessKind,
        /// Target to package/check.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Package format.
        #[arg(long, value_enum)]
        format: Option<package::PackageFormat>,
        /// Check release packaging requirements where package readiness is selected.
        #[arg(long)]
        release: bool,
        /// Distribution provider.
        #[arg(long, value_enum)]
        provider: Option<DistributionProvider>,
        /// Artifact manifest emitted by `fission package`.
        #[arg(long)]
        artifact: Option<PathBuf>,
        /// Named distribution site/profile from fission.toml.
        #[arg(long, default_value = "production")]
        site: String,
        /// Provider track/channel/group, such as internal, testflight, or production.
        #[arg(long)]
        track: Option<String>,
        /// Locale to include in release readiness. Can be repeated.
        #[arg(long = "locale")]
        locales: Vec<String>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Edit, validate, import, diff, or push release metadata.
    ReleaseConfig {
        #[command(subcommand)]
        command: release::ReleaseConfigCommand,
    },
    /// Capture, render, validate, or push release screenshots and store assets.
    ReleaseContent {
        #[command(subcommand)]
        command: release::ReleaseContentCommand,
    },
    /// Manage beta groups, testers, and beta distribution.
    Beta {
        #[command(subcommand)]
        command: release::BetaCommand,
    },
    /// Inspect or import signing assets for release builds.
    Signing {
        #[command(subcommand)]
        command: release::SigningCommand,
    },
    /// List and reply to provider store reviews.
    Reviews {
        #[command(subcommand)]
        command: release::ReviewsCommand,
    },
    /// Run project-defined release workflows.
    ReleaseWorkflow {
        #[command(subcommand)]
        command: release::ReleaseWorkflowCommand,
    },
    /// Manage release provider authentication.
    Auth {
        #[command(subcommand)]
        command: release::AuthCommand,
    },
    /// Attach to logs for an already-running Fission app.
    Logs {
        /// Restrict device selection to one target.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Device id or exact/prefix device name from `fission devices`.
        #[arg(long)]
        device: Option<String>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Continue following logs instead of printing the current buffer.
        #[arg(long)]
        follow: bool,
    },
    /// Open the interactive Fission publish terminal UI.
    Ui {
        /// Publish provider to open in the TUI.
        #[arg(long, value_enum, default_value = "play-store")]
        provider: DistributionProvider,
        /// Target to package before publishing. Defaults from provider.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Package format. Defaults from target/provider.
        #[arg(long, value_enum)]
        format: Option<package::PackageFormat>,
        /// Provider track/channel/group.
        #[arg(long)]
        track: Option<String>,
        /// Locale to include in this publish decision. Can be repeated.
        #[arg(long = "locale")]
        locales: Vec<String>,
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Write a PNG screenshot of the rendered terminal frame.
        #[arg(long)]
        screenshot: Option<PathBuf>,
        /// Render once and exit; useful for screenshots and smoke tests.
        #[arg(long)]
        exit_after_render: bool,
        /// Override terminal width in cells.
        #[arg(long)]
        width: Option<u16>,
        /// Override terminal height in cells.
        #[arg(long)]
        height: Option<u16>,
    },
    /// Hidden helper used by `fission run --target web --detach`.
    #[command(hide = true)]
    ServeWeb {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8123)]
        port: u16,
        #[arg(long)]
        open: bool,
    },
}

#[derive(clap::Args, Debug)]
pub(crate) struct VisualReviewArgs {
    /// Capture a browser route/viewport matrix for explicit image inspection.
    #[arg(long, requires_all = ["target", "output_dir"], conflicts_with_all = ["headless", "variant"])]
    pub visual_review: bool,
    /// Directory for deterministic PNG files and the self-contained report.
    #[arg(long, requires = "visual_review")]
    pub output_dir: Option<PathBuf>,
    /// Repeatable app-relative route path. Content sites discover metadata routes.
    #[arg(long = "route", requires = "visual_review")]
    pub routes: Vec<String>,
    /// Repeatable WIDTHxHEIGHT CSS pixels. Defaults: 390x900,800x900,1280x900; scale 1.
    #[arg(long = "viewport", requires = "visual_review")]
    pub viewports: Vec<fission_command_run::review::Viewport>,
    #[arg(long, default_value = "/", requires = "visual_review")]
    pub mount: String,
    #[arg(long, default_value_t = 0, requires = "visual_review")]
    pub port: u16,
    #[arg(long, requires = "visual_review")]
    pub release: bool,
    #[arg(long, default_value_t = 300, requires = "visual_review")]
    pub startup_timeout_seconds: u64,
    #[arg(long, default_value_t = 60, requires = "visual_review")]
    pub case_timeout_seconds: u64,
    /// Fail geometry warning candidates. Normal mode records warnings with exit 0.
    #[arg(long, requires = "visual_review")]
    pub strict: bool,
    /// One JSON report on stdout. Exit: 0 supported checks passed (possibly warnings),
    /// 1 execution, 2 incomplete/cancelled, 3 confirmed errors or strict warnings.
    #[arg(long, requires = "visual_review")]
    pub json: bool,
}

#[derive(clap::Args, Debug)]
pub(crate) struct PreviewArgs {
    /// Browser target: web or static-site.
    #[arg(long, value_enum)]
    pub target: PreviewTarget,
    #[arg(long, default_value = ".")]
    pub project_dir: PathBuf,
    #[arg(long)]
    pub release: bool,
    #[arg(long, value_delimiter = ',')]
    pub features: Vec<String>,
    #[arg(long)]
    pub no_default_features: bool,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Requested port; 0 allocates an owned available port. Occupied ports fail.
    #[arg(long, default_value_t = 8123)]
    pub port: u16,
    /// Public URL mount, e.g. /repository-name/. Config is preserved.
    #[arg(long, default_value = "/")]
    pub mount: String,
    /// Expected HTML entry relative to the target's configured output directory.
    #[arg(long, default_value = "index.html")]
    pub entry: String,
    /// Deadline for the complete build and readiness sequence (1..=3600).
    #[arg(long, default_value_t = 300)]
    pub startup_timeout_seconds: u64,
    /// Web only: compile development test control and verify it in Chrome.
    #[arg(long)]
    pub live_test: bool,
    /// Emit flushed JSON lifecycle events on stdout; diagnostics use stderr.
    #[arg(long)]
    pub json: bool,
    /// Stop on stdin `stop` + newline or EOF, including during startup.
    #[arg(long)]
    pub stdin_control: bool,
    /// Open the verified URL in the default browser.
    #[arg(long)]
    pub open: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub(crate) enum PreviewTarget {
    Web,
    #[value(name = "static-site", alias = "site")]
    StaticSite,
}

impl From<PreviewTarget> for Target {
    fn from(target: PreviewTarget) -> Self {
        match target {
            PreviewTarget::Web => Target::Web,
            PreviewTarget::StaticSite => Target::Site,
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum SiteCommand {
    /// Build the static site into its configured output directory.
    Build {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Build in release mode.
        #[arg(long)]
        release: bool,
    },
    /// Check the static site by rendering all routes.
    Check {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Build in release mode.
        #[arg(long)]
        release: bool,
    },
    /// Serve the generated static site locally.
    Serve {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Host for the local site server.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port for the local site server.
        #[arg(long, default_value_t = 8123)]
        port: u16,
        /// Build in release mode before serving.
        #[arg(long)]
        release: bool,
        /// Do not open a browser.
        #[arg(long)]
        no_open: bool,
    },
    /// List custom and content routes.
    Routes {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum ServerCommand {
    /// Build the server binary and route-local browser artifacts.
    Build {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Build in release mode.
        #[arg(long)]
        release: bool,
    },
    /// Check that the server app renders all declared routes.
    Check {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Build in release mode.
        #[arg(long)]
        release: bool,
    },
    /// Serve the server-rendered app locally.
    Serve {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Host for the local server.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port for the local server.
        #[arg(long, default_value_t = 8124)]
        port: u16,
        /// Build in release mode before serving.
        #[arg(long)]
        release: bool,
    },
    /// List server routes and their rendering modes.
    Routes {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },
    /// Generate and optionally compile per-worker/per-island browser WASM shims.
    Artifacts {
        /// Project directory; defaults to the current working directory.
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        /// Build in release mode.
        #[arg(long)]
        release: bool,
        /// Write shim crates without compiling them.
        #[arg(long)]
        no_compile: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_browser_only_and_workers_are_hidden() {
        use clap::CommandFactory;
        for target in ["web", "static-site"] {
            let parsed = Cli::try_parse_from([
                "fission",
                "preview",
                "--target",
                target,
                "--port",
                "0",
                "--mount",
                "/repository-name/",
                "--json",
                "--stdin-control",
            ])
            .unwrap();
            assert!(matches!(parsed.command, Command::Preview(_)));
        }
        assert!(Cli::try_parse_from(["fission", "preview", "--target", "macos"]).is_err());
        let help = Cli::command().render_help().to_string();
        assert!(help.contains("preview"));
        assert!(!help.contains("preview-build"));
        assert!(!help.contains("preview-probe"));
    }

    #[test]
    fn visual_review_is_an_explicit_browser_test_mode() {
        let cli = Cli::try_parse_from([
            "fission",
            "test",
            "--visual-review",
            "--target",
            "web",
            "--output-dir",
            "review",
            "--viewport",
            "390x844",
            "--strict",
            "--json",
        ])
        .unwrap();
        let Command::Test { visual, target, .. } = cli.command else {
            panic!("wrong authority")
        };
        assert_eq!(target, Some(Target::Web));
        assert!(visual.visual_review && visual.strict && visual.json);
        assert_eq!(visual.viewports[0].height, 844);
        assert!(Cli::try_parse_from([
            "fission",
            "review",
            "--target",
            "web",
            "--output-dir",
            "review"
        ])
        .is_err());
        for args in [
            vec![
                "fission",
                "test",
                "--visual-review",
                "--output-dir",
                "review",
            ],
            vec!["fission", "test", "--visual-review", "--target", "web"],
            vec!["fission", "test", "--strict"],
            vec!["fission", "test", "--route", "/"],
            vec!["fission", "test", "--viewport", "390x900"],
            vec!["fission", "test", "--output-dir", "review"],
            vec![
                "fission",
                "test",
                "--visual-review",
                "--target",
                "web",
                "--output-dir",
                "review",
                "--headless",
            ],
            vec![
                "fission",
                "test",
                "--visual-review",
                "--target",
                "web",
                "--output-dir",
                "review",
                "--variant",
                "scanner",
            ],
        ] {
            assert!(Cli::try_parse_from(args.clone()).is_err(), "{args:?}");
        }
        for target in ["web", "macos", "ios", "android"] {
            let cli =
                Cli::try_parse_from(["fission", "test", "--target", target, "--headless"]).unwrap();
            let Command::Test { visual, .. } = cli.command else {
                panic!("wrong authority")
            };
            assert!(!visual.visual_review);
        }
    }

    fn selected_variant(command: Command) -> Option<NativeVariant> {
        match command {
            Command::Run { variant, .. }
            | Command::Build { variant, .. }
            | Command::Test { variant, .. }
            | Command::Package { variant, .. } => variant,
            _ => None,
        }
    }

    #[test]
    fn desktop_lifecycle_commands_parse_native_variant() {
        for arguments in [
            vec![
                "fission",
                "run",
                "--target",
                "macos",
                "--variant",
                "scanner",
            ],
            vec![
                "fission",
                "build",
                "--target",
                "windows",
                "--variant",
                "scanner",
            ],
            vec![
                "fission",
                "test",
                "--target",
                "linux",
                "--variant",
                "scanner",
            ],
            vec![
                "fission",
                "package",
                "--target",
                "macos",
                "--format",
                "pkg",
                "--variant",
                "scanner",
            ],
        ] {
            let cli = Cli::try_parse_from(arguments).unwrap();
            assert_eq!(
                selected_variant(cli.command)
                    .as_ref()
                    .map(NativeVariant::as_str),
                Some("scanner")
            );
        }
    }

    #[test]
    fn cli_rejects_non_stable_variant_names() {
        let result = Cli::try_parse_from([
            "fission",
            "package",
            "--target",
            "macos",
            "--format",
            "app",
            "--variant",
            "Scanner Debug",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn web_lifecycle_commands_parse_cargo_feature_overrides() {
        for arguments in [
            vec![
                "fission",
                "run",
                "--target",
                "web",
                "--features",
                "fixtures,diagnostics",
                "--features",
                "testing",
                "--no-default-features",
            ],
            vec![
                "fission",
                "build",
                "--target",
                "web",
                "--features",
                "fixtures,diagnostics",
                "--features",
                "testing",
                "--no-default-features",
            ],
            vec![
                "fission",
                "test",
                "--target",
                "web",
                "--features",
                "fixtures,diagnostics",
                "--features",
                "testing",
                "--no-default-features",
            ],
        ] {
            let cli = Cli::try_parse_from(arguments).unwrap();
            let (features, no_default_features) = match cli.command {
                Command::Run {
                    features,
                    no_default_features,
                    ..
                }
                | Command::Build {
                    features,
                    no_default_features,
                    ..
                }
                | Command::Test {
                    features,
                    no_default_features,
                    ..
                } => (features, no_default_features),
                command => panic!("unexpected command: {command:?}"),
            };

            assert_eq!(features, ["fixtures", "diagnostics", "testing"]);
            assert!(no_default_features);
        }
    }

    #[test]
    fn features_command_parses_without_project_context() {
        let cli = Cli::try_parse_from(["fission", "features"]).unwrap();
        assert!(matches!(cli.command, Command::Features));
    }
}
