# RFC: Build, package, publish, and release workflow

Status: draft
Audience: Fission CLI, release tooling, target shell, documentation, and CI implementers
Scope: `fission.toml`, build/package/sign/release-content/distribution/publish flows, local guided UX, TUI, windowed app, and CI

## 1. Purpose

Fission must behave like a complete app platform, not a renderer with a few packaging scripts. A developer should be able to start from a Fission project and understand exactly what is missing before an app can be built, signed, packaged, distributed, submitted, or published for each supported target.

This RFC consolidates the release/build/publish requirements that were spread across documentation, implementation notes, and prior design discussion. Where this RFC disagrees with older notes, this RFC wins.

The failure mode this RFC prevents is a CLI that technically uploads an artifact but produces a poor release: no release notes, no screenshots, no version/build management, no store metadata guidance, duplicated shell logic, unsafe secret handling, or unclear next actions.

## 2. Supported targets and providers

### 2.1 Targets

The public target names are:

- `macOS`
- `Windows`
- `Linux`
- `Web`
- `Android`
- `iOS`
- `Terminal`
- `Static site`
- `SSR`

The command model must support these target identifiers consistently across docs, `fission.toml`, generated project files, readiness checks, package commands, guided flows, TUI, windowed UI, and CI JSON.

### 2.2 Package formats

Fission must model build output separately from publish destination.

Required package outputs:

- Android: `apk`, `aab`
- iOS: `ipa`
- macOS: `app`, `pkg`
- Windows: `exe`, `msi`, `msix`
- Linux: `appimage`, `run`
- Web: `static`
- Static site: `static`, `docker-image`
- SSR: `docker-image`
- Terminal: executable/package format defined by the terminal-shell RFC

### 2.3 Providers

Required provider families:

- Google Play
- App Store Connect
- Microsoft Store
- GitHub Releases
- GitHub Pages
- Cloudflare Pages
- Netlify
- Docker/OCI registry
- S3-compatible object storage
- Google Drive
- OneDrive
- Dropbox

Provider support does not mean every provider supports every metadata field or lifecycle action. The readiness model must report supported, unsupported, recommended, optional, and required items explicitly.

### 2.4 Linux package roles

AppImage and `.run` are both first-class Linux outputs, but they are not aliases:

- `appimage` is the recommended portable download for an ordinary graphical
  application. It runs as one architecture-specific file without first
  installing the application.
- `run` is the installable format for applications that need an installed
  location, desktop or system integration, services, privileged helpers,
  explicit repair/uninstall behavior, or operation without an AppImage mount.
- Terminal continues to use `run`; AppImage's desktop-oriented application
  bundle does not replace the Terminal package contract.

"Portable Linux" does not mean one artifact works on every architecture or
arbitrary distribution baseline. Every AppImage is produced for a declared
architecture, bundles the non-base libraries it needs, is built against the
oldest supported runtime baseline, and is qualified on the supported
distributions.

## 3. Product contract

Fission has one public command surface: `fission`.

Internal crates may split responsibilities, but the user-facing workflow must not feel like multiple unrelated tools. The CLI, guided mode, TUI, windowed app, and CI mode must all drive the same workflow engine.

Required developer entry points:

```text
fission init
fission add-target <target>
fission run --target <target>
fission package --target <target> --format <format> --release
fission publish --provider <provider> [--target <target>] [--format <format>] [--track <track>] [--locale <locale>]
fission publish --provider <provider> --artifact <artifact-manifest.json> --yes
fission readiness release --target <target> --format <format> --provider <provider> [--locale <locale>]
fission release-config ...
fission release-content ...
```

`cargo fission` may exist as a Rust-friendly proxy, but docs and generated project files must primarily teach `fission`.

## 4. Required workflow architecture

### 4.1 One workflow engine

The implementation must expose a shared publish/release engine used by:

- direct CLI commands;
- line-oriented guided CLI;
- Fission Terminal TUI;
- Fission windowed app via `--app`;
- CI JSON mode;
- future IDE integrations.

The shell/UI layers must render and drive the same plan. They must not reimplement package building, secret resolution, key generation, readiness checks, provider upload, receipts, or version/build logic.

Required core model:

```rust
pub struct ReleaseContext {
    pub project_dir: PathBuf,
    pub app_name: String,
    pub target: Target,
    pub format: PackageFormat,
    pub provider: DistributionProvider,
    pub release_id: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub interactive: bool,
    pub ci: bool,
}

pub struct ReleasePlan {
    pub context: ReleaseContext,
    pub steps: Vec<ReleaseStep>,
    pub requirements: Vec<ReleaseRequirement>,
}

pub struct ReleaseStep {
    pub id: String,
    pub title: String,
    pub status: StepStatus,
    pub jobs: Vec<ReleaseJob>,
}

pub enum RequirementLevel {
    ProviderRequired,
    FissionRecommended,
    Optional,
    NotApplicable,
}

pub struct ReleaseRequirement {
    pub id: String,
    pub level: RequirementLevel,
    pub status: RequirementStatus,
    pub summary: String,
    pub remediation: Vec<String>,
    pub can_fix_interactively: bool,
}
```

Every UI mode must render this model. Every mutating action must call a shared job/tool API.

### 4.2 Job/event model

Long-running operations must emit structured events:

- check started/completed;
- command spawned;
- stdout/stderr lines;
- progress;
- artifact created;
- metadata synced;
- provider request/response summary;
- receipt written;
- failure with diagnostic.

The guided CLI may render only the latest 10 log lines while a job runs, but the full event stream must be retained for receipts and diagnostics.

### 4.3 No false readiness

No UI may display `ok`, green state, or completion unless backed by a deterministic check or completed job event.

Placeholders must be displayed as `not checked`, `recommended`, `skipped`, or `pending`, not as success.

## 5. `fission.toml` contract

### 5.1 What belongs in `fission.toml`

`fission.toml` is the authoritative, reviewable release root.

It may contain:

- stable app identity;
- app name, version, build number;
- package identifiers;
- target choices;
- provider public IDs;
- track/channel names;
- locale lists;
- non-secret provider options;
- paths to source assets and release-content files;
- env var names for secrets;
- release records;
- receipt paths.

It must remain hand-editable and code-reviewable.

### 5.2 What must not belong in `fission.toml`

`fission.toml` must never contain:

- private keys;
- keystores;
- certificate files containing private keys;
- service-account JSON;
- App Store `.p8` key paths;
- Play service-account paths;
- inline secrets;
- passwords;
- access tokens;
- machine-specific absolute secret paths.

There is no legacy compatibility path for secret fields. Secret-bearing fields are invalid.

### 5.3 Secret sources

Supported secret sources:

- environment variables;
- base64 environment variables decoded into secure temporary files;
- platform-owned stores where a platform supports them safely;
- provider CLI-auth state where the provider CLI owns the secret;
- user-selected local files outside the repo for interactive local runs. If a selected secret file is inside the project tree, Fission may copy or move it into the private local workspace but must not keep a live reference to that project path.

The old Fission vault concept is rejected. It adds hidden local state and creates a false sense of security. Local release still uses production keys, so it must be as explicit and secure as CI.

### 5.4 Local workspace

Interactive local publishing uses:

```text
~/.fission/<app-name>/
  release.env
  upload-key.jks
  app-store-key.p8
  windows-signing.pfx
  provider-service-account.json
```

The exact files depend on target/provider. The directory must be created with private permissions. The env file must be private. This workspace is local developer state and is never required in CI.

### 5.5 Example schema slice

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

[package.android]
package_name = "com.example.todo"
version_code = 42
version_name = "1.2.3"
min_sdk = 26
target_sdk = 36
keystore_alias = "upload"
keystore_env = "ANDROID_KEYSTORE"
keystore_base64_env = "ANDROID_KEYSTORE_BASE64"

[package.ios]
bundle_id = "com.example.todo"
marketing_version = "1.2.3"
build_number = "42"
team_id = "TEAMID1234"

[package.macos]
bundle_id = "com.example.todo"
marketing_version = "1.2.3"
build_number = "42"
team_id = "TEAMID1234"
minimum_os = "13.0"
entitlements = "platforms/macos/entitlements.plist"
provisioning_profile = "Todo.provisionprofile"
signing_identity = "Developer ID Application: Example Software Ltd"
installer_identity = "Developer ID Installer: Example Software Ltd"
notarize = true

[package.windows]
identity_name = "ExampleSoftware.Todo"
publisher = "CN=Example Software Ltd, O=Example Software Ltd, C=GB"
installer = "msix"
certificate_thumbprint_env = "WINDOWS_CERTIFICATE_THUMBPRINT"
certificate_base64_env = "WINDOWS_CERTIFICATE_BASE64"

[distribution.play_store]
package_name = "com.example.todo"
default_track = "internal"
access_token_env = "PLAY_STORE_ACCESS_TOKEN"
service_account_json_env = "PLAY_STORE_SERVICE_ACCOUNT_JSON"
service_account_json_base64_env = "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64"
google_application_credentials_env = "GOOGLE_APPLICATION_CREDENTIALS"

[distribution.app_store]
bundle_id = "com.example.todo"
issuer_id = "00000000-0000-0000-0000-000000000000"
key_id = "ABC123DEFG"
access_token_env = "APP_STORE_CONNECT_ACCESS_TOKEN"
issuer_id_env = "APP_STORE_CONNECT_ISSUER_ID"
key_id_env = "APP_STORE_CONNECT_KEY_ID"
api_key_env = "APP_STORE_CONNECT_API_KEY"
api_key_base64_env = "APP_STORE_CONNECT_API_KEY_BASE64"
api_key_path_env = "APP_STORE_CONNECT_API_KEY_PATH"
default_track = "testflight"

[distribution.microsoft_store]
product_id = "9N0000000000"
package_identity_name = "ExampleSoftware.Todo"
package_type = "msix"
token_env = "MICROSOFT_STORE_TOKEN"
tenant_id_env = "AZURE_TENANT_ID"
client_id_env = "AZURE_CLIENT_ID"
client_secret_env = "MICROSOFT_STORE_CLIENT_SECRET"
seller_id_env = "MICROSOFT_STORE_SELLER_ID"
submit = false

[distribution.s3.production]
bucket = "example-downloads"
region = "eu-west-2"
prefix = "todo/releases/{version}/"
visibility = "private"
overwrite = false
cache_control = "public, max-age=31536000, immutable"

[release]
active_release = "1.2.3+42"
default_locales = ["en-US"]
content_output_dir = "release-content"
metadata_root = "release-content/metadata"
skip_requirements = [] # explicit ids for provider-optional/recommended items the team chose to skip

[release.provider_locks.play_store]
provider = "play-store"
subject = "com.example.todo"
locales = ["en-US"]
remote_revision = "sha256:<provider-metadata-baseline>"
locked_at_unix_seconds = 1780000000 # example Unix timestamp

[[releases]]
id = "1.2.3+42"
version = "1.2.3"
build = 42
status = "candidate"
tracks = ["play-store:internal"]
locales = ["en-US"]
metadata = "release-content/metadata/1.2.3+42/release.toml"
release_notes = "release-content/metadata/1.2.3+42/notes"
review = "release-content/metadata/1.2.3+42/review.toml"
privacy = "release-content/metadata/1.2.3+42/privacy.toml"
```

## 6. Version and build management

Version/build management is a first-class release step.

Before packaging a store-bound artifact, Fission must:

1. resolve the requested release version/build;
2. sync it into platform-native files;
3. validate it against platform rules;
4. query provider state where the provider exposes enough data;
5. prevent avoidable upload failures.

### 6.1 Android

Fission must manage:

- `versionCode`;
- `versionName`;
- package name;
- signing alias;
- release/debug debuggability.

For Google Play, Fission must query existing track/artifact state before upload. If the version code was already used, Fission must not upload and wait for Play to reject it. It must produce an actionable diagnostic and, in interactive mode, offer to increment the build number with `fission release-config bump-build --target android --yes` and rebuild.

### 6.2 iOS and macOS

Fission must manage:

- marketing version;
- build number;
- bundle id;
- team id;
- signing/notarization state where applicable.

For App Store Connect, Fission must check app/build state before upload where possible and separate upload from TestFlight assignment or App Review submission.

### 6.3 Windows

Fission must manage:

- package identity;
- package version;
- publisher identity;
- signing certificate source;
- MSIX/MSI/EXE-specific rules.

For Microsoft Store, Fission must distinguish draft update, private flight, public submission, certification, and staged rollout.

### 6.4 Providers with overwrite/update semantics

Providers differ:

- Google Play does not allow reused Android version codes.
- App Store Connect has app-version/build-number rules and processing states.
- Microsoft Store has draft/submission/package rules.
- GitHub Releases may edit an existing release and replace assets if explicitly configured.
- S3/object storage may overwrite objects only when overwrite is explicitly configured or confirmed.
- Static hosts may create immutable deploys and then promote/rollback.

The release plan must model this provider-specific idempotency instead of using a single "upload again" behavior.

## 7. Required stages

### 7.1 Inspect

Read:

- `Cargo.toml`;
- `fission.toml`;
- generated platform folders;
- release records;
- release-content references;
- target/provider config;
- environment availability;
- provider auth state where safe.

### 7.2 Preflight

Preflight must cover:

- project config validity;
- target configured;
- package format supported for target;
- platform toolchain installed;
- platform SDK/NDK/Xcode/Windows tooling availability;
- signing input availability;
- provider credential availability;
- provider access to the app/package/product;
- target/package identity match;
- version/build validity;
- release-content status;
- store metadata status;
- artifact reuse/rebuild decision.

### 7.3 Configure

Interactive mode must guide the developer through:

- app id/package id/bundle id/product id confirmation;
- version/build selection;
- track/channel selection;
- locale selection;
- signing setup;
- provider auth setup;
- release notes/metadata/screenshot setup;
- CI export.

### 7.4 Build and package

Packaging must:

- sync release version/build into target-native config;
- sync app identity into target-native config;
- generate/copy icons and package assets;
- apply release signing only from approved secret sources;
- create the artifact;
- validate the artifact;
- write `artifact-manifest.json` with hashes, MIME types, target, format, version, build, and source config facts.

#### 7.4.1 Linux AppImage

```text
fission package --target linux --format appimage --release
```

produces:

```text
target/fission/release/linux/appimage/<app-name>-<version>-<arch>.AppImage
target/fission/release/linux/appimage/artifact-manifest.json
```

Fission stages a standard AppDir before creating the final image:

```text
<app-name>.AppDir/
  AppRun
  <app-id>.desktop
  <application-icon>
  usr/bin/<install-name>
  usr/lib/<bundled-runtime-libraries>
  usr/libexec/<native-helper-products>
  usr/share/<app-id>/...
  usr/share/icons/hicolor/<size>/apps/<app-id>.<ext>
  usr/share/metainfo/<app-id>.metainfo.xml
```

The staged executable, application assets, generated icons, native runtime
products, desktop entry, and AppStream metadata must come from the same package
inputs used by the rest of the Fission lifecycle. AppImage packaging must not
rebuild the app, search arbitrary host directories, or create a second package
configuration authority.

Fission should invoke the current official `appimagetool` over the staged AppDir
rather than maintaining an independent ELF/SquashFS writer. Tool acquisition is
explicit and versioned: readiness reports whether the approved tool and runtime
for the selected architecture are available, and the artifact manifest records
the tool/runtime identity used. Packaging must not silently download and execute
an unpinned tool.

The packager must follow AppImage library-bundling guidance: bundle application
dependencies that cannot be assumed on the target baseline, do not indiscriminately
bundle foundational host libraries, avoid compiled-in installation paths, and
build on a baseline no newer than the oldest supported distribution. Readiness
must reject a package whose declared architecture does not match its executable
or embedded AppImage runtime.

AppImage execution normally mounts its embedded filesystem through the Linux
FUSE facility. Current static runtimes avoid requiring a separately installed
`libfuse` library, but the host must still permit the userspace filesystem mount.
The AppImage extraction/run fallback must be documented and qualified; `.run`
remains available for environments where mounting is unavailable or installation
is required.

AppImage itself is not a sandbox. Fission must not describe the format as adding
process, filesystem, network, or capability isolation.

The package must not embed a conventional AppImage zsync update route by default.
Fission's [TUF application-update RFC](rfc-tuf-application-updates.md) owns
authenticated update discovery and policy. When enabled there, the `.zsync`
control file is a separately authenticated TUF
target, zsync is only a transfer optimization, and the reconstructed AppImage is
verified against the authoritative final TUF target before activation. Projects
that explicitly choose the conventional AppImage updater are selecting a
different update policy and must do so visibly.

AppImage validation must, at minimum:

- inspect the ELF and AppImage architecture rather than trusting the filename;
- verify the expected AppImage magic/runtime and a readable embedded filesystem;
- extract the AppDir and validate `AppRun`, the executable, desktop entry, icon,
  AppStream metadata, assets, libraries, and native-product destinations;
- verify executable permissions and reject unsafe absolute paths or missing
  runtime libraries detectable at packaging time;
- run an extraction-based launch smoke so validation does not depend solely on
  FUSE availability in CI;
- run the finished image normally on clean Linux baselines for the declared
  support matrix;
- record structural, extraction, and launch evidence in package validation and
  `artifact-manifest.json`.

GitHub Releases and general artifact/file providers may distribute AppImage and
optional TUF/zsync sidecars exactly like other manifest-listed artifacts. Store
or portal integration is not implied by producing an AppImage.

### 7.5 Release content

Release content includes:

- release notes;
- screenshots;
- app previews/videos/trailers where supported;
- localized store listings;
- privacy/data-safety answers;
- review instructions;
- demo account/review attachments;
- tester groups;
- rollout notes.

These are not all hard blockers. Each item must be classified for the selected provider/track as:

- provider required;
- Fission recommended;
- optional;
- not applicable.

The UI must present recommended items in the recommended order, but it must allow explicit skip when the provider allows omission.
Explicit skips are recorded in `[release].skip_requirements` using exact release-plan requirement ids. Provider-required requirements cannot be skipped; Fission reports those skip attempts as warnings and keeps the underlying requirement blocking.

### 7.6 Dry-run

Dry-run must show:

- exact artifact;
- version/build;
- provider destination;
- track/channel;
- locales;
- metadata/release-content that would be pushed;
- assets that would be uploaded;
- whether existing remote state would be updated, replaced, promoted, or rejected.

Dry-run must not mutate provider state.

### 7.7 Publish

Publish must:

- re-run required checks or verify plan freshness;
- upload or update artifact;
- upload or update metadata/assets where applicable and selected;
- commit/submit/promote only when explicitly selected or confirmed;
- write a receipt;
- report provider processing state and follow-up.

### 7.8 Observe

Status commands must fetch provider state after publishing:

- Play track/release state;
- App Store build processing/TestFlight/review state;
- Microsoft Store draft/certification state;
- static host deployment status;
- object storage object status;
- GitHub Release status;
- Docker manifest status.

## 8. Interactive UX contract

### 8.1 Guided CLI

`--guided` is the low-friction terminal prompt flow. It may be line-oriented, but it still uses the shared release plan.

It must:

- show real checks only;
- prompt for file paths directly when it cannot browse;
- present release metadata, release notes, screenshots, privacy/review files, and skippable recommended content as a first-class review step;
- stream subprocess logs with a bounded live log view;
- show spinner/progress during waits;
- show exact next actions;
- retain full logs for failure diagnostics.

### 8.2 TUI

Default local `fission publish` may open a Fission terminal app.

The TUI must:

- be a real Fission app;
- use the same publish workflow model;
- replace, not duplicate, the old terminal UI;
- support keyboard navigation, including arrows;
- support a terminal file browser starting at the project directory and allowing navigation upward;
- when a file is selected, ask whether to reference, copy, or move it into the local workspace where appropriate;
- be screenshot-testable through terminal LiveTest.

### 8.3 Windowed app

`fission publish --app` opens a native windowed Fission app.

The windowed app must:

- use the same workflow model/components as TUI;
- use native OS file dialogs, not the terminal tree picker;
- follow the approved design-board wizard structure;
- be responsive;
- be screenshot-tested through Fission LiveTest.

### 8.4 Wizard structure

The product workflow is a wizard, not one page with every panel rendered at once.

Shared wizard sections:

1. project and target;
2. provider account and app identity;
3. signing/secrets;
4. release version/build and track;
5. metadata, locales, and release content;
6. build, package, and validate;
7. dry-run, publish, receipt, and CI export.

Provider-specific flows extend the shared wizard:

- Android/Play Store: SDK/NDK/build-tools/bundletool, upload key generation/import, service-account guidance, package access, track/status, AAB/APK validation.
- iOS/App Store: Xcode, team, bundle id, provisioning, App Store Connect API key, TestFlight/App Review choice, build processing.
- Windows/Microsoft Store: MSIX identity, certificate source, Partner Center/msstore auth, private flights, public submission, staged rollout.
- S3/object storage: bucket, region, endpoint, credentials mode, ACL/visibility, cache headers, object plan, overwrite policy.
- GitHub Releases/static hosts/Docker/drives: provider-specific target, auth, idempotency, publish/rollback/status.

## 9. CI contract

CI must be non-interactive and scriptable.

CI flow:

```sh
fission readiness release \
  --target android \
  --format aab \
  --provider play-store \
  --track internal \
  --locale en-US \
  --json

fission package \
  --target android \
  --format aab \
  --release \
  --json

fission publish \
  --provider play-store \
  --artifact target/fission/release/android/aab/artifact-manifest.json \
  --track internal \
  --locale en-US \
  --yes \
  --json
```

CI must:

- use env/base64 secrets;
- fail only on provider-required or operation-required missing data;
- report Fission-recommended omissions as warnings;
- never assume `~/.fission/<app-name>`;
- never prompt;
- write JSON diagnostics and receipts.

### 9.1 Project-defined release workflows

Projects may define named release workflows in `fission.toml`. Workflows are
declarative recipes over Fission commands, not arbitrary shell scripts. Each
step can declare inspectable inputs, outputs, and readiness gates. Gates run
before the mutating command and block the step if they fail.

```toml
[release_workflows.beta]
commands = [
  "readiness release --target ios --format ipa --provider app-store --json",
]

[[release_workflows.beta.steps]]
command = "package --target ios --format ipa --release --json"
inputs = ["fission.toml", "release-content/metadata"]
outputs = ["target/fission/release/ios/ipa/artifact-manifest.json"]
readiness = [
  "readiness release --target ios --format ipa --provider app-store --json",
  "release-content validate --provider app-store --json",
]
```

`fission release-workflow run <name> --dry-run --json` prints the normalized
commands and writes a receipt without mutating provider state. A non-dry run
writes a workflow receipt only after explicit confirmation with `--yes`. The
receipt contains the command argv, readiness-gate results, declared inputs,
declared outputs, status, and exit codes. Workflow commands must be Fission
commands; arbitrary shell commands are rejected before execution.

## 10. Provider requirements

### 10.1 Google Play

Required support:

- service-account auth from env/base64 env;
- package access check;
- edit create/validate/commit with correct empty-body handling;
- AAB/APK upload;
- track update;
- release status;
- existing version-code preflight;
- release notes where provider requires or user selects them;
- screenshots/feature graphic/listing metadata push where available;
- tester/track support where available;
- clear handling of internal, closed, open, and production tracks.

### 10.2 App Store Connect

Required support:

- issuer id/key id/key material from approved secret sources;
- bundle id access check;
- IPA upload;
- build processing status;
- TestFlight assignment path;
- App Review submission path when supported;
- localizations/release notes/review info/assets where available;
- explicit distinction between upload, TestFlight, and App Review.

### 10.3 Microsoft Store

Required support:

- product id/package identity check;
- MSIX publish path;
- EXE/MSI package URL path where supported;
- private flight/public submission;
- draft vs committed submission state;
- staged rollout percentage where supported;
- no client secret in process arguments.

### 10.4 GitHub Releases

Required support:

- owner/repo/tag resolution;
- create/edit release;
- notes/notes file;
- draft/prerelease;
- asset upload;
- explicit replace-assets behavior;
- receipt with release URL and uploaded assets.

### 10.5 Static hosts

Required support:

- static artifact validation;
- provider setup/status;
- upload/deploy;
- preview URL;
- promote/rollback where provider supports it;
- custom-domain status where provider supports it.

### 10.6 Object/file storage

Required support:

- upload plan before mutation;
- destination prefix/path;
- MIME type;
- cache headers where supported;
- visibility policy;
- overwrite policy;
- receipt with object IDs/URLs.

### 10.7 Docker registry

Required support:

- image metadata;
- tags;
- login/readiness;
- push;
- manifest status;
- digest in receipt where available.

## 11. Diagnostics and receipts

Every failure must include:

- step id;
- provider/target/format;
- exact failing command/API call summary;
- status code when available;
- provider error body when available;
- rejected requirement;
- candidate remediation;
- path to logs;
- path to screenshot/semantic tree when UI automation failed.

Every publish/distribution action must write a receipt containing:

- release id;
- target;
- format;
- provider;
- track/channel;
- locales;
- artifact manifest path;
- artifact hash;
- version/build;
- uploaded asset list;
- provider deployment/submission/edit/build ids;
- canonical URL/preview URL where available;
- status;
- manual follow-up.

## 12. Implementation boundaries

The implementation should be split, but not duplicated.

Recommended internal boundaries:

- command core: manifest, targets, diagnostics, process/job/event types;
- package: target package builders and artifact manifests;
- release: release plan, release config, release content, signing policy, version/build sync;
- providers: provider-specific APIs and idempotency;
- UI: renderers over the shared plan;
- test drivers: terminal, native, browser, and CI verification.

The UI crate must not shell out independently to package/publish commands for behavior that exists in the shared engine. It may invoke the shared engine and render events.

## 13. Acceptance criteria

### 13.1 Shared workflow

- Direct CLI, guided CLI, TUI, windowed app, and CI all use the same release plan and job APIs.
- A check fixed in the shared engine appears fixed in all shells.
- A provider upload event has the same receipt shape regardless of shell.

### 13.2 Security

- No secret-bearing path/material can be written to `fission.toml`.
- Android JKS, Windows PFX/P12, App Store `.p8`, Play service-account JSON, tokens, and passwords are rejected if configured as project manifest values.
- Base64 env secrets are decoded only into secure temporary files and cleaned up.
- Generated project templates never imply that keystores/certs should be committed.

### 13.3 Version/build

- Android Play publish detects an already-used version code before artifact upload and offers a rebuild path in interactive mode.
- CI fails with a clear diagnostic if version/build must be changed.
- iOS and Windows version/build/package identity are synced before packaging.

### 13.4 Release content

- Store-bound guided flows present release notes, screenshots, localized metadata, privacy/review info, and tester/track setup.
- Provider-required content blocks publishing.
- Recommended content can be explicitly skipped with a visible warning and receipt entry.

### 13.5 UI

- The TUI and windowed app follow the same wizard model.
- The TUI is tested with terminal LiveTest screenshots.
- The windowed app is tested with Fission LiveTest screenshots.
- Windowed mode uses native file dialogs.
- Terminal mode uses a terminal-appropriate file picker.

### 13.6 Provider-specific

- Google Play upload validates/commits with correct HTTP semantics and records edit/version/track in the receipt.
- App Store upload records build status and next action.
- Microsoft Store publish distinguishes draft update, flight, submit, and certification.
- GitHub Releases handles existing releases and duplicate assets according to explicit config.
- S3/object upload prints an upload plan and respects overwrite policy.

### 13.7 Linux packaging

- `linux/appimage` produces a standards-conforming, architecture-specific
  AppImage and a complete artifact manifest from the shared staged package
  inputs.
- The packaged AppImage passes structural extraction validation and launch
  qualification on every declared Linux baseline.
- `linux/run` remains available and is not implemented as an AppImage alias.
- A project can select AppImage for portable graphical distribution and `.run`
  for installation, helpers, services, or explicit system integration without
  changing its application code.
- Enabling TUF/zsync output never permits the reconstructed AppImage to activate
  before its final TUF target length and hash are verified.

## 14. Current implementation status

The current implementation now shares the release workflow across command surfaces and records auditable package/distribution state. Remaining gaps are called out explicitly where behavior still depends on provider handoff or future target work:

- Linux `.run` packaging is implemented; `linux/appimage`, AppDir staging,
  pinned `appimagetool` integration, AppImage validation, and optional
  TUF-authenticated zsync sidecars remain implementation work under this RFC;
- Play Store metadata and release-content images are pushed during `fission publish`; Play Store `--track internal-sharing` uses the internal app sharing upload path for APK/AAB artifacts and returns the generated download URL without mutating a formal Play release track; release-content capture scenarios can wait for semantic/test/accessibility/widget-id/role/label selectors through the Fission test-control protocol and can drive selector actions such as tap, focus, scroll-into-view, fill, clear, toggle, and select before screenshots are captured; App Store Connect screenshots, app previews, and review attachments use direct asset reservation/upload/commit and require explicit display-type/preview-type directories such as `APP_IPHONE_67`, `APP_IPAD_PRO_3GEN_129`, or `IPHONE_67`; Microsoft Store screenshots/logos use direct listing asset SAS upload/commit; Microsoft Store trailers are still staged into auditable handoff manifests rather than fully uploaded through provider asset APIs;
- direct CLI, guided CLI, TUI, and windowed UI consume the shared release plan and publish workflow; release plans now include an explicit context block, distribution setup writes an action receipt, distribution publish/status/lifecycle operations return structured events retained in unique receipts, receipt-written events are retained in the receipt payload, non-dry-run publish observes provider status after upload, provider request/response summaries, upload plans, per-asset planned/uploaded events, and captured uploaded assets are reported as events, provider stdout/stderr retained in receipts is also split into bounded line events, direct distribution config, publish, and lifecycle failures write failed distribution receipts, package, metadata, release-content, and provider publish failures write failed workflow receipts with the stage event and error attached to the relevant receipt payload such as `release_metadata.error`, `release_content.error`, or `distribution.error`, Docker registry status inspects every configured tag or falls back to tags from the supplied docker-image artifact metadata and records per-tag manifest status plus manifest digest where Docker exposes it, S3 status can inspect the exact object keys planned from a supplied artifact manifest and reports `ok`, `partial`, or `missing` object state, S3, Google Drive, OneDrive, and Dropbox now emit per-item upload events plus chunk/session progress where the provider API is chunked, TUI/windowed wizard navigation uses the same state model with arrow-key step navigation, terminal file picker arrows/Enter/Escape, UI gates remain `not checked` until deterministic check results exist, and that receipt includes release id, version/build, artifact hashes, release-content assets, uploaded assets, uploaded byte totals, provider ids/URLs, provider status, manual follow-up, and omitted/skipped requirements; store and provider-CLI backends still expose provider-stage events rather than every underlying HTTP request;
- direct publish/readiness resolves missing target/format/track from the artifact manifest when available or from the same provider defaults used by the guided flow, JSON readiness output still exits non-zero when provider-required or operation-required checks block the run, and Microsoft Store readiness uses the selected Windows package format before an artifact exists so MSIX checks do not incorrectly require an MSI/EXE package URL;
- package readiness validates the selected application icon source and reports stale existing artifact manifests before expensive package work, release readiness includes signing input checks for Android, iOS, macOS, and Windows, generated Android targets include both APK and signed Play Store AAB package scripts, generated iOS targets include signed IPA package scripts that require explicit signing identity/provisioning inputs for release builds, generated Windows targets include full-trust MSIX manifests plus MSIX/MSI package scripts backed by `makeappx`, WiX, and approved Windows certificate sources, and package artifact manifests resolve version/build using the selected target, record source configuration hashes, record the generated `target/fission/icons/icon-manifest.json` path/hash/output count for targets that use app icons, record signing/notarization state where relevant without storing secret material, validate deterministic package structure for macOS `.app`, Android APK/AAB, iOS IPA, and Windows MSIX artifacts, macOS app bundles resolve `CFBundleShortVersionString`/`CFBundleVersion` from `[package.macos]` or the active release and package readiness validates the macOS bundle id, Windows MSIX appx manifests sync package identity/publisher/version before release packaging, Microsoft Store readiness validates the configured Windows package version before an artifact exists and validates the exact artifact package version once a manifest is available, Play Store readiness checks the exact artifact version code when an artifact exists, and App Store readiness checks the exact IPA build number where credentials permit provider lookup;
- install-smoke package validation no longer treats any `install-smoke.json` file as proof: the receipt must be valid JSON, match the artifact target/format, and report a passed status before the package validation check passes; Linux/Terminal `.run` packages now perform an embedded archive verify, temporary install, receipt check, and uninstall during packaging so their validation receipt is generated deterministically; static packages now also write `package-validation/load-smoke.json` so the package root, `index.html`, and document-like output are validated before the artifact manifest is finalized, while browser/device launch smoke remains part of the platform test path;
- distribution readiness now reads the artifact manifest validation state and blocks provider mutation when package validation recorded an error-level failure, verifies artifact/source hashes, and reports whether debug-symbol or crash-diagnostic secondary artifacts in the manifest are covered by the selected distribution provider, so publish flows cannot silently upload stale, failed, or unsupported release assets;
- GitHub Releases readiness now treats duplicate-asset behavior as an explicit recommended requirement: omitted `replace_assets` warns and publishing still defaults to fail-safe non-clobber behavior unless replacement is configured intentionally;
- release-config scalar mutators (`set`, `add-release`, `skip-requirement`, and `bump-build`) support `--dry-run`, `--yes`, and structured `--json` receipts, and `set` preserves/infers TOML integer and boolean scalars for release fields such as `app.build`, `package.android.version_code`, and provider overwrite flags, `bump-build --target windows` now advances the fourth Windows package-version component, `release-config version-state` reports the resolved local version/build against provider status, identifies Play/App Store monotonic build failures when provider status exposes comparable build numbers, and returns the next action such as `bump-build` or `upload-new-build`, provider metadata import supports `--dry-run` before writing local release files, signing import requires `--yes` and supports dry-run before rewriting signing references, and auth login/logout/import/rotate commands expose provider-owned credential guidance without restoring a Fission vault, so CI and guided shells use the same mutation semantics;
- guided and windowed local flows refuse to keep live references to selected secret files inside the project tree; the user must copy or move those files into the private `~/.fission/<app-name>/` workspace instead;
- release-config validation now checks app-level release identity and recommended app metadata (`name`, `id`/`app_id`, `publisher`, `homepage`, `support_url`, `privacy_url`, and `license`) so store/package guidance starts from the same release root, while the project parser accepts `[app].id` as a short alias for the existing `[app].app_id` field;
- `release-config lock` records the current provider metadata revision under `[release.provider_locks.<provider>]`, provider metadata import refreshes the same lock after importing remote state, and non-dry `release-config push`/`publish` verifies the lock plus referenced release files before mutating store metadata unless `--overwrite-remote` is passed explicitly; the guided shell carries the same `--overwrite-remote` decision into release-plan, dry-run, and publish workflow requests instead of silently dropping it;
- `release-config set` and dry-run receipts resolve TOML dotted-key field paths, including quoted segments such as `distribution."github-releases"."release.notes"`;
- `release-config write-file` addresses referenced release sidecar files by release id, kind, optional provider, and locale, supports `--dry-run`, `--yes`, and `--json`, and avoids requiring scripts to know physical release-content paths;
- `release-workflow run` supports declarative command recipes plus step-level inputs, outputs, and readiness gates, requires `--yes` for non-dry execution, and writes non-overwriting timestamped JSON receipts with normalized argv, gate results, statuses, and exit codes; publish workflow receipts are also non-overwriting when repeated runs finish in the same second;
- `release-workflow run` prevalidates every command and readiness gate before executing the first step, keeping workflows as declarative Fission recipes rather than arbitrary shell scripts;
- beta group/tester mutations and provider review replies require explicit `--yes` outside dry-run mode, App Store TestFlight build-to-group assignment is exposed through `fission beta distribute --provider app-store --group <group>` after build processing, App Review submission is exposed through `fission distribute promote --provider app-store --track app-store-review --artifact <manifest>` after build processing, and App Store status observes build processing, TestFlight beta groups, and review submissions, so tester, review, track, and build-assignment changes follow the same confirmation policy as publishing and metadata pushes;
- provider-specific capabilities are represented in the release plan as supported, manual, handoff, unsupported, or not-applicable; this must stay current as provider integrations become more complete;
- S3 uses provider streaming uploads, Google Drive/OneDrive use resumable upload sessions, Dropbox uploads stream from disk, and Google Play artifact/listing uploads plus App Store/Microsoft Store release-content asset uploads stream file bodies or upload ranges instead of loading full release artifacts into memory;
- Terminal target packaging is represented as `terminal/run`, while richer terminal package formats remain owned by the terminal-shell RFC;
- older lifecycle docs are superseded where they conflict with this RFC;
- some provider secret handling was historically manifest/path based and must stay removed.

## 15. Non-goals

- Fission does not reimplement provider portals. If a provider requires manual setup, Fission reports exact steps and verifies readiness afterward.
- Fission does not require users to provide optional marketing assets that the provider does not require.
- Fission does not hide release mutations behind `package`. Packaging builds artifacts; publishing/distribution mutates external systems.
- Fission does not maintain hidden project state for release readiness. State is in `fission.toml`, release-content files, local env/secrets, generated artifacts, and receipts.
