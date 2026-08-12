use std::collections::BTreeSet;
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
    artifact_id: String,
    target: String,
    profile: String,
    qualified: bool,
    url: String,
    archive_sha256: String,
    archive_size: u64,
    manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
struct InstalledManifest {
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
#[serde(deny_unknown_fields)]
struct InstalledFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct ResolveRequest<'a> {
    pub(crate) lock_json: &'a [u8],
    pub(crate) fission_version: &'a str,
    pub(crate) skia_revision: &'a str,
    pub(crate) bridge_abi_version: u32,
    pub(crate) target: &'a str,
    pub(crate) profile: &'a str,
    pub(crate) cache_root: &'a Path,
    pub(crate) offline: bool,
}

pub(crate) fn resolve(request: ResolveRequest<'_>) -> Result<PathBuf, String> {
    resolve_with(request, download_https)
}

fn resolve_with<F>(request: ResolveRequest<'_>, fetch: F) -> Result<PathBuf, String>
where
    F: FnOnce(&str, u64, &Path) -> Result<(), String>,
{
    let entry = select_locked_artifact(request)?;
    let cache_path = cache_path(request, &entry)?;
    if cache_path.exists() {
        verify_installed(&cache_path, request, &entry).map_err(|error| {
            format!(
                "cached Skia artifact {} is corrupt: {error}; remove only this cache entry and retry: {}",
                entry.artifact_id,
                cache_path.display()
            )
        })?;
        return Ok(cache_path);
    }
    if request.offline {
        return Err(format!(
            "offline mode has no exact verified Skia artifact for {}/{} at {}; pre-populate that cache entry or set FISSION_SKIA_ARTIFACT_DIR to a verified local artifact",
            request.profile,
            request.target,
            cache_path.display()
        ));
    }

    let cache_parent = cache_path
        .parent()
        .ok_or_else(|| "Skia cache path has no parent".to_owned())?;
    fs::create_dir_all(cache_parent).map_err(|error| {
        format!(
            "failed to create Skia artifact cache {}: {error}",
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
            "failed to create private Skia extraction directory {}: {error}",
            extracted.display()
        )
    })?;
    let artifact_root = extract_archive(&archive_path, &extracted, &entry.artifact_id)?;
    verify_installed(&artifact_root, request, &entry)?;

    match fs::rename(&artifact_root, &cache_path) {
        Ok(()) => {}
        Err(error) if cache_path.exists() => {
            verify_installed(&cache_path, request, &entry).map_err(|winner_error| {
                format!(
                    "another process populated {} while this build was resolving it, but the published cache entry is invalid: {winner_error} (rename error: {error})",
                    cache_path.display()
                )
            })?;
        }
        Err(error) => {
            return Err(format!(
                "failed to atomically publish verified Skia artifact to {}: {error}",
                cache_path.display()
            ));
        }
    }
    Ok(cache_path)
}

pub(crate) fn cache_root_from_environment() -> Result<PathBuf, String> {
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

pub(crate) fn offline_from_environment() -> Result<bool, String> {
    let mut offline = false;
    for variable in ["FISSION_SKIA_OFFLINE", "CARGO_NET_OFFLINE"] {
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

fn select_locked_artifact(request: ResolveRequest<'_>) -> Result<LockedArtifact, String> {
    let lock: ArtifactLock = serde_json::from_slice(request.lock_json)
        .map_err(|error| format!("failed to parse the bundled Skia artifact lock: {error}"))?;
    if lock.schema_version != LOCK_SCHEMA_VERSION {
        return Err(format!(
            "unsupported bundled Skia artifact lock schema {}",
            lock.schema_version
        ));
    }
    if lock.fission_version != request.fission_version {
        return Err(format!(
            "bundled Skia artifact lock is for Fission {}, not {}",
            lock.fission_version, request.fission_version
        ));
    }
    if lock.skia_revision != request.skia_revision {
        return Err("bundled Skia artifact lock has the wrong pinned Skia revision".to_owned());
    }
    if lock.bridge_abi_version != request.bridge_abi_version {
        return Err(format!(
            "bundled Skia artifact lock has bridge ABI {}, not {}",
            lock.bridge_abi_version, request.bridge_abi_version
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

    let mut matches = lock
        .artifacts
        .into_iter()
        .filter(|entry| entry.target == request.target && entry.profile == request.profile);
    let entry = matches.next().ok_or_else(|| {
        format!(
            "no published Skia artifact is locked for {}/{} in fission-skia-sys {}; use an explicitly verified FISSION_SKIA_ARTIFACT_DIR or opt into skia-build-from-source",
            request.profile, request.target, request.fission_version
        )
    })?;
    if matches.next().is_some() {
        return Err(format!(
            "bundled Skia artifact lock contains duplicate entries for {}/{}",
            request.profile, request.target
        ));
    }
    validate_locked_artifact(&entry, request)?;
    Ok(entry)
}

fn validate_locked_artifact(
    entry: &LockedArtifact,
    request: ResolveRequest<'_>,
) -> Result<(), String> {
    if !entry.qualified {
        return Err(format!(
            "published Skia artifact {} is not marked production-qualified in the bundled lock",
            entry.artifact_id
        ));
    }
    let expected_id = format!(
        "fission-skia-{}-abi{}-{}-{}",
        request.fission_version, request.bridge_abi_version, request.profile, request.target
    );
    if entry.artifact_id != expected_id {
        return Err(format!(
            "locked Skia artifact id {} does not match expected {expected_id}",
            entry.artifact_id
        ));
    }
    if !entry.url.starts_with("https://")
        || entry.url.chars().any(char::is_whitespace)
        || entry.url.contains('#')
    {
        return Err(format!(
            "locked Skia artifact {} must use a canonical HTTPS URL without whitespace or a fragment",
            entry.artifact_id
        ));
    }
    validate_sha256(&entry.archive_sha256, "locked archive")?;
    validate_sha256(&entry.manifest_sha256, "locked artifact manifest")?;
    if entry.archive_size == 0 || entry.archive_size > MAX_ARCHIVE_BYTES {
        return Err(format!(
            "locked Skia archive size {} is outside the supported 1..={MAX_ARCHIVE_BYTES} byte range",
            entry.archive_size
        ));
    }
    Ok(())
}

fn cache_path(request: ResolveRequest<'_>, entry: &LockedArtifact) -> Result<PathBuf, String> {
    for (name, component) in [
        ("Fission version", request.fission_version),
        ("Skia revision", request.skia_revision),
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
        .join(request.target)
        .join(request.profile)
        .join(&entry.archive_sha256))
}

fn verify_installed(
    root: &Path,
    request: ResolveRequest<'_>,
    entry: &LockedArtifact,
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
    if manifest_digest != entry.manifest_sha256 {
        return Err("artifact manifest failed the digest pinned in the bundled lock".to_owned());
    }
    let raw = fs::read(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let manifest: InstalledManifest = serde_json::from_slice(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", manifest_path.display()))?;
    if manifest.schema_version != ARTIFACT_SCHEMA_VERSION
        || manifest.artifact_id != entry.artifact_id
        || !manifest.qualified
        || manifest.fission_version != request.fission_version
        || manifest.skia.revision != request.skia_revision
        || manifest.bridge_abi_version != request.bridge_abi_version
        || manifest.target != request.target
        || manifest.profile != request.profile
    {
        return Err(
            "artifact manifest identity, qualification, ABI, target, or profile does not match the bundled lock"
                .to_owned(),
        );
    }
    verify_payload_tree(root, &manifest.files)
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
                "fission-skia-0.10.1-abi13-native-raster-x86_64-unknown-linux-gnu/manifest.json",
                &manifest_bytes,
            );
            append(
                &mut tar,
                "fission-skia-0.10.1-abi13-native-raster-x86_64-unknown-linux-gnu/lib/libskia.a",
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
                    "artifact_id": "fission-skia-0.10.1-abi13-native-raster-x86_64-unknown-linux-gnu",
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
                "artifact_id": "fission-skia-0.10.1-abi13-native-raster-x86_64-unknown-linux-gnu",
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
            "artifact_id": "fission-skia-0.10.1-abi13-native-raster-x86_64-unknown-linux-gnu",
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
