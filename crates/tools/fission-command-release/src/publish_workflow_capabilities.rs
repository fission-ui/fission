use super::*;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct ProviderCapability {
    pub(super) id: String,
    pub(super) status: ProviderCapabilityStatus,
    pub(super) summary: String,
    pub(super) details: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ProviderCapabilityStatus {
    Supported,
    Manual,
    Handoff,
    Unsupported,
    NotApplicable,
}

pub(super) fn provider_capability_readiness_check(
    capability: ProviderCapability,
) -> ReadinessCheck {
    let (status, summary_prefix) = match capability.status {
        ProviderCapabilityStatus::Supported => (CheckStatus::Passed, "supported"),
        ProviderCapabilityStatus::Manual => (CheckStatus::Warning, "manual"),
        ProviderCapabilityStatus::Handoff => (CheckStatus::Warning, "handoff"),
        ProviderCapabilityStatus::Unsupported => (CheckStatus::Skipped, "unsupported"),
        ProviderCapabilityStatus::NotApplicable => (CheckStatus::Skipped, "not applicable"),
    };
    ReadinessCheck {
        id: capability.id,
        severity: CheckSeverity::Info,
        status,
        summary: format!("{summary_prefix}: {}", capability.summary),
        details: capability.details,
        remediation: Vec::new(),
    }
}

pub(super) fn provider_capabilities(provider: DistributionProvider) -> Vec<ProviderCapability> {
    use ProviderCapabilityStatus::*;

    let mut capabilities = Vec::new();
    match provider {
        DistributionProvider::PlayStore => {
            capabilities.push(capability(
                "artifact_upload",
                Supported,
                "Android APK/AAB upload through Google Play edits",
                Some("Uses the Android Publisher API, validates the edit, and commits it."),
            ));
            capabilities.push(capability(
                "metadata_sync",
                Supported,
                "localized Google Play listing metadata sync",
                Some("Uses release-config push for configured locales."),
            ));
            capabilities.push(capability(
                "release_content_assets",
                Supported,
                "Google Play preview image upload",
                Some("Uploads rendered release-content images where configured."),
            ));
            capabilities.push(capability(
                "beta_distribution",
                Supported,
                "internal, closed, open, and production tracks",
                Some("Track selection is modeled as --track and provider config."),
            ));
            capabilities.push(capability(
                "rollback",
                Unsupported,
                "Play version codes are immutable after upload",
                Some("Publish a newer version code or use Play Console track controls."),
            ));
        }
        DistributionProvider::AppStore => {
            capabilities.push(capability(
                "artifact_upload",
                Supported,
                "IPA upload through App Store Connect tooling",
                Some("Uses Xcode upload tooling and App Store Connect API checks."),
            ));
            capabilities.push(capability(
                "metadata_sync",
                Supported,
                "App Store Connect version and localization metadata sync",
                Some("Uses release-config push for configured locales."),
            ));
            capabilities.push(capability(
                "release_content_assets",
                Supported,
                "App Store screenshot, preview, and review attachment upload",
                Some("Screenshots, app previews, and review attachments upload through App Store Connect asset reservation/upload/commit APIs."),
            ));
            capabilities.push(capability(
                "beta_distribution",
                Supported,
                "TestFlight group/build assignment",
                Some("Use `fission beta distribute --provider app-store --group <group>` after the build is processed."),
            ));
            capabilities.push(capability(
                "app_review_submission",
                Supported,
                "App Review submission",
                Some("Use `fission distribute promote --provider app-store --track app-store-review --artifact <manifest>` after the build is processed."),
            ));
            capabilities.push(capability(
                "rollback",
                Unsupported,
                "App Store build numbers are immutable after upload",
                Some("Upload a newer build or manage phased release state in App Store Connect."),
            ));
        }
        DistributionProvider::MicrosoftStore => {
            capabilities.push(capability(
                "artifact_upload",
                Supported,
                "Microsoft Store package submission",
                Some("Uses Store submission APIs for MSI/EXE and msstore for MSIX/MSIXUPLOAD."),
            ));
            capabilities.push(capability(
                "metadata_sync",
                Supported,
                "Microsoft Store listing metadata sync",
                Some("Uses release-config push where provider config is complete."),
            ));
            capabilities.push(capability(
                "release_content_assets",
                Supported,
                "Microsoft Store screenshot and logo upload",
                Some("Screenshots and logos upload through Microsoft Store listing asset SAS URLs; trailers are staged into an auditable handoff manifest."),
            ));
            capabilities.push(capability(
                "trailer_assets",
                Handoff,
                "Microsoft Store trailer handoff",
                Some("Trailer upload remains a Partner Center/provider-tool follow-up recorded in the handoff manifest."),
            ));
            capabilities.push(capability(
                "beta_distribution",
                Supported,
                "private flight package publishing",
                Some("Private flights are modeled through track/flight configuration."),
            ));
            capabilities.push(capability(
                "rollback",
                Manual,
                "Partner Center rollback or replacement",
                Some("Fission records the submission state; rollback remains a provider-console action."),
            ));
        }
        DistributionProvider::GithubReleases => {
            capabilities.push(capability(
                "artifact_upload",
                Supported,
                "GitHub release asset upload",
                Some("Uses gh release commands and explicit duplicate-asset policy."),
            ));
            capabilities.push(capability(
                "metadata_sync",
                Supported,
                "GitHub release title/body/draft/prerelease metadata",
                Some("Metadata is sourced from distribution.github_releases config and release files."),
            ));
            capabilities.push(capability(
                "release_content_assets",
                NotApplicable,
                "store listing screenshots are not a GitHub Releases concept",
                None,
            ));
            capabilities.push(capability(
                "rollback",
                Manual,
                "release rollback through GitHub release/tag management",
                Some("Use explicit tags/releases; rollback is provider repository state."),
            ));
        }
        DistributionProvider::GithubPages
        | DistributionProvider::CloudflarePages
        | DistributionProvider::Netlify => {
            capabilities.push(capability(
                "artifact_upload",
                Supported,
                "static site deployment",
                Some("Publishes a built static artifact to the selected static hosting provider."),
            ));
            capabilities.push(capability(
                "metadata_sync",
                NotApplicable,
                "app-store listing metadata is not a static-hosting concept",
                None,
            ));
            capabilities.push(capability(
                "release_content_assets",
                NotApplicable,
                "store screenshots are not uploaded to static hosting providers",
                None,
            ));
            capabilities.push(capability(
                "promote",
                Manual,
                "provider-specific preview promotion",
                Some("Where supported, promote/status commands model provider deployment state."),
            ));
        }
        DistributionProvider::DockerRegistry => {
            capabilities.push(capability(
                "artifact_upload",
                Supported,
                "OCI image push",
                Some("Tags and pushes images described by the artifact manifest."),
            ));
            capabilities.push(capability(
                "metadata_sync",
                NotApplicable,
                "store listing metadata is not an OCI registry concept",
                None,
            ));
            capabilities.push(capability(
                "release_content_assets",
                NotApplicable,
                "store screenshots are not uploaded to OCI registries",
                None,
            ));
            capabilities.push(capability(
                "rollback",
                Manual,
                "registry tag rollback",
                Some("Move tags or deploy an earlier digest using your deployment system."),
            ));
        }
        DistributionProvider::S3
        | DistributionProvider::GoogleDrive
        | DistributionProvider::OneDrive
        | DistributionProvider::Dropbox => {
            capabilities.push(capability(
                "artifact_upload",
                Supported,
                "file/object upload",
                Some("Uploads files from the artifact manifest to the selected storage provider."),
            ));
            capabilities.push(capability(
                "metadata_sync",
                NotApplicable,
                "store listing metadata is not a file storage concept",
                None,
            ));
            capabilities.push(capability(
                "release_content_assets",
                NotApplicable,
                "store screenshots are not uploaded as listing assets",
                None,
            ));
            capabilities.push(capability(
                "rollback",
                Manual,
                "object/file replacement or restore",
                Some("Use object versioning, backups, or provider-side restore where configured."),
            ));
        }
    }
    capabilities
}

pub(super) fn capability(
    id: &str,
    status: ProviderCapabilityStatus,
    summary: &str,
    details: Option<&str>,
) -> ProviderCapability {
    ProviderCapability {
        id: format!("provider.capability.{id}"),
        status,
        summary: summary.to_string(),
        details: details.map(str::to_string),
    }
}

pub(super) fn provider_capability_status_label(status: ProviderCapabilityStatus) -> &'static str {
    match status {
        ProviderCapabilityStatus::Supported => "supported",
        ProviderCapabilityStatus::Manual => "manual",
        ProviderCapabilityStatus::Handoff => "handoff",
        ProviderCapabilityStatus::Unsupported => "unsupported",
        ProviderCapabilityStatus::NotApplicable => "not-applicable",
    }
}
