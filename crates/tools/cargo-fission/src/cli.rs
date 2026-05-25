use clap::{Parser, Subcommand};
use fission_command_core::{DistributionProvider, Target};
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
    },
    /// Run the generated smoke test for a configured target.
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
    },
    /// Build, check, serve, or list routes for a static Fission site.
    Site {
        #[command(subcommand)]
        command: SiteCommand,
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
    /// Run package or distribution readiness checks.
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
    /// Capture, render, or validate release screenshots and store assets.
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
    /// Open the interactive Fission command terminal UI.
    Ui {
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
