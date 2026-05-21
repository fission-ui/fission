# Post-build lifecycle: packaging, signing, distribution, and release automation

Status: proposal/specification  
Audience: Fission CLI, platform shell, tooling, and release engineering implementers  
Scope: everything after `fission build` produces a release binary or web bundle

## 1. Purpose

Fission should manage the whole lifecycle of an application, not just compile and run it during development. A production team should be able to use the Fission CLI to answer four questions for every supported platform:

1. Can this project be packaged for this platform on this machine?
2. Can this project be signed or notarized where the platform requires it?
3. Can this artifact be distributed to the selected store, track, bucket, drive, or folder?
4. If any answer is no, what exact setup step is missing?

This document specifies the required command model, package formats, store distribution flows, cloud upload flows, release metadata, release-content automation, beta testing operations, credential storage model, readiness checks, and acceptance criteria for that post-build lifecycle. `fission.toml` is the authoritative release root: it declares provider IDs, active releases, tracks, locales, asset paths, and references to release-specific metadata files. Long-form store text, full release notes, review instructions, privacy declarations, and other bulky per-release content should live in referenced files so `fission.toml` remains reviewable by humans. Release-content commands produce or validate the files referenced by that root, such as screenshots, app preview videos, trailers, review attachments, receipts, and generated manifests.

The design is Rust-first at the orchestration layer. Fission should use Rust crates and libraries for project configuration, manifests, readiness checks, release-content generation, cloud uploads, receipts, credential handling, and provider API clients where they are production-ready. Platform-owned packaging, signing, notarization, validation, device deployment, and store upload paths remain anchored on the tools, SDKs, command-line programs, and APIs provided by Apple, Google/Android, and Microsoft.

## 2. Requirement language

- MUST means required for the feature to be accepted.
- SHOULD means required unless there is a documented platform limitation.
- MAY means optional behavior.
- Interactive means a command can open a browser, prompt the user, or guide manual portal steps.
- Non-interactive means suitable for CI and must fail with machine-readable diagnostics instead of prompting.

## 3. Lifecycle model

Fission needs separate concepts for build, package, sign, capture, render, validate, distribute, publish, promote, and observe.

- Build: compile Rust, platform shells, assets, and web output into platform-specific build directories.
- Package: turn a build output into an installable or uploadable artifact such as `.run`, `.app`, `.pkg`, `.msix`, `.apk`, `.aab`, `.ipa`, or a static web archive.
- Sign: apply platform signing or artifact signing where required.
- Notarize: submit macOS direct-distribution artifacts to Apple's notary service and staple the ticket when required.
- Capture: run scripted app states on simulators, emulators, browsers, or devices and capture screenshots, videos, logs, and diagnostics.
- Render: transform raw captures and release text into store-ready localized screenshots, app preview videos, and marketing assets without changing the application binary.
- Validate: check structure, required metadata, signing state, store rules, beta testing configuration, store assets, and installability before upload.
- Distribute: upload an artifact, release metadata, or release-content asset set somewhere, including stores and object/file storage providers.
- Publish: make an uploaded store submission available to testers or users when the provider separates upload from rollout.
- Promote: move an existing release from one track/channel to another, such as Android internal testing to production.
- Observe: fetch release status, tester state, review feedback, and provider-side processing results after upload.

The CLI must keep these steps separate internally even when the user chooses a single high-level command that runs several of them.

## 4. Command surface

The public CLI should use `package` for artifact creation and `distribute` for everything that sends artifacts to a destination. `doctor` remains the broad environment diagnostic command, while `readiness` is the focused release preflight command.

```text
fission readiness package --project-dir . --target <target> --format <format> [--release]
fission readiness distribute --project-dir . --provider <provider> [--artifact <manifest>] [--track <track>]
fission readiness release --project-dir . --target <target> --format <format> --provider <provider>

fission package --project-dir . --target linux --format run --release
fission package --project-dir . --target macos --format app --release
fission package --project-dir . --target macos --format pkg --release
fission package --project-dir . --target windows --format exe --release
fission package --project-dir . --target windows --format msi --release
fission package --project-dir . --target windows --format msix --release
fission package --project-dir . --target android --format apk --release
fission package --project-dir . --target android --format aab --release
fission package --project-dir . --target ios --format ipa --release
fission package --project-dir . --target web --format static --release

fission distribute --project-dir . --provider s3 --artifact <manifest>
fission distribute --project-dir . --provider google-drive --artifact <manifest>
fission distribute --project-dir . --provider onedrive --artifact <manifest>
fission distribute --project-dir . --provider dropbox --artifact <manifest>
fission distribute --project-dir . --provider github-releases --artifact <manifest> --site production --deploy v1.2.3
fission distribute --project-dir . --provider github-pages --artifact <manifest> --site production
fission distribute --project-dir . --provider cloudflare-pages --artifact <manifest> --site production
fission distribute --project-dir . --provider netlify --artifact <manifest> --site production
fission distribute setup --project-dir . --provider github-pages --site production
fission distribute status --project-dir . --provider cloudflare-pages --site production
fission distribute promote --project-dir . --provider netlify --deploy <deploy-id> --site production
fission distribute rollback --project-dir . --provider netlify --deploy <deploy-id> --site production
fission distribute --project-dir . --provider play-store --artifact <manifest> --track internal
fission distribute --project-dir . --provider app-store --artifact <manifest> --track testflight
fission distribute --project-dir . --provider microsoft-store --artifact <manifest> --track public

fission release-config edit --project-dir .
fission release-config edit --project-dir . --tui
fission release-config import --project-dir . --provider play-store --locales en-US,fr-FR
fission release-config diff --project-dir . --provider play-store
fission release-config validate --project-dir . --provider app-store
fission release-config push --project-dir . --provider play-store --locales en-US,fr-FR --yes

fission release-content capture --project-dir . --target ios --set app-store
fission release-content capture --project-dir . --target android --set play-store
fission release-content render --project-dir . --provider app-store
fission release-content validate --project-dir . --provider app-store

fission beta groups list --project-dir . --provider app-store
fission beta groups sync --project-dir . --provider app-store --from fission.toml
fission beta testers import --project-dir . --provider app-store --group external-beta --csv testers.csv
fission beta testers export --project-dir . --provider play-store --track closed --output testers.csv
fission beta distribute --project-dir . --provider app-store --artifact <manifest> --group external-beta
fission beta distribute --project-dir . --provider play-store --artifact <manifest> --track internal

fission signing status --project-dir . --target ios
fission signing sync --project-dir . --target ios --readonly
fission signing import --project-dir . --target android --keystore upload.jks --alias upload

fission reviews list --project-dir . --provider play-store --since 30d
fission reviews reply --project-dir . --provider app-store --review <id> --message-file reply.txt

fission auth login <provider>
fission auth status [provider]
fission auth logout <provider>
fission auth import <provider> --from <file-or-env>
fission auth rotate <provider>
fission auth audit
```

The `release-config`, `release-content`, `beta`, `signing`, and `reviews` commands are part of the release workflow, but they must not become hidden side effects of packaging. A package command may point to the content manifest it expects, but release metadata editing, content capture, beta group management, and customer review response are explicit operations. The `readiness` commands MUST emit both human output and JSON output. JSON output is required for CI and IDE integrations.

`fission.toml` is the authoritative release root. It can contain short metadata directly, but bulky release-specific content should be referenced by path. The CLI MUST support three ways to maintain it:

- manual editing for developers who prefer normal text files and code review;
- TUI editing for interactive guided setup on a developer machine;
- non-interactive subcommands for CI, scripts, and users who prefer explicit command arguments.

The TUI MUST only write `fission.toml`, asset files, or generated receipts that the user can inspect. It must not store hidden project state. Non-interactive commands MUST support `--json`, `--yes`, `--dry-run`, and explicit field paths so they can be used in repeatable automation.

```text
fission readiness release --target android --format aab --provider play-store --json
```

The JSON schema MUST include:

```json
{
  "project_dir": "/absolute/path",
  "target": "android",
  "format": "aab",
  "provider": "play-store",
  "status": "blocked",
  "checks": [
    {
      "id": "android.sdk.build_tools.apksigner",
      "severity": "error",
      "status": "missing",
      "summary": "Android SDK build-tools apksigner was not found",
      "details": "Checked ANDROID_HOME, ANDROID_SDK_ROOT, and standard SDK locations.",
      "remediation": [
        "Install Android SDK build-tools from Android Studio or sdkmanager.",
        "Set ANDROID_HOME or ANDROID_SDK_ROOT if the SDK is installed in a custom location."
      ]
    }
  ]
}
```

Severity is `error`, `warning`, or `info`. `error` blocks packaging or distribution. `warning` allows the operation but must be visible. `info` records detected facts.

## 5. Project configuration

Release configuration and release metadata are rooted in `fission.toml`. Secrets do not belong in `fission.toml`. The root file should stay compact: it contains stable app identity, provider configuration, the list of releases, short labels, tracks, locales, and paths to the files that hold long-form release content. Asset files such as screenshots, app preview videos, trailers, icons, review attachments, and large generated images live on disk and are referenced by path from `fission.toml`. Generated manifests and provider receipts live under `release-content/` or `target/fission/`; they are audit outputs, not editable inputs.

`fission.toml` must remain hand-editable. The schema should be explicit enough for code review and precise enough for automation, but it must not become a dumping ground for every localized paragraph of every release. The CLI may reorganize, normalize, or populate fields, but it must preserve comments where practical and avoid destructive rewrites of unrelated sections. `fission release-config edit` opens the configured text editor by default; `fission release-config edit --tui` opens the guided terminal UI. Both edit the same root manifest and the referenced content files, and both must produce the same validation result.

```toml
[app]
id = "com.example.todo"
name = "Todo"
version = "1.2.3"
build = 42
publisher = "Example Software Ltd"
homepage = "https://example.com/todo"
support_url = "https://example.com/support"
privacy_url = "https://example.com/privacy"
license = "MIT"

[package]
icon = "assets/app-icon.png"
category = "Productivity"
copyright = "Copyright (c) 2026 Example Software Ltd"

[package.linux.run]
install_name = "todo"
default_scope = "user"
include_desktop_entry = true
include_appstream_metadata = true
compression = "zstd"

[package.macos]
bundle_id = "com.example.todo"
minimum_os = "13.0"
entitlements = "platforms/macos/entitlements.plist"
signing_identity = "Developer ID Application: Example Software Ltd (TEAMID1234)"
installer_identity = "Developer ID Installer: Example Software Ltd (TEAMID1234)"
notarize = true

[package.windows]
identity_name = "ExampleSoftware.Todo"
publisher = "CN=Example Software Ltd, O=Example Software Ltd, C=GB"
installer = "msix"
capabilities = []

[package.android]
package_name = "com.example.todo"
version_code = 42
version_name = "1.2.3"
min_sdk = 26
target_sdk = 36
keystore_alias = "upload"

[package.ios]
bundle_id = "com.example.todo"
build_number = "42"
marketing_version = "1.2.3"
team_id = "TEAMID1234"
entitlements = "platforms/ios/entitlements.plist"
provisioning_profile = "Todo App Store.mobileprovision"

[distribution.s3.production]
endpoint = "https://s3.amazonaws.com"
bucket = "example-downloads"
prefix = "todo/releases/{version}/"
visibility = "presigned"
presign_ttl_seconds = 604800

[distribution.github_pages.production]
owner = "example"
repo = "todo-site"
mode = "actions"
source = "github-actions"
site_kind = "project"
base_path = "/todo-site/"
custom_domain = ""
enforce_https = true

[distribution.github_pages.docs]
owner = "example"
repo = "todo"
mode = "branch"
source_branch = "gh-pages"
source_path = "/"
site_kind = "project"
base_path = "/todo/"
custom_domain = "docs.example.com"
enforce_https = true

[distribution.github_releases.production]
owner = "example"
repo = "todo"
tag = "v1.2.3"
name = "Todo 1.2.3"
notes_file = "release-content/metadata/1.2.3+42/notes/en-US.md"
draft = true
prerelease = false
replace_assets = true
upload_artifact_manifest = true

[distribution.cloudflare_pages.production]
account_id = "00000000000000000000000000000000"
project_name = "todo"
environment = "production"
custom_domain = "todo.example.com"
base_path = "/"

[distribution.netlify.production]
site_id = "00000000-0000-0000-0000-000000000000"
team_slug = "example"
production = true
custom_domain = "todo.example.com"
base_path = "/"

[distribution.play_store]
package_name = "com.example.todo"
default_track = "internal"

[distribution.app_store]
bundle_id = "com.example.todo"
issuer_id = "00000000-0000-0000-0000-000000000000"
key_id = "ABC123DEFG"
default_track = "testflight"

[distribution.microsoft_store]
product_id = "9N0000000000"
package_identity_name = "ExampleSoftware.Todo"
package_type = "msix"
submit = false
# Optional private flight used when running `--track private`.
flight_id = "insiders"
# Optional staged package rollout percentage for committed submissions.
package_rollout_percentage = 25
# Optional CI mode. When false, Fission expects `msstore` to already be configured.
msstore_reconfigure = false

[release]
default_locales = ["en-US"]
default_theme_modes = ["light", "dark"]
content_output_dir = "release-content"
metadata_root = "release-content/metadata"
active_release = "1.2.3+42"

[[releases]]
id = "1.2.3+42"
version = "1.2.3"
build = 42
status = "candidate"
tracks = ["app-store:testflight", "play-store:internal", "microsoft-store:private"]
locales = ["en-US", "fr-FR"]
metadata = "release-content/metadata/1.2.3+42/release.toml"
release_notes = "release-content/metadata/1.2.3+42/notes"
review = "release-content/metadata/1.2.3+42/review.toml"
privacy = "release-content/metadata/1.2.3+42/privacy.toml"

[release.store_listing.app_store.en-US]
name = "Todo"
subtitle = "Plan the work that matters"
keywords = ["todo", "tasks", "productivity"]
support_url = "https://example.com/support"
marketing_url = "https://example.com/todo"
privacy_url = "https://example.com/privacy"
# Long description, promotional text, release notes, and review notes are read from the active release files.

[release.store_listing.play_store.en-US]
title = "Todo"
short_description = "A focused task manager built with Fission."
# Full description and per-release notes are read from the active release files.

[release.store_listing.microsoft_store.en-US]
title = "Todo"
short_description = "A focused task manager built with Fission."
# Full description and per-release notes are read from the active release files.

[release.screenshots]
raw_dir = "release-content/screenshots/raw"
rendered_dir = "release-content/screenshots/rendered"
status_bar = "clean"
font = "assets/marketing/Inter.ttf"

[[release.screenshots.scenarios]]
id = "home"
name = "Home screen"
targets = ["ios", "android", "web"]
script = "tests/release_screenshots/home.toml"
wait_for = "semantic:todo-list"

[[release.screenshots.scenarios]]
id = "edit-task"
name = "Editing a task"
targets = ["ios", "android", "web"]
script = "tests/release_screenshots/edit_task.toml"
wait_for = "semantic:task-editor"

[release.assets.app_store]
screenshot_sets_dir = "release-content/screenshots/rendered/app-store"
app_previews_dir = "release-content/previews/app-store"
review_attachments = ["release-content/review/demo-account.pdf"]

[release.assets.play_store]
screenshot_sets_dir = "release-content/screenshots/rendered/play-store"
preview_video_dir = "release-content/previews/play-store"
feature_graphic = "assets/store/play-feature-graphic.png"

[release.assets.microsoft_store]
screenshot_sets_dir = "release-content/screenshots/rendered/microsoft-store"
trailers_dir = "release-content/previews/microsoft-store"
logo_dir = "assets/store/microsoft"

[beta.app_store.groups.external_beta]
kind = "external"
public_link = true
feedback_email = "beta@example.com"

[beta.play_store.tracks.closed]
tester_source = "google_group"
group = "todo-closed-testers@example.com"

[beta.microsoft_store.flights.insiders]
description = "Private validation flight before public release"
```

The CLI MUST validate that:

- `app.id` is stable once a store release exists.
- platform bundle/package identifiers match the corresponding store configuration.
- `app.version` is semantic for Fission metadata, but platform-specific build numbers follow platform rules.
- secrets are referenced by key name or provider account, not embedded as plaintext values.
- icon and store asset paths exist before packaging.
- `fission.toml` contains the required release root entries for the selected provider, release, and locales.
- every file referenced by `[[releases]]` exists, is inside an allowed project path, and validates against the release-content schema.
- release-content output roots are inside the project or an explicitly allowed workspace path.
- screenshot scenarios reference runnable Fission test scripts or declarative app states.
- beta group names, track names, and tester sources are valid for the selected provider.
- static-site distribution entries match the generated site's base path, custom-domain mode, and provider publishing source.
- signing asset references point to keychain/certificate-store/vault entries or CI secret names, not plaintext secrets.

## 6. Artifact manifest

Every successful `fission package` command MUST emit an artifact manifest. Distribution commands consume the manifest instead of rediscovering files by path conventions.

Path convention:

```text
target/fission/<profile>/<target>/<format>/artifact-manifest.json
```

Minimum schema:

```json
{
  "schema_version": 1,
  "created_at": "2026-05-19T12:00:00Z",
  "project": {
    "app_id": "com.example.todo",
    "name": "Todo",
    "version": "1.2.3",
    "build": "42"
  },
  "target": "android",
  "format": "aab",
  "profile": "release",
  "artifacts": [
    {
      "kind": "primary",
      "path": "target/fission/release/android/aab/Todo-1.2.3+42.aab",
      "sha256": "...",
      "size_bytes": 12345678,
      "mime_type": "application/octet-stream"
    }
  ],
  "signing": {
    "state": "signed",
    "identity": "android-upload-key:upload",
    "certificate_sha256": "..."
  },
  "notarization": null,
  "validation": {
    "state": "passed",
    "checks": []
  }
}
```

The manifest MUST be immutable once distribution starts. A distribute command that modifies provider state MUST write a separate distribution receipt.

```text
target/fission/<profile>/<target>/<format>/distribution/<provider>/<timestamp>.json
```

Distribution receipts MUST contain provider IDs, URLs, submitted track/channel, uploaded bytes, hashes, release status, and any manual follow-up required.

Release-content commands MUST emit a separate content manifest because screenshots, preview media, rendered store assets, and referenced release files can change without rebuilding the binary. Store metadata values in the manifest are resolved from `fission.toml` plus the files referenced by the active `[[releases]]` entry. The manifest records the exact values and hashes submitted, but does not become an editable input.

```text
release-content/content-manifest.json
```

Minimum content manifest schema:

```json
{
  "schema_version": 1,
  "provider": "app-store",
  "locales": ["en-US", "fr-FR"],
  "screenshots": [
    {
      "locale": "en-US",
      "target": "ios",
      "device_class": "iphone-6_9",
      "scenario": "home",
      "raw_path": "release-content/screenshots/raw/ios/en-US/iphone-6_9/home.png",
      "rendered_path": "release-content/screenshots/rendered/app-store/en-US/iphone-6_9/home.png",
      "sha256": "..."
    }
  ],
  "metadata": [
    {
      "locale": "en-US",
      "source": "fission.toml:[[releases]].metadata + release.store_listing.app_store.en-US",
      "files": [
        {
          "path": "release-content/metadata/1.2.3+42/release.toml",
          "sha256": "..."
        },
        {
          "path": "release-content/metadata/1.2.3+42/notes/en-US.md",
          "sha256": "..."
        }
      ],
      "resolved_sha256": "..."
    }
  ],
  "beta": {
    "groups": ["external-beta"],
    "tester_files": ["release-content/beta/testers.csv"]
  }
}
```

A distribution command that submits store metadata or screenshots MUST record both the artifact manifest and content manifest hashes in the distribution receipt. If the command updates provider metadata from `fission.toml` and referenced release files, it MUST first materialize a content manifest so the exact submitted state is auditable.

## 7. Packaging architecture

The implementation should be structured as normal Rust modules, not ad hoc scripts hidden in templates.

```text
fission_release
  config
  readiness
  artifact_manifest
  package
    linux_run
    macos_app
    macos_pkg
    windows_exe
    windows_msi
    windows_msix
    android_apk
    android_aab
    ios_ipa
    web_static
  sign
    apple
    android
    windows
    generic_artifact
  validate
  release_config
    edit
    import
    diff
    validate
    push
    tui
  release_content
    capture
    render
    screenshots
    previews
    content_manifest
  beta
    groups
    testers
    distribution
  reviews
  signing_assets
  distribute
    s3
    github_releases
    github_pages
    cloudflare_pages
    netlify
    google_drive
    onedrive
    dropbox
    play_store
    app_store
    microsoft_store
  auth
  vault
```

Each packager MUST implement this trait shape, or an equivalent design with the same semantics:

```rust
trait Packager {
    fn id(&self) -> PackageFormat;
    fn readiness(&self, ctx: &ReleaseContext) -> Vec<ReadinessCheck>;
    fn package(&self, ctx: &ReleaseContext) -> anyhow::Result<ArtifactManifest>;
    fn validate(&self, ctx: &ReleaseContext, manifest: &ArtifactManifest) -> Vec<ReadinessCheck>;
}
```

Each distributor MUST implement:

```rust
trait Distributor {
    fn id(&self) -> DistributionProvider;
    fn readiness(&self, ctx: &ReleaseContext, manifest: &ArtifactManifest) -> Vec<ReadinessCheck>;
    fn distribute(&self, ctx: &ReleaseContext, manifest: &ArtifactManifest) -> anyhow::Result<DistributionReceipt>;
}
```

Providers that support ongoing lifecycle management SHOULD also implement an extended static-hosting/provider lifecycle trait:

```rust
trait DistributionLifecycle {
    fn setup(&self, ctx: &ReleaseContext) -> anyhow::Result<ProviderSetupReceipt>;
    fn status(&self, ctx: &ReleaseContext) -> anyhow::Result<ProviderStatus>;
    fn promote(&self, ctx: &ReleaseContext, deployment: DeploymentId) -> anyhow::Result<DistributionReceipt>;
    fn rollback(&self, ctx: &ReleaseContext, deployment: DeploymentId) -> anyhow::Result<DistributionReceipt>;
}
```

Unsupported operations must fail with stable diagnostics, not silent no-ops. For example, a provider that cannot rollback through an API must return `release.provider.rollback_unsupported` with the exact provider and deployment state.

Readiness checks MUST be available without building or packaging. Package validation MUST be available without uploading. Release-config validation MUST validate `fission.toml` and all referenced release files without contacting a store unless the user explicitly requests provider-side diffing. Release-content validation MUST validate rendered assets without contacting a store unless provider-side validation is requested. Beta tester and signing asset operations MUST have dry-run modes that show intended changes before modifying provider state.

## 8. Rust-first dependency policy

The release tooling should prefer Rust for Fission-owned control flow and data handling while using provider tooling for provider-owned artifact semantics:

| Area | Fission/Rust responsibility | Provider/platform tool | Notes |
| --- | --- | --- | --- |
| Desktop bundle generation | `cargo-packager` integration where it fits, plus Fission manifest and asset staging | platform-specific packager | `cargo-packager` supports macOS `.app`, Linux AppImage/deb, Windows NSIS/WiX, but not every Fission-required format such as Linux `.run`, macOS `.pkg`, MSIX, AAB, or IPA [R25]. |
| macOS `.app` basics | bundle metadata, assets, `Info.plist` inputs, and readiness checks | Xcode command-line tools for signing/notarization | `.app` bundle structure and `Info.plist` requirements are Apple platform rules [R8][R9]. |
| macOS `.pkg` | configuration, staging, receipts, and readiness checks | `pkgbuild`, `productbuild`, `productsign` | Apple's package tools create installer component packages and product archives [R13]. |
| Apple signing/notarization | credential discovery, command orchestration, logs, receipts, and diagnostics | `codesign`, `xcrun notarytool`, `stapler`, Transporter | Apple notarization officially uses Xcode command-line tools [R7]. |
| Windows MSI | `cargo-wix` or `cargo-packager` integration plus manifest/readiness handling | WiX Toolset, Windows SDK SignTool | `cargo-wix` builds MSI installers and can sign through SignTool [R27]. |
| Windows MSIX | manifest inputs, visual assets, package identity checks, and receipt handling | MakeAppx/MSIX tooling, Windows SDK SignTool, Store APIs | MSIX is the modern Windows package format and all sideloaded MSIX packages must be signed [R16][R17]. |
| Android APK | manifest/resources/native-library staging, SDK discovery, and readiness checks | Android SDK `aapt2`, `zipalign`, `apksigner` | Android package signing and SDK tooling are platform requirements. |
| Android AAB | bundle inputs, feature/module configuration, validation orchestration, and receipts | `bundletool` and Android SDK tools | `bundletool` is the underlying tool used by Android Studio, Android Gradle Plugin, and Google Play for app bundles [R1]. |
| iOS IPA | app metadata, asset staging, provisioning selection, and readiness checks | Xcode signing/export tools and Transporter | Device/App Store IPAs require Apple signing assets and provisioning. |
| S3-compatible upload | AWS SDK for Rust | none by default | AWS provides Rust SDK S3 examples for upload and multipart flows [R20]. |
| GitHub Releases | release metadata, artifact manifest interpretation, duplicate-asset policy, status, receipts, and `gh` command orchestration | GitHub CLI for release creation, editing, status, and asset upload | GitHub CLI exposes release create/view/edit/upload commands and uses the same authenticated account model developers already use for GitHub Pages workflows [R64][R65]. |
| GitHub Pages | GitHub REST/GraphQL clients, workflow generation, branch publish staging, DNS/readiness checks, and receipts | GitHub Actions for artifact deployment when `mode = "actions"` | GitHub Pages supports branch sources and custom GitHub Actions workflows; Actions deployments use `configure-pages`, `upload-pages-artifact`, and `deploy-pages` with `pages: write` and `id-token: write` permissions [R54][R55]. |
| Cloudflare Pages | Cloudflare API client for projects, deployments, domains, status, credentials, and receipts; Wrangler as the explicit upload backend | Cloudflare-provided tooling for prebuilt upload | Cloudflare Pages supports Direct Upload of prebuilt assets through Wrangler and API-token based Pages API access [R58][R59]. |
| Netlify | Netlify API client for site lookup/create, atomic deploys, domains, status polling, and receipts | Netlify CLI only as a fallback when a new API capability is not yet implemented | Netlify supports API deployments using file digests or ZIP uploads and supports custom domain management through UI/API flows [R61][R62][R63]. |
| Google Drive | `reqwest` + OAuth crates | none by default | Drive supports resumable uploads for large files [R21]. |
| OneDrive | `reqwest` + OAuth crates | none by default | Microsoft Graph supports upload sessions for large files [R23]. |
| Dropbox | `reqwest` + OAuth crates | none by default | Dropbox supports OAuth and upload sessions [R24]. |
| Credential key storage | `keyring` crate | fail with remediation if no OS credential service | `keyring` supports macOS, Windows, Linux, BSDs, and iOS with platform credential stores [R29]. |
| Vault encryption | `age` for export/import, `chacha20poly1305` for local AEAD records | none by default | `age` supports file encryption; `chacha20poly1305` provides XChaCha20Poly1305 AEAD [R30][R31]. |
| Secret handling | `secrecy`, `zeroize` | none by default | These reduce accidental secret exposure and wipe memory on drop [R32][R33]. |
| Screenshot capture | Fission platform test runner and platform device APIs | browser/device/simulator command-line tools where required | Capture must reuse Fission smoke/integration infrastructure instead of inventing a separate UI automation stack. |
| Screenshot rendering | Rust image/vector/text rendering crates owned through Fission abstractions | platform image tools only when required by asset format | Raw captures and rendered marketing assets must be deterministic and reproducible from `fission.toml` config plus referenced release files and asset inputs. |
| Store metadata sync | `reqwest` + provider API clients generated or maintained by Fission | provider CLIs only when upload APIs are incomplete or unstable | Apple, Google, and Microsoft all expose APIs for metadata or asset submission in at least some release paths [R39][R41][R45][R18]. |
| Review/customer feedback | provider APIs through Rust HTTP clients | none by default | Store review listing/reply support belongs in release operations, not in application runtime code [R52][R53]. |

External dependencies MUST be declared by readiness checks with exact detection logic and remediation. A command must not fail halfway through packaging because it discovers a missing tool only after expensive work has already been done.

## 9. Linux `.run` packaging

Fission MUST support Linux `.run` as a first-class package format.

### 9.1 Output

```text
fission package --target linux --format run --release
```

Produces:

```text
target/fission/release/linux/run/<app-name>-<version>-<arch>.run
target/fission/release/linux/run/artifact-manifest.json
```

The `.run` file is a POSIX shell self-extracting installer with a compressed tar payload. This matches the established `.run` pattern used by tools such as Makeself, where a shell stub contains or extracts a compressed tar archive and runs an installer command [R37].

### 9.2 Required payload layout

```text
payload/
  manifest.json
  bin/<app-executable>
  lib/<optional-runtime-libraries>
  share/<app-id>/assets/...
  share/applications/<app-id>.desktop
  share/icons/hicolor/<size>/apps/<app-id>.png
  share/metainfo/<app-id>.metainfo.xml
  install.sh
  uninstall.sh
```

The `.desktop` file MUST follow the Freedesktop Desktop Entry Specification so the app appears in menus, launchers, docks, and task switchers on Linux desktops [R35]. AppStream metadata SHOULD be generated because software centers use it for screenshots, descriptions, icons, and presentation metadata [R36].

### 9.3 Install modes

The installer MUST support:

```text
./Todo.run --install --scope user
./Todo.run --install --scope system
./Todo.run --uninstall
./Todo.run --target-dir <path>
./Todo.run --no-desktop-entry
./Todo.run --verify
./Todo.run --help
```

User install path:

```text
~/.local/share/<app-id>/
~/.local/bin/<install-name>
~/.local/share/applications/<app-id>.desktop
~/.local/share/icons/hicolor/<size>/apps/<app-id>.png
```

System install path:

```text
/opt/<app-id>/
/usr/local/bin/<install-name>
/usr/share/applications/<app-id>.desktop
/usr/share/icons/hicolor/<size>/apps/<app-id>.png
```

The installer MUST write an installation receipt containing the installed paths and payload hash. Uninstall MUST use that receipt rather than guessing paths.

### 9.4 Implementation

Fission SHOULD implement the `.run` writer directly in Rust:

1. create a deterministic payload directory;
2. write manifest, desktop entry, icons, metadata, installer scripts;
3. tar and compress the payload;
4. prepend the Fission-maintained POSIX shell stub;
5. append the payload;
6. write offsets, hashes, and payload length into the stub header;
7. mark the file executable.

Makeself MAY be supported as an optional backend, but it must not be the only implementation because `.run` is a Fission-supported format, not an external side project.

### 9.5 Readiness checks

Linux `.run` readiness MUST check:

- release binary exists or can be built;
- icon source exists and can produce required sizes;
- app id can be used as a desktop file id;
- desktop entry fields are complete: `Name`, `Exec`, `Icon`, `Type`, `Categories`;
- AppStream metadata is complete when enabled;
- `tar`, selected compressor, and shell requirements are available if the implementation shells out;
- target architecture is declared;
- install name does not contain path separators or shell metacharacters.

## 10. macOS packaging

Fission MUST support `.app` bundles and `.pkg` installers.

### 10.1 `.app` bundle

```text
fission package --target macos --format app --release
```

Produces:

```text
target/fission/release/macos/app/Todo.app/
target/fission/release/macos/app/artifact-manifest.json
```

Required bundle layout:

```text
Todo.app/
  Contents/
    Info.plist
    MacOS/Todo
    Resources/AppIcon.icns
    Resources/assets/...
    Frameworks/<optional-frameworks>
```

Apple describes bundles as standardized directory hierarchies containing executable code and resources, with `Info.plist` as the bundle configuration file [R8][R9]. Fission MUST generate the bundle structure from `fission.toml` and platform target configuration, not from hand-written user scripts.

`Info.plist` MUST include at minimum:

- `CFBundleIdentifier` from `package.macos.bundle_id`;
- `CFBundleName`;
- `CFBundleDisplayName`;
- `CFBundleExecutable`;
- `CFBundleVersion`;
- `CFBundleShortVersionString`;
- `CFBundlePackageType = APPL`;
- icon file key;
- minimum system version when configured;
- document types, URL schemes, and entitlements-derived metadata when configured.

### 10.2 macOS direct distribution signing and notarization

For direct distribution outside the Mac App Store, Fission MUST support Developer ID signing, hardened runtime, notarization, and stapling where configured. Apple documents that notarization uses `notarytool` and `stapler`, and notarization requires the hardened runtime [R7]. Apple also documents that `altool` notarization is deprecated and notary workflows should use `notarytool` [R10].

Required direct distribution pipeline:

1. build `.app`;
2. sign nested frameworks, helpers, and binaries in deterministic order;
3. sign the outer `.app` using Developer ID Application;
4. verify codesign state;
5. create the distributable archive when needed;
6. submit to notary service with `notarytool`;
7. poll or wait for result;
8. download and store the notary log on failure;
9. staple the ticket to the app or package;
10. verify stapling and Gatekeeper assessment where available.

Readiness MUST detect:

- macOS host when using Apple command-line tools;
- Xcode or Command Line Tools;
- signing identities;
- team id;
- entitlements file;
- `notarytool` credentials or App Store Connect API key;
- Hardened Runtime setting;
- `stapler` availability;
- whether all nested code paths can be signed.

### 10.3 `.pkg` installer

```text
fission package --target macos --format pkg --release
```

Produces:

```text
target/fission/release/macos/pkg/Todo-1.2.3.pkg
target/fission/release/macos/pkg/artifact-manifest.json
```

Fission MUST produce a product archive `.pkg` suitable for direct installation by orchestrating Apple's package tools. Apple's package tools distinguish component packages from product archives; `pkgbuild` builds component packages and `productbuild` creates deployable product archives [R13].

Required `.pkg` behavior:

- install the `.app` into `/Applications` by default;
- support a configurable install location only when the app is designed for it;
- sign with Developer ID Installer for direct distribution;
- notarize and staple when configured;
- write receipts through the normal Installer path;
- include license, welcome, readme, and conclusion resources when configured.

Readiness MUST detect `pkgbuild`, `productbuild`, `productsign` when used, installer signing identity, component identifier, version, install location, and notarization credentials.

### 10.4 Mac App Store

Mac App Store distribution is a separate distribution provider, not the same as direct `.pkg` distribution. It normally requires sandboxing, App Store provisioning, App Store signing identities, App Store Connect metadata, screenshots, app previews, privacy information, beta testing configuration, and review submission.

Fission MUST guide the first setup because App Store Connect app records are created on the website, not by the Apps API. Apple's Apps API documentation states not to use that API to create new apps; new apps must be created in App Store Connect on the web [R14]. For binary upload, the CLI SHOULD use Transporter with App Store Connect API JWTs or another Apple-supported upload path that satisfies current App Store Connect requirements [R15].

Mac App Store release-content readiness MUST validate screenshot and app preview requirements before review submission. Apple documents one to ten screenshots per supported device size and supports app previews as separate video assets [R38][R39]. App Store Connect API screenshot sets group screenshots by locale and display target, and Apple's asset upload APIs use a reservation/upload/commit/processing workflow for screenshots, previews, review attachments, and routing files [R40][R41]. Fission must model those as release-content objects, not as loose files.

Mac App Store metadata sync MUST use `fission.toml` as the local, version-controlled root of truth, with long-form per-release content stored in referenced files. Required data includes localized name, subtitle, description, keywords, promotional text, release notes, support URL, marketing URL, privacy URL, review contact, review notes, screenshot captions where used, and optional preview-video metadata. Short stable listing fields may live directly in `fission.toml`; long localized descriptions, release notes, and review instructions should live in release files referenced by the active `[[releases]]` entry. The CLI MUST support import, diff, validate, and push operations so a user can import existing store state before Fission becomes authoritative for a listing.

## 11. Windows packaging

Fission MUST support a direct installer and Microsoft Store packaging.

### 11.1 Installer formats

```text
fission package --target windows --format exe --release
fission package --target windows --format msi --release
```

The `.exe` installer SHOULD use a production-proven backend such as NSIS through `cargo-packager` when possible. The `.msi` installer SHOULD use WiX through `cargo-wix` or `cargo-packager` where it fits. `cargo-wix` is a Rust cargo subcommand that builds MSI installers using the WiX Toolset and can sign with Windows SDK SignTool [R27].

Readiness MUST detect:

- target Windows binary;
- icon and visual assets;
- installer upgrade code/product code;
- code signing certificate or configured signing provider;
- WiX Toolset when MSI is selected;
- NSIS when NSIS EXE is selected;
- Windows SDK SignTool when signing uses SignTool.

Direct-distribution signing MUST support local certificates and cloud signing providers. The signing abstraction must not assume certificates live as plaintext PFX files in the project.

### 11.2 MSIX

```text
fission package --target windows --format msix --release
```

MSIX is the required modern Windows package model for Store-like installation and package identity. Microsoft documents MSIX as the modern Windows packaging format and package identity as the key concept for Store distribution and Windows platform features [R16]. MSIX packages contain an app manifest, block map, payload, and signature [R16].

Fission MUST provide the inputs required by Microsoft packaging tools and verify the generated package outputs:

```text
AppxManifest.xml
Assets/<visual-assets>
VFS or app payload tree as required
```

The MSIX package, block map, and local signature are produced through Microsoft-supported packaging and signing tools. Fission records those outputs in the artifact manifest and validates that they match the configured identity, payload, capabilities, and signing mode.

The MSIX manifest MUST include:

- package identity name;
- publisher;
- version;
- processor architecture;
- display name;
- logo assets;
- executable entry point;
- capabilities;
- protocol/file associations when configured.

Microsoft's MSIX signing documentation states that self-signed certificates are appropriate for local testing, production distribution should use a trusted signing method, and Microsoft Store submissions are signed by the Store [R17]. Therefore:

- direct sideloaded MSIX MUST be signed before install;
- Store MSIX SHOULD be packaged with the correct identity and submitted unsigned when the Store signing path requires it;
- readiness MUST distinguish sideload signing requirements from Store signing behavior.

### 11.3 Microsoft Store distribution

```text
fission distribute --provider microsoft-store --artifact <manifest> --track public
```

Fission MUST support Microsoft Store submission automation where Microsoft's platform tooling and APIs allow it. Microsoft documents Store submission APIs for programmatic MSI/EXE package submissions and Partner Center workflows [R18][R19]. Microsoft also provides the Microsoft Store Developer CLI for publishing MSIX packages, package flights, and submission status from local and CI environments [R66][R67]. Fission uses those official paths instead of implementing a custom MSIX uploader.

Microsoft Store release automation MUST handle listing metadata from `fission.toml`, screenshots, logos, trailers, and flight configuration. Microsoft documents screenshots, logos, trailers, and other Store listing image assets for MSIX submissions, including required and optional screenshot counts by device family [R50]. Microsoft also documents package flights through the Store submission APIs [R51]. Fission's Microsoft Store provider must therefore support public submissions and private/test flights through the same artifact/content manifest model.

MSI/EXE submissions use the Store submission API and require a durable HTTPS `package_url` because the Store pulls the installer package from that location. MSIX and MSIXUPLOAD submissions use `msstore publish` against the package file recorded in the artifact manifest. For MSIX, `distribution.microsoft_store.package_type = "msix"` selects this path; `--track public` publishes to the public submission, `--track private` uses `distribution.microsoft_store.flight_id`, and any other non-empty `--track <value>` is treated as the Partner Center package-flight id.

By default, Fission keeps MSIX submissions as drafts by passing the no-commit option to the Store developer CLI. A committed submission requires explicit release intent: set `distribution.microsoft_store.submit = true` or run with `--track public --yes`. If `package_rollout_percentage` is present, Fission passes it to the Store developer CLI during MSIX publishing. `msstore_project` can point at the project directory that `msstore publish` should use; if it is omitted, Fission passes the Fission project directory and relies on the explicit `--inputFile` artifact. If `msstore_reconfigure = true`, Fission configures the Store developer CLI from `tenant_id`, `client_id`, `seller_id`, and the Partner Center client secret before publishing; otherwise it assumes the developer or CI runner has already configured the tool.

Readiness MUST check:

- Partner Center authentication;
- product id or reserved app name;
- package identity name matches the Store product identity;
- Microsoft Store Developer CLI availability for MSIX/MSIXUPLOAD artifacts;
- `package_url` only for MSI/EXE submissions;
- required visual assets exist;
- `fission.toml` release root entries and referenced release metadata files exist for each configured language;
- screenshot/trailer assets match Microsoft Store requirements for the selected app type;
- flight names and package-flight targets exist when distributing to a private flight;
- age rating/category metadata exists;
- privacy URL exists;
- package validation passes;
- submission target is valid;
- whether the first submission requires manual Partner Center work.

## 12. Android packaging and distribution

Fission MUST support APK for sideload/testing and AAB for Play distribution.

### 12.1 APK

```text
fission package --target android --format apk --release
```

APK output is required for local installation, emulator/device smoke tests, and direct distribution where allowed. Every installable Android APK must be signed. Readiness MUST detect Android SDK, build-tools, NDK, Rust Android target, `aapt2`, `zipalign`, `apksigner`, package name, version code, version name, manifest, icon assets, and signing configuration.

### 12.2 AAB

```text
fission package --target android --format aab --release
```

Android App Bundle is the primary Google Play publishing format. Android documents AAB as a publishing format that contains compiled code and resources while deferring APK generation and signing to Google Play [R3]. `bundletool` is the underlying tool Android Studio, Android Gradle Plugin, and Google Play use to build and work with app bundles [R1].

Fission MUST construct the Android bundle inputs from the Fission project model, then invoke Android-supported build and validation tooling for the final AAB. Fission remains authoritative for package identity, versions, assets, native library selection, and release metadata, while `bundletool` and Android SDK tools remain authoritative for app-bundle packaging and validation.

AAB readiness MUST check:

- app package name;
- monotonic `version_code`;
- `version_name`;
- min SDK and target SDK;
- target API level policy warning;
- upload signing key;
- Play App Signing state when distributing to Play;
- native libraries for all configured ABIs;
- asset packs if used;
- `bundletool` available for validation/deployment.

Google Play requires Play App Signing for modern Play-managed release flows, and new apps use Android App Bundles for publishing [R4][R5]. Fission MUST present this clearly in readiness output.

### 12.3 Google Play distribution

```text
fission distribute --provider play-store --artifact <manifest> --track internal
fission distribute --provider play-store --artifact <manifest> --track closed
fission distribute --provider play-store --artifact <manifest> --track open
fission distribute --provider play-store --artifact <manifest> --track production --rollout 0.05
```

The Play Developer Publishing API supports transactional edits for grouping changes and committing them together [R2]. The Publishing API supports uploading app bundles and assigning releases to tracks [R2][R6]. Google also documents API support for localized store listings, screenshots, and promotional graphics through listing/image edit resources [R45][R48][R49].

Required automated flow after first setup:

1. authenticate using a service account or OAuth account authorized for the Play Console app;
2. create an edit;
3. upload AAB;
4. upload native debug symbols/mapping files when available;
5. push localized store listing metadata and image assets when requested;
6. set release notes per locale;
7. assign the uploaded version code to the selected track;
8. configure rollout fraction if staged rollout is requested;
9. validate the edit;
10. commit the edit;
11. write distribution receipt with edit id, track, version code, rollout fraction, uploaded listing/image changes, and console URL.

First setup MUST be guided because the developer must create a Play developer account, create the app in Play Console, configure app signing, complete policy/store listing requirements, and prepare the first release. Google's Play Console help documents creating and setting up apps in Play Console [R5], and the Edits API documentation states that publishing certain app changes still requires Play Console [R2].

Guided first setup output MUST include:

- create Play developer account if missing;
- create app in Play Console;
- set package name exactly to `package.android.package_name`;
- choose app signing key strategy;
- configure upload key;
- complete store listing, privacy policy, content rating, data safety, target audience, ads declaration, and app access fields;
- add preview assets such as screenshots, feature graphics, and preview video where required by Google Play [R44];
- create internal, closed, or open test tracks and tester lists where needed; Google documents internal, closed, and open testing setup in Play Console [R46];
- create the first internal testing release if required;
- grant API access/service account permissions;
- rerun `fission readiness distribute --provider play-store`.

Fission MUST also support a Play internal-sharing distribution mode for quick team validation outside the formal track release flow. Google Play internal app sharing supports uploading an APK or app bundle and generating a share link, and the Publishing API exposes upload endpoints for internal sharing artifacts [R47][R48]. This mode must write a distribution receipt with the generated link and must not be confused with a production/internal-track release.

## 13. iOS packaging and distribution

Fission MUST support simulator builds for development and IPA builds for device/TestFlight/App Store distribution.

### 13.1 IPA

```text
fission package --target ios --format ipa --release
```

The IPA packager MUST stage:

```text
Payload/Todo.app/
  Info.plist
  Todo
  Assets.car or generated asset resources
  embedded.mobileprovision
  Frameworks/<optional-frameworks>
```

Then it MUST invoke Apple signing/export tooling with the selected certificate, provisioning profile, entitlements, bundle id, team id, build number, and marketing version.

Readiness MUST check:

- host OS is macOS for official Apple toolchain paths;
- Xcode and command-line tools;
- Rust iOS targets;
- bundle id;
- team id;
- signing certificate;
- provisioning profile;
- entitlements;
- device family;
- app icon set;
- launch screen/storyboard equivalent if required by current platform rules;
- privacy manifest and usage description keys where app capabilities require them;
- App Store Connect API key or Transporter credentials for distribution.

### 13.2 TestFlight and App Store distribution

```text
fission distribute --provider app-store --artifact <manifest> --track testflight
fission distribute --provider app-store --artifact <manifest> --track app-store-review
```

Apple's App Store Connect help documents uploading builds with Xcode, altool, Transporter, or build upload mechanisms, and notes that API JWTs can be used with Transporter [R15]. Apple's Apps API documentation states that the API manages existing apps and must not be used to create new apps [R14]. TestFlight supports internal and external tester groups, public links, and large external tester programs managed through App Store Connect and its API [R42][R43].

Required automated flow after first setup:

1. authenticate with App Store Connect API key or configured Transporter credentials;
2. validate IPA metadata before upload;
3. upload binary using Transporter or an Apple-supported upload path;
4. poll App Store Connect for build processing;
5. assign build to TestFlight group or app version when requested;
6. sync configured beta groups, tester CSV files, and public-link settings before distribution when requested;
7. submit for beta review or App Review when configured;
8. upload or update screenshots, app previews, review attachments, and localized metadata resolved from `fission.toml` plus referenced release files when the content manifest requests it;
9. write distribution receipt with build id, processing status, app version id, beta group ids, public-link state, content upload ids, and App Store Connect URL.

Guided first setup output MUST include:

- enroll in Apple Developer Program;
- create bundle identifier/capabilities;
- create certificates;
- create provisioning profiles;
- create App Store Connect app record manually;
- configure required agreements, tax, and banking if needed;
- add app metadata, screenshots, app previews, privacy details, age rating, and pricing;
- create beta groups and decide whether public links are allowed;
- create App Store Connect API key with required role;
- rerun readiness.

Fission MUST not pretend the first App Store release can be fully automated when Apple requires web portal setup or review actions.

## 14. Web packaging and distribution

```text
fission package --target web --format static --release
```

Produces:

```text
target/fission/release/web/static/
  index.html
  assets/...
  pkg/...
  manifest.webmanifest optional
  service-worker.js optional
  artifact-manifest.json
```

Readiness MUST check:

- Rust WASM target;
- generated JS/WASM bindings;
- asset hashes;
- MIME type mapping for `.wasm`;
- base path configuration;
- cache policy;
- service worker/PWA settings when enabled;
- icon sizes and web manifest fields.

Static-site packages MUST include a route manifest, asset manifest, MIME map, cache policy, redirect/header files where the selected provider supports them, and the resolved canonical URL/base path used during rendering. The renderer must know whether the site will live at `/`, at a repository subpath such as `/todo/`, or behind a custom domain. Links, canonical URLs, Open Graph images, service-worker scope, and web manifest URLs must be generated against that resolved public base.

Distribution providers for web include dedicated static hosting providers, S3-compatible object stores, Google Drive, OneDrive, and Dropbox. Dedicated static hosting providers are preferred when the output is a public website because they support production URLs, preview deployments, custom domains, HTTPS, cache behavior, deployment status, and rollback/status APIs. Object/file providers are still valid when the desired output is a downloadable archive, private review link, or internal distribution folder.

Static web distribution MUST preserve content type metadata. A `.wasm` object uploaded as `application/octet-stream` when the hosting provider requires `application/wasm` is a release bug. Distribution receipts MUST record the deployed canonical URL, provider preview URL where available, provider deployment ID, custom-domain state, HTTPS state, and any DNS or provider-side manual work still required.

## 15. Static hosting and cloud/file distribution providers

Static hosting and cloud/file providers use `fission distribute`. They upload artifacts and return durable links, provider IDs, deployment status, and receipts. Public website providers also participate in post-build lifecycle management: domain readiness, HTTPS readiness, preview/production promotion, rollback metadata, deployment status, and link reporting. GitHub Releases is deliberately separate from GitHub Pages: it is an artifact distribution channel for binaries, installers, bundles, archives, debug symbols, checksums, and optionally a zipped static-site package.

Static hosting providers MUST implement the lifecycle operations they can support:

- `setup`: create or validate the provider-side site/project, configure the selected publishing source, and attach custom domains where provider APIs allow it;
- `distribute`: upload or deploy the artifact manifest output;
- `status`: fetch the latest deployment, domain, HTTPS, and provider processing state;
- `promote`: move a preview/draft deployment to production where the provider supports it;
- `rollback`: restore a previous deployment where the provider supports it, or emit a precise unsupported-operation diagnostic;
- `observe`: record provider events, build/deploy logs, URLs, and pending manual actions into receipts.

### 15.1 GitHub Releases

```text
fission distribute --provider github-releases --artifact <manifest> --site production --deploy v1.2.3
```

GitHub Releases distribution MUST work for any package artifact emitted by `fission package`. It is not a static-site provider and must not require `index.html`, route manifests, web base paths, or custom-domain configuration. It can distribute Linux `.run` installers, macOS `.app` archives or `.pkg` installers, Windows `.exe`, `.msi`, and `.msix` installers, Android `.apk` and `.aab` files, iOS `.ipa` files, zipped static-site output, debug symbols, crash diagnostics, checksums, SBOMs, and any other files listed in the artifact manifest.

Configuration:

```toml
[distribution.github_releases.production]
owner = "example"
repo = "todo"
tag = "v1.2.3"
name = "Todo 1.2.3"
notes_file = "release-content/metadata/1.2.3+42/notes/en-US.md"
draft = true
prerelease = false
replace_assets = true
upload_artifact_manifest = true
```

Publishing behavior:

- resolve owner/repository from `distribution.github_releases.<site>` or the GitHub origin remote;
- resolve the release tag from `--deploy`, `distribution.github_releases.<site>.tag`, or the package version as `v<version>`;
- create the release if the tag has no release yet, otherwise update the existing release metadata;
- upload every file listed in the artifact manifest, not just static-site files or a hard-coded extension list;
- optionally upload `artifact-manifest.json` itself so consumers and CI can verify hashes, sizes, MIME types, target, format, profile, and validation state;
- fail if a same-named asset already exists unless `replace_assets = true`, in which case the existing asset is deleted and re-uploaded;
- preserve draft/prerelease/latest behavior through explicit config rather than inferring release status from filenames;
- write a distribution receipt containing the GitHub release ID, release URL, uploaded asset JSON, and any manual follow-up.

Fission MUST use the GitHub CLI for GitHub Releases rather than implementing direct REST calls in the first-party provider. This keeps GitHub authentication consistent with GitHub Pages/local repository workflows: a developer who has already run `gh auth login` can publish releases without separately teaching Fission about GitHub credentials. Internally Fission still owns package-manifest validation, tag resolution, release metadata resolution, duplicate-asset policy, receipts, and dry-run output. The provider backend invokes `gh release view`, `gh release create`, `gh release edit`, and `gh release upload --clobber` as needed [R64][R65].

Readiness MUST check:

- owner and repository are configured or inferable;
- `gh` is installed and authenticated, or `GH_TOKEN`/`GITHUB_TOKEN`/Fission vault credentials are available for the GitHub CLI process;
- a tag can be resolved before publishing;
- the artifact manifest exists and contains at least one existing uploadable file;
- duplicate-asset policy is explicit when republishing is expected;
- release notes file exists when configured.

Authentication:

- GitHub Releases primarily uses `gh auth login`.
- CI can use `GH_TOKEN` or `GITHUB_TOKEN`, and the Fission vault may inject `GH_TOKEN` for the `gh` subprocess when no token is already present.
- Publishing requires repository Contents write permission because release and release-asset operations mutate repository release state [R64][R65].
- Public status checks may work unauthenticated, but Fission should still prefer the user's `gh` authentication so private repositories and draft releases behave consistently.

### 15.2 GitHub Pages

```text
fission distribute --provider github-pages --artifact <manifest> --site production
```

GitHub Pages distribution MUST support publishing both without a custom domain and with a custom domain.

Supported modes:

- `mode = "actions"`: Fission generates or validates a GitHub Actions workflow that builds the static site package, uploads the Pages artifact, and deploys through GitHub's Pages Actions path.
- `mode = "branch"`: Fission publishes the packaged static output to a configured branch and folder, normally `gh-pages:/`, for repositories that want branch-source publishing.
- `mode = "manual"`: Fission emits setup instructions and readiness checks only, useful when an organization owns deployment workflows outside Fission.

`mode = "actions"` SHOULD be the default for repositories hosted on GitHub. GitHub documents custom workflows for Pages using `configure-pages`, `upload-pages-artifact`, and `deploy-pages`; the deploy job requires `pages: write` and `id-token: write` permissions [R55]. This mode avoids committing generated static files into the source repository and works with Fission's static site builder. Readiness MUST check that the repository Pages source is set to GitHub Actions, or emit a remediation explaining how to set it. The generated workflow must run `fission site build --release`, upload the produced static directory as the Pages artifact, and deploy it only from the configured production branch.

`mode = "branch"` is still required because some projects and organizations prefer branch-source publishing. GitHub documents publishing from a selected branch and folder [R54]. Fission MUST generate `.nojekyll` in the published output unless the user explicitly disables it. If the site uses a custom domain in branch mode, Fission MUST write a root `CNAME` file containing exactly that domain because branch-source GitHub Pages uses that file as part of the custom-domain workflow. If the branch is updated from GitHub Actions, Fission MUST warn that commits made with the default `GITHUB_TOKEN` do not trigger a Pages build in the branch-source path [R54].

Custom-domain behavior:

- without `custom_domain`, a project site defaults to `https://<owner>.github.io/<repo>/`, so `base_path` normally must be `/<repo>/`;
- without `custom_domain`, a user or organization site defaults to `https://<owner>.github.io/`, so `base_path` normally must be `/`;
- with `custom_domain`, `base_path` normally must be `/` unless the user intentionally serves below a path;
- for Actions-based Pages publishing, GitHub states that a `CNAME` file is not required and existing `CNAME` files are ignored; Fission MUST configure or validate the custom domain through repository settings/API instead [R56];
- for branch-source publishing, Fission MUST keep the `CNAME` file in the publishing root and verify that the repository Pages settings agree with it;
- readiness MUST run GitHub's Pages DNS health check API where credentials permit it and must report required DNS records when it cannot fix them automatically [R57].

Authentication:

- CI Actions mode SHOULD use the built-in workflow token with explicit `pages: write` and `id-token: write` permissions.
- Local mode SHOULD use `gh` authentication when available or a GitHub App installation token/fine-grained personal access token stored in the Fission vault.
- Secrets MUST not be written to the workflow. Generated workflows should rely on GitHub's token where possible.

Required behavior:

- verify repository ownership and Pages availability;
- verify the selected source mode;
- verify no generated site secrets are included in the artifact;
- verify base path and custom-domain consistency;
- upload/deploy the exact artifact manifest output;
- poll deployment/build status;
- record page URL, custom-domain URL, deployment ID, source branch/workflow run, and DNS/HTTPS status in the receipt.

### 15.3 Cloudflare Pages

```text
fission distribute --provider cloudflare-pages --artifact <manifest> --site production
```

Cloudflare Pages is a good first dedicated static-hosting provider because it supports Direct Upload for prebuilt assets through the provider CLI and API-token based management. Cloudflare documents Direct Upload for prebuilt assets and CI use, including `wrangler pages deploy <DIRECTORY> --project-name=<PROJECT_NAME>` with `CLOUDFLARE_ACCOUNT_ID` and an API token [R58]. Cloudflare's Pages REST API uses bearer API tokens and documents Pages Read/Write permissions for project/deployment access [R59].

Implementation approach:

- Fission SHOULD use the Cloudflare API directly for account, project, deployment, custom-domain, DNS, status, and receipt operations.
- Fission MUST invoke Wrangler as the Cloudflare Pages upload backend. This is not a fallback path; it is the supported provider-owned upload tool for prebuilt Pages assets.
- Fission MUST store Cloudflare API tokens only in the vault or CI secrets.

Custom-domain behavior:

- without `custom_domain`, the canonical site URL is the Pages-provided `https://<project>.pages.dev` URL;
- with a subdomain custom domain, readiness MUST require a CNAME pointing the desired host to `<project>.pages.dev` unless the zone is managed by the same Cloudflare account and can be changed automatically [R60];
- with an apex custom domain, readiness MUST require that the apex domain is a Cloudflare zone in the same account before automation can complete [R60];
- when the zone is managed by the same account, Fission MAY create or update the required DNS records after explicit confirmation or non-interactive `--yes`;
- readiness MUST check CAA records where Cloudflare reports certificate issuance problems, because CAA can block certificate issuance [R60].

Required behavior:

- verify account ID, project name, token scopes, and project existence;
- verify provider upload limits against the artifact manifest before starting an upload;
- create the project only when explicitly requested by `fission distribute setup` or equivalent setup command;
- deploy the static output to production or preview environment;
- set or validate custom domains;
- poll deployment status;
- report preview URL, production URL, custom-domain status, deployment ID, and any DNS/certificate remediation.

### 15.3 Netlify

```text
fission distribute --provider netlify --artifact <manifest> --site production
```

Netlify is a good first API-driven static-hosting provider because it exposes site/deploy APIs, supports direct manual deploys without Git integration, supports draft deploys, and uses bearer access tokens. Netlify documents API deployment using either file digests plus uploads or ZIP uploads, and documents polling deploy state until it becomes ready [R61]. Netlify also documents manual deploys and production deploys from the CLI, while the API remains the right integration point for Fission automation [R62].

Implementation approach:

- Fission SHOULD use the Netlify API directly with `reqwest` and the vault-managed access token.
- Fission SHOULD prefer the file-digest deploy API for large repeated deploys because it avoids uploading files Netlify already has.
- Fission MAY use ZIP upload for the first implementation or for small sites where the simpler path is acceptable.
- Fission MUST poll deploy state and fail with provider diagnostics if processing fails.

Custom-domain behavior:

- without `custom_domain`, the canonical URL is the Netlify site URL returned by the provider;
- with `custom_domain`, readiness MUST verify the domain is attached to the site or emit exact setup steps;
- if Netlify DNS manages the domain, Fission MAY manage DNS records through Netlify APIs;
- if DNS is external, Fission MUST report the required records and verify them where provider APIs expose enough information;
- readiness MUST distinguish "domain attached", "DNS configured", "certificate ready", and "production deploy live".

Required behavior:

- verify token, team/site access, and site ID or site slug;
- optionally create a site during setup;
- deploy as draft or production according to config;
- support deploy previews for pull-request/review flows;
- poll deploy state;
- return deploy ID, deploy URL, production URL, SSL URL, custom-domain status, and provider state in the receipt.

### 15.4 S3-compatible object stores

```text
fission distribute --provider s3 --artifact <manifest> --profile production
```

Implementation MUST use `aws-sdk-s3` by default. AWS's Rust SDK examples cover S3 upload, object actions, and multipart upload flows [R20]. Fission MUST support AWS S3 and S3-compatible endpoints by accepting endpoint URL, region, bucket, prefix, path-style mode, access key source, and session token source.

Required behavior:

- upload all manifest artifacts;
- set content type;
- set cache control when configured;
- use multipart upload above a configurable threshold;
- verify uploaded size and checksum when provider exposes it;
- support public URL, private URL, and presigned URL modes;
- write distribution receipt with object keys, etags, version ids where available, and URLs.

Readiness MUST check credentials, bucket existence/access, prefix writability, public/presigned mode, object overwrite policy, and clock skew if presigned URLs are used.

### 15.5 Google Drive

Google Drive distribution MUST use OAuth and Drive resumable upload for large artifacts. Google documents resumable upload as the appropriate Drive upload type for files larger than 5 MB or unstable network conditions [R21]. Sharing links require permissions to be created or updated through the Drive permissions API [R22].

Required behavior:

- authenticate via browser loopback OAuth where possible;
- fall back to device authorization flow for headless terminals when supported by the provider;
- upload artifacts using resumable upload;
- place files in configured folder;
- create or update sharing permissions if `visibility = "link"`;
- return file IDs and web links.

### 15.6 OneDrive

OneDrive distribution MUST use Microsoft Graph. Microsoft Graph supports `createUploadSession` for large file uploads [R23]. Microsoft identity platform supports OAuth device code flow, which is useful for CLI and headless sign-in [R34].

Required behavior:

- authenticate through Microsoft identity platform;
- select personal OneDrive or business drive/site when configured;
- upload using upload sessions above the small-file threshold;
- create sharing links when requested;
- return drive item IDs and web URLs.

### 15.7 Dropbox

Dropbox distribution MUST use Dropbox OAuth and upload sessions for large artifacts. Dropbox documents OAuth for API authorization and upload session endpoints for chunked uploads [R24].

Required behavior:

- authenticate with OAuth authorization code plus PKCE where available;
- store refresh token in the Fission vault;
- upload large artifacts with upload sessions;
- create or reuse shared links when requested;
- return file IDs and shared links.

## 16. Authentication and credential storage

Fission needs an authorization story that works for interactive developer machines and CI without storing secrets in plaintext.

### 16.1 Principles

- Fission MUST never write OAuth refresh tokens, service account keys, signing passwords, API private keys, or cloud access keys to plaintext config files.
- Fission MUST redact secrets from logs, JSON diagnostics, panic reports, and receipts.
- Fission MUST support revocation and logout.
- Fission MUST support CI without an interactive keyring by reading secrets from environment variables, CI secret files, OIDC-based provider auth, or external secret managers.
- Fission MUST distinguish account identity from credential material. It is acceptable for config to say `account = "work-google"`; it is not acceptable for config to contain the refresh token.

### 16.2 Local credential vault

Local developer credentials MUST be protected by a two-layer model:

1. OS keyring stores a randomly generated vault master key or unwrap key.
2. Encrypted vault file stores provider credentials and metadata.

The `keyring` crate is the preferred OS credential abstraction because it supports platform credential stores including macOS, Windows, Linux, BSDs, and iOS [R29]. If no platform credential service is available, readiness MUST fail with remediation instead of falling back to plaintext.

Vault file path should follow platform config directories. Example logical path:

```text
<platform-config-dir>/fission/credentials.vault
```

On Unix-like platforms the file MUST be created with owner-only permissions. On Windows it MUST be created in the user's profile config area and rely on the OS ACL plus encryption.

Vault record encryption SHOULD use XChaCha20Poly1305 from `chacha20poly1305` for local AEAD record encryption [R31]. `age` SHOULD be used for explicit encrypted import/export and team handoff bundles because it is a file encryption format with Rust support [R30].

Every encrypted record MUST include authenticated associated data:

```text
vault_schema_version
record_id
provider
account_id
created_at
last_rotated_at
```

The ciphertext MUST contain:

```json
{
  "kind": "oauth_refresh_token | service_account_json | api_private_key | signing_password | access_key_pair",
  "provider": "github-pages | cloudflare-pages | netlify | google-drive | onedrive | dropbox | play-store | app-store | microsoft-store | s3",
  "account_label": "work-google",
  "scopes": ["..."],
  "expires_at": null,
  "secret": "...",
  "metadata": {}
}
```

Secret values in memory SHOULD use `secrecy` wrappers, which make secret access explicit and prevent accidental debug leakage, and `zeroize` for memory cleanup on drop [R32][R33].

### 16.3 Auth flows

Interactive desktop CLI:

- prefer authorization-code-with-PKCE loopback browser flow for OAuth providers that support native apps;
- open the browser automatically unless `--no-open-browser` is passed;
- show the URL and code manually when browser launch fails;
- store refresh token in the vault.

Headless terminal:

- use OAuth Device Authorization Grant when the provider supports it;
- show verification URL and user code;
- poll at the provider-specified interval;
- store refresh token in the vault.

RFC 8628 defines the OAuth Device Authorization Grant for clients that lack a browser or are input constrained [R34]. Fission SHOULD use that model for SSH sessions, CI setup terminals, and remote development machines when supported by the provider.

CI:

- support provider-native environment variables;
- support encrypted vault import only when a passphrase or age identity is supplied by the CI secret system;
- support OIDC federation where the provider supports it;
- never prompt;
- fail with JSON diagnostics if credentials are missing.

### 16.4 Signing credentials

Fission SHOULD avoid importing long-lived private signing keys into its own vault when the platform has a better native key store:

- Apple signing certificates should live in the macOS Keychain or an explicitly imported CI keychain.
- Windows code signing should use Windows certificate store, Azure Artifact Signing, HSM-backed signing, or a CI secret store.
- Android upload keystores may live as files, but passwords must be in vault/CI secrets.
- App Store Connect API private keys may be stored in vault for local development or CI secrets for automation.
- Google Play service account JSON may be stored in vault for local development or CI secrets for automation.

The vault can store references, labels, passwords, OAuth tokens, and API private keys. It should not become an unmanaged dumping ground for every signing private key.

## 17. Readiness checks

Readiness is the main product surface. It must be precise, actionable, and available before users waste time.

### 17.1 Common package readiness

All platforms MUST check:

- project has `fission.toml`;
- selected target is configured;
- release profile can be built;
- app id, app name, version, and build number exist;
- icon source exists and can generate platform sizes;
- license, copyright, homepage, support URL, and privacy URL are present when required by target/provider;
- artifact output directory is writable;
- no stale manifest conflicts with current build unless `--force` is passed;
- app has required platform shell files;
- package format is supported on the current host or remote builder.

### 17.2 Common distribution readiness

All providers MUST check:

- artifact manifest exists and hashes match files;
- artifact format is accepted by provider;
- provider account is authenticated;
- account has required scopes/roles;
- destination exists or can be created;
- overwrite/release policy is explicit;
- version/build is newer than existing release when provider requires monotonic versions;
- release notes exist if required;
- `fission.toml` contains the release root entries required to resolve listing metadata, preview media, and review notes;
- referenced release metadata, notes, review, and privacy files exist when required;
- release-content manifest exists when the provider requires screenshots, rendered assets, preview media, or review attachments;
- local release-config lock is not stale relative to provider state unless explicit overwrite is requested;
- beta groups/tracks/flights exist and tester sources are valid when distributing to a beta destination;
- dry-run is supported or provider limitation is explicitly reported;
- receipts path is writable.

### 17.3 Manual-step readiness

When automation cannot complete setup, readiness MUST not simply say "not configured". It MUST emit a checklist with provider portal links and exact values copied from project config.

Example:

```text
Blocked: Google Play app record does not exist or API cannot access it.

Do this once in Play Console:
1. Create app.
2. Set package name to com.example.todo.
3. Configure Play App Signing.
4. Complete Store listing, Data safety, Content rating, Target audience, Privacy policy.
5. Link Google Cloud project and grant the service account Release Manager access.
6. Run: fission readiness distribute --provider play-store --track internal
```

Manual setup steps MUST be included in JSON diagnostics as structured remediation items.

## 18. Store release flows

### 18.1 Tracks and channels

Fission should normalize provider release destinations into tracks/channels while preserving provider-specific names.

| Provider | Fission destination | Provider term |
| --- | --- | --- |
| Google Play | `internal`, `closed`, `open`, `production` | track |
| App Store Connect | `testflight`, `app-store-review`, `app-store-release` | TestFlight group, App Review, phased/manual release |
| Microsoft Store | `private`, `public`, provider-supported flights | submission/channel/flight |
| S3/Drive/OneDrive/Dropbox | `folder`, `prefix`, `release-channel` | object prefix or folder |

### 18.2 Rollout control

Fission MUST support staged rollout where the store supports it. For Google Play, the Publishing API supports limited/staged production rollout through track release configuration [R2]. App Store release timing/phased release is managed through App Store Connect release controls [R15]. Microsoft Store release controls depend on Partner Center capabilities for the selected app type.

### 18.3 Release notes and metadata

Store distribution MUST support locale-specific release notes and complete store metadata, not just binary upload. `fission.toml` is the authoritative root, but it should not contain every long-form localized paragraph for every release. The release-content directory stores referenced release files, generated captures, rendered assets, app preview videos, review attachments, receipts, and content manifests.

Representative root layout:

```toml
[release]
active_release = "1.2.3+42"
metadata_root = "release-content/metadata"
default_locales = ["en-US", "fr-FR"]

[[releases]]
id = "1.2.3+42"
version = "1.2.3"
build = 42
status = "candidate"
tracks = ["app-store:testflight", "play-store:internal"]
locales = ["en-US", "fr-FR"]
metadata = "release-content/metadata/1.2.3+42/release.toml"
release_notes = "release-content/metadata/1.2.3+42/notes"
review = "release-content/metadata/1.2.3+42/review.toml"
privacy = "release-content/metadata/1.2.3+42/privacy.toml"

[release.store_listing.app_store.en-US]
name = "Todo"
subtitle = "Plan the work that matters"
keywords = ["todo", "tasks", "productivity"]
support_url = "https://example.com/support"
marketing_url = "https://example.com/todo"
privacy_url = "https://example.com/privacy"
```

Representative referenced release file:

```toml
# release-content/metadata/1.2.3+42/release.toml
[app_store.en-US]
description = """
Todo helps you capture, plan, and finish work across every Fission platform.
"""
promotional_text = "A faster task list with improved editing."

[play_store.en-US]
full_description = """
Todo helps you capture, plan, and finish work across desktop, web, Android, and iOS.
"""

[microsoft_store.en-US]
description = """
Todo helps you capture, plan, and finish work across Windows devices.
"""
```

Representative release notes layout:

```text
release-content/metadata/1.2.3+42/notes/
  en-US.md
  fr-FR.md
```

The distributor MUST validate maximum lengths, supported locales, prohibited empty fields, required URLs, and provider-specific file requirements before upload when provider limits are known. Release-config sync MUST support:

- `import` to populate `fission.toml` and referenced release files from current provider state;
- `diff` to show local-vs-remote changes without mutation;
- `validate` to run local rules and optional provider-side validation;
- `push` to upload the resolved metadata from `fission.toml` and referenced release files;
- `lock` to record the provider revision or timestamp the local root and files were based on.

A push MUST fail by default if the remote provider state has changed since the last import or lock, unless the user passes an explicit overwrite flag. A push MUST also fail if a referenced release file has changed since the content manifest was materialized.


### 18.3.1 Interactive and non-interactive release editing

Fission MUST provide both TUI and non-interactive ways to edit the release root and referenced release files. The TUI is a guided editor for humans; non-interactive commands are the stable automation interface for scripts and CI. Both paths update the same schema and must keep `fission.toml` as the root manifest.

Required TUI behavior:

- `fission release-config edit --tui` opens a provider-aware guided editor.
- The TUI shows missing required fields, provider limits, current local values, and remote values when authenticated.
- The TUI can import remote metadata into `fission.toml` and referenced release files, but it must preview changes before writing.
- The TUI can select screenshot scenarios and asset paths, but it must not hide generated files outside the project.
- The TUI writes normal TOML/Markdown files and exits with a summary of changed files and fields.

Required non-interactive behavior:

```text
fission release-config set app.version 1.2.4
fission release-config set 'release.active_release' '1.2.4+43'
fission release-config add-release --version 1.2.4 --build 43 --from 1.2.3+42
fission release-config set 'release.store_listing.play_store."en-US".short_description' "A focused task manager."
fission release-config edit-file --release 1.2.4+43 --kind notes --locale en-US
fission release-config import --provider app-store --locales en-US,fr-FR --yes
fission release-config validate --provider play-store --json
fission release-config push --provider microsoft-store --locales en-US --dry-run --json
```

Non-interactive commands MUST support stable field paths, structured JSON output, `--dry-run`, `--yes`, and clear exit codes. Field paths use TOML dotted-key semantics; segments that contain characters such as `-` must be quoted. Commands that edit referenced files MUST address them by release id, kind, provider, and locale instead of making scripts know the physical path. Commands MUST fail instead of prompting when `--non-interactive` or CI mode is active.

### 18.4 Store assets

Fission MUST not block packaging on store screenshots, but distribution readiness MUST block store submission if required store assets or metadata are missing.

Required asset categories:

- app icon;
- screenshots per platform/device class;
- app preview videos, trailers, or preview video links where supported;
- feature graphic or promotional assets where required;
- localized captions and marketing text rendered into screenshots where configured;
- privacy policy URL;
- support URL;
- category;
- age/content rating inputs;
- data safety/privacy declarations;
- accessibility declaration where provider requires it.

### 18.5 Screenshot and preview capture

Fission MUST provide a release screenshot pipeline that uses the same platform run/test infrastructure as the app, then captures deterministic screenshots from declared scenarios. It must not require users to manually click through every locale and device size.

Required capture flow:

1. read screenshot scenarios, captions, and active release metadata from `fission.toml` and referenced release files;
2. build or reuse the requested app artifact;
3. launch the app on the requested browser, simulator, emulator, or device;
4. apply locale, theme, text scale, orientation, viewport, and seeded test data;
5. drive the app to the declared semantic state;
6. wait for stable rendering;
7. capture raw screenshot or video;
8. normalize status/navigation bars where the provider permits it;
9. write raw captures, logs, and a capture receipt;
10. render provider-ready assets from raw captures and local captions.

The render step MUST support provider-specific output sets without recapturing the app. For example, a single raw tablet screenshot may produce App Store, Play Store, documentation, and website assets if each output satisfies the target provider's size and content rules. Apple documents screenshot requirements and app preview upload support in App Store Connect [R38][R39]. Google Play documents preview assets for app store listings [R44]. Microsoft documents screenshots, logos, trailers, and other Store listing assets [R50].

Screenshot scenarios SHOULD be based on semantic selectors, not pixel coordinates. A scenario that says `wait_for = "semantic:checkout-button"` is stable across devices; a scenario that taps `(481, 914)` is not.

### 18.6 Beta testers and private distribution

Fission MUST treat beta distribution as a managed release mode, not just a package upload. Beta configuration belongs under `beta.*` in `fission.toml` and can be backed by CSV files, provider groups, email lists, or provider-native groups.

Required beta operations:

- list provider groups/tracks/flights;
- create provider groups/tracks/flights when APIs allow it;
- import tester CSV files;
- export current testers;
- diff local tester files against provider state;
- add/remove testers;
- assign builds to groups/tracks/flights;
- generate or fetch public/private beta links where supported;
- expire or stop testing for old builds where supported;
- write beta distribution receipts.

Provider behavior differs. App Store Connect supports internal and external TestFlight testers and groups [R42][R43]. Google Play supports internal, closed, and open testing tracks and tester configuration through Play Console [R46]. Microsoft Store supports package flights through Store submission APIs [R51]. Fission must expose one coherent command model while preserving provider-specific limits in readiness output.

### 18.7 Submission validation and review readiness

Fission MUST run a pre-submission validation pass before sending a store submission for review. This validation is not a replacement for provider review, but it catches errors that can be detected locally or through provider APIs.

Validation MUST check:

- required metadata is present for each configured locale after resolving `fission.toml` and referenced release files;
- screenshots and videos match provider dimensions, file formats, and count rules;
- URLs are reachable and use HTTPS where required;
- app id, bundle id, package name, Store identity, and manifest identifiers agree;
- privacy/data-safety files exist when a provider requires them;
- review notes explain login credentials, demo modes, hardware requirements, regional restrictions, and sensitive features;
- release notes exist for every locale selected for the release;
- no placeholder strings such as `TODO`, `TBD`, `lorem ipsum`, or generated template text remain in release content;
- the submission target is compatible with the artifact format and signing state.

Provider-side validation results MUST be stored in the distribution receipt. Local validation failures MUST be stable readiness errors.

### 18.8 Symbols, crash assets, and release diagnostics

Fission MUST track debug symbols and crash-diagnostics assets as release artifacts. Native symbols, dSYM bundles, ProGuard/R8 mappings, Breakpad/Crashpad symbols, or Fission runtime symbol maps are not store listing content, but they are part of making a production release supportable.

Artifact manifests MUST support secondary artifacts with explicit purposes:

```json
{
  "kind": "debug_symbols",
  "platform": "ios",
  "path": "target/fission/release/ios/symbols/Todo.dSYM.zip",
  "sha256": "...",
  "upload_provider": "crash-service"
}
```

Readiness MUST report when symbols are available but not uploaded to a configured crash provider. Store distributors SHOULD upload provider-native symbol assets when the store accepts them; third-party crash providers should be modeled as additional distribution providers.

### 18.9 Version intelligence and release workflows

Fission MUST understand provider-side version state. Before packaging or distribution it should be able to answer:

- what is the latest build number uploaded to each provider;
- what is the latest build number released to each track/channel;
- whether the local version/build is monotonically valid for the selected provider;
- whether a version is already in review, released, rejected, or expired;
- whether the next action should upload, submit for review, resume rollout, halt rollout, or promote an existing artifact.

Fission should support project-defined release workflows as declarative recipes, not as an embedded scripting language. A recipe is an ordered list of Fission commands with explicit inputs, outputs, and readiness gates.

```toml
[release_workflows.beta]
steps = [
  "readiness release --target ios --format ipa --provider app-store",
  "release-content validate --provider app-store",
  "package --target ios --format ipa --release",
  "distribute --provider app-store --track testflight",
  "beta distribute --provider app-store --group external-beta"
]
```

Recipes MUST remain inspectable and reproducible. They should compose Fission commands rather than hiding behavior in arbitrary code.

## 19. Remote builders

Some package formats are host-bound:

- official Apple signing, notarization, iOS device packaging, and App Store upload generally require macOS and Xcode tooling;
- Windows Store/MSIX validation and signing may require Windows SDK tools;
- Android can run on macOS, Windows, or Linux if SDK/NDK/JDK are installed.

Fission SHOULD support remote builders, but readiness must make host limitations explicit.

```text
fission package --target ios --format ipa --builder macos-ci
fission readiness package --target windows --format msix --builder windows-ci
```

Remote builder contracts MUST include:

- same Fission CLI version or compatible protocol;
- clean checkout or artifact input;
- credential source policy;
- artifact return path;
- logs and receipts;
- reproducible environment report.

## 20. Validation, screenshots, and install smoke tests

Packaging is not complete until the artifact can be installed or loaded in the expected environment.

Required smoke checks:

| Target | Format | Smoke check |
| --- | --- | --- |
| Linux | `.run` | run `--verify`, install to temp prefix, launch app with smoke flag/headless check, uninstall, verify receipt removal |
| macOS | `.app` | verify bundle structure, codesign verify, Gatekeeper assessment where available, launch smoke |
| macOS | `.pkg` | expand or inspect package, verify signature/notarization, install to temp volume/user test where possible |
| Windows | `.exe`/`.msi` | verify signature, silent install to test VM where possible, launch smoke, uninstall |
| Windows | `.msix` | validate manifest/block map/signature, install in test environment, launch smoke |
| Android | `.apk` | install on emulator/device, launch, collect logcat, uninstall |
| Android | `.aab` | validate with `bundletool`, generate APK set, install generated APKs on emulator/device |
| iOS | `.ipa` | validate signing/provisioning, install on configured device/test service where possible |
| Web | static | serve locally, open with browser automation, verify canvas/app boot and logs |

Smoke tests SHOULD integrate with the existing `fission test --target` platform testing path. Screenshot capture MUST also integrate with this path so the same semantic selectors, seeded state, logs, and device discovery are reused for package validation and release-content generation. A screenshot capture failure must produce the same quality of diagnostic output as a failing smoke test: platform logs, selector wait failure, last rendered frame if available, and the exact device/locale/theme configuration used.

## 21. Security and compliance requirements

The release tooling handles signing keys, store credentials, and user accounts. It must be treated as sensitive infrastructure.

Required controls:

- no plaintext secrets in project files;
- no secrets in artifact manifests or distribution receipts;
- redaction in all logs;
- `--verbose` must not print secret values;
- credentials have provider, account, scopes, created time, and last-used time;
- `fission auth audit` lists accounts and scopes without revealing secrets;
- credential rotation is supported per provider;
- `fission auth logout` revokes tokens where provider supports revocation and deletes local vault records;
- vault unlock failures fail closed;
- CI mode refuses interactive prompts;
- distribution commands support `--dry-run` where provider APIs allow validation without mutation;
- destructive provider operations require explicit flags.

## 22. Error model

Errors MUST use stable IDs. Human text can improve over time, but IDs should remain stable for docs, tests, and IDEs.

Examples:

```text
release.android.sdk.missing
release.android.bundletool.missing
release.android.play.first_setup_required
release.apple.xcode.missing
release.apple.notary.credentials_missing
release.apple.app_record_manual_required
release.windows.msix.identity_mismatch
release.windows.signing.certificate_missing
release.content.screenshot_size_invalid
release.content.remote_state_changed
release.beta.group_missing
release.beta.tester_import_invalid
release.store.metadata_placeholder_text
release.symbols.not_uploaded
release.s3.bucket_not_writable
release.github_pages.source_not_actions
release.github_pages.custom_domain_dns_unhealthy
release.github_pages.base_path_mismatch
release.cloudflare_pages.token_missing
release.cloudflare_pages.domain_not_active
release.cloudflare_pages.upload_failed
release.netlify.site_missing
release.netlify.deploy_not_ready
release.netlify.domain_not_configured
release.provider.rollback_unsupported
release.oauth.device_flow_timeout
release.vault.keyring_unavailable
```

Every error MUST include:

- what was checked;
- what was found;
- why it matters;
- how to fix it;
- whether the fix is local CLI config, OS setup, provider portal setup, or credential setup.

## 23. Implementation milestones

These are implementation milestones, not partial product definitions. The final accepted feature requires all relevant milestones for each supported platform/provider.

1. Add release config schema and artifact/distribution receipt schema.
2. Add credential vault and `fission auth` commands.
3. Add release-config root schema in `fission.toml`, `[[releases]]` file references, referenced release-file schemas, TUI editing, non-interactive edit/import/diff/validate/push commands, screenshot scenario model, and content manifest.
4. Add readiness engine with JSON output and stable error IDs.
5. Implement Linux `.run` packager and smoke install/uninstall checks.
6. Implement macOS `.app`, `.pkg`, signing, notarization, and readiness checks.
7. Implement Windows `.exe`, `.msi`, `.msix`, signing/provider distinctions, and readiness checks.
8. Implement Android APK/AAB packaging, bundle validation, signing, and Play readiness.
9. Implement iOS IPA packaging, signing/provisioning readiness, and App Store/TestFlight distribution path.
10. Implement web static packaging with MIME/cache metadata.
11. Implement screenshot capture/rendering for web, Android, iOS, macOS, and Windows where device automation is available.
12. Implement store metadata import/diff/validate/push for supported providers using `fission.toml` plus referenced release files as the authoritative inputs.
13. Implement beta group/tester/flight management for supported providers.
14. Implement version-state queries, release recipes, and provider-side status observation.
15. Implement GitHub Pages distribution for Actions and branch-source modes, including custom-domain readiness and DNS health checks.
16. Implement Cloudflare Pages distribution with API-token auth, Wrangler-based prebuilt upload, project/domain readiness, and receipts.
17. Implement Netlify distribution with token auth, API deploys, draft/production deploys, custom-domain readiness, and receipts.
18. Implement S3-compatible distribution and receipts.
19. Implement Google Drive, OneDrive, and Dropbox distribution.
20. Implement store distribution for Google Play, App Store Connect/TestFlight, and Microsoft Store.
21. Implement review/customer-feedback list/reply where provider APIs support it.
22. Add install/upload/screenshot smoke tests and CI coverage for non-secret paths.
23. Document provider setup walkthroughs and first-release manual checklists.

Each milestone MUST land with unit tests for config/readiness logic, integration tests for local packaging where possible, and mocked provider tests for distribution APIs.

## 24. Acceptance criteria

The post-build lifecycle work is accepted when the following are true:

- `fission readiness package` accurately reports missing local packaging prerequisites for Linux, macOS, Windows, Android, iOS, and web.
- `fission readiness distribute` accurately reports missing provider credentials, store setup, and cloud destination access.
- `fission package` creates real artifacts for all required formats: `.run`, `.app`, `.pkg`, `.exe`, `.msi`, `.msix`, `.apk`, `.aab`, `.ipa`, and web static bundle.
- Package commands write artifact manifests with hashes and signing/notarization state.
- Distribution commands consume artifact manifests and write distribution receipts.
- Store distributors guide first-release manual setup where platform APIs cannot create the required store state.
- Release-config commands edit, import, diff, validate, and push store metadata through `fission.toml` plus referenced release files in TUI and non-interactive modes.
- Release-content commands capture and render screenshots/previews and validate provider requirements without rebuilding the binary.
- Beta commands manage provider groups/tracks/flights, tester lists, build assignment, and beta distribution receipts.
- Version-state commands can report provider-side latest build/release status and block invalid monotonic version changes.
- Cloud distributors upload to S3-compatible storage, Google Drive, OneDrive, and Dropbox and return usable links/provider IDs.
- Credentials are stored through OS keyring plus encrypted vault for local development, and through explicit CI secret sources in CI.
- No provider token, signing password, or private key is stored in plaintext under the project or home directory by Fission.
- Platform smoke checks prove artifacts install or load in the expected environment.
- Screenshot capture failures produce actionable logs and selector diagnostics.
- Store review/customer-feedback operations are available where provider APIs support them and never require app runtime changes.
- All commands support JSON output for CI and IDEs.

## References

[R1] Android Developers, `bundletool`: https://developer.android.com/tools/bundletool  
[R2] Google Play Developer API, Edits: https://developers.google.com/android-publisher/edits  
[R3] Android Developers, Android App Bundles: https://developer.android.com/guide/app-bundle  
[R4] Google Play Console Help, Play App Signing: https://support.google.com/googleplay/android-developer/answer/9842756  
[R5] Google Play Console Help, Create and set up your app: https://support.google.com/googleplay/android-developer/answer/9859152  
[R6] Google Play Developer API, Tracks: https://developers.google.com/android-publisher/api-ref/rest/v3/edits.tracks  
[R7] Apple Developer, Notarizing macOS software before distribution: https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution  
[R8] Apple Developer, Bundle Structures: https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html  
[R9] Apple Developer, Placing content in a bundle: https://developer.apple.com/documentation/bundleresources/placing-content-in-a-bundle  
[R10] Apple Developer, Customizing the notarization workflow: https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution/customizing_the_notarization_workflow  
[R11] Apple Support, macOS app code signing process: https://support.apple.com/guide/security/app-code-signing-process-in-macos-sec3ad8e6e53/web  
[R12] Apple Developer, Bundle configuration and Info.plist: https://developer.apple.com/documentation/bundleresources/bundle-configuration  
[R13] Apple command-line package tools, `pkgbuild` and `productbuild` man pages: https://keith.github.io/xcode-man-pages/pkgbuild.1.html and https://keith.github.io/xcode-man-pages/productbuild.1.html  
[R14] Apple Developer, App Store Connect API Apps resource: https://developer.apple.com/documentation/appstoreconnectapi/apps  
[R15] Apple Developer, App Store Connect upload builds: https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds/  
[R16] Microsoft Learn, What is MSIX?: https://learn.microsoft.com/en-us/windows/msix/overview  
[R17] Microsoft Learn, Sign your MSIX package: https://learn.microsoft.com/en-us/windows/msix/package/sign-msix-package-guide  
[R18] Microsoft Learn, Microsoft Store submission API: https://learn.microsoft.com/en-us/windows/apps/publish/store-submission-api  
[R19] Microsoft Learn, Create an app submission for your MSIX app: https://learn.microsoft.com/en-gb/windows/apps/publish/publish-your-app/msix/create-app-submission  
[R20] AWS SDK for Rust, Amazon S3 examples: https://docs.aws.amazon.com/sdk-for-rust/latest/dg/rust_s3_code_examples.html  
[R21] Google Drive API, Upload file data: https://developers.google.com/drive/api/v3/manage-uploads  
[R22] Google Drive API, Create permissions: https://developers.google.com/drive/api/reference/rest/v3/permissions/create  
[R23] Microsoft Graph, driveItem createUploadSession: https://learn.microsoft.com/en-us/graph/api/driveitem-createuploadsession  
[R24] Dropbox API HTTP documentation and OAuth guide: https://www.dropbox.com/developers/documentation/http/documentation and https://developers.dropbox.com/oauth-guide  
[R25] `cargo-packager` crate documentation: https://docs.rs/cargo-packager  
[R26] `cargo-bundle` crate documentation: https://docs.rs/crate/cargo-bundle/latest  
[R27] `cargo-wix` crate documentation: https://docs.rs/crate/cargo-wix/latest  
[R28] Apple Codesign / `rcodesign` documentation: https://gregoryszorc.com/docs/apple-codesign/stable/  
[R29] `keyring` crate documentation: https://docs.rs/keyring/latest/keyring/  
[R30] `age` crate documentation: https://docs.rs/age/  
[R31] `chacha20poly1305` crate documentation: https://docs.rs/chacha20poly1305/latest/chacha20poly1305/  
[R32] `secrecy` crate documentation: https://docs.rs/secrecy/latest/secrecy/  
[R33] `zeroize` crate documentation: https://docs.rs/zeroize/latest/zeroize/  
[R34] RFC 8628, OAuth 2.0 Device Authorization Grant: https://www.rfc-editor.org/rfc/rfc8628  
[R35] Freedesktop Desktop Entry Specification: https://specifications.freedesktop.org/desktop-entry-spec/latest/  
[R36] Freedesktop AppStream Desktop Applications metadata: https://www.freedesktop.org/software/appstream/docs/sect-Metadata-Application.html  
[R37] Makeself self-extracting archives: https://makeself.io/  
[R38] Apple Developer, Screenshot specifications: https://developer.apple.com/help/app-store-connect/reference/screenshot-specifications  
[R39] Apple Developer, Upload app previews and screenshots: https://developer.apple.com/help/app-store-connect/manage-app-information/upload-app-previews-and-screenshots/  
[R40] Apple Developer, App Screenshot Sets API: https://developer.apple.com/documentation/appstoreconnectapi/app_store/app_metadata/app_screenshot_sets  
[R41] Apple Developer, Uploading assets to App Store Connect: https://developer.apple.com/documentation/appstoreconnectapi/uploading_assets_to_app_store_connect  
[R42] Apple Developer, TestFlight overview: https://developer.apple.com/help/app-store-connect/test-a-beta-version/testflight-overview/  
[R43] Apple Developer, Add testers to builds: https://developer.apple.com/help/app-store-connect/test-a-beta-version/add-testers-to-builds/  
[R44] Google Play Console Help, Add preview assets to showcase your app: https://support.google.com/googleplay/android-developer/answer/9866151  
[R45] Android Developers, Google Play Developer APIs: https://developer.android.com/google/play/developer-api  
[R46] Google Play Console Help, Set up an open, closed, or internal test: https://support.google.com/googleplay/android-developer/answer/9845334  
[R47] Google Play Console Help, Share app bundles and APKs internally: https://support.google.com/googleplay/android-developer/answer/9844679  
[R48] Google Play Developer API, internal app sharing upload APK: https://developers.google.com/android-publisher/api-ref/rest/v3/internalappsharingartifacts/uploadapk  
[R49] Google Play Developer API, localized store listings and images: https://developers.google.com/android-publisher/edits and https://developers.google.com/android-publisher/api-ref/rest/v3/edits.images  
[R50] Microsoft Learn, App screenshots, images, and trailers for MSIX app: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/screenshots-and-images  
[R51] Microsoft Learn, Manage package flights: https://learn.microsoft.com/en-us/windows/uwp/monetize/manage-flights  
[R52] Google Play Developer API, Reply to reviews: https://developers.google.com/android-publisher/reply-to-reviews  
[R53] Apple Developer, Create or update a response to a customer review: https://developer.apple.com/documentation/appstoreconnectapi/post-v1-customerreviewresponses
[R54] GitHub Docs, Configuring a publishing source for your GitHub Pages site: https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site
[R55] GitHub Docs, Using custom workflows with GitHub Pages: https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages
[R56] GitHub Docs, Managing a custom domain for your GitHub Pages site: https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site
[R57] GitHub Docs, REST API endpoints for GitHub Pages: https://docs.github.com/en/rest/pages
[R58] Cloudflare Docs, Use Direct Upload with continuous integration: https://developers.cloudflare.com/pages/how-to/use-direct-upload-with-continuous-integration/
[R59] Cloudflare Docs, Pages REST API: https://developers.cloudflare.com/pages/configuration/api/
[R60] Cloudflare Docs, Pages custom domains: https://developers.cloudflare.com/pages/configuration/custom-domains/
[R61] Netlify Docs, Get started with the Netlify API: https://docs.netlify.com/api-and-cli-guides/api-guides/get-started-with-api/
[R62] Netlify Docs, Create deploys: https://docs.netlify.com/deploy/create-deploys/
[R63] Netlify Docs, Manage domains for a site or app: https://docs.netlify.com/domains/manage-domains/manage-domains-for-a-site-app/
[R64] GitHub CLI manual, `gh release create`: https://cli.github.com/manual/gh_release_create
[R65] GitHub CLI manual, `gh release upload`: https://cli.github.com/manual/gh_release_upload
[R66] Microsoft Learn, Microsoft Store Developer CLI overview: https://learn.microsoft.com/en-us/windows/apps/publish/msstore-dev-cli/overview
[R67] Microsoft Learn, Microsoft Store Developer CLI commands: https://learn.microsoft.com/en-us/windows/apps/publish/msstore-dev-cli/commands
