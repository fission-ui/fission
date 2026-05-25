use anyhow::{bail, Context, Result};
use clap::Subcommand;
use fission_command_core::{DistributionProvider, Target};
use fission_command_package as publish;
use fission_credentials as credentials;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{
    Array as TomlEditArray, DocumentMut, Item as TomlEditItem, Table as TomlEditTable,
    Value as TomlEditValue,
};

mod content;
mod microsoft_store_ops;
mod model;
mod signing_ops;
mod store_ops;
mod workflow_ops;

fn provider_secret(provider: DistributionProvider, env_names: &[&str]) -> Result<Option<String>> {
    credentials::provider_secret(provider, env_names)
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Subcommand, Debug)]
pub enum ReleaseConfigCommand {
    /// Open release configuration in an editor or the Fission terminal UI.
    Edit {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        tui: bool,
    },
    /// Import provider metadata into local release files.
    Import {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        locales: Option<String>,
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
        yes: bool,
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
        yes: bool,
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
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BetaGroupsCommand {
    List {
        #[arg(long, value_enum)]
        provider: DistributionProvider,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
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
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BetaTestersCommand {
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
        json: bool,
    },
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
    Status {
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
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
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReviewsCommand {
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
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    Setup {
        #[arg(value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        json: bool,
    },
    Login {
        #[arg(value_enum)]
        provider: DistributionProvider,
    },
    Status {
        #[arg(value_enum)]
        provider: Option<DistributionProvider>,
        #[arg(long)]
        json: bool,
    },
    Logout {
        #[arg(value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        yes: bool,
    },
    Import {
        #[arg(value_enum)]
        provider: DistributionProvider,
        #[arg(long)]
        from: String,
        #[arg(long)]
        yes: bool,
    },
    Rotate {
        #[arg(value_enum)]
        provider: DistributionProvider,
    },
    Audit {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Serialize)]
struct LifecycleReport {
    area: String,
    status: String,
    provider: Option<String>,
    target: Option<String>,
    checks: Vec<LifecycleCheck>,
}

#[derive(Debug, Serialize)]
struct LifecycleCheck {
    id: String,
    status: String,
    summary: String,
    details: Option<String>,
    remediation: Vec<String>,
}

pub fn release_config(command: ReleaseConfigCommand) -> Result<()> {
    match command {
        ReleaseConfigCommand::Edit { project_dir, tui } => edit_release_config(&project_dir, tui),
        ReleaseConfigCommand::Validate {
            provider,
            project_dir,
            json,
        } => print_report(
            model::validate_release_config_model(&project_dir, provider)?,
            json,
        ),
        ReleaseConfigCommand::Set {
            field,
            value,
            project_dir,
            yes,
        } => set_release_field(&project_dir, &field, &value, yes),
        ReleaseConfigCommand::AddRelease {
            version,
            build,
            from,
            project_dir,
            yes,
        } => add_release(&project_dir, &version, build, from.as_deref(), yes),
        ReleaseConfigCommand::EditFile {
            release,
            kind,
            locale,
            project_dir,
        } => edit_release_file(&project_dir, &release, &kind, locale.as_deref()),
        ReleaseConfigCommand::Import {
            provider,
            locales,
            yes,
            project_dir,
            json,
        } => store_ops::release_config_import(provider, locales, yes, &project_dir, json),
        ReleaseConfigCommand::Diff {
            provider,
            project_dir,
            json,
        } => store_ops::release_config_diff(provider, &project_dir, json),
        ReleaseConfigCommand::Push {
            provider,
            locales,
            dry_run,
            yes,
            project_dir,
            json,
        } => store_ops::release_config_push(provider, locales, dry_run, yes, &project_dir, json),
    }
}

pub fn release_content(command: ReleaseContentCommand) -> Result<()> {
    match command {
        ReleaseContentCommand::Validate {
            provider,
            project_dir,
            json,
        } => print_report(
            content::validate_release_content_model(&project_dir, provider),
            json,
        ),
        ReleaseContentCommand::Capture {
            target,
            set,
            project_dir,
            json,
        } => print_report(
            content::capture_release_content(&project_dir, target, &set)?,
            json,
        ),
        ReleaseContentCommand::Render {
            provider,
            project_dir,
            json,
        } => print_report(
            content::render_release_content(&project_dir, provider)?,
            json,
        ),
    }
}

pub fn beta(command: BetaCommand) -> Result<()> {
    match command {
        BetaCommand::Groups { command } => match command {
            BetaGroupsCommand::List {
                provider,
                project_dir,
                json,
            } => store_ops::beta_groups_list(provider, &project_dir, json),
            BetaGroupsCommand::Sync {
                provider,
                from,
                project_dir,
                dry_run,
                json,
            } => store_ops::beta_groups_sync(provider, &from, &project_dir, dry_run, json),
        },
        BetaCommand::Testers { command } => match command {
            BetaTestersCommand::Import {
                provider,
                group,
                track,
                csv,
                project_dir,
                dry_run,
                json,
            } => store_ops::beta_testers_import(
                provider,
                group.as_deref(),
                track.as_deref(),
                &csv,
                &project_dir,
                dry_run,
                json,
            ),
            BetaTestersCommand::Export {
                provider,
                group,
                track,
                output,
                project_dir,
                json,
            } => store_ops::beta_testers_export(
                provider,
                group.as_deref(),
                track.as_deref(),
                &output,
                &project_dir,
                json,
            ),
        },
        BetaCommand::Distribute {
            provider,
            artifact,
            group,
            track,
            project_dir,
            dry_run,
            json,
        } => publish::distribute(publish::DistributeOptions {
            project_dir,
            provider,
            action: publish::DistributeAction::Publish,
            artifact: Some(artifact),
            site: group.unwrap_or_else(|| "beta".to_string()),
            deploy: None,
            track,
            dry_run,
            yes: true,
            json,
        }),
    }
}

pub fn signing(command: SigningCommand) -> Result<()> {
    match command {
        SigningCommand::Status {
            target,
            project_dir,
            json,
        } => signing_ops::status(&project_dir, target, json),
        SigningCommand::Sync {
            target,
            readonly,
            project_dir,
            json,
        } => signing_ops::sync(&project_dir, target, readonly, json),
        SigningCommand::Import {
            target,
            keystore,
            alias,
            project_dir,
            json,
        } => signing_ops::import(&project_dir, target, keystore, alias, json),
    }
}

pub fn reviews(command: ReviewsCommand) -> Result<()> {
    match command {
        ReviewsCommand::List {
            provider,
            since,
            project_dir,
            json,
        } => store_ops::reviews_list(provider, since, &project_dir, json),
        ReviewsCommand::Reply {
            provider,
            review,
            message_file,
            project_dir,
            dry_run,
            json,
        } => store_ops::reviews_reply(
            provider,
            &review,
            &message_file,
            &project_dir,
            dry_run,
            json,
        ),
    }
}

pub fn release_workflow(command: ReleaseWorkflowCommand) -> Result<()> {
    match command {
        ReleaseWorkflowCommand::List { project_dir, json } => {
            workflow_ops::list(&project_dir, json)
        }
        ReleaseWorkflowCommand::Run {
            name,
            project_dir,
            dry_run,
            json,
        } => workflow_ops::run(&project_dir, &name, dry_run, json),
    }
}

pub fn auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Status { provider, json } => {
            print_report(auth_report("auth.status", provider), json)
        }
        AuthCommand::Setup { provider, json } => print_report(auth_setup_report(provider), json),
        AuthCommand::Audit { json } => print_report(auth_report("auth.audit", None), json),
        AuthCommand::Login { provider } => login_provider(provider),
        AuthCommand::Logout { provider, yes } => {
            if !yes {
                bail!(
                    "refusing to delete {} credentials without --yes",
                    provider.as_str()
                );
            }
            let path = credentials::vault_record_path(provider)?;
            if path.exists() {
                fs::remove_file(&path)?;
                println!(
                    "Removed {} credentials from {}",
                    provider.as_str(),
                    path.display()
                );
            } else {
                println!("No stored {} credentials found", provider.as_str());
            }
            Ok(())
        }
        AuthCommand::Import {
            provider,
            from,
            yes,
        } => {
            if !yes {
                bail!(
                    "refusing to import {} credentials without --yes",
                    provider.as_str()
                );
            }
            if let Some(path) = from.strip_prefix("file:") {
                fs::metadata(path)
                    .with_context(|| format!("credential file {path} does not exist"))?;
            }
            let secret = credentials::read_secret_source(&from)?;
            credentials::store_provider_secret(provider, secret.as_bytes())?;
            println!(
                "Stored {} credentials in the encrypted Fission release vault",
                provider.as_str()
            );
            Ok(())
        }
        AuthCommand::Rotate { provider } => {
            credentials::rotate_provider_secret(provider)?;
            println!("Rotated {} vault encryption record", provider.as_str());
            Ok(())
        }
    }
}

fn login_provider(provider: DistributionProvider) -> Result<()> {
    print_login_instructions(provider);
    let secret = if io::stdin().is_terminal() {
        println!("Paste the provider token, service-account JSON, API key contents, or a file:<path>/env:<name> source, then press Enter:");
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        line.trim().to_string()
    } else {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text)?;
        text.trim().to_string()
    };
    if secret.is_empty() {
        bail!("no credential was provided for {}", provider.as_str());
    }
    let resolved = if secret.starts_with("env:") || secret.starts_with("file:") {
        credentials::read_secret_source(&secret)?
    } else {
        secret
    };
    credentials::store_provider_secret(provider, resolved.as_bytes())?;
    println!(
        "Stored {} credentials in the encrypted Fission release vault",
        provider.as_str()
    );
    Ok(())
}

fn print_login_instructions(provider: DistributionProvider) {
    match provider {
        DistributionProvider::PlayStore => println!(
            "Google Play uses an Android Publisher API service-account JSON file or a short-lived access token."
        ),
        DistributionProvider::AppStore => println!(
            "App Store Connect uses an issuer id, key id, and .p8 API private key; paste the key contents or import APP_STORE_CONNECT_API_KEY_PATH separately."
        ),
        DistributionProvider::MicrosoftStore => println!(
            "Microsoft Store uses Partner Center/Entra credentials; paste the client secret or pipe it from your secret manager."
        ),
        DistributionProvider::GithubPages => println!(
            "GitHub Pages uses a GitHub token with repository Pages/workflow permissions when direct API access is needed."
        ),
        DistributionProvider::GithubReleases => println!(
            "GitHub Releases uses the GitHub CLI. Run `gh auth login`, set GH_TOKEN/GITHUB_TOKEN, or import a token into the Fission vault."
        ),
        DistributionProvider::CloudflarePages => println!(
            "Cloudflare Pages uses an API token with Pages project edit/deploy permissions."
        ),
        DistributionProvider::Netlify => println!(
            "Netlify uses a personal access token with deploy permissions for the configured site."
        ),
        DistributionProvider::S3 => println!(
            "S3-compatible uploads normally use AWS_PROFILE or access-key environment variables; paste a provider credential only for local vault-backed workflows."
        ),
        DistributionProvider::GoogleDrive => println!(
            "Google Drive uses an OAuth access token for the target account or service account flow you manage outside the project."
        ),
        DistributionProvider::OneDrive => println!(
            "OneDrive uses a Microsoft Graph OAuth access token for the target account."
        ),
        DistributionProvider::Dropbox => println!(
            "Dropbox uses an OAuth access token with files.content.write/read scopes."
        ),
    }
}

fn edit_release_config(project_dir: &Path, tui: bool) -> Result<()> {
    let path = project_dir.join("fission.toml");
    fs::metadata(&path).with_context(|| format!("{} does not exist", path.display()))?;
    if tui {
        return fission_command_ui::run_ui(fission_command_ui::UiOptions {
            project_dir: project_dir.to_path_buf(),
            screenshot: None,
            exit_after_render: false,
            width: None,
            height: None,
        });
    }
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = Command::new(editor)
        .arg(&path)
        .status()
        .context("failed to launch editor")?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(())
}

fn set_release_field(project_dir: &Path, field: &str, value: &str, yes: bool) -> Result<()> {
    if !yes {
        bail!("set rewrites fission.toml; pass --yes after reviewing the field path");
    }
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut doc = parse_toml_edit_document(&data, &path)?;
    set_toml_edit_path(&mut doc, field, toml_edit::value(value.to_string()))?;
    write_toml_edit_document(&path, &doc)?;
    Ok(())
}

fn add_release(
    project_dir: &Path,
    version: &str,
    build: u64,
    from: Option<&str>,
    yes: bool,
) -> Result<()> {
    if !yes {
        bail!("add-release appends to fission.toml; pass --yes after reviewing the release id");
    }
    let path = project_dir.join("fission.toml");
    let mut text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let id = format!("{version}+{build}");
    text.push_str(&format!(
        "\n[[releases]]\nid = \"{id}\"\nversion = \"{version}\"\nbuild = {build}\nstatus = \"candidate\"\nmetadata = \"release-content/metadata/{id}/release.toml\"\nrelease_notes = \"release-content/metadata/{id}/notes\"\nreview = \"release-content/metadata/{id}/review.toml\"\nprivacy = \"release-content/metadata/{id}/privacy.toml\"\n"
    ));
    if let Some(source) = from {
        text.push_str(&format!("# copied_from = \"{source}\"\n"));
    }
    fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn parse_toml_edit_document(text: &str, path: &Path) -> Result<DocumentMut> {
    text.parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn write_toml_edit_document(path: &Path, doc: &DocumentMut) -> Result<()> {
    fs::write(path, format!("{doc}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn set_toml_edit_path(root: &mut DocumentMut, path: &str, value: TomlEditItem) -> Result<()> {
    let parts = path.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.trim().is_empty()) {
        bail!("field path must be dot-separated and non-empty");
    }
    let mut current = root.as_table_mut();
    for part in &parts[..parts.len() - 1] {
        current = current
            .entry(part)
            .or_insert(TomlEditItem::Table(TomlEditTable::new()))
            .as_table_mut()
            .context("field path traversed through a non-table value")?;
    }
    current[parts[parts.len() - 1]] = value;
    Ok(())
}

fn toml_edit_string_array(values: impl IntoIterator<Item = String>) -> TomlEditItem {
    let mut array = TomlEditArray::default();
    for value in values {
        array.push(value);
    }
    TomlEditItem::Value(TomlEditValue::Array(array))
}

fn edit_release_file(
    project_dir: &Path,
    release: &str,
    kind: &str,
    locale: Option<&str>,
) -> Result<()> {
    let relative = match (kind, locale) {
        ("notes", Some(locale)) => format!("release-content/metadata/{release}/notes/{locale}.md"),
        ("notes", None) => format!("release-content/metadata/{release}/notes/en-US.md"),
        ("review", _) => format!("release-content/metadata/{release}/review.toml"),
        ("privacy", _) => format!("release-content/metadata/{release}/privacy.toml"),
        ("metadata", _) | ("release", _) => {
            format!("release-content/metadata/{release}/release.toml")
        }
        other => bail!("unsupported release file kind `{}`", other.0),
    };
    let path = project_dir.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(&path, "")?;
    }
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = Command::new(editor).arg(&path).status()?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(())
}

fn auth_report(area: &str, provider: Option<DistributionProvider>) -> LifecycleReport {
    let mut report = base_report(area, provider, None);
    let providers = provider
        .map(|provider| vec![provider])
        .unwrap_or_else(auth_providers);
    for provider in providers {
        report.checks.push(provider_env_check(provider));
    }
    finalize_status(&mut report);
    report
}

fn auth_setup_report(provider: Option<DistributionProvider>) -> LifecycleReport {
    let mut report = base_report("auth.setup", provider, None);
    let providers = provider
        .map(|provider| vec![provider])
        .unwrap_or_else(auth_providers);
    for provider in providers {
        let spec = provider_auth_spec(provider);
        report.checks.push(LifecycleCheck {
            id: format!(
                "auth.{}.credential_kind",
                provider.as_str().replace('-', "_")
            ),
            status: "passed".to_string(),
            summary: format!("{} credential kind is documented", provider.as_str()),
            details: Some(spec.kind.to_string()),
            remediation: Vec::new(),
        });
        report.checks.push(LifecycleCheck {
            id: format!("auth.{}.env", provider.as_str().replace('-', "_")),
            status: "passed".to_string(),
            summary: format!("{} accepted environment variables", provider.as_str()),
            details: Some(spec.env.join(", ")),
            remediation: Vec::new(),
        });
        report.checks.push(LifecycleCheck {
            id: format!("auth.{}.setup", provider.as_str().replace('-', "_")),
            status: "passed".to_string(),
            summary: format!("{} setup command", provider.as_str()),
            details: Some(spec.command.to_string()),
            remediation: Vec::new(),
        });
        report.checks.push(LifecycleCheck {
            id: format!("auth.{}.scopes", provider.as_str().replace('-', "_")),
            status: "passed".to_string(),
            summary: format!("{} required provider permissions", provider.as_str()),
            details: Some(spec.permissions.to_string()),
            remediation: Vec::new(),
        });
    }
    finalize_status(&mut report);
    report
}

fn auth_providers() -> Vec<DistributionProvider> {
    vec![
        DistributionProvider::GithubPages,
        DistributionProvider::GithubReleases,
        DistributionProvider::CloudflarePages,
        DistributionProvider::Netlify,
        DistributionProvider::S3,
        DistributionProvider::GoogleDrive,
        DistributionProvider::OneDrive,
        DistributionProvider::Dropbox,
        DistributionProvider::PlayStore,
        DistributionProvider::AppStore,
        DistributionProvider::MicrosoftStore,
    ]
}

struct ProviderAuthSpec {
    kind: &'static str,
    env: &'static [&'static str],
    command: &'static str,
    permissions: &'static str,
}

fn provider_auth_spec(provider: DistributionProvider) -> ProviderAuthSpec {
    match provider {
        DistributionProvider::GithubPages => ProviderAuthSpec {
            kind: "GitHub token or GitHub App installation token",
            env: &["GH_TOKEN", "GITHUB_TOKEN"],
            command: "fission auth import github-pages --from env:GH_TOKEN --yes",
            permissions: "repository contents/workflows/pages permissions for local API operations; Actions deployment uses repository workflow permissions",
        },
        DistributionProvider::GithubReleases => ProviderAuthSpec {
            kind: "Authenticated GitHub CLI session, GitHub token, or GitHub App installation token",
            env: &["GH_TOKEN", "GITHUB_TOKEN"],
            command: "gh auth login",
            permissions: "repository Contents write permission to create/update releases and upload/delete release assets",
        },
        DistributionProvider::CloudflarePages => ProviderAuthSpec {
            kind: "Cloudflare API token plus Wrangler login/config for uploads",
            env: &["CLOUDFLARE_API_TOKEN", "CLOUDFLARE_ACCOUNT_ID"],
            command: "fission auth import cloudflare-pages --from env:CLOUDFLARE_API_TOKEN --yes",
            permissions: "Pages edit/deploy permission for the target account/project",
        },
        DistributionProvider::Netlify => ProviderAuthSpec {
            kind: "Netlify personal access token",
            env: &["NETLIFY_AUTH_TOKEN"],
            command: "fission auth import netlify --from env:NETLIFY_AUTH_TOKEN --yes",
            permissions: "site read/deploy permissions for the configured site",
        },
        DistributionProvider::S3 => ProviderAuthSpec {
            kind: "AWS/S3 profile or access key credentials",
            env: &["AWS_PROFILE", "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"],
            command: "fission auth import s3 --from env:AWS_SECRET_ACCESS_KEY --yes",
            permissions: "s3:PutObject, s3:ListBucket, and optional s3:PutObjectAcl for public artifacts",
        },
        DistributionProvider::GoogleDrive => ProviderAuthSpec {
            kind: "Google OAuth access token or service-account flow managed outside fission.toml",
            env: &["GOOGLE_DRIVE_ACCESS_TOKEN"],
            command: "fission auth import google-drive --from env:GOOGLE_DRIVE_ACCESS_TOKEN --yes",
            permissions: "Drive file create/update permission for the selected folder",
        },
        DistributionProvider::OneDrive => ProviderAuthSpec {
            kind: "Microsoft Graph OAuth access token",
            env: &["ONEDRIVE_ACCESS_TOKEN"],
            command: "fission auth import onedrive --from env:ONEDRIVE_ACCESS_TOKEN --yes",
            permissions: "Files.ReadWrite or equivalent delegated/application permission for the target drive",
        },
        DistributionProvider::Dropbox => ProviderAuthSpec {
            kind: "Dropbox OAuth access token",
            env: &["DROPBOX_ACCESS_TOKEN"],
            command: "fission auth import dropbox --from env:DROPBOX_ACCESS_TOKEN --yes",
            permissions: "files.content.write and files.metadata.read for the destination path",
        },
        DistributionProvider::PlayStore => ProviderAuthSpec {
            kind: "Google Play Android Publisher service-account JSON or access token",
            env: &["PLAY_STORE_SERVICE_ACCOUNT_JSON"],
            command: "fission auth import play-store --from file:play-service-account.json --yes",
            permissions: "Android Publisher API access to the configured package and release tracks",
        },
        DistributionProvider::AppStore => ProviderAuthSpec {
            kind: "App Store Connect API private key plus issuer/key ids",
            env: &[
                "APP_STORE_CONNECT_API_KEY",
                "APP_STORE_CONNECT_API_KEY_PATH",
                "APP_STORE_CONNECT_ISSUER_ID",
                "APP_STORE_CONNECT_KEY_ID",
            ],
            command: "fission auth import app-store --from file:AuthKey.p8 --yes",
            permissions: "App Manager or equivalent App Store Connect API role for metadata, uploads, TestFlight, and submissions",
        },
        DistributionProvider::MicrosoftStore => ProviderAuthSpec {
            kind: "Partner Center/Entra application secret or access token",
            env: &["MICROSOFT_STORE_TOKEN", "MICROSOFT_STORE_CLIENT_SECRET"],
            command: "fission auth import microsoft-store --from env:MICROSOFT_STORE_CLIENT_SECRET --yes",
            permissions: "Partner Center app submission permissions for the configured product",
        },
    }
}

fn provider_env_check(provider: DistributionProvider) -> LifecycleCheck {
    let vars: &[&str] = match provider {
        DistributionProvider::GithubPages => &["GH_TOKEN", "GITHUB_TOKEN"],
        DistributionProvider::GithubReleases => &["GH_TOKEN", "GITHUB_TOKEN"],
        DistributionProvider::CloudflarePages => &["CLOUDFLARE_API_TOKEN"],
        DistributionProvider::Netlify => &["NETLIFY_AUTH_TOKEN"],
        DistributionProvider::S3 => &["AWS_PROFILE", "AWS_ACCESS_KEY_ID"],
        DistributionProvider::GoogleDrive => &["GOOGLE_DRIVE_ACCESS_TOKEN"],
        DistributionProvider::OneDrive => &["ONEDRIVE_ACCESS_TOKEN"],
        DistributionProvider::Dropbox => &["DROPBOX_ACCESS_TOKEN"],
        DistributionProvider::PlayStore => &["PLAY_STORE_SERVICE_ACCOUNT_JSON"],
        DistributionProvider::AppStore => &["APP_STORE_CONNECT_API_KEY"],
        DistributionProvider::MicrosoftStore => &["MICROSOFT_STORE_TOKEN"],
    };
    let found = vars.iter().find(|name| env::var_os(name).is_some());
    let vault_path = credentials::vault_record_path(provider).ok();
    let vault_present = vault_path.as_ref().is_some_and(|path| path.exists());
    LifecycleCheck {
        id: format!("auth.{}.credentials", provider.as_str().replace('-', "_")),
        status: if found.is_some() || vault_present {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: format!("{} credentials are available", provider.as_str()),
        details: found
            .map(|name| format!("using {name}"))
            .or_else(|| vault_path.map(|path| format!("vault: {}", path.display()))),
        remediation: vec![format!(
            "Set one of {} or use `fission auth import {} --from env:<NAME> --yes` to store credentials in the encrypted local vault.",
            vars.join(", "),
            provider.as_str()
        )],
    }
}

fn set_toml_path(root: &mut toml::Value, path: &str, value: toml::Value) -> Result<()> {
    let mut current = root;
    let parts = path.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.trim().is_empty()) {
        bail!("field path must be dot-separated and non-empty");
    }
    for part in &parts[..parts.len() - 1] {
        let table = current
            .as_table_mut()
            .context("field path traversed through a non-table value")?;
        current = table
            .entry((*part).to_string())
            .or_insert_with(|| toml::Value::Table(Default::default()));
    }
    let table = current
        .as_table_mut()
        .context("field path parent is not a table")?;
    table.insert(parts[parts.len() - 1].to_string(), value);
    Ok(())
}

fn base_report(
    area: &str,
    provider: Option<DistributionProvider>,
    target: Option<Target>,
) -> LifecycleReport {
    LifecycleReport {
        area: area.to_string(),
        status: "ready".to_string(),
        provider: provider.map(|provider| provider.as_str().to_string()),
        target: target.map(|target| target.as_str().to_string()),
        checks: Vec::new(),
    }
}

fn path_check(id: &str, path: PathBuf, summary: &str) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if path.exists() { "passed" } else { "missing" }.to_string(),
        summary: summary.to_string(),
        details: Some(path.display().to_string()),
        remediation: vec![
            "Create the file/directory or update fission.toml to point at the correct path."
                .to_string(),
        ],
    }
}

fn value_path_check(value: &toml::Value, path: &str, id: &str, summary: &str) -> LifecycleCheck {
    let exists = path
        .split('.')
        .try_fold(value, |current, segment| current.get(segment))
        .is_some();
    LifecycleCheck {
        id: id.to_string(),
        status: if exists { "passed" } else { "missing" }.to_string(),
        summary: summary.to_string(),
        details: Some(path.to_string()),
        remediation: vec![
            "Add the missing release configuration or use fission release-config add-release/set."
                .to_string(),
        ],
    }
}

fn ok_check(id: &str, details: impl Into<String>) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: "passed".to_string(),
        summary: id.replace('_', " "),
        details: Some(details.into()),
        remediation: Vec::new(),
    }
}

fn warning_check(id: &str, details: String) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: "warning".to_string(),
        summary: id.replace('_', " "),
        details: Some(details),
        remediation: vec![
            "Wire the provider backend before using this command to mutate remote state."
                .to_string(),
        ],
    }
}

fn failed_check(id: &str, details: String) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: "failed".to_string(),
        summary: id.replace('_', " "),
        details: Some(details),
        remediation: vec!["Fix the reported error and rerun the command.".to_string()],
    }
}

fn finalize_status(report: &mut LifecycleReport) {
    report.status = if report
        .checks
        .iter()
        .any(|check| check.status == "failed" || check.status == "missing")
    {
        "blocked"
    } else if report.checks.iter().any(|check| check.status == "warning") {
        "warning"
    } else {
        "ready"
    }
    .to_string();
}

fn print_report(mut report: LifecycleReport, json: bool) -> Result<()> {
    finalize_status(&mut report);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}: {}", report.area, report.status);
        for check in &report.checks {
            println!("[{}] {} - {}", check.status, check.id, check.summary);
            if let Some(details) = &check.details {
                println!("  {details}");
            }
            for remediation in &check.remediation {
                println!("  fix: {remediation}");
            }
        }
    }
    if report.status == "blocked" {
        bail!("{} is blocked", report.area);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn auth_setup_documents_provider_credentials_without_secrets() {
        let report = auth_setup_report(Some(DistributionProvider::CloudflarePages));
        assert_eq!(report.status, "ready");
        assert!(report.checks.iter().any(|check| {
            check.id == "auth.cloudflare_pages.env"
                && check
                    .details
                    .as_deref()
                    .is_some_and(|details| details.contains("CLOUDFLARE_API_TOKEN"))
        }));
        assert!(report.checks.iter().any(|check| {
            check.id == "auth.cloudflare_pages.scopes"
                && check
                    .details
                    .as_deref()
                    .is_some_and(|details| details.contains("Pages"))
        }));
    }

    #[test]
    fn release_config_set_preserves_existing_comments_and_formatting() {
        let dir =
            std::env::temp_dir().join(format!("fission-release-config-set-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fission.toml");
        fs::write(&path, "# keep this comment\n[app]\nname = \"Todo\"\n").unwrap();

        set_release_field(&dir, "app.version", "1.2.3", true).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep this comment"));
        assert!(text.contains("version = \"1.2.3\""));
        assert!(text.contains("name = \"Todo\""));

        let _ = fs::remove_dir_all(&dir);
    }
}
