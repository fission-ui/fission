//! Offline, project-local guidance. The included assets are the sole source of truth.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

mod version;
pub use version::{Compatibility, FrameworkDependency};

pub const SCHEMA_VERSION: u32 = 1;
pub const GUIDANCE_VERSION: u32 = 3;
/// API revision reviewed for these maintained assets; independent of CLI packaging.
pub const FRAMEWORK_API_VERSION: &str = "0.15.1";
pub const MANIFEST_PATH: &str = ".fission/guidance-manifest.json";
const TRANSACTION_PATH: &str = ".fission/guidance-transaction";
const SKILL_PATH: &str = ".fission/skills/fission-web/SKILL.md";
const SHARED_PATH: &str = ".fission/references/shared-app.md";
const LEGACY: &str = include_str!("../assets/legacy/AGENTS-v1.md");
const LEGACY_V2: &str = include_str!("../assets/legacy/AGENTS-v2.md");
const ROUTER: &str = include_str!("../assets/AGENTS.md");
const ASSETS: &[(&str, &str)] = &[
    (
        SHARED_PATH,
        include_str!("../assets/guidance/shared-app.md"),
    ),
    (
        SKILL_PATH,
        include_str!("../assets/guidance/fission-web/SKILL.md"),
    ),
    (
        ".fission/skills/fission-web/references/setup-targets.md",
        include_str!("../assets/guidance/fission-web/references/setup-targets.md"),
    ),
    (
        ".fission/skills/fission-web/references/widgets-state-routing.md",
        include_str!("../assets/guidance/fission-web/references/widgets-state-routing.md"),
    ),
    (
        ".fission/skills/fission-web/references/design-i18n.md",
        include_str!("../assets/guidance/fission-web/references/design-i18n.md"),
    ),
    (
        ".fission/skills/fission-web/references/testing-review.md",
        include_str!("../assets/guidance/fission-web/references/testing-review.md"),
    ),
];
const INSTRUCTIONS: &[&str] = &["AGENTS.md", "AGENTS.fission.md", ".fission/AGENTS.md"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagedFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub guidance_version: u32,
    pub cli_version: String,
    pub framework_api_version: String,
    pub files: Vec<ManagedFile>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileHealth {
    Current,
    Missing,
    Stale,
    Customized,
    Unsafe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFinding {
    pub path: String,
    pub health: FileHealth,
    pub installed_sha256: Option<String>,
    pub recorded_sha256: Option<String>,
    pub bundled_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    ManifestMissing,
    ManifestStale,
    ManifestInvalid,
    UnsafePath,
    Customized,
    PreservedInstructions,
    IncompleteTransaction,
    Io,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub path: PathBuf,
    pub message: String,
    pub blocks_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recovery {
    pub reason: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Healthy,
    NeedsAttention,
    Updated,
    Conflict,
    InvalidArguments,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coverage {
    pub domains: Vec<String>,
    pub browser_readiness: String,
    pub geometry: String,
    pub unavailable_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceResult {
    pub schema_version: u32,
    pub operation: String,
    pub status: Status,
    pub project_dir: PathBuf,
    pub guidance_root: PathBuf,
    pub bundled: Manifest,
    pub installed: Option<Manifest>,
    pub installed_manifest_sha256: Option<String>,
    pub framework_dependency: FrameworkDependency,
    pub coverage: Coverage,
    pub files: Vec<FileFinding>,
    pub findings: Vec<Finding>,
    pub recovery: Vec<Recovery>,
    pub instruction_paths: Vec<PathBuf>,
}

impl GuidanceResult {
    /// Check requires matching resolved registry version evidence as well as healthy files.
    /// Update success establishes asset installation, not framework compatibility.
    pub fn success(&self) -> bool {
        matches!(self.status, Status::Healthy | Status::Updated)
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn render(raw: &str, instruction: bool, path: &str) -> String {
    render_version(raw, instruction, path, GUIDANCE_VERSION)
}

fn render_version(raw: &str, instruction: bool, path: &str, guidance_version: u32) -> String {
    let skill = if path == ".fission/AGENTS.md" {
        "skills/fission-web/SKILL.md"
    } else {
        SKILL_PATH
    };
    let mut text = raw
        .replace("{{CLI_VERSION}}", super::CURRENT_VERSION)
        .replace("{{FRAMEWORK_VERSION}}", FRAMEWORK_API_VERSION)
        .replace("{{GUIDANCE_VERSION}}", &guidance_version.to_string())
        .replace("{{SCHEMA_VERSION}}", &SCHEMA_VERSION.to_string());
    if instruction {
        text = text.replace("{{SKILL_PATH}}", skill);
        text = text.replace(
            "{{SHARED_PATH}}",
            if path == ".fission/AGENTS.md" {
                "references/shared-app.md"
            } else {
                SHARED_PATH
            },
        );
    }
    text
}

fn bundle(entrypoint: &str) -> BTreeMap<String, Vec<u8>> {
    let mut files = ASSETS
        .iter()
        .map(|(path, raw)| ((*path).into(), render(raw, false, path).into_bytes()))
        .collect::<BTreeMap<_, _>>();
    files.insert(
        entrypoint.into(),
        render(ROUTER, true, entrypoint).into_bytes(),
    );
    files
}

fn manifest(files: &BTreeMap<String, Vec<u8>>) -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        guidance_version: GUIDANCE_VERSION,
        cli_version: super::CURRENT_VERSION.into(),
        framework_api_version: FRAMEWORK_API_VERSION.into(),
        files: files
            .iter()
            .map(|(path, bytes)| ManagedFile {
                path: path.clone(),
                sha256: hash(bytes),
            })
            .collect(),
    }
}

/// Published assets and their exact rendered content hashes; no checkout or network required.
pub fn bundled_manifest() -> Manifest {
    manifest(&bundle("AGENTS.md"))
}

fn allowed(path: &str) -> bool {
    INSTRUCTIONS.contains(&path) || ASSETS.iter().any(|(p, _)| *p == path)
}

fn validate_manifest(m: &Manifest) -> Result<()> {
    if m.schema_version != SCHEMA_VERSION {
        bail!("unsupported guidance schema {}", m.schema_version);
    }
    if m.guidance_version == 0
        || semver::Version::parse(&m.cli_version).is_err()
        || semver::Version::parse(&m.framework_api_version).is_err()
    {
        bail!("invalid bundle version metadata");
    }
    let mut seen = BTreeSet::new();
    for file in &m.files {
        validate_relative(&file.path)?;
        if !allowed(&file.path) {
            bail!("unrecognized managed path {}", file.path);
        }
        if !seen.insert(&file.path) {
            bail!("duplicate managed path {}", file.path);
        }
        if file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            bail!("invalid SHA-256 for {}", file.path);
        }
    }
    if m.files
        .iter()
        .filter(|f| INSTRUCTIONS.contains(&f.path.as_str()))
        .count()
        != 1
    {
        bail!("manifest must record exactly one managed instruction entrypoint");
    }
    Ok(())
}

fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!("unsafe relative path {path:?}");
    }
    Ok(())
}

/// Check every existing component without following links, including dangling links.
fn safe_destination(root: &Path, relative: &str, directory: bool) -> Result<PathBuf> {
    validate_relative(relative)?;
    let mut path = root.to_path_buf();
    let parts = relative.split('/').collect::<Vec<_>>();
    for (i, part) in parts.iter().enumerate() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    bail!("symlink destination {}", path.display());
                }
                let expect_dir = i + 1 < parts.len() || directory;
                if (expect_dir && !meta.is_dir()) || (!expect_dir && !meta.is_file()) {
                    bail!("file/directory collision {}", path.display());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("inspect {}", path.display())),
        }
    }
    Ok(path)
}

fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

fn generated_candidate(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes).contains("fission-cli-generated-agents:")
}

fn known_instruction(bytes: &[u8], path: &str) -> bool {
    bytes == LEGACY.as_bytes()
        || bytes == render_version(LEGACY_V2, true, path, 2).as_bytes()
        || bytes == render(ROUTER, true, path).as_bytes()
}

fn entrypoint(root: &Path, old: Option<&Manifest>) -> Result<String> {
    if let Some(old) = old {
        return Ok(old
            .files
            .iter()
            .find(|f| INSTRUCTIONS.contains(&f.path.as_str()))
            .context("missing instruction entrypoint")?
            .path
            .clone());
    }
    for name in INSTRUCTIONS {
        let path = safe_destination(root, name, false)?;
        let bytes = optional_bytes(&path)?;
        if bytes
            .as_ref()
            .is_none_or(|b| known_instruction(b, name) || generated_candidate(b))
        {
            return Ok((*name).into());
        }
    }
    bail!("all instruction entrypoints are customized");
}

fn recovery(operation: &str, project: &Path, reason: &str) -> Recovery {
    Recovery {
        reason: reason.into(),
        argv: vec![
            "fission".into(),
            "skills".into(),
            operation.into(),
            "--project-dir".into(),
            project.to_string_lossy().into_owned(),
            "--json".into(),
        ],
    }
}

pub fn check(project: &Path) -> GuidanceResult {
    inspect(project, "check")
}

fn inspect(project: &Path, operation: &str) -> GuidanceResult {
    let absolute = super::lexical_absolute(project);
    let project = absolute
        .as_ref()
        .cloned()
        .unwrap_or_else(|_| project.to_path_buf());
    let root = super::find_git_root(&project).unwrap_or_else(|| project.clone());
    let mut r = GuidanceResult {
        schema_version: SCHEMA_VERSION, operation: operation.into(), status: Status::NeedsAttention,
        project_dir: project.clone(), guidance_root: root.clone(), bundled: bundled_manifest(), installed: None,
        framework_dependency: version::inspect(&project),
        coverage: Coverage {
            domains: vec!["shared_app_rules".into(), "website_setup_targets".into(), "widgets_state_routing".into(), "design_i18n".into(), "testing_review".into()],
            browser_readiness: "DOM, canvas/renderer, and test-bridge smoke evidence; not complete paint validation".into(),
            geometry: "Partial semantic-node bounds; inspect screenshots and product behavior".into(),
            unavailable_commands: vec!["preview".into(), "review".into(), "verified_in_session_resize".into()],
        },
        files: vec![], findings: vec![], recovery: vec![], instruction_paths: vec![],
        installed_manifest_sha256: None,
    };
    if let Err(error) = absolute {
        finding(
            &mut r,
            FindingKind::Io,
            project,
            format!("resolve current directory: {error}"),
            true,
        );
        r.status = Status::Error;
        return r;
    }
    if !project.is_dir() {
        finding(
            &mut r,
            FindingKind::Io,
            project,
            "project directory is absent or not a directory".into(),
            true,
        );
        r.status = Status::Error;
        return r;
    }
    if let Err(e) = safe_destination(&root, TRANSACTION_PATH, true) {
        finding(
            &mut r,
            FindingKind::UnsafePath,
            root.join(TRANSACTION_PATH),
            e.to_string(),
            true,
        );
    } else if root.join(TRANSACTION_PATH).exists() {
        finding(&mut r, FindingKind::IncompleteTransaction, root.join(TRANSACTION_PATH),
            "An interrupted/concurrent update is present. Inspect journal.json and originals/; restore listed originals (remove destinations marked absent) before explicitly removing this transaction directory. Do not run concurrent writers.".into(), true);
    }
    match safe_destination(&root, MANIFEST_PATH, false).and_then(|p| optional_bytes(&p)) {
        Ok(None) => finding(
            &mut r,
            FindingKind::ManifestMissing,
            root.join(MANIFEST_PATH),
            "No managed manifest; existing files need exact known-template proof.".into(),
            false,
        ),
        Ok(Some(bytes)) => {
            r.installed_manifest_sha256 = Some(hash(&bytes));
            match serde_json::from_slice::<Manifest>(&bytes)
                .context("invalid guidance manifest")
                .and_then(|m| {
                    validate_manifest(&m)?;
                    Ok(m)
                }) {
                Ok(m) => r.installed = Some(m),
                Err(e) => finding(
                    &mut r,
                    FindingKind::ManifestInvalid,
                    root.join(MANIFEST_PATH),
                    e.to_string(),
                    true,
                ),
            }
        }
        Err(e) => finding(
            &mut r,
            FindingKind::UnsafePath,
            root.join(MANIFEST_PATH),
            e.to_string(),
            true,
        ),
    }
    let entry = match entrypoint(&root, r.installed.as_ref()) {
        Ok(entry) => entry,
        Err(e) => {
            finding(
                &mut r,
                FindingKind::UnsafePath,
                root.clone(),
                e.to_string(),
                true,
            );
            "AGENTS.md".into()
        }
    };
    let desired = bundle(&entry);
    r.bundled = manifest(&desired);
    if let Some(m) = &r.installed {
        if m != &r.bundled {
            finding(
                &mut r,
                FindingKind::ManifestStale,
                root.join(MANIFEST_PATH),
                "Managed manifest differs from this CLI bundle.".into(),
                false,
            );
        }
    }
    for name in INSTRUCTIONS {
        let path = root.join(name);
        if *name != entry {
            match safe_destination(&root, name, false).and_then(|p| optional_bytes(&p)) {
                Ok(Some(_)) => {
                    r.instruction_paths.push(path.clone());
                    finding(&mut r, FindingKind::PreservedInstructions, path, "User/contributor instructions are outside managed ownership and will be preserved.".into(), false);
                }
                Ok(None) => {}
                Err(e) => finding(&mut r, FindingKind::UnsafePath, path, e.to_string(), true),
            }
        }
    }
    r.instruction_paths.push(root.join(&entry));
    // Nested instructions are never managed by the Git-root bundle.
    for ancestor in project.ancestors().take_while(|ancestor| *ancestor != root) {
        for name in ["AGENTS.md", "AGENTS.fission.md"] {
            let path = ancestor.join(name);
            if fs::symlink_metadata(&path).is_ok() {
                r.instruction_paths.push(path);
            }
        }
    }
    for (path, bytes) in desired {
        let expected = hash(&bytes);
        let recorded = r
            .installed
            .as_ref()
            .and_then(|m| m.files.iter().find(|f| f.path == path))
            .map(|f| f.sha256.clone());
        let (health, actual) = match safe_destination(&root, &path, false)
            .and_then(|p| optional_bytes(&p))
        {
            Ok(None) => (FileHealth::Missing, None),
            Ok(Some(actual)) => {
                let actual_hash = hash(&actual);
                // A matching current template is safe even if metadata was lost.
                let health = if actual == bytes {
                    FileHealth::Current
                } else if recorded.as_deref() == Some(actual_hash.as_str())
                    || (INSTRUCTIONS.contains(&path.as_str()) && known_instruction(&actual, &path))
                {
                    FileHealth::Stale
                } else {
                    FileHealth::Customized
                };
                if health == FileHealth::Customized {
                    finding(&mut r, FindingKind::Customized, root.join(&path),
                        "Preserved customized/unowned file. Back it up, merge desired guidance explicitly, or move it aside before retrying update; retain user instructions. A generated marker is not edit proof.".into(), true);
                }
                (health, Some(actual_hash))
            }
            Err(e) => {
                finding(
                    &mut r,
                    FindingKind::UnsafePath,
                    root.join(&path),
                    e.to_string(),
                    true,
                );
                (FileHealth::Unsafe, None)
            }
        };
        r.files.push(FileFinding {
            path,
            health,
            installed_sha256: actual,
            recorded_sha256: recorded,
            bundled_sha256: expected,
        });
    }
    r.instruction_paths.retain(|path| {
        fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    });
    if r.findings.iter().any(|f| f.blocks_update) {
        r.status = Status::Conflict;
    } else if r.files.iter().all(|f| f.health == FileHealth::Current)
        && r.installed.as_ref() == Some(&r.bundled)
        && r.framework_dependency.compatibility == Compatibility::ResolvedVersionMatch
    {
        r.status = Status::Healthy;
    }
    if !r.success() {
        if r.findings.iter().any(|f| f.blocks_update) {
            r.recovery.push(recovery("check", &project, "Resolve reported conflicts explicitly without losing customized bytes, then inspect again."));
        } else if r.installed.as_ref() != Some(&r.bundled)
            || r.files.iter().any(|f| f.health != FileHealth::Current)
        {
            r.recovery.push(recovery(
                "update",
                &project,
                "Install missing assets or refresh unmodified managed assets from this CLI.",
            ));
        }
        if r.framework_dependency.compatibility != Compatibility::ResolvedVersionMatch {
            r.recovery.push(Recovery {
                reason: "Inspect the dependency/lockfile or local source and choose a matching CLI/framework revision; asset update alone cannot resolve compatibility.".into(),
                argv: vec!["fission".into(), "--version".into()],
            });
        }
    }
    r
}

fn finding(
    r: &mut GuidanceResult,
    kind: FindingKind,
    path: PathBuf,
    message: String,
    blocks_update: bool,
) {
    r.findings.push(Finding {
        kind,
        path,
        message,
        blocks_update,
    });
}

pub fn invalid_arguments(message: String) -> GuidanceResult {
    let mut r = inspect(Path::new("."), "parse");
    r.status = Status::InvalidArguments;
    r.findings = vec![Finding {
        kind: FindingKind::Io,
        path: PathBuf::new(),
        message,
        blocks_update: true,
    }];
    r.recovery = vec![Recovery {
        reason: "Use skills check|update with --project-dir PATH and optional --json.".into(),
        argv: vec!["fission".into(), "skills".into(), "--help".into()],
    }];
    r
}

/// Validate the entire plan before any managed writes. An on-disk journal and
/// original bytes make interruption recoverable; the manifest is committed last.
pub fn update(project: &Path) -> GuidanceResult {
    let mut r = inspect(project, "update");
    if matches!(r.status, Status::Conflict | Status::Error) {
        return r;
    }
    let entry = r
        .bundled
        .files
        .iter()
        .find(|f| INSTRUCTIONS.contains(&f.path.as_str()))
        .unwrap()
        .path
        .clone();
    let mut desired = bundle(&entry);
    let mut metadata = serde_json::to_vec_pretty(&r.bundled).expect("serializable manifest");
    metadata.push(b'\n');
    desired.insert(MANIFEST_PATH.into(), metadata);
    let mut expected = r
        .files
        .iter()
        .map(|f| (f.path.clone(), f.installed_sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    expected.insert(MANIFEST_PATH.into(), r.installed_manifest_sha256.clone());
    match write_transaction(&r.guidance_root, desired, &expected) {
        Ok(()) => {
            r = inspect(project, "update");
            if r.findings.iter().any(|f| f.blocks_update)
                || r.files.iter().any(|f| f.health != FileHealth::Current)
                || r.installed.as_ref() != Some(&r.bundled)
            {
                r.status = Status::Error;
            } else {
                r.status = Status::Updated;
            }
            r.recovery
                .retain(|x| !x.argv.iter().any(|arg| arg == "update"));
        }
        Err(e) => {
            let root = r.guidance_root.clone();
            finding(&mut r, FindingKind::Io, root, format!("{e:#}"), true);
            r.status = Status::Error;
        }
    }
    r
}

#[derive(Serialize)]
struct JournalEntry {
    path: String,
    original_present: bool,
}

fn write_transaction(
    root: &Path,
    desired: BTreeMap<String, Vec<u8>>,
    expected: &BTreeMap<String, Option<String>>,
) -> Result<()> {
    let mut plan = Vec::new();
    for (relative, bytes) in desired {
        let path = safe_destination(root, &relative, false)?;
        let original = optional_bytes(&path)?;
        if expected.get(&relative) != Some(&original.as_ref().map(|b| hash(b))) {
            bail!(
                "concurrent modification at {}; no managed files written",
                path.display()
            );
        }
        if original.as_deref() != Some(bytes.as_slice()) {
            plan.push((relative, bytes, original));
        }
    }
    // All destinations have been checked. Skip even directory writes when idempotent.
    if plan.is_empty() {
        return Ok(());
    }
    plan.sort_by_key(|(path, _, _)| path == MANIFEST_PATH);
    let tx = safe_destination(root, TRANSACTION_PATH, true)?;
    fs::create_dir_all(tx.parent().unwrap())?;
    fs::create_dir(&tx).context("guidance update lock/transaction already exists")?;
    let stage = (|| -> Result<()> {
        fs::create_dir(tx.join("originals"))?;
        fs::create_dir(tx.join("new"))?;
        for (i, (_, bytes, original)) in plan.iter().enumerate() {
            fs::write(tx.join("new").join(i.to_string()), bytes)?;
            if let Some(original) = original {
                fs::write(tx.join("originals").join(i.to_string()), original)?;
            }
        }
        let journal = plan
            .iter()
            .map(|(path, _, original)| JournalEntry {
                path: path.clone(),
                original_present: original.is_some(),
            })
            .collect::<Vec<_>>();
        fs::write(
            tx.join("journal.json"),
            serde_json::to_vec_pretty(&journal)?,
        )?;
        Ok(())
    })();
    if let Err(e) = stage {
        fs::remove_dir_all(&tx)?;
        return Err(e).context("stage guidance update");
    }
    let mut committed = 0;
    let commit = (|| -> Result<()> {
        for (i, (relative, _, original)) in plan.iter().enumerate() {
            let path = safe_destination(root, relative, false)?;
            if optional_bytes(&path)? != *original {
                bail!("concurrent modification at {}", path.display());
            }
            fs::create_dir_all(path.parent().unwrap())?;
            safe_destination(root, relative, false)?;
            fs::rename(tx.join("new").join(i.to_string()), &path)?;
            committed += 1;
        }
        Ok(())
    })();
    if let Err(error) = commit {
        for (i, (relative, _, original)) in plan.iter().enumerate().take(committed).rev() {
            let path = safe_destination(root, relative, false)
                .with_context(|| format!("rollback blocked; recover from {}", tx.display()))?;
            if original.is_some() {
                fs::rename(tx.join("originals").join(i.to_string()), &path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
        fs::remove_dir_all(&tx)?;
        return Err(error).context("guidance update rolled back");
    }
    fs::remove_dir_all(&tx)
        .context("guidance committed; transaction cleanup failed (inspect journal)")?;
    Ok(())
}

pub fn print_result(result: &GuidanceResult, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(result)?);
    } else {
        println!(
            "Guidance {:?}: CLI {}, API {}, guidance {}, schema {}",
            result.status,
            result.bundled.cli_version,
            result.bundled.framework_api_version,
            result.bundled.guidance_version,
            result.schema_version
        );
        println!(
            "Framework: {:?}: {}",
            result.framework_dependency.compatibility, result.framework_dependency.message
        );
        println!("Guidance root: {}", result.guidance_root.display());
        for file in &result.files {
            println!(
                "{:?}: {}",
                file.health,
                result.guidance_root.join(&file.path).display()
            );
        }
        for f in &result.findings {
            println!("{}: {}", f.path.display(), f.message);
        }
        for recovery in &result.recovery {
            println!("{}\nargv: {:?}", recovery.reason, recovery.argv);
        }
    }
    Ok(())
}

/// Init prints every actual instruction path, including custom fallbacks, without
/// replacing custom guidance. Existing projects must resolve conflicts explicitly.
pub(crate) fn install_for_init(project: &Path) -> Result<()> {
    let r = update(project);
    println!("Fission guidance: {:?}", r.status);
    for path in &r.instruction_paths {
        println!("Read instructions: {}", path.display());
    }
    let router = r.guidance_root.join(SKILL_PATH);
    if fs::symlink_metadata(&router).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink()) {
        println!("Web router: {}", router.display());
    } else {
        eprintln!("Web router unavailable; resolve guidance findings with fission skills check --project-dir {}", project.display());
    }
    for f in r.findings.iter().filter(|f| f.blocks_update) {
        eprintln!("Guidance conflict at {}: {}", f.path.display(), f.message);
    }
    if r.status == Status::Error {
        bail!(
            "guidance installation failed; run fission skills check --project-dir {}",
            project.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
