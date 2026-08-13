//! Exact, verified delivery of Fission-maintained Skia and CanvasKit artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const LOCK_SCHEMA_VERSION: u32 = 1;
const ARTIFACT_SCHEMA_VERSION: u32 = 1;
const CACHE_SCHEMA: &str = "v1";
const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ARTIFACT_FILES: usize = 8_192;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

/// Exact Skia source revision selected by this Fission release.
pub const SKIA_REVISION: &str = "cf5c36972b73698eb3939cda147ea47152670312";
/// Exact native bridge ABI selected by this Fission release.
pub const BRIDGE_ABI_VERSION: u32 = 14;
/// CanvasKit wire protocol consumed by this Fission release.
pub const CANVASKIT_PROTOCOL_VERSION: u32 = 1;
/// Production CanvasKit artifact profile served by interactive Web apps.
pub const CANVASKIT_PROFILE: &str = "canvaskit-production";
/// Interactive Web target used by CanvasKit artifacts.
pub const CANVASKIT_TARGET: &str = "wasm32-unknown-unknown";
/// Fission release version whose artifact identities this crate accepts.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const BUNDLED_LOCK_JSON: &[u8] = include_bytes!("../artifacts.lock.json");

/// Selects the native or browser artifact contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub enum ArtifactKind {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "canvaskit")]
    CanvasKit,
}

impl ArtifactKind {
    fn cache_key(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::CanvasKit => "canvaskit",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Native => "Skia",
            Self::CanvasKit => "CanvasKit",
        }
    }

    fn local_override(self) -> &'static str {
        match self {
            Self::Native => "FISSION_SKIA_ARTIFACT_DIR",
            Self::CanvasKit => "FISSION_CANVASKIT_ARTIFACT_DIR",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactLock {
    schema_version: u32,
    fission_version: String,
    skia_revision: String,
    bridge_abi_version: u32,
    provenance: ProvenancePolicy,
    artifacts: Vec<LockedArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenancePolicy {
    repository: String,
    predicate_type: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockedArtifact {
    kind: ArtifactKind,
    artifact_id: String,
    target: String,
    profile: String,
    qualified: bool,
    url: String,
    archive_sha256: String,
    archive_size: u64,
    manifest_sha256: String,
    #[serde(default)]
    web_protocol_version: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct InstalledNativeManifest {
    schema_version: u32,
    artifact_id: String,
    qualified: bool,
    fission_version: String,
    skia: InstalledSkia,
    bridge_abi_version: u32,
    target: String,
    profile: String,
    files: Vec<InstalledFile>,
}

#[derive(Debug, Deserialize)]
struct InstalledSkia {
    revision: String,
}

#[derive(Debug, Deserialize)]
struct InstalledCanvasKitManifest {
    schema_version: u32,
    artifact_id: String,
    qualified: bool,
    fission_version: String,
    source: InstalledSource,
    abi: InstalledWebAbi,
    target: String,
    profile: String,
    assets: BTreeMap<String, InstalledAsset>,
    files: Vec<InstalledFile>,
}

#[derive(Debug, Deserialize)]
struct InstalledSource {
    revision: String,
}

#[derive(Debug, Deserialize)]
struct InstalledWebAbi {
    bridge_abi_version: u32,
    web_protocol_version: u32,
}

#[derive(Debug, Deserialize)]
struct InstalledAsset {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Clone, Copy, Debug)]
struct ResolveRequest<'a> {
    kind: ArtifactKind,
    lock_json: &'a [u8],
    fission_version: &'a str,
    skia_revision: &'a str,
    bridge_abi_version: u32,
    target: &'a str,
    profile: &'a str,
    cache_root: &'a Path,
    offline: bool,
}

fn resolve(request: ResolveRequest<'_>) -> Result<PathBuf, String> {
    resolve_with(request, download_https)
}

/// Resolves an exact artifact from the lock bundled with this crate release.
pub fn resolve_bundled(
    kind: ArtifactKind,
    target: &str,
    profile: &str,
    cache_root: &Path,
    offline: bool,
) -> Result<PathBuf, String> {
    resolve(ResolveRequest {
        kind,
        lock_json: BUNDLED_LOCK_JSON,
        fission_version: env!("CARGO_PKG_VERSION"),
        skia_revision: SKIA_REVISION,
        bridge_abi_version: BRIDGE_ABI_VERSION,
        target,
        profile,
        cache_root,
        offline,
    })
}

/// Verifies an explicit local artifact tree against this release's identity.
///
/// Callers must require an explicit development opt-in before passing
/// `allow_unqualified = true`.
pub fn verify_local(
    root: &Path,
    kind: ArtifactKind,
    target: &str,
    profile: &str,
    allow_unqualified: bool,
) -> Result<(), String> {
    let cache_root = Path::new(".");
    let request = ResolveRequest {
        kind,
        lock_json: BUNDLED_LOCK_JSON,
        fission_version: env!("CARGO_PKG_VERSION"),
        skia_revision: SKIA_REVISION,
        bridge_abi_version: BRIDGE_ABI_VERSION,
        target,
        profile,
        cache_root,
        offline: true,
    };
    verify_installed(root, request, None, !allow_unqualified)
}

/// Checks the bundled selection document without resolving an artifact.
pub fn validate_bundled_lock() -> Result<(), String> {
    let lock = parse_lock(
        BUNDLED_LOCK_JSON,
        env!("CARGO_PKG_VERSION"),
        SKIA_REVISION,
        BRIDGE_ABI_VERSION,
    )?;
    let cache_root = Path::new(".");
    let mut identities = BTreeSet::new();
    for entry in &lock.artifacts {
        let identity = (
            entry.kind.cache_key(),
            entry.target.as_str(),
            entry.profile.as_str(),
        );
        if !identities.insert(identity) {
            return Err(format!(
                "bundled artifact lock contains duplicate {} entries for {}/{}",
                entry.kind.display_name(),
                entry.profile,
                entry.target
            ));
        }
        validate_locked_artifact(
            entry,
            ResolveRequest {
                kind: entry.kind,
                lock_json: BUNDLED_LOCK_JSON,
                fission_version: env!("CARGO_PKG_VERSION"),
                skia_revision: SKIA_REVISION,
                bridge_abi_version: BRIDGE_ABI_VERSION,
                target: &entry.target,
                profile: &entry.profile,
                cache_root,
                offline: true,
            },
        )?;
    }
    Ok(())
}

fn resolve_with<F>(request: ResolveRequest<'_>, fetch: F) -> Result<PathBuf, String>
where
    F: FnOnce(&str, u64, &Path) -> Result<(), String>,
{
    let entry = select_locked_artifact(request)?;
    let cache_path = cache_path(request, &entry)?;
    if cache_path.exists() {
        verify_installed(&cache_path, request, Some(&entry), true).map_err(|error| {
            format!(
                "cached {} artifact {} is corrupt: {error}; remove only this cache entry and retry: {}",
                request.kind.display_name(),
                entry.artifact_id,
                cache_path.display()
            )
        })?;
        return Ok(cache_path);
    }
    if request.offline {
        return Err(format!(
            "offline mode has no exact verified {} artifact for {}/{} at {}; pre-populate that cache entry or set {} to a verified local artifact",
            request.kind.display_name(),
            request.profile,
            request.target,
            cache_path.display(),
            request.kind.local_override(),
        ));
    }

    let cache_parent = cache_path
        .parent()
        .ok_or_else(|| "artifact cache path has no parent".to_owned())?;
    fs::create_dir_all(cache_parent).map_err(|error| {
        format!(
            "failed to create artifact cache {}: {error}",
            cache_parent.display()
        )
    })?;
    let staging = TemporaryDirectory::new(cache_parent)?;
    let archive_path = staging.path().join("artifact.tar.gz");
    fetch(&entry.url, entry.archive_size, &archive_path)?;
    verify_archive_file(&archive_path, &entry)?;

    let extracted = staging.path().join("unpacked");
    fs::create_dir(&extracted).map_err(|error| {
        format!(
            "failed to create private artifact extraction directory {}: {error}",
            extracted.display()
        )
    })?;
    let artifact_root = extract_archive(&archive_path, &extracted, &entry.artifact_id)?;
    verify_installed(&artifact_root, request, Some(&entry), true)?;

    match fs::rename(&artifact_root, &cache_path) {
        Ok(()) => {}
        Err(error) if cache_path.exists() => {
            verify_installed(&cache_path, request, Some(&entry), true).map_err(|winner_error| {
                format!(
                    "another process populated {} while this build was resolving it, but the published cache entry is invalid: {winner_error} (rename error: {error})",
                    cache_path.display()
                )
            })?;
        }
        Err(error) => {
            return Err(format!(
                "failed to atomically publish verified artifact to {}: {error}",
                cache_path.display()
            ));
        }
    }
    Ok(cache_path)
}

pub fn cache_root_from_environment() -> Result<PathBuf, String> {
    if let Some(explicit) = env::var_os("FISSION_SKIA_CACHE_DIR") {
        return nonempty_path(explicit, "FISSION_SKIA_CACHE_DIR");
    }
    if let Some(cargo_home) = env::var_os("CARGO_HOME") {
        return Ok(nonempty_path(cargo_home, "CARGO_HOME")?.join("fission/skia"));
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(nonempty_path(home, "HOME")?.join(".cargo/fission/skia"));
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        return Ok(nonempty_path(profile, "USERPROFILE")?.join(".cargo/fission/skia"));
    }
    Err(
        "cannot locate a persistent Skia artifact cache; set FISSION_SKIA_CACHE_DIR or CARGO_HOME"
            .to_owned(),
    )
}

pub fn offline_from_environment() -> Result<bool, String> {
    let mut offline = false;
    for variable in [
        "FISSION_SKIA_OFFLINE",
        "FISSION_CANVASKIT_OFFLINE",
        "CARGO_NET_OFFLINE",
    ] {
        if let Some(value) = env::var_os(variable) {
            let value = value
                .to_str()
                .ok_or_else(|| format!("{variable} is not valid UTF-8"))?;
            offline |= match value.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" => true,
                "0" | "false" | "no" => false,
                _ => Err(format!(
                    "{variable} must be one of 1, 0, true, false, yes, or no"
                ))?,
            };
        }
    }
    Ok(offline)
}

/// Reads one optional boolean environment switch using the resolver vocabulary.
pub fn boolean_from_environment(variable: &str) -> Result<bool, String> {
    let Some(value) = env::var_os(variable) else {
        return Ok(false);
    };
    let value = value
        .to_str()
        .ok_or_else(|| format!("{variable} is not valid UTF-8"))?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => Err(format!(
            "{variable} must be one of 1, 0, true, false, yes, or no"
        )),
    }
}

fn select_locked_artifact(request: ResolveRequest<'_>) -> Result<LockedArtifact, String> {
    let lock = parse_lock(
        request.lock_json,
        request.fission_version,
        request.skia_revision,
        request.bridge_abi_version,
    )?;
    let mut matches = lock.artifacts.into_iter().filter(|entry| {
        entry.kind == request.kind
            && entry.target == request.target
            && entry.profile == request.profile
    });
    let entry = matches.next().ok_or_else(|| {
        format!(
            "no published {} artifact is locked for {}/{} in fission-skia-artifacts {}; use an explicitly verified {} or the documented source-build path",
            request.kind.display_name(),
            request.profile,
            request.target,
            request.fission_version,
            request.kind.local_override(),
        )
    })?;
    if matches.next().is_some() {
        return Err(format!(
            "bundled artifact lock contains duplicate {} entries for {}/{}",
            request.kind.display_name(),
            request.profile,
            request.target
        ));
    }
    validate_locked_artifact(&entry, request)?;
    Ok(entry)
}

fn parse_lock(
    raw: &[u8],
    fission_version: &str,
    skia_revision: &str,
    bridge_abi_version: u32,
) -> Result<ArtifactLock, String> {
    let lock: ArtifactLock = serde_json::from_slice(raw)
        .map_err(|error| format!("failed to parse the bundled Skia artifact lock: {error}"))?;
    if lock.schema_version != LOCK_SCHEMA_VERSION {
        return Err(format!(
            "unsupported bundled Skia artifact lock schema {}",
            lock.schema_version
        ));
    }
    if lock.fission_version != fission_version {
        return Err(format!(
            "bundled Skia artifact lock is for Fission {}, not {}",
            lock.fission_version, fission_version
        ));
    }
    if lock.skia_revision != skia_revision {
        return Err("bundled Skia artifact lock has the wrong pinned Skia revision".to_owned());
    }
    if lock.bridge_abi_version != bridge_abi_version {
        return Err(format!(
            "bundled Skia artifact lock has bridge ABI {}, not {}",
            lock.bridge_abi_version, bridge_abi_version
        ));
    }
    if lock.provenance.repository != "fission-ui/fission"
        || lock.provenance.predicate_type != "https://slsa.dev/provenance/v1"
    {
        return Err(
            "bundled Skia artifact lock does not select the required fission-ui/fission SLSA v1 provenance policy"
                .to_owned(),
        );
    }

    Ok(lock)
}

fn validate_locked_artifact(
    entry: &LockedArtifact,
    request: ResolveRequest<'_>,
) -> Result<(), String> {
    if !entry.qualified {
        return Err(format!(
            "published {} artifact {} is not marked production-qualified in the bundled lock",
            request.kind.display_name(),
            entry.artifact_id
        ));
    }
    let expected_id = expected_artifact_id(request, entry.web_protocol_version)?;
    if entry.artifact_id != expected_id {
        return Err(format!(
            "locked {} artifact id {} does not match expected {expected_id}",
            request.kind.display_name(),
            entry.artifact_id
        ));
    }
    if !entry.url.starts_with("https://")
        || entry.url.chars().any(char::is_whitespace)
        || entry.url.contains('#')
    {
        return Err(format!(
            "locked artifact {} must use a canonical HTTPS URL without whitespace or a fragment",
            entry.artifact_id
        ));
    }
    validate_sha256(&entry.archive_sha256, "locked archive")?;
    validate_sha256(&entry.manifest_sha256, "locked artifact manifest")?;
    if entry.archive_size == 0 || entry.archive_size > MAX_ARCHIVE_BYTES {
        return Err(format!(
            "locked artifact archive size {} is outside the supported 1..={MAX_ARCHIVE_BYTES} byte range",
            entry.archive_size
        ));
    }
    Ok(())
}

fn expected_artifact_id(
    request: ResolveRequest<'_>,
    web_protocol_version: Option<u32>,
) -> Result<String, String> {
    match request.kind {
        ArtifactKind::Native => {
            if web_protocol_version.is_some() {
                return Err("native artifact lock entry must not declare a Web protocol".to_owned());
            }
            Ok(format!(
                "fission-skia-{}-abi{}-{}-{}",
                request.fission_version,
                request.bridge_abi_version,
                request.profile,
                request.target
            ))
        }
        ArtifactKind::CanvasKit => {
            let protocol = web_protocol_version.ok_or_else(|| {
                "CanvasKit artifact lock entry must declare web_protocol_version".to_owned()
            })?;
            if protocol != CANVASKIT_PROTOCOL_VERSION {
                return Err(format!(
                    "CanvasKit artifact uses wire protocol {protocol}, not {CANVASKIT_PROTOCOL_VERSION}"
                ));
            }
            Ok(format!(
                "fission-canvaskit-{}-{}-{}-abi{}-wire{}",
                request.fission_version,
                request.target,
                request.profile,
                request.bridge_abi_version,
                protocol
            ))
        }
    }
}

fn cache_path(request: ResolveRequest<'_>, entry: &LockedArtifact) -> Result<PathBuf, String> {
    for (name, component) in [
        ("Fission version", request.fission_version),
        ("Skia revision", request.skia_revision),
        ("artifact kind", request.kind.cache_key()),
        ("target", request.target),
        ("profile", request.profile),
        ("archive digest", entry.archive_sha256.as_str()),
    ] {
        validate_cache_component(component, name)?;
    }
    Ok(request
        .cache_root
        .join(CACHE_SCHEMA)
        .join(request.fission_version)
        .join(request.skia_revision)
        .join(format!("abi{}", request.bridge_abi_version))
        .join(request.kind.cache_key())
        .join(request.target)
        .join(request.profile)
        .join(&entry.archive_sha256))
}

fn verify_installed(
    root: &Path,
    request: ResolveRequest<'_>,
    entry: Option<&LockedArtifact>,
    require_qualified: bool,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("failed to inspect {}: {error}", root.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!("{} is not a real directory", root.display()));
    }
    let manifest_path = root.join("manifest.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        format!(
            "failed to inspect artifact manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    if !manifest_metadata.is_file()
        || manifest_metadata.file_type().is_symlink()
        || manifest_metadata.len() > MAX_MANIFEST_BYTES
    {
        return Err("artifact manifest is not a bounded regular file".to_owned());
    }
    let manifest_digest = sha256_file(&manifest_path)?;
    if let Some(entry) = entry {
        if manifest_digest != entry.manifest_sha256 {
            return Err(
                "artifact manifest failed the digest pinned in the bundled lock".to_owned(),
            );
        }
    }
    let raw = fs::read(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    match request.kind {
        ArtifactKind::Native => {
            let manifest: InstalledNativeManifest = serde_json::from_slice(&raw)
                .map_err(|error| format!("failed to parse {}: {error}", manifest_path.display()))?;
            let expected_id = expected_artifact_id(request, None)?;
            if manifest.schema_version != ARTIFACT_SCHEMA_VERSION
                || manifest.artifact_id != expected_id
                || entry.is_some_and(|locked| locked.artifact_id != manifest.artifact_id)
                || (require_qualified && !manifest.qualified)
                || manifest.fission_version != request.fission_version
                || manifest.skia.revision != request.skia_revision
                || manifest.bridge_abi_version != request.bridge_abi_version
                || manifest.target != request.target
                || manifest.profile != request.profile
            {
                return Err(
                    "native artifact manifest identity, qualification, ABI, target, or profile does not match the request"
                        .to_owned(),
                );
            }
            verify_payload_tree(root, &manifest.files)
        }
        ArtifactKind::CanvasKit => {
            let manifest: InstalledCanvasKitManifest = serde_json::from_slice(&raw)
                .map_err(|error| format!("failed to parse {}: {error}", manifest_path.display()))?;
            let protocol = manifest.abi.web_protocol_version;
            let expected_id = expected_artifact_id(request, Some(protocol))?;
            if manifest.schema_version != ARTIFACT_SCHEMA_VERSION
                || manifest.artifact_id != expected_id
                || entry.is_some_and(|locked| {
                    locked.artifact_id != manifest.artifact_id
                        || locked.web_protocol_version != Some(protocol)
                })
                || (require_qualified && !manifest.qualified)
                || manifest.fission_version != request.fission_version
                || manifest.source.revision != request.skia_revision
                || manifest.abi.bridge_abi_version != request.bridge_abi_version
                || manifest.target != request.target
                || manifest.profile != request.profile
            {
                return Err(
                    "CanvasKit artifact manifest identity, qualification, ABI, target, or profile does not match the request"
                        .to_owned(),
                );
            }
            verify_canvaskit_contract(&manifest)?;
            verify_payload_tree(root, &manifest.files)
        }
    }
}

fn verify_canvaskit_contract(manifest: &InstalledCanvasKitManifest) -> Result<(), String> {
    const REQUIRED_ASSETS: &[(&str, &str)] = &[
        ("canvaskit_js", "web/canvaskit.js"),
        ("canvaskit_wasm", "web/canvaskit.wasm"),
        ("fission_web_bridge", "web/fission_skia_web.js"),
        ("fission_command_decoder", "web/fission_skia_commands.js"),
        ("fission_frame_executor", "web/fission_skia_executor.js"),
        (
            "fission_paragraph_wire",
            "web/fission_skia_paragraph_wire.js",
        ),
        (
            "fission_paragraph_unicode",
            "web/fission_skia_paragraph_unicode.js",
        ),
        ("fission_paragraph_host", "web/fission_skia_paragraph.js"),
    ];
    const REQUIRED_LICENSES: &[&str] = &[
        "brotli",
        "fission",
        "freetype",
        "harfbuzz",
        "icu",
        "libjpeg-turbo",
        "libpng",
        "libwebp",
        "skia",
        "woff2",
        "wuffs",
        "zlib",
    ];

    let expected_asset_names = REQUIRED_ASSETS
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    if manifest
        .assets
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_asset_names
    {
        return Err(
            "CanvasKit artifact asset map is incomplete or contains unknown assets".to_owned(),
        );
    }

    let declared = manifest
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let mut expected_files = REQUIRED_ASSETS
        .iter()
        .map(|(_, path)| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    expected_files.extend(
        REQUIRED_LICENSES
            .iter()
            .map(|name| format!("licenses/{name}.txt")),
    );
    if declared
        .keys()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>()
        != expected_files
    {
        return Err(
            "CanvasKit artifact payload does not match the production Web profile".to_owned(),
        );
    }

    for (name, path) in REQUIRED_ASSETS {
        let asset = manifest
            .assets
            .get(*name)
            .ok_or_else(|| format!("CanvasKit artifact is missing asset {name}"))?;
        let file = declared
            .get(path)
            .ok_or_else(|| format!("CanvasKit artifact is missing payload {path}"))?;
        if asset.path != *path || asset.sha256 != file.sha256 || asset.size != file.size {
            return Err(format!(
                "CanvasKit asset {name} does not bind exactly to its declared payload"
            ));
        }
    }
    Ok(())
}

fn verify_payload_tree(root: &Path, files: &[InstalledFile]) -> Result<(), String> {
    if files.is_empty() || files.len() > MAX_ARTIFACT_FILES {
        return Err(format!(
            "artifact manifest file count must be in 1..={MAX_ARTIFACT_FILES}"
        ));
    }
    let mut declared = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut total = 0u64;
    for file in files {
        let relative = safe_relative(&file.path)?;
        validate_sha256(&file.sha256, "artifact payload")?;
        total = total
            .checked_add(file.size)
            .ok_or_else(|| "artifact manifest payload size overflowed u64".to_owned())?;
        if total > MAX_EXPANDED_BYTES {
            return Err(format!(
                "artifact payload exceeds the {MAX_EXPANDED_BYTES}-byte expanded limit"
            ));
        }
        if !declared.insert(file.path.clone()) || !folded.insert(file.path.to_lowercase()) {
            return Err(format!(
                "artifact manifest contains a duplicate or case-colliding path {:?}",
                file.path
            ));
        }
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "artifact payload is not a regular file: {}",
                path.display()
            ));
        }
        if metadata.len() != file.size {
            return Err(format!(
                "artifact payload has the wrong size: {}",
                path.display()
            ));
        }
        if sha256_file(&path)? != file.sha256 {
            return Err(format!(
                "artifact payload failed SHA-256 verification: {}",
                path.display()
            ));
        }
    }

    let mut actual = BTreeSet::new();
    collect_tree_files(root, root, &mut actual)?;
    actual.remove("manifest.json");
    if actual != declared {
        return Err(format!(
            "artifact payload set differs from its manifest; missing={:?}, undeclared={:?}",
            declared.difference(&actual).collect::<Vec<_>>(),
            actual.difference(&declared).collect::<Vec<_>>()
        ));
    }
    Ok(())
}

fn collect_tree_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), String> {
    for item in fs::read_dir(directory).map_err(|error| {
        format!(
            "failed to read artifact directory {}: {error}",
            directory.display()
        )
    })? {
        let item = item.map_err(|error| {
            format!(
                "failed to enumerate artifact directory {}: {error}",
                directory.display()
            )
        })?;
        let path = item.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "artifact tree contains a symbolic link: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_tree_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "artifact tree escaped its root".to_owned())?
                .to_str()
                .ok_or_else(|| "artifact tree contains a non-UTF-8 path".to_owned())?
                .replace('\\', "/");
            if !files.insert(relative.clone()) {
                return Err(format!(
                    "artifact tree contains duplicate path {relative:?}"
                ));
            }
        } else {
            return Err(format!(
                "artifact tree contains a special file: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn verify_archive_file(path: &Path, entry: &LockedArtifact) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect downloaded Skia archive: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("downloaded Skia archive is not a regular file".to_owned());
    }
    if metadata.len() != entry.archive_size {
        return Err(format!(
            "downloaded Skia archive has {} bytes, expected {}",
            metadata.len(),
            entry.archive_size
        ));
    }
    if sha256_file(path)? != entry.archive_sha256 {
        return Err("downloaded Skia archive failed SHA-256 verification".to_owned());
    }
    Ok(())
}

fn extract_archive(
    archive: &Path,
    destination: &Path,
    expected_root: &str,
) -> Result<PathBuf, String> {
    let input = File::open(archive)
        .map_err(|error| format!("failed to open downloaded Skia archive: {error}"))?;
    let decoder = GzDecoder::new(BufReader::new(input));
    let mut archive = tar::Archive::new(decoder);
    let mut paths = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut expanded = 0u64;
    let mut count = 0usize;
    let entries = archive
        .entries()
        .map_err(|error| format!("failed to read Skia archive entries: {error}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| format!("invalid Skia archive entry: {error}"))?;
        count = count
            .checked_add(1)
            .ok_or_else(|| "Skia archive entry count overflowed usize".to_owned())?;
        if count > MAX_ARTIFACT_FILES + 1_024 {
            return Err("Skia archive contains too many entries".to_owned());
        }
        let path = entry
            .path()
            .map_err(|error| format!("invalid Skia archive path: {error}"))?
            .into_owned();
        let text = path
            .to_str()
            .ok_or_else(|| "Skia archive contains a non-UTF-8 path".to_owned())?;
        if text.contains('\\') {
            return Err(format!(
                "Skia archive contains a non-canonical path {text:?}"
            ));
        }
        let components = path
            .components()
            .map(|component| match component {
                Component::Normal(value) => value.to_str().map(str::to_owned),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("Skia archive contains an unsafe path {text:?}"))?;
        if components.is_empty() || components[0] != expected_root {
            return Err(format!(
                "Skia archive entry {text:?} is outside expected root {expected_root:?}"
            ));
        }
        let canonical = components.join("/");
        if !paths.insert(canonical.clone()) || !folded.insert(canonical.to_lowercase()) {
            return Err(format!(
                "Skia archive contains a duplicate or case-colliding path {canonical:?}"
            ));
        }
        let entry_type = entry.header().entry_type();
        let output = components
            .iter()
            .fold(destination.to_path_buf(), |path, component| {
                path.join(component)
            });
        if entry_type.is_dir() {
            fs::create_dir_all(&output).map_err(|error| {
                format!(
                    "failed to create Skia artifact directory {}: {error}",
                    output.display()
                )
            })?;
            continue;
        }
        if !entry_type.is_file() {
            return Err(format!(
                "Skia archive may contain only regular files and directories: {canonical:?}"
            ));
        }
        let size = entry.size();
        expanded = expanded
            .checked_add(size)
            .ok_or_else(|| "Skia archive expanded size overflowed u64".to_owned())?;
        if expanded > MAX_EXPANDED_BYTES {
            return Err(format!(
                "Skia archive exceeds the {MAX_EXPANDED_BYTES}-byte expanded limit"
            ));
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Skia artifact directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
            .map_err(|error| format!("failed to create {}: {error}", output.display()))?;
        let written = io::copy(&mut entry, &mut file)
            .map_err(|error| format!("failed to extract {}: {error}", output.display()))?;
        if written != size {
            return Err(format!(
                "Skia archive entry {canonical:?} produced {written} bytes, expected {size}"
            ));
        }
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", output.display()))?;
    }
    let root = destination.join(expected_root);
    if !root.join("manifest.json").is_file() {
        return Err("Skia archive does not contain its root manifest.json".to_owned());
    }
    Ok(root)
}

fn download_https(url: &str, expected_size: u64, destination: &Path) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .https_only(true)
        .redirects(5)
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(600))
        .build();
    let response = agent
        .get(url)
        .set("Accept-Encoding", "identity")
        .call()
        .map_err(|error| format!("failed to download the locked Skia artifact: {error}"))?;
    if !response.get_url().starts_with("https://") {
        return Err("Skia artifact download redirected away from HTTPS".to_owned());
    }
    if let Some(raw_length) = response.header("Content-Length") {
        let length = raw_length.parse::<u64>().map_err(|_| {
            "Skia artifact server returned an invalid Content-Length header".to_owned()
        })?;
        if length != expected_size {
            return Err(format!(
                "Skia artifact server reported {length} bytes, expected {expected_size}"
            ));
        }
    }
    let mut reader = response.into_reader().take(expected_size.saturating_add(1));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| format!("failed to create private Skia download: {error}"))?;
    let written = io::copy(&mut reader, &mut output)
        .map_err(|error| format!("failed while downloading the locked Skia artifact: {error}"))?;
    if written != expected_size {
        return Err(format!(
            "downloaded Skia artifact has {written} bytes, expected {expected_size}"
        ));
    }
    output
        .sync_all()
        .map_err(|error| format!("failed to sync the downloaded Skia artifact: {error}"))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut input = File::open(path)
        .map_err(|error| format!("failed to open {} for hashing: {error}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn safe_relative(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.contains('\\') {
        return Err(format!("artifact manifest contains unsafe path {value:?}"));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("artifact manifest contains unsafe path {value:?}"));
    }
    Ok(path.to_path_buf())
}

fn validate_sha256(value: &str, description: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{description} SHA-256 must be 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn validate_cache_component(value: &str, description: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(format!(
            "{description} contains characters unsafe for a Skia cache key"
        ));
    }
    Ok(())
}

fn nonempty_path(value: std::ffi::OsString, variable: &str) -> Result<PathBuf, String> {
    if value.is_empty() {
        return Err(format!("{variable} must not be empty"));
    }
    Ok(PathBuf::from(value))
}

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(parent: &Path) -> Result<Self, String> {
        for _ in 0..32 {
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let sequence = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".fission-skia-{}-{time}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to create private Skia staging directory {}: {error}",
                        path.display()
                    ));
                }
            }
        }
        Err("failed to allocate a unique private Skia staging directory".to_owned())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::Cursor;

    use super::*;

    const VERSION: &str = "0.10.1";
    const REVISION: &str = "cf5c36972b73698eb3939cda147ea47152670312";
    const ABI: u32 = 13;
    const TARGET: &str = "x86_64-unknown-linux-gnu";
    const PROFILE: &str = "native-raster";

    struct Fixture {
        _temporary: tempfile::TempDir,
        archive: PathBuf,
        cache: PathBuf,
        lock: Vec<u8>,
        manifest_sha256: String,
    }

    impl Fixture {
        fn new(mut manifest: serde_json::Value) -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let payload = b"verified library bytes";
            manifest["files"] = serde_json::json!([{
                "path": "lib/libskia.a",
                "sha256": format!("{:x}", Sha256::digest(payload)),
                "size": payload.len(),
            }]);
            let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            let manifest_sha256 = format!("{:x}", Sha256::digest(&manifest_bytes));
            let archive = temporary.path().join("artifact.tar.gz");
            let output = File::create(&archive).unwrap();
            let gzip = flate2::write::GzEncoder::new(output, flate2::Compression::default());
            let mut tar = tar::Builder::new(gzip);
            append(
                &mut tar,
                "fission-skia-0.10.1-abi14-native-raster-x86_64-unknown-linux-gnu/manifest.json",
                &manifest_bytes,
            );
            append(
                &mut tar,
                "fission-skia-0.10.1-abi14-native-raster-x86_64-unknown-linux-gnu/lib/libskia.a",
                payload,
            );
            tar.into_inner().unwrap().finish().unwrap();
            let archive_sha256 = sha256_file(&archive).unwrap();
            let archive_size = fs::metadata(&archive).unwrap().len();
            let lock = serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "fission_version": VERSION,
                "skia_revision": REVISION,
                "bridge_abi_version": ABI,
                "provenance": {
                    "repository": "fission-ui/fission",
                    "predicate_type": "https://slsa.dev/provenance/v1"
                },
                "artifacts": [{
                    "kind": "native",
                    "artifact_id": "fission-skia-0.10.1-abi14-native-raster-x86_64-unknown-linux-gnu",
                    "target": TARGET,
                    "profile": PROFILE,
                    "qualified": true,
                    "url": "https://github.com/fission-ui/fission/releases/download/v0.10.1/fission-skia.tar.gz",
                    "archive_sha256": archive_sha256,
                    "archive_size": archive_size,
                    "manifest_sha256": manifest_sha256,
                }]
            }))
            .unwrap();
            Self {
                cache: temporary.path().join("cache"),
                _temporary: temporary,
                archive,
                lock,
                manifest_sha256,
            }
        }

        fn valid() -> Self {
            Self::new(serde_json::json!({
                "schema_version": 1,
                "artifact_id": "fission-skia-0.10.1-abi14-native-raster-x86_64-unknown-linux-gnu",
                "qualified": true,
                "fission_version": VERSION,
                "skia": { "revision": REVISION },
                "bridge_abi_version": ABI,
                "target": TARGET,
                "profile": PROFILE,
                "files": [],
            }))
        }

        fn request(&self, offline: bool) -> ResolveRequest<'_> {
            ResolveRequest {
                kind: ArtifactKind::Native,
                lock_json: &self.lock,
                fission_version: VERSION,
                skia_revision: REVISION,
                bridge_abi_version: ABI,
                target: TARGET,
                profile: PROFILE,
                cache_root: &self.cache,
                offline,
            }
        }
    }

    fn append(
        builder: &mut tar::Builder<flate2::write::GzEncoder<File>>,
        path: &str,
        bytes: &[u8],
    ) {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, path, Cursor::new(bytes))
            .unwrap();
    }

    #[test]
    fn resolves_a_qualified_canvaskit_artifact_with_the_exact_wire_contract() {
        const WEB_PROFILE: &str = "canvaskit-production";
        const WEB_TARGET: &str = "wasm32-unknown-unknown";
        const WEB_ID: &str =
            "fission-canvaskit-0.10.1-wasm32-unknown-unknown-canvaskit-production-abi14-wire1";
        const WEB_ASSETS: &[(&str, &str)] = &[
            ("canvaskit_js", "web/canvaskit.js"),
            ("canvaskit_wasm", "web/canvaskit.wasm"),
            ("fission_web_bridge", "web/fission_skia_web.js"),
            ("fission_command_decoder", "web/fission_skia_commands.js"),
            ("fission_frame_executor", "web/fission_skia_executor.js"),
            (
                "fission_paragraph_wire",
                "web/fission_skia_paragraph_wire.js",
            ),
            (
                "fission_paragraph_unicode",
                "web/fission_skia_paragraph_unicode.js",
            ),
            ("fission_paragraph_host", "web/fission_skia_paragraph.js"),
        ];
        const LICENSES: &[&str] = &[
            "brotli",
            "fission",
            "freetype",
            "harfbuzz",
            "icu",
            "libjpeg-turbo",
            "libpng",
            "libwebp",
            "skia",
            "woff2",
            "wuffs",
            "zlib",
        ];

        let temporary = tempfile::tempdir().unwrap();
        let mut payloads = Vec::new();
        let mut assets = serde_json::Map::new();
        for (name, path) in WEB_ASSETS {
            let bytes = if *path == "web/canvaskit.wasm" {
                b"\0asm\x01\0\0\0fixture".to_vec()
            } else {
                format!("// {path}\n").into_bytes()
            };
            let digest = format!("{:x}", Sha256::digest(&bytes));
            assets.insert(
                (*name).to_owned(),
                serde_json::json!({
                    "path": path,
                    "sha256": digest,
                    "size": bytes.len(),
                }),
            );
            payloads.push(((*path).to_owned(), bytes));
        }
        for license in LICENSES {
            payloads.push((
                format!("licenses/{license}.txt"),
                format!("{license} licence\n").into_bytes(),
            ));
        }
        let files = payloads
            .iter()
            .map(|(path, bytes)| {
                serde_json::json!({
                    "path": path,
                    "sha256": format!("{:x}", Sha256::digest(bytes)),
                    "size": bytes.len(),
                })
            })
            .collect::<Vec<_>>();
        let manifest = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "artifact_id": WEB_ID,
            "qualified": true,
            "fission_version": VERSION,
            "source": { "revision": REVISION },
            "abi": {
                "bridge_abi_version": ABI,
                "web_protocol_version": CANVASKIT_PROTOCOL_VERSION,
            },
            "target": WEB_TARGET,
            "profile": WEB_PROFILE,
            "assets": assets,
            "files": files,
        }))
        .unwrap();
        let manifest_sha256 = format!("{:x}", Sha256::digest(&manifest));
        let archive = temporary.path().join("canvaskit.tar.gz");
        let output = File::create(&archive).unwrap();
        let gzip = flate2::write::GzEncoder::new(output, flate2::Compression::default());
        let mut tar = tar::Builder::new(gzip);
        append(&mut tar, &format!("{WEB_ID}/manifest.json"), &manifest);
        for (path, bytes) in &payloads {
            append(&mut tar, &format!("{WEB_ID}/{path}"), bytes);
        }
        tar.into_inner().unwrap().finish().unwrap();
        let archive_sha256 = sha256_file(&archive).unwrap();
        let lock = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "fission_version": VERSION,
            "skia_revision": REVISION,
            "bridge_abi_version": ABI,
            "provenance": {
                "repository": "fission-ui/fission",
                "predicate_type": "https://slsa.dev/provenance/v1",
            },
            "artifacts": [{
                "kind": "canvaskit",
                "artifact_id": WEB_ID,
                "target": WEB_TARGET,
                "profile": WEB_PROFILE,
                "qualified": true,
                "url": "https://github.com/fission-ui/fission/releases/download/v0.10.1/fission-canvaskit.tar.gz",
                "archive_sha256": archive_sha256,
                "archive_size": fs::metadata(&archive).unwrap().len(),
                "manifest_sha256": manifest_sha256,
                "web_protocol_version": CANVASKIT_PROTOCOL_VERSION,
            }],
        }))
        .unwrap();
        let cache = temporary.path().join("cache");
        let request = ResolveRequest {
            kind: ArtifactKind::CanvasKit,
            lock_json: &lock,
            fission_version: VERSION,
            skia_revision: REVISION,
            bridge_abi_version: ABI,
            target: WEB_TARGET,
            profile: WEB_PROFILE,
            cache_root: &cache,
            offline: false,
        };

        let installed = resolve_with(request, |_, _, destination| {
            fs::copy(&archive, destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .unwrap();

        assert!(installed.join("web/canvaskit.wasm").is_file());
        assert!(installed.join("licenses/skia.txt").is_file());
    }

    #[test]
    fn installs_once_then_uses_the_verified_cache_offline() {
        let fixture = Fixture::valid();
        let calls = Cell::new(0);
        let installed = resolve_with(fixture.request(false), |_, _, destination| {
            calls.set(calls.get() + 1);
            fs::copy(&fixture.archive, destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert!(installed.join("lib/libskia.a").is_file());

        let cached = resolve_with(fixture.request(true), |_, _, _| {
            panic!("offline cache hit must not fetch")
        })
        .unwrap();
        assert_eq!(cached, installed);
    }

    #[test]
    fn offline_miss_is_explicit_and_does_not_fetch() {
        let fixture = Fixture::valid();
        let error = resolve_with(fixture.request(true), |_, _, _| {
            panic!("offline cache miss must not fetch")
        })
        .unwrap_err();
        assert!(error.contains("offline mode has no exact verified Skia artifact"));
    }

    #[test]
    fn rejects_archive_digest_mismatch_before_extraction() {
        let mut fixture = Fixture::valid();
        let mut lock: serde_json::Value = serde_json::from_slice(&fixture.lock).unwrap();
        lock["artifacts"][0]["archive_sha256"] = serde_json::Value::String("0".repeat(64));
        fixture.lock = serde_json::to_vec(&lock).unwrap();
        let error = resolve_with(fixture.request(false), |_, _, destination| {
            fs::copy(&fixture.archive, destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .unwrap_err();
        assert!(error.contains("failed SHA-256 verification"));
    }

    #[test]
    fn rejects_manifest_target_before_publishing_the_cache() {
        let mut manifest = serde_json::json!({
            "schema_version": 1,
            "artifact_id": "fission-skia-0.10.1-abi14-native-raster-x86_64-unknown-linux-gnu",
            "qualified": true,
            "fission_version": VERSION,
            "skia": { "revision": REVISION },
            "bridge_abi_version": ABI,
            "target": "aarch64-unknown-linux-gnu",
            "profile": PROFILE,
            "files": [],
        });
        let fixture = Fixture::new(manifest.take());
        let error = resolve_with(fixture.request(false), |_, _, destination| {
            fs::copy(&fixture.archive, destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .unwrap_err();
        assert!(error.contains("identity, qualification, ABI, target, or profile"));
    }

    #[test]
    fn rejects_unqualified_locked_artifact() {
        let mut fixture = Fixture::valid();
        let mut lock: serde_json::Value = serde_json::from_slice(&fixture.lock).unwrap();
        lock["artifacts"][0]["qualified"] = serde_json::Value::Bool(false);
        fixture.lock = serde_json::to_vec(&lock).unwrap();
        let error = resolve_with(fixture.request(false), |_, _, _| unreachable!()).unwrap_err();
        assert!(error.contains("not marked production-qualified"));
    }

    #[test]
    fn detects_payload_corruption_in_an_existing_cache() {
        let fixture = Fixture::valid();
        let installed = resolve_with(fixture.request(false), |_, _, destination| {
            fs::copy(&fixture.archive, destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .unwrap();
        fs::write(installed.join("lib/libskia.a"), b"tampered").unwrap();
        let error = resolve_with(fixture.request(false), |_, _, _| {
            panic!("a corrupt content-addressed cache must fail closed")
        })
        .unwrap_err();
        assert!(error.contains("cached Skia artifact"));
        assert!(error.contains("corrupt"));
    }

    #[test]
    fn manifest_digest_is_part_of_cache_acceptance() {
        let mut fixture = Fixture::valid();
        let mut lock: serde_json::Value = serde_json::from_slice(&fixture.lock).unwrap();
        lock["artifacts"][0]["manifest_sha256"] = serde_json::Value::String("f".repeat(64));
        fixture.lock = serde_json::to_vec(&lock).unwrap();
        let error = resolve_with(fixture.request(false), |_, _, destination| {
            fs::copy(&fixture.archive, destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .unwrap_err();
        assert!(error.contains("manifest failed the digest"));
        assert_ne!(fixture.manifest_sha256, "f".repeat(64));
    }
}
