use super::*;

#[derive(Subcommand, Debug)]
pub enum ReleaseConfigCommand {
    /// Open release configuration in an editor or the Fission terminal UI.
    Edit {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        tui: bool,
        #[arg(long, value_enum)]
        provider: Option<DistributionProvider>,
    },
    /// Import provider metadata into local release files.
    Import {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        locales: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Diff local release metadata against provider state.
    Diff {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Validate fission.toml and referenced release files.
    Validate {
        #[arg(long, value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Push release metadata to a provider.
    Push {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        locales: Option<String>,
        #[arg(long)]
        overwrite_remote: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Record the current provider metadata revision as the local editing baseline.
    Lock {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        locales: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Set a scalar field in fission.toml.
    Set {
        field: String,
        value: String,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Append a release entry to fission.toml.
    AddRelease {
        #[arg(long)]
        version: String,
        #[arg(long)]
        build: u64,
        #[arg(long)]
        from: Option<String>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Record an explicit skip for a provider-optional or Fission-recommended release check.
    SkipRequirement {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Increment the build number used for a target release package.
    BumpBuild {
        #[arg(long, value_enum)]
        target: Option<Target>,
        #[arg(long, default_value_t = 1)]
        by: u64,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Report local version/build state against provider-side release state.
    VersionState {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long, value_enum)]
        target: Option<Target>,
        #[arg(long, value_enum)]
        format: Option<publish::PackageFormat>,
        #[arg(long)]
        track: Option<String>,
        #[arg(long, default_value = "production")]
        site: String,
        #[arg(long)]
        artifact: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Open or create a release metadata sidecar file.
    EditFile {
        #[arg(long)]
        release: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        locale: Option<String>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },
    /// Write a referenced release metadata sidecar file non-interactively.
    WriteFile {
        #[arg(long)]
        release: String,
        #[arg(long)]
        kind: String,
        #[arg(long, value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        locale: Option<String>,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        from_file: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReleaseContentCommand {
    /// Capture screenshots/videos from configured release scenarios.
    Capture {
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        set: String,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Render store-ready screenshot/video assets from raw captures.
    Render {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Validate release-content assets and manifests.
    Validate {
        #[arg(long, value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Push rendered release-content assets to a provider.
    Push {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        locales: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BetaCommand {
    /// Manage beta groups/flights/tracks.
    Groups {
        #[command(subcommand)]
        command: BetaGroupsCommand,
    },
    /// Manage beta testers.
    Testers {
        #[command(subcommand)]
        command: BetaTestersCommand,
    },
    /// Distribute an artifact to a beta track/group.
    Distribute {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        track: Option<String>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BetaGroupsCommand {
    /// List provider beta groups, flights, or tester-track settings.
    List {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Sync configured beta groups from fission.toml to the provider.
    Sync {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long, default_value = "fission.toml")]
        from: PathBuf,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BetaTestersCommand {
    /// Import testers from a CSV into a provider group or track.
    Import {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        track: Option<String>,
        #[arg(long)]
        csv: PathBuf,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Export provider testers from a group or track to a CSV file.
    Export {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        track: Option<String>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SigningCommand {
    /// Inspect signing configuration and credential references for a target.
    Status {
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Synchronize platform signing metadata without storing secrets in fission.toml.
    Sync {
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        readonly: bool,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Import signing references into fission.toml from explicit files or environment-backed secrets.
    Import {
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        keystore: Option<PathBuf>,
        #[arg(long)]
        alias: Option<String>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReviewsCommand {
    /// List provider store reviews and recent user feedback.
    List {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Reply to a provider store review after explicit confirmation.
    Reply {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        review: String,
        #[arg(long)]
        message_file: PathBuf,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReleaseWorkflowCommand {
    /// List configured release workflows.
    List {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Run a named release workflow from fission.toml.
    Run {
        name: String,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Show provider-owned login/setup steps without storing credentials in Fission state.
    Login {
        #[arg(value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        json: bool,
    },
    /// Show provider auth setup requirements.
    Setup {
        #[arg(value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        json: bool,
    },
    /// Report whether required environment/tooling credentials are available.
    Status {
        #[arg(value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        json: bool,
    },
    /// Show how to revoke provider-owned credentials.
    Logout {
        #[arg(value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        json: bool,
    },
    /// Explain safe import paths without copying secrets into fission.toml.
    Import {
        #[arg(value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show provider-owned credential rotation steps.
    Rotate {
        #[arg(value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        json: bool,
    },
    /// Audit all provider auth requirements.
    Audit {
        #[arg(long)]
        json: bool,
    },
}
