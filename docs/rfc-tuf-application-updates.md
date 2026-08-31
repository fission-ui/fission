# RFC: TUF-authenticated application updates

## Status

Proposed.

## Summary

Fission will provide one cross-platform application-update capability with The
Update Framework (TUF) as the authority for release discovery and update policy.
The actual transfer and installation mechanism remains platform-appropriate:

Package production, manifests, and distribution remain governed by the
[build, package, publish, and release workflow RFC](rfc-build-package-publish-workflow.md).

```text
TUF metadata
    authenticates available version and policy
        |
        v
platform delivery provider
    downloads or asks the platform store to deliver the update
        |
        v
TUF and platform verification
    authorize the candidate before activation
```

TUF answers:

- which release is current for this application, channel, platform, and
  architecture;
- whether it is optional, recommended, or required;
- which artifact or platform-store listing is authorized;
- the length and hashes of directly downloadable artifacts;
- whether metadata is current and consistently signed;
- how signing authority is delegated and rotated.

TUF does not dictate how bytes are transferred or how an operating system
installs an application. zsync, MSIX differential delivery, macOS binary deltas,
Google Play, the Apple App Store, and direct package installation are delivery
providers below that authority.

The architecture supports a range of application experiences:

- silently check and do nothing;
- show that a new release exists;
- let the user open the relevant store or download page;
- run a platform-managed flexible update with progress;
- require a platform-managed immediate update before continuing;
- directly download, verify, and replace an application where the platform and
  distribution model permit it.

Permissions, entitlements, native libraries, workers, and manifest declarations
for the more powerful mechanisms are included only when the application enables
the corresponding capability.

## Motivation

Fission produces multiple package formats with materially different update
facilities. Treating each package's conventional updater as an independent
authority would create inconsistent trust and policy:

- AppImage has conventional zsync update information but zsync is not a
  repository-security framework;
- Windows MSIX can update efficiently through Windows but does not express a
  Fission-wide release/channel policy;
- directly distributed macOS apps can replace themselves, while Mac App Store
  apps must use the store;
- Google Play exposes flexible and immediate in-app updates;
- an ordinary iOS App Store application cannot install its own executable
  replacement;
- direct APK installation requires a different permission and consent model
  from Play delivery.

The common capability must therefore standardize authenticated discovery,
policy, state, and application-facing actions without pretending every target
has the same installation power.

## Goals

- Use TUF as the single Fission update and release-policy authority.
- Authenticate update metadata before an application acts on it.
- Support optional, recommended, and required update presentation.
- Use native store and package facilities where they provide the best delivery
  and installation behavior.
- Support efficient differential delivery without making a delta format a trust
  authority.
- Verify a reconstructed direct-download artifact against its authoritative TUF
  target before activation.
- Preserve platform code-signing, package-identity, notarization, and installer
  checks in addition to TUF.
- Expose one declarative, asynchronous Fission API across interactive shells.
- Avoid requesting Android permissions, Apple entitlements, or native update
  dependencies unless the selected provider requires them.
- Leave the running application usable after compilation, network, metadata,
  transfer, verification, installation, or restart failures wherever platform
  policy allows.

## Non-goals

- Replacing the Apple App Store, Google Play, Microsoft Store, enterprise MDM,
  or alternative marketplace authorities.
- Allowing an iOS App Store application to replace its own executable.
- Treating zsync, a binary patch, HTTPS, or an embedded package signature as a
  substitute for TUF metadata.
- Giving applications a cross-platform promise of silent installation.
- Adding executable-code updates to static sites or SSR processes through an
  application widget.
- Circumventing store rules, platform consent, administrator policy, signing,
  sandboxing, or package ownership.
- Loading downloaded executable code as an ordinary Fission resource update.

## Terminology

**Release authority** is the trusted source that states which version and policy
are approved. TUF is the Fission release authority.

**Delivery provider** obtains or installs an approved release. It can be a
Fission HTTP downloader, zsync, Windows deployment, Sparkle-compatible delta
tooling, Google Play, an app store link, or Android `PackageInstaller`.

**Direct target** is an artifact whose bytes Fission can download and verify,
such as an AppImage, `.run`, `.msix`, application archive, `.pkg`, or APK.

**Store release** is a release whose final application bytes are delivered by a
platform store and are not necessarily available to the running app for TUF
hash verification.

**Release descriptor** is a signed TUF target that describes an available
version, update policy, supported platforms, and delivery providers.

**Required update** is application policy that prevents entry into normal
application functionality after an installable update is confirmed. It is not a
promise that every platform can silently install that update.

## Security authority

The client embeds an initial trusted TUF root. It persists and rotates trusted
root and role metadata according to the TUF client workflow. It verifies role
thresholds, versions, expiry, delegations, consistent snapshots, target length,
and target hashes before accepting an update.

The top-level or delegated Targets role signs release descriptors. The Timestamp
and Snapshot roles prevent stale or mixed repository views. Release targets are
partitioned by application and channel; platform or product delegation is used
only when separate signing authority is actually required.

TUF target metadata includes exact lengths and cryptographic hashes for directly
downloaded files. The client must reject a direct target whose final bytes do
not match. The TUF target-fetch workflow explicitly requires this verification:
<https://theupdateframework.github.io/specification/v1.0.28/>.

Platform verification is an additional requirement, not an alternative:

- Windows validates MSIX identity and signing;
- macOS validates the application or installer signature, expected Team ID, and
  applicable notarization policy;
- Android validates package name, version code, and signing lineage;
- Apple and Google stores validate packages delivered through their stores.

An update is acceptable only when both the Fission/TUF policy and the applicable
platform policy accept it.

## Repository model

A release descriptor is a small signed TUF target. A representative logical
shape is:

```json
{
  "schema": 1,
  "application_id": "com.example.notes",
  "channel": "stable",
  "version": "2.4.0",
  "build": 20400,
  "published_at": "2026-08-31T12:00:00Z",
  "importance": "recommended",
  "minimum_supported_build": 20100,
  "platforms": {
    "linux-aarch64": {
      "provider": "direct-appimage",
      "target": "linux/aarch64/notes-2.4.0.AppImage",
      "transfer": {
        "kind": "zsync",
        "control_target": "linux/aarch64/notes-2.4.0.AppImage.zsync"
      }
    },
    "windows-x86_64": {
      "provider": "windows-msix",
      "target": "windows/x86_64/notes-2.4.0.msix",
      "appinstaller_target": "windows/notes.appinstaller"
    },
    "macos-aarch64": {
      "provider": "direct-macos",
      "target": "macos/aarch64/notes-2.4.0.tar.zst"
    },
    "android": {
      "provider": "google-play",
      "package": "com.example.notes",
      "update_mode": "flexible"
    },
    "ios": {
      "provider": "apple-app-store",
      "store_id": "1234567890"
    }
  }
}
```

This is illustrative rather than a frozen wire format. The eventual schema must
use platform-native monotonic build numbers for comparisons; a display version
string is not an ordering authority.

For a direct target, TUF Targets metadata separately contains the artifact's
length and hashes. Transfer metadata never replaces that target entry.

For a store release, TUF authenticates Fission's announcement, channel,
importance, minimum supported build, store identity, and intended UX. The store
authenticates and delivers the actual binary. Fission must describe this
accurately: it cannot claim to have verified store-delivered bytes against a TUF
hash it never obtained.

## Publication ordering

A release descriptor must not be published merely because a package was
submitted to a store. Store review, phased rollout, regional availability, and
device eligibility can delay installation.

Publication proceeds in this order:

1. Build and qualify the exact candidate.
2. Publish direct artifacts or submit store packages.
3. Confirm each referenced artifact or store release is retrievable for the
   intended channel, region, and cohort.
4. Generate optional transport metadata and delta artifacts.
5. Add direct artifacts, delta/control files, and the release descriptor to TUF
   Targets metadata.
6. Publish Snapshot metadata.
7. Publish Timestamp metadata last.

The application reports an update as actionable only when TUF authorizes it and
the selected platform provider confirms that the current installation can
obtain it. This prevents a required-update screen from trapping a user whose
store cohort has not received the release.

## Common client state

The framework exposes one state machine independent of delivery provider:

```text
Idle
Checking
Current
Available
AwaitingConsent
Downloading
Downloaded
Verifying
ReadyToInstall
Installing
RestartRequired
Completed
Failed
```

`Available` carries:

- current and available display versions;
- current and available monotonic builds;
- optional, recommended, or required importance;
- release notes reference;
- selected delivery provider;
- whether progress, cancellation, background download, direct installation, or
  restart is supported;
- provider confirmation that the release is installable for this device;
- a bounded diagnostic when policy and platform availability disagree.

The app decides how to present optional and recommended updates. A required
decision may be wrapped in a Fission route/surface boundary so protected
application content is not constructed while the update gate is active.

Metadata and update work is asynchronous. Applications request it through
actions/effects or an update capability and retain the result in normal state.
No network check or installer invocation occurs during widget conversion.

## Candidate application API

The exact API requires separate approval, but the intended responsibility split
is:

```rust
UpdateManager::check()
UpdateManager::start(UpdateMode::PlatformRecommended)
UpdateManager::cancel()
UpdateManager::complete_installation()
UpdateManager::open_store()
```

Applications receive typed state and progress as action input. A high-level
widget may provide the standard optional, recommended, required, progress,
failure, and restart presentation, while applications remain free to build their
own UI over the same capability.

Provider selection is fixed by the installed distribution identity, not guessed
solely from the operating system. A macOS App Store build and a directly
distributed macOS build need different providers; the same is true for Google
Play and directly installed APKs.

## Transfer strategies

Transport is selected after TUF has authenticated the target:

```text
FullDownload
Zsync
BinaryDelta
PlatformManaged
```

`FullDownload` is required as the initial universal implementation and as the
fallback for failed or unavailable delta transfer.

`Zsync` authenticates the `.zsync` control file as its own TUF target, uses the
current artifact as a local seed, and reconstructs the new target with HTTP
range requests. The final reconstructed artifact must match the new artifact's
TUF length and hash.

`BinaryDelta` authenticates a patch as a TUF target and binds it to exact source
and destination artifact hashes. It applies only when the installed source hash
matches. The reconstructed destination must match the full destination target's
TUF length and hash. A failure falls back to `FullDownload`.

`PlatformManaged` delegates bytes and installation to an operating-system store
or package service after TUF has approved the release policy. Fission reports
the platform's progress and result where an API exposes them.

## Linux

### AppImage

AppImage uses optional zsync as a transport optimization beneath TUF. Fission
does not embed conventional AppImage update information by default because a
generic AppImage updater could then follow an update route that bypasses the
Fission TUF policy.

Fission publishes the AppImage and `.zsync` control file as separate TUF
targets. After reconstruction it verifies the final AppImage against TUF, writes
it beside the current file, sets executable permissions, fsyncs the file and
parent directory where available, and atomically replaces the selected path.
The old executable remains available as a bounded rollback copy until the new
version successfully starts or the retention policy expires.

If the AppImage path is not writable, the provider requests a user-selected
writable destination rather than escalating privileges implicitly.

### `.run`

The initial `.run` provider downloads the complete TUF target and invokes its
verified installer flow. `.run` remains appropriate for installed applications,
Terminal packages, services, helpers, desktop integration, and machines without
usable FUSE support.

A future `.run` delta format is not required while full downloads are acceptable.
The installer receipt and installed application identity, rather than the
self-extracting archive alone, determine the currently installed version.

## Windows

### MSIX and App Installer

MSIX already contains an `AppxBlockMap.xml` with hashes for 64 KiB blocks.
Windows compares installed and new packages and downloads changed blocks rather
than requiring Fission to introduce a zsync equivalent. Microsoft documents that
the mechanism works between package versions and reuses unchanged blocks:
<https://learn.microsoft.com/en-us/windows/msix/app-package-updates>.

An `.appinstaller` file can configure checks on launch or in the background,
prompts, silent application, and activation-blocking required updates. Windows
also supports this for non-Store MSIX distribution:
<https://learn.microsoft.com/en-us/windows/msix/app-installer/auto-update-and-repair--overview>.

Fission supports two modes:

1. **TUF-controlled MSIX:** Fission authenticates the release and direct MSIX or
   App Installer target, then invokes Windows deployment. This is the default
   when the application promises TUF-controlled updates.
2. **Windows-managed MSIX:** the `.appinstaller` configuration lets Windows
   check independently. This is an explicit alternative policy because Windows
   may discover and install a release without consulting the running Fission TUF
   client.

The strict TUF mode must not silently enable a second independent automatic
App Installer path. Windows package signature and identity checks remain
mandatory in both modes.

### Microsoft Store

For Store-distributed MSIX, Fission authenticates release policy through TUF and
uses the Store update APIs for availability, download, consent, installation,
and progress where supported. Windows documents APIs for checking, downloading,
and requesting installation of Store-published updates:
<https://learn.microsoft.com/en-us/windows/msix/store-developer-package-update>.

The Store controls final bytes and installation. No Fission delta transport is
needed.

### EXE and MSI

The initial provider performs a full TUF-authenticated installer download.
Windows Installer patch (`.msp`) support may later be modeled as authenticated
`BinaryDelta` targets, but it is not required for the first complete update
capability. The installer must preserve expected publisher identity and provide
its ordinary elevation UI when system changes require it.

## macOS

### Direct distribution

The first provider downloads a complete TUF target containing a signed and
notarized application archive or installer. Before activation it verifies:

- TUF target length and hash;
- Apple code signature validity;
- expected bundle identifier;
- expected Team ID/designated requirement;
- applicable notarization and hardened-runtime policy.

For a directly distributed `.app`, the updater stages the new bundle on the same
volume, verifies it before replacing anything, terminates or coordinates restart
with the running app, and atomically exchanges/renames the bundle where the
filesystem permits. It retains a bounded rollback copy until successful launch.

For `.pkg`, installation is delegated to the system Installer and may require
authorization. Fission does not promise silent installation.

The macOS equivalent to zsync is an optional authenticated binary delta. Sparkle
provides a mature delta format and `BinaryDelta` tooling; each delta upgrades
one known old application to one new application and falls back to the full
release if it cannot apply:
<https://sparkle-project.org/documentation/delta-updates/>.

Fission may use the compatible delta tooling without adopting Sparkle's appcast
or signature system as a second update authority. The delta is a TUF target,
names its exact source and destination hashes, and the reconstructed complete
archive/bundle must match the destination TUF target before installation.

Adopting Sparkle libraries or tools is a substantial native dependency decision
and requires explicit approval after license, architecture, signing, sandbox,
and build implications are qualified.

### Mac App Store

Mac App Store applications use the store for executable updates. Fission uses
TUF to determine and present policy, then opens or invokes the available store
experience. It does not embed a direct updater in the sandboxed Store build.
Apple documents that App Store applications update through the App Store and
that non-Store Mac applications obtain updates from their developer:
<https://support.apple.com/en-us/102629>.

The App Store owns optimized delivery, so a Fission zsync or binary-delta layer
is unnecessary.

## Android

### Google Play

Google Play In-App Updates is the default Android provider for Play-distributed
applications. It exposes:

- **Flexible update:** download while the app remains usable, progress events,
  then an application-requested restart/installation completion;
- **Immediate update:** fullscreen, consented update flow that blocks normal use
  and lets Google Play install and restart the application.

Google documents both flows and requires the app to check whether a requested
mode is currently allowed:
<https://developer.android.com/guide/playcore/in-app-updates>.

TUF determines Fission's optional/recommended/required policy. The Play provider
then confirms that the release is available to this device and rollout cohort.
A required policy maps to an immediate flow only when Play permits it. Flexible
and immediate progress is surfaced through the common state machine.

Google Play owns split APK generation, optimized/differential transfer, package
verification, installation, and restart. Fission must not add zsync around that
delivery.

The Play update capability adds the required Play Core dependency only when
enabled. It does not request `REQUEST_INSTALL_PACKAGES`.

### Direct APK

A separately enabled direct-APK provider may download a TUF-authenticated APK
and submit it through Android `PackageInstaller`. It verifies TUF length/hash,
package name, version code, and signing lineage before committing the session.

This provider adds `android.permission.REQUEST_INSTALL_PACKAGES` to the generated
manifest only when explicitly enabled. Android may require user action and
reports `STATUS_PENDING_USER_ACTION`; every implementation must handle that path
even on releases where self-update can sometimes proceed without another prompt.
The platform contract is documented by Android's `PackageInstaller` APIs:
<https://developer.android.com/reference/android/content/pm/PackageInstaller>.

Fission does not request privileged or device-owner permissions to manufacture
a silent updater. Managed-device installation belongs to an explicit enterprise
provider implemented through the organization's device-management authority.

The initial direct provider downloads the complete APK. An optional later
binary-delta provider may reconstruct the APK, but it must authenticate the
patch through TUF and verify the complete reconstructed APK against the
destination target before handing it to `PackageInstaller`.

### Other Android stores

Other stores can implement the platform provider interface. The minimum adapter
opens the authenticated store listing. Richer adapters may expose store-specific
availability, progress, or in-app update APIs. No proprietary dependency or
permission is added until that provider is selected.

## iOS and iPadOS

An ordinary App Store application cannot download and install its own executable
replacement. Apple requires applications to remain self-contained and prohibits
downloading, installing, or executing code that introduces or changes app
functionality. The current App Review Guidelines state this in section 2.5.2:
<https://developer.apple.com/app-store/review/guidelines/>.

The normal provider therefore supports:

1. TUF-authenticated release and minimum-version checks.
2. Optional, recommended, or required in-app presentation.
3. A user action that opens the app's authenticated App Store product page.
4. Rechecking the installed build after the application returns or restarts.

There is no promise of download progress, in-app binary replacement, silent
installation, or immediate restart. Apple controls automatic and manual App
Store updates:
<https://support.apple.com/en-us/102629>.

A required update can gate normal application routes, but only after the
provider confirms that the store release is available to the device's region
and storefront as far as the available APIs permit. If the app cannot prove the
release is obtainable, it must show a bounded availability diagnostic or defer
the hard gate rather than trapping the user permanently.

Opening an App Store product page requires no special installation permission.
The base iOS update-check capability therefore adds no update entitlement.

TestFlight, custom applications, enterprise/MDM, alternative marketplaces, and
eligible regional distribution mechanisms are separate providers. They may
open their owning distribution application or expose provider-specific status,
but Fission must not assume that the application itself can install the update.
Entitlements or frameworks are added only by the explicitly selected provider.

## Required-update behavior

`minimum_supported_build` is signed release policy. It is not inferred from a
human-readable semantic version or from an unauthenticated server response.

The default required-update rule is:

```text
trusted, unexpired TUF metadata says current build is unsupported
AND
the selected platform provider confirms an obtainable update
    => required update gate may activate
```

If TUF metadata is unavailable or expired, the client may display last-known
information but must not newly escalate an optional update into a required gate.
The default application-availability policy is fail-open for offline use;
security-sensitive applications may explicitly choose a stricter policy. A
backend remains free to enforce its own authenticated minimum client version for
network APIs independently.

A previously downloaded direct target can still be installed after metadata
refresh fails only when its target metadata was valid when persisted and the TUF
client workflow permits the cached target. Expiry and rollback rules must not be
invented by the UI layer.

Required-update UI always provides:

- the required and installed versions;
- the reason or release-note summary supplied by trusted application content;
- an update/open-store action when obtainable;
- progress or platform handoff state;
- retry and a clear diagnostic for unavailable delivery;
- access to any explicitly allowed offline/export/recovery function.

## Capability and permission model

The base capability contains TUF metadata verification, update policy, state,
and application-facing actions. It uses Fission's normal network and persistent
state facilities and installs no package-management permission.

Delivery providers are separately selected at build time. Representative
capabilities are:

```text
updates                    TUF checking and common state only
updates-linux-appimage     direct AppImage replacement, optional zsync
updates-linux-run          verified .run installer handoff
updates-windows-msix       Windows deployment/App Installer integration
updates-windows-store      Microsoft Store integration
updates-macos-direct       direct signed application/pkg updates
updates-macos-store        Mac App Store handoff
updates-android-play       Play In-App Updates dependency
updates-android-apk        REQUEST_INSTALL_PACKAGES + PackageInstaller
updates-ios-store          App Store handoff
```

Names are illustrative. The generator validates that the chosen provider matches
the package/distribution identity. Unsupported combinations fail during project
generation or compilation rather than reaching a runtime "not supported"
branch after the application has shipped.

No provider may quietly fall back from a store build to direct installation or
from TUF-authenticated delivery to an unauthenticated download URL.

## Atomicity, restart, and rollback

Direct providers stage updates without modifying the active installation. They
verify before activation and keep the currently running generation usable after
download, patch, or verification failure.

Activation rules are platform-specific but share these invariants:

- never overwrite the only working artifact before the candidate is verified;
- perform same-volume atomic rename/exchange where supported;
- fsync durable files and directories where the platform exposes meaningful
  durability;
- preserve application data independently from executable rollback;
- keep only a bounded number or size of rollback artifacts;
- record candidate, activation, first-launch, and rollback state without storing
  signing secrets;
- do not report completion until the platform installation or first-launch
  contract has actually completed.

Store providers report handoff, download, installation, and restart states only
to the degree their platform API proves them. Opening a store page is not the
same as installing an update.

## Failure behavior

- TUF signature, threshold, rollback, expiry, length, or hash failure aborts the
  cycle and leaves the active application unchanged.
- A delta mismatch falls back to the authenticated full target.
- A store release not yet available to the current cohort remains pending and
  does not activate a required gate.
- User-declined consent returns to an available/deferred state unless platform
  immediate-update UX owns the screen.
- Package identity or platform signature mismatch is fatal for that candidate.
- Insufficient disk space, unwritable destination, installer denial, and restart
  failure produce typed diagnostics and preserve the working version.
- An unknown provider never becomes an unauthenticated browser download.

## Privacy

Update checks disclose application identity, channel, current build, platform,
and architecture unless metadata is served as static anonymous files. The base
protocol should use cacheable static TUF metadata and avoid stable installation
identifiers.

Phased rollout, if later required, should use a locally generated random cohort
value and deterministic policy without sending a durable device identifier to
the repository. Store-controlled rollout remains the store's responsibility.

Update diagnostics must not log authentication tokens, signed private
repository URLs, device identifiers, or complete query strings containing
credentials.

## Qualification

The capability requires focused and end-to-end coverage for:

- trusted root bootstrap and root rotation;
- signature thresholds and delegated targets;
- expired timestamp/snapshot/targets metadata;
- metadata and target rollback;
- mix-and-match and inconsistent repository states;
- wrong target length or hash;
- channel, platform, architecture, package identity, and monotonic build
  selection;
- optional, recommended, and required state transitions;
- store cohort not-yet-available behavior;
- full-download success and interruption;
- zsync/delta success, source mismatch, corrupted patch, and full fallback;
- final reconstructed target verification;
- platform signature and application identity mismatch;
- insufficient storage and unwritable installation;
- user consent, denial, cancellation, progress, and restart;
- atomic replacement and rollback after failed first launch;
- build-generated permissions and dependencies being absent when providers are
  disabled;
- real packaged artifacts on every supported native architecture.

Store integrations require sandbox/test-track qualification through the actual
store APIs. A mocked provider does not prove that a Play immediate update, App
Store handoff, Microsoft Store update, or platform installer works.

## Delivery sequence

1. Implement and qualify the TUF repository schema, publisher, embedded-root
   bootstrap, client metadata state, and common update state machine.
2. Implement full-download direct targets and an application-controlled
   notification/open-URL provider.
3. Add Linux AppImage atomic replacement and `.run` installer handoff.
4. Add Windows MSIX/App Installer and Store providers, using Windows differential
   delivery rather than a Fission delta format.
5. Add macOS direct full updates and Mac App Store handoff.
6. Add Google Play flexible/immediate flows and iOS App Store presentation.
7. Add direct Android APK installation as an explicitly permissioned provider.
8. Add zsync and macOS binary deltas as optional transport optimizations after
   full authenticated updates are correct.

This ordering establishes one trustworthy update story before optimizing bytes.
No delta mechanism is allowed to become an alternate release authority.

## Decision

Fission will pursue TUF independently as the cross-platform update authority.
It will not build its trust model on AppImage update metadata, zsync, Sparkle
appcasts, App Installer polling, or store version responses.

Platform delivery is deliberately heterogeneous:

| Platform/package | Efficient delivery | Application control |
| --- | --- | --- |
| Linux AppImage | Optional zsync beneath TUF | Complete direct download, verify, replace, restart |
| Linux `.run` | Full target initially | Verify and invoke installer; installer owns integration |
| Windows MSIX | Native 64 KiB block differential delivery | Check/prompt/background/block through Windows APIs and policy |
| Windows Store | Store-managed optimized delivery | In-app check/download/install APIs where available |
| Windows EXE/MSI | Full target initially; authenticated MSP later | Installer/elevation controlled by Windows |
| Direct macOS `.app` | Full target; optional authenticated binary delta | Verify signature/notarization, replace, restart, rollback |
| Mac App Store | Store-managed delivery | Notify and hand off to store |
| Google Play | Play-managed optimized delivery | Flexible or immediate in-app update with progress |
| Direct Android APK | Full target; authenticated delta later | PackageInstaller with explicit capability and required consent handling |
| iOS/iPadOS App Store | App Store-managed delivery | Secure TUF check, notify/require in UI, open App Store; no self-install |

TUF determines what is trusted. The provider determines what the platform can
actually download and install. Fission exposes the strongest coherent control
available without claiming powers the operating system does not grant.
