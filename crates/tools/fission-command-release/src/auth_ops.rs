use anyhow::Result;
use fission_command_core::DistributionProvider;
use std::env;

use super::{
    base_report, finalize_status, print_report, AuthCommand, LifecycleCheck, LifecycleReport,
};

pub(super) fn auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login { provider, json } => {
            print_report(auth_setup_report("auth.login", provider), json)
        }
        AuthCommand::Status { provider, json } => {
            print_report(auth_report("auth.status", provider), json)
        }
        AuthCommand::Setup { provider, json } => {
            print_report(auth_setup_report("auth.setup", provider), json)
        }
        AuthCommand::Logout { provider, json } => print_report(auth_logout_report(provider), json),
        AuthCommand::Import {
            provider,
            from,
            json,
        } => print_report(auth_import_report(provider, from.as_deref()), json),
        AuthCommand::Rotate { provider, json } => print_report(auth_rotate_report(provider), json),
        AuthCommand::Audit { json } => print_report(auth_report("auth.audit", None), json),
    }
}

pub(super) fn auth_report(area: &str, provider: Option<DistributionProvider>) -> LifecycleReport {
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

pub(super) fn auth_setup_report(
    area: &str,
    provider: Option<DistributionProvider>,
) -> LifecycleReport {
    let mut report = base_report(area, provider, None);
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

pub(super) fn auth_logout_report(provider: Option<DistributionProvider>) -> LifecycleReport {
    let mut report = base_report("auth.logout", provider, None);
    let providers = provider
        .map(|provider| vec![provider])
        .unwrap_or_else(auth_providers);
    for provider in providers {
        let spec = provider_auth_spec(provider);
        report.checks.push(LifecycleCheck {
            id: format!("auth.{}.logout", provider.as_str().replace('-', "_")),
            status: "warning".to_string(),
            summary: format!("{} credential revocation is provider-owned", provider.as_str()),
            details: Some(spec.logout.to_string()),
            remediation: vec![
                "Remove related environment variables from the current shell/CI secret store."
                    .to_string(),
                "Revoke or rotate the provider credential in the provider console when the key is no longer trusted."
                    .to_string(),
            ],
        });
    }
    finalize_status(&mut report);
    report
}

pub(super) fn auth_import_report(
    provider: DistributionProvider,
    from: Option<&str>,
) -> LifecycleReport {
    let mut report = base_report("auth.import", Some(provider), None);
    let spec = provider_auth_spec(provider);
    report.checks.push(LifecycleCheck {
        id: format!("auth.{}.import_policy", provider.as_str().replace('-', "_")),
        status: "warning".to_string(),
        summary: "Fission does not import provider secrets into project state".to_string(),
        details: from.map(|value| format!("requested source: {value}")),
        remediation: vec![
            format!(
                "Expose the credential through one of these environment variables instead: {}.",
                spec.env.join(", ")
            ),
            "For CI, store secret material in the CI secret store; for local release, use an owner-only ~/.fission/<app>/release.env sourced by your shell."
                .to_string(),
            "Do not write secret file paths, tokens, service-account JSON, .p8 keys, JKS files, or PFX/P12 files to fission.toml."
                .to_string(),
        ],
    });
    finalize_status(&mut report);
    report
}

pub(super) fn auth_rotate_report(provider: Option<DistributionProvider>) -> LifecycleReport {
    let mut report = base_report("auth.rotate", provider, None);
    let providers = provider
        .map(|provider| vec![provider])
        .unwrap_or_else(auth_providers);
    for provider in providers {
        let spec = provider_auth_spec(provider);
        report.checks.push(LifecycleCheck {
            id: format!("auth.{}.rotate", provider.as_str().replace('-', "_")),
            status: "warning".to_string(),
            summary: format!("{} credential rotation is provider-owned", provider.as_str()),
            details: Some(spec.rotate.to_string()),
            remediation: vec![
                "Create the replacement credential in the provider console or provider CLI."
                    .to_string(),
                format!(
                    "Update the CI/local environment variables used by Fission: {}.",
                    spec.env.join(", ")
                ),
                "Run `fission auth status` and the relevant `fission readiness release` check before publishing."
                    .to_string(),
            ],
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
        DistributionProvider::DockerRegistry,
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
    logout: &'static str,
    rotate: &'static str,
    permissions: &'static str,
}

fn provider_auth_spec(provider: DistributionProvider) -> ProviderAuthSpec {
    match provider {
        DistributionProvider::GithubPages => ProviderAuthSpec {
            kind: "GitHub token or GitHub App installation token",
            env: &["GH_TOKEN", "GITHUB_TOKEN"],
            command: "export GH_TOKEN=<github-token>",
            logout: "unset GH_TOKEN GITHUB_TOKEN; revoke the GitHub token in GitHub settings when needed",
            rotate: "create a replacement GitHub token/App installation token, update GH_TOKEN or GITHUB_TOKEN, then revoke the old token",
            permissions: "repository contents/workflows/pages permissions for local API operations; Actions deployment uses repository workflow permissions",
        },
        DistributionProvider::GithubReleases => ProviderAuthSpec {
            kind: "Authenticated GitHub CLI session, GitHub token, or GitHub App installation token",
            env: &["GH_TOKEN", "GITHUB_TOKEN"],
            command: "gh auth login",
            logout: "gh auth logout --hostname github.com and unset GH_TOKEN GITHUB_TOKEN",
            rotate: "run gh auth refresh or create a replacement GitHub token, update GH_TOKEN/GITHUB_TOKEN, then revoke the old credential",
            permissions: "repository Contents write permission to create/update releases and upload/delete release assets",
        },
        DistributionProvider::CloudflarePages => ProviderAuthSpec {
            kind: "Cloudflare API token plus Wrangler login/config for uploads",
            env: &["CLOUDFLARE_API_TOKEN", "CLOUDFLARE_ACCOUNT_ID"],
            command: "export CLOUDFLARE_API_TOKEN=<cloudflare-token>",
            logout: "unset CLOUDFLARE_API_TOKEN CLOUDFLARE_ACCOUNT_ID; revoke the API token in Cloudflare",
            rotate: "create a replacement Cloudflare API token with the same Pages scope, update CLOUDFLARE_API_TOKEN, then revoke the old token",
            permissions: "Pages edit/deploy permission for the target account/project",
        },
        DistributionProvider::DockerRegistry => ProviderAuthSpec {
            kind: "Authenticated Docker CLI session for the target registry",
            env: &["DOCKER_CONFIG"],
            command: "docker login <registry>",
            logout: "docker logout <registry>",
            rotate: "create a replacement registry token/password, run docker login with it, then revoke the old credential",
            permissions: "push permission for every image repository configured in [distribution.docker_registry.<profile>].tags",
        },
        DistributionProvider::Netlify => ProviderAuthSpec {
            kind: "Netlify personal access token",
            env: &["NETLIFY_AUTH_TOKEN"],
            command: "export NETLIFY_AUTH_TOKEN=<netlify-token>",
            logout: "unset NETLIFY_AUTH_TOKEN; revoke the personal access token in Netlify",
            rotate: "create a replacement Netlify personal access token, update NETLIFY_AUTH_TOKEN, then revoke the old token",
            permissions: "site read/deploy permissions for the configured site",
        },
        DistributionProvider::S3 => ProviderAuthSpec {
            kind: "AWS/S3 profile or access key credentials",
            env: &["AWS_PROFILE", "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"],
            command: "export AWS_PROFILE=<profile> # or set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY",
            logout: "unset AWS_PROFILE AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN",
            rotate: "rotate the IAM/S3-compatible access key in the provider, update AWS_PROFILE or AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY, then disable the old key",
            permissions: "s3:PutObject, s3:ListBucket, and optional s3:PutObjectAcl for public artifacts",
        },
        DistributionProvider::GoogleDrive => ProviderAuthSpec {
            kind: "Google OAuth access token or service-account flow managed outside fission.toml",
            env: &["GOOGLE_DRIVE_ACCESS_TOKEN"],
            command: "export GOOGLE_DRIVE_ACCESS_TOKEN=<access-token>",
            logout: "unset GOOGLE_DRIVE_ACCESS_TOKEN; revoke the OAuth token or service account key in Google Cloud",
            rotate: "create a replacement Google OAuth/service-account credential, update GOOGLE_DRIVE_ACCESS_TOKEN, then revoke the old credential",
            permissions: "Drive file create/update permission for the selected folder",
        },
        DistributionProvider::OneDrive => ProviderAuthSpec {
            kind: "Microsoft Graph OAuth access token",
            env: &["ONEDRIVE_ACCESS_TOKEN"],
            command: "export ONEDRIVE_ACCESS_TOKEN=<access-token>",
            logout: "unset ONEDRIVE_ACCESS_TOKEN; revoke the app grant/token in Microsoft Entra",
            rotate: "create a replacement Microsoft Graph credential, update ONEDRIVE_ACCESS_TOKEN, then revoke the old credential",
            permissions: "Files.ReadWrite or equivalent delegated/application permission for the target drive",
        },
        DistributionProvider::Dropbox => ProviderAuthSpec {
            kind: "Dropbox OAuth access token",
            env: &["DROPBOX_ACCESS_TOKEN"],
            command: "export DROPBOX_ACCESS_TOKEN=<access-token>",
            logout: "unset DROPBOX_ACCESS_TOKEN; revoke the Dropbox OAuth token/app grant",
            rotate: "create a replacement Dropbox OAuth token, update DROPBOX_ACCESS_TOKEN, then revoke the old token",
            permissions: "files.content.write and files.metadata.read for the destination path",
        },
        DistributionProvider::PlayStore => ProviderAuthSpec {
            kind: "Google Play Android Publisher service-account JSON or access token",
            env: &[
                "PLAY_STORE_ACCESS_TOKEN",
                "PLAY_STORE_SERVICE_ACCOUNT_JSON",
                "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64",
                "GOOGLE_APPLICATION_CREDENTIALS",
            ],
            command: "export PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64=<base64-service-account-json>",
            logout: "unset PLAY_STORE_ACCESS_TOKEN PLAY_STORE_SERVICE_ACCOUNT_JSON PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64 GOOGLE_APPLICATION_CREDENTIALS",
            rotate: "create a replacement Google service-account key, update PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64 or CI secret files, then delete the old key",
            permissions: "Android Publisher API access to the configured package and release tracks",
        },
        DistributionProvider::AppStore => ProviderAuthSpec {
            kind: "App Store Connect API private key plus issuer/key ids",
            env: &[
                "APP_STORE_CONNECT_API_KEY",
                "APP_STORE_CONNECT_API_KEY_BASE64",
                "APP_STORE_CONNECT_API_KEY_PATH",
                "APP_STORE_CONNECT_ISSUER_ID",
                "APP_STORE_CONNECT_KEY_ID",
            ],
            command: "export APP_STORE_CONNECT_API_KEY_BASE64=<base64-p8-key>",
            logout: "unset APP_STORE_CONNECT_ACCESS_TOKEN APP_STORE_CONNECT_API_KEY APP_STORE_CONNECT_API_KEY_BASE64 APP_STORE_CONNECT_API_KEY_PATH APP_STORE_CONNECT_ISSUER_ID APP_STORE_CONNECT_KEY_ID",
            rotate: "create a replacement App Store Connect API key, update APP_STORE_CONNECT_API_KEY_BASE64 plus issuer/key ids, then revoke the old key",
            permissions: "App Manager or equivalent App Store Connect API role for metadata, uploads, TestFlight, and submissions",
        },
        DistributionProvider::MicrosoftStore => ProviderAuthSpec {
            kind: "Partner Center/Entra application secret or access token",
            env: &[
                "MICROSOFT_STORE_TOKEN",
                "MICROSOFT_STORE_CLIENT_SECRET",
                "PARTNER_CENTER_CLIENT_SECRET",
            ],
            command: "export MICROSOFT_STORE_CLIENT_SECRET=<partner-center-secret>",
            logout: "unset MICROSOFT_STORE_TOKEN MICROSOFT_STORE_CLIENT_SECRET PARTNER_CENTER_CLIENT_SECRET",
            rotate: "create a replacement Entra/Partner Center client secret, update MICROSOFT_STORE_CLIENT_SECRET or PARTNER_CENTER_CLIENT_SECRET, then revoke the old secret",
            permissions: "Partner Center app submission permissions for the configured product",
        },
    }
}

fn provider_env_check(provider: DistributionProvider) -> LifecycleCheck {
    let vars: &[&str] = match provider {
        DistributionProvider::GithubPages => &["GH_TOKEN", "GITHUB_TOKEN"],
        DistributionProvider::GithubReleases => &["GH_TOKEN", "GITHUB_TOKEN"],
        DistributionProvider::CloudflarePages => &["CLOUDFLARE_API_TOKEN"],
        DistributionProvider::DockerRegistry => &["DOCKER_CONFIG"],
        DistributionProvider::Netlify => &["NETLIFY_AUTH_TOKEN"],
        DistributionProvider::S3 => &["AWS_PROFILE", "AWS_ACCESS_KEY_ID"],
        DistributionProvider::GoogleDrive => &["GOOGLE_DRIVE_ACCESS_TOKEN"],
        DistributionProvider::OneDrive => &["ONEDRIVE_ACCESS_TOKEN"],
        DistributionProvider::Dropbox => &["DROPBOX_ACCESS_TOKEN"],
        DistributionProvider::PlayStore => &[
            "PLAY_STORE_ACCESS_TOKEN",
            "PLAY_STORE_SERVICE_ACCOUNT_JSON",
            "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64",
            "GOOGLE_APPLICATION_CREDENTIALS",
        ],
        DistributionProvider::AppStore => &[
            "APP_STORE_CONNECT_ACCESS_TOKEN",
            "APP_STORE_CONNECT_API_KEY",
            "APP_STORE_CONNECT_API_KEY_BASE64",
            "APP_STORE_CONNECT_API_KEY_PATH",
        ],
        DistributionProvider::MicrosoftStore => &[
            "MICROSOFT_STORE_TOKEN",
            "MICROSOFT_STORE_CLIENT_SECRET",
            "PARTNER_CENTER_CLIENT_SECRET",
        ],
    };
    let found = vars.iter().find(|name| env::var_os(name).is_some());
    LifecycleCheck {
        id: format!("auth.{}.credentials", provider.as_str().replace('-', "_")),
        status: if found.is_some() { "passed" } else { "missing" }.to_string(),
        summary: format!("{} credentials are available", provider.as_str()),
        details: found.map(|name| format!("using {name}")),
        remediation: vec![format!(
            "Set one of {} from your shell, CI secret store, or platform-specific credential tool.",
            vars.join(", ")
        )],
    }
}
