use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fission-guidance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n[dependencies]\nfission = \"0.15.1\"\n").unwrap();
        fs::write(
            root.join("fission.toml"),
            "# Preserve all bytes\n[app]\nname = \"Fixture\"\napp_id = \"com.example.fixture\"\n",
        )
        .unwrap();
        Self(fs::canonicalize(root).unwrap())
    }
    fn install(&self) -> GuidanceResult {
        let r = update(&self.0);
        assert_eq!(r.status, Status::Updated, "{r:?}");
        r
    }
    fn lock(&self) {
        fs::write(self.0.join("Cargo.lock"), format!("version = 4\n[[package]]\nname = \"fission\"\nversion = \"{}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n", crate::CURRENT_VERSION)).unwrap();
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &entry.path(), out);
            } else if entry.file_type().unwrap().is_file() {
                out.insert(
                    entry.path().strip_prefix(root).unwrap().into(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn write_manifest(root: &Path, m: &Manifest) {
    fs::write(
        root.join(MANIFEST_PATH),
        serde_json::to_vec_pretty(m).unwrap(),
    )
    .unwrap();
}

#[test]
fn fresh_bundle_versions_hashes_and_links_resolve() {
    let p = Project::new();
    let before = fs::read(p.0.join("fission.toml")).unwrap();
    let r = p.install();
    assert_eq!(
        r.framework_dependency.compatibility,
        Compatibility::Unresolved
    );
    assert_eq!(r.bundled, bundled_manifest());
    assert_eq!(r.bundled.cli_version, crate::CURRENT_VERSION);
    assert_eq!(r.bundled.framework_api_version, FRAMEWORK_API_VERSION);
    assert_eq!(r.bundled.guidance_version, GUIDANCE_VERSION);
    assert_eq!(fs::read(p.0.join("fission.toml")).unwrap(), before);
    for file in &r.bundled.files {
        let bytes = fs::read(p.0.join(&file.path)).unwrap();
        assert_eq!(hash(&bytes), file.sha256);
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("{{"));
        // All shipped Markdown links are relative, local and resolvable.
        for segment in text.split("](").skip(1) {
            let target = segment.split(')').next().unwrap();
            assert!(
                p.0.join(&file.path)
                    .parent()
                    .unwrap()
                    .join(target)
                    .is_file(),
                "{file:?}: {target}"
            );
        }
    }
    assert_eq!(r.bundled.files.len(), 6);
}

#[test]
fn check_is_read_only_and_idempotent_update_does_not_rewrite_files() {
    let p = Project::new();
    p.lock();
    p.install();
    let before = snapshot(&p.0);
    let times = before
        .keys()
        .map(|path| {
            (
                path.clone(),
                fs::metadata(p.0.join(path)).unwrap().modified().unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(check(&p.0).status, Status::Healthy);
    assert_eq!(snapshot(&p.0), before);
    p.install();
    assert_eq!(snapshot(&p.0), before);
    for (path, time) in times {
        assert_eq!(
            fs::metadata(p.0.join(path)).unwrap().modified().unwrap(),
            time
        );
    }
    assert!(!p.0.join(TRANSACTION_PATH).exists());
}

#[test]
fn missing_assets_are_recreated_without_touching_unrelated_skills() {
    let p = Project::new();
    p.lock();
    p.install();
    fs::create_dir_all(p.0.join(".fission/skills/user")).unwrap();
    fs::write(
        p.0.join(".fission/skills/user/SKILL.md"),
        "custom user skill",
    )
    .unwrap();
    fs::remove_file(p.0.join(SKILL_PATH)).unwrap();
    let before = snapshot(&p.0);
    let r = check(&p.0);
    assert_eq!(r.status, Status::NeedsAttention);
    assert!(r
        .files
        .iter()
        .any(|f| f.path == SKILL_PATH && f.health == FileHealth::Missing));
    assert_eq!(snapshot(&p.0), before);
    p.install();
    assert_eq!(
        fs::read_to_string(p.0.join(".fission/skills/user/SKILL.md")).unwrap(),
        "custom user skill"
    );
    assert_eq!(check(&p.0).status, Status::Healthy);
}

#[test]
fn older_hash_proven_bundle_refreshes_coherently() {
    let p = Project::new();
    p.lock();
    let mut old = p.install().bundled;
    old.guidance_version = 1;
    old.cli_version = "0.14.0".into();
    old.framework_api_version = "0.14.0".into();
    for f in &mut old.files {
        let bytes = format!("old bundled content: {}", f.path).into_bytes();
        fs::write(p.0.join(&f.path), &bytes).unwrap();
        f.sha256 = hash(&bytes);
    }
    write_manifest(&p.0, &old);
    let before = snapshot(&p.0);
    let r = check(&p.0);
    assert_eq!(r.status, Status::NeedsAttention);
    assert!(r.files.iter().all(|f| f.health == FileHealth::Stale));
    assert_eq!(snapshot(&p.0), before);
    p.install();
    assert_eq!(check(&p.0).status, Status::Healthy);
}

#[test]
fn customization_blocks_every_write_even_with_missing_and_stale_files() {
    let p = Project::new();
    let mut old = p.install().bundled;
    old.cli_version = "0.14.0".into();
    old.guidance_version = 1;
    let reference = &mut old
        .files
        .iter_mut()
        .find(|f| f.path.ends_with("testing-review.md"))
        .unwrap();
    fs::write(p.0.join(&reference.path), "old managed reference").unwrap();
    reference.sha256 = hash(b"old managed reference");
    write_manifest(&p.0, &old);
    fs::write(
        p.0.join("AGENTS.md"),
        format!(
            "{}\nUser instructions: never erase me.\n",
            render(ROUTER, true, "AGENTS.md")
        ),
    )
    .unwrap();
    fs::remove_file(p.0.join(SKILL_PATH)).unwrap();
    let before = snapshot(&p.0);
    let r = update(&p.0);
    assert_eq!(r.status, Status::Conflict);
    assert!(r.files.iter().any(|f| f.health == FileHealth::Customized));
    assert_eq!(snapshot(&p.0), before);
    assert!(!p.0.join(TRANSACTION_PATH).exists());
}

#[test]
fn custom_root_fallback_and_nested_instructions_are_preserved() {
    let p = Project::new();
    fs::write(p.0.join("AGENTS.md"), "contributor policy").unwrap();
    fs::write(p.0.join("AGENTS.fission.md"), "custom fallback").unwrap();
    let app = p.0.join("apps/site");
    fs::create_dir_all(&app).unwrap();
    fs::write(app.join("AGENTS.md"), "nested user policy").unwrap();
    fs::write(app.join("AGENTS.fission.md"), "nested fission policy").unwrap();
    fs::copy(p.0.join("Cargo.toml"), app.join("Cargo.toml")).unwrap();
    let before = snapshot(&p.0);
    let r = update(&app);
    assert_eq!(r.status, Status::Updated);
    assert_eq!(r.guidance_root, p.0);
    assert!(r
        .bundled
        .files
        .iter()
        .any(|f| f.path == ".fission/AGENTS.md"));
    for (path, bytes) in before {
        assert_eq!(fs::read(p.0.join(path)).unwrap(), bytes);
    }
    assert!(r.instruction_paths.contains(&app.join("AGENTS.md")));
    let text = fs::read_to_string(p.0.join(".fission/AGENTS.md")).unwrap();
    assert!(p
        .0
        .join(".fission")
        .join(text.split("](").nth(1).unwrap().split(')').next().unwrap())
        .is_file());
}

#[test]
fn custom_root_gets_root_fission_fallback() {
    let p = Project::new();
    fs::write(p.0.join("AGENTS.md"), "root policy").unwrap();
    let r = p.install();
    assert!(r
        .bundled
        .files
        .iter()
        .any(|f| f.path == "AGENTS.fission.md"));
    assert_eq!(
        fs::read_to_string(p.0.join("AGENTS.md")).unwrap(),
        "root policy"
    );
}

#[test]
fn known_legacy_template_migrates_but_edited_markers_never_prove_ownership() {
    let p = Project::new();
    fs::write(p.0.join("AGENTS.md"), LEGACY).unwrap();
    let r = check(&p.0);
    assert_eq!(
        r.files
            .iter()
            .find(|f| f.path == "AGENTS.md")
            .unwrap()
            .health,
        FileHealth::Stale
    );
    p.install();
    for content in [
        format!("{LEGACY}\nuser edit"),
        "<!-- fission-cli-generated-agents:v1 -->\nuser policy".into(),
    ] {
        let p = Project::new();
        fs::write(p.0.join("AGENTS.md"), &content).unwrap();
        let before = snapshot(&p.0);
        assert_eq!(update(&p.0).status, Status::Conflict);
        assert_eq!(snapshot(&p.0), before);
    }
}

#[test]
fn malformed_and_unsafe_manifests_block_without_partial_edits() {
    let p = Project::new();
    let original = p.install().bundled;
    for path in [
        "../outside",
        "/absolute",
        "C:/drive",
        ".fission/../escape",
        ".fission//x",
        ".fission/./x",
        ".fission\\escape",
        ".fission/skills/user/SKILL.md",
    ] {
        let mut m = original.clone();
        m.files[0].path = path.into();
        write_manifest(&p.0, &m);
        let before = snapshot(&p.0);
        assert_eq!(update(&p.0).status, Status::Conflict, "{path}");
        assert_eq!(snapshot(&p.0), before);
    }
    for mutate in 0..4 {
        let mut m = original.clone();
        match mutate {
            0 => m.files.push(m.files[0].clone()),
            1 => m.schema_version = 900,
            2 => m.files[0].sha256 = "invalid".into(),
            _ => m.files.retain(|f| !INSTRUCTIONS.contains(&f.path.as_str())),
        }
        write_manifest(&p.0, &m);
        let before = snapshot(&p.0);
        assert_eq!(update(&p.0).status, Status::Conflict);
        assert_eq!(snapshot(&p.0), before);
    }
    fs::write(p.0.join(MANIFEST_PATH), "{bad JSON").unwrap();
    let before = snapshot(&p.0);
    assert_eq!(update(&p.0).status, Status::Conflict);
    assert_eq!(snapshot(&p.0), before);
}

#[test]
fn file_directory_collisions_are_detected_before_assets_are_written() {
    for relative in [".fission", ".fission/skills", ".fission/skills/fission-web"] {
        let p = Project::new();
        let path = p.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "user file").unwrap();
        let before = snapshot(&p.0);
        assert_eq!(update(&p.0).status, Status::Conflict);
        assert_eq!(snapshot(&p.0), before);
    }
    let p = Project::new();
    fs::create_dir(p.0.join("AGENTS.md")).unwrap();
    assert_eq!(update(&p.0).status, Status::Conflict);
    assert!(!p.0.join(".fission").exists());
}

#[cfg(unix)]
#[test]
fn unsafe_symlinks_including_dangling_destinations_never_write_outside_root() {
    use std::os::unix::fs::symlink;
    for relative in [
        "AGENTS.md",
        MANIFEST_PATH,
        ".fission",
        ".fission/skills",
        SKILL_PATH,
        TRANSACTION_PATH,
    ] {
        let p = Project::new();
        let outside = Project::new();
        let path = p.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(outside.0.join("absent"), &path).unwrap();
        let before = snapshot(&p.0);
        let external = snapshot(&outside.0);
        assert_eq!(update(&p.0).status, Status::Conflict, "{relative}");
        assert_eq!(snapshot(&p.0), before);
        assert_eq!(snapshot(&outside.0), external);
        assert!(fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}

#[test]
fn interruption_requires_explicit_recovery() {
    let p = Project::new();
    p.install();
    fs::create_dir(p.0.join(TRANSACTION_PATH)).unwrap();
    fs::write(
        p.0.join(TRANSACTION_PATH).join("journal.json"),
        "recoverable journal",
    )
    .unwrap();
    let before = snapshot(&p.0);
    let r = update(&p.0);
    assert_eq!(r.status, Status::Conflict);
    assert!(r
        .findings
        .iter()
        .any(|f| f.kind == FindingKind::IncompleteTransaction));
    assert_eq!(snapshot(&p.0), before);
}

#[test]
fn changes_between_inspection_and_transaction_are_not_overwritten() {
    let p = Project::new();
    p.install();
    let r = check(&p.0);
    let expected = r
        .files
        .iter()
        .map(|f| (f.path.clone(), f.installed_sha256.clone()))
        .collect();
    fs::write(p.0.join(SKILL_PATH), "concurrent edit").unwrap();
    let before = snapshot(&p.0);
    assert!(write_transaction(&p.0, bundle("AGENTS.md"), &expected).is_err());
    assert_eq!(snapshot(&p.0), before);
}

#[test]
fn dependency_versions_distinguish_evidence_from_assets() {
    let p = Project::new();
    p.lock();
    p.install();
    assert_eq!(
        check(&p.0).framework_dependency.compatibility,
        Compatibility::ResolvedVersionMatch
    );
    let cases = [
        (
            "{ version = \"0.15.1\", path = \"../missing\" }",
            Compatibility::LocalPathUnverified,
        ),
        (
            "{ version = \"0.15.1\", git = \"https://example.invalid/fission\" }",
            Compatibility::GitUnverified,
        ),
        ("{ workspace = true }", Compatibility::Unresolved),
        ("\"not a version\"", Compatibility::Unparseable),
        ("\"=0.14.0\"", Compatibility::VersionMismatch),
    ];
    for (dep, expected) in cases {
        fs::write(
            p.0.join("Cargo.toml"),
            format!("[dependencies]\nfission = {dep}\n"),
        )
        .unwrap();
        let before = snapshot(&p.0);
        let r = check(&p.0);
        assert_eq!(r.framework_dependency.compatibility, expected);
        assert!(!r.success());
        assert_eq!(snapshot(&p.0), before);
    }
    fs::write(p.0.join("Cargo.toml"), "[dependencies]\nother = \"1\"").unwrap();
    assert_eq!(
        check(&p.0).framework_dependency.compatibility,
        Compatibility::Missing
    );
    fs::write(p.0.join("Cargo.toml"), "unparseable TOML").unwrap();
    assert_eq!(
        check(&p.0).framework_dependency.compatibility,
        Compatibility::Unparseable
    );
}

#[test]
fn json_schema_round_trip_and_recovery_argv_preserve_project_path() {
    let p = Project::new();
    let path = p.0.join("app with spaces");
    fs::create_dir(&path).unwrap();
    fs::copy(p.0.join("Cargo.toml"), path.join("Cargo.toml")).unwrap();
    let r = check(&path);
    let json = serde_json::to_string(&r).unwrap();
    let parsed: GuidanceResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.recovery[0].argv[4], path.to_string_lossy());
    assert_eq!(parsed.coverage.domains.len(), 4);
    assert!(!json.contains("updateAvailable"));
}

#[test]
fn relative_paths_are_absolute_without_canonicalizing_caller_spelling() {
    let p = Project::new();
    let cwd = std::env::current_dir().unwrap();
    // Compute a relative path without changing the process-wide working directory.
    let from = cwd.components().collect::<Vec<_>>();
    let to = p.0.components().collect::<Vec<_>>();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for part in &to[common..] {
        relative.push(part.as_os_str());
    }
    let spelling = cwd.join(&relative);
    let r = update(&relative);
    assert_eq!(r.status, Status::Updated);
    assert_eq!(r.project_dir.as_os_str(), spelling.as_os_str());
    assert_eq!(r.guidance_root.as_os_str(), spelling.as_os_str());
    assert!(r
        .instruction_paths
        .iter()
        .any(|path| path.as_os_str() == spelling.join("AGENTS.md").as_os_str()));
    assert_eq!(
        check(&relative).project_dir.as_os_str(),
        spelling.as_os_str()
    );
}

#[cfg(unix)]
#[test]
fn mapped_symlink_paths_use_lexical_git_ancestors_and_keep_reported_spelling() {
    use std::os::unix::fs::symlink;
    let actual = Project::new();
    let mapped = Project::new();
    let app = actual.0.join("apps/nested/app");
    fs::create_dir_all(&app).unwrap();
    fs::copy(actual.0.join("Cargo.toml"), app.join("Cargo.toml")).unwrap();
    fs::write(actual.0.join("apps/AGENTS.md"), "intermediate policy").unwrap();
    fs::write(app.join("AGENTS.fission.md"), "nested policy").unwrap();
    symlink(actual.0.join("apps"), mapped.0.join("apps")).unwrap();
    let alias = mapped.0.join("apps/nested/app");
    let r = update(&alias);
    assert_eq!(r.status, Status::Updated);
    assert_eq!(r.project_dir, alias);
    assert_eq!(r.guidance_root, mapped.0);
    assert!(r
        .instruction_paths
        .contains(&mapped.0.join("apps/AGENTS.md")));
    assert!(r
        .instruction_paths
        .contains(&alias.join("AGENTS.fission.md")));
    assert!(!actual.0.join("AGENTS.md").exists());
    assert!(!actual.0.join(MANIFEST_PATH).exists());
    assert_eq!(
        fs::read_to_string(actual.0.join("apps/AGENTS.md")).unwrap(),
        "intermediate policy"
    );

    // An explicitly supplied root alias is supported; symlinks *inside managed
    // destinations* are still rejected by the existing unsafe-destination tests.
    symlink(&actual.0, mapped.0.join("root alias")).unwrap();
    let root_alias = mapped.0.join("root alias");
    let r = update(&root_alias);
    assert_eq!(r.status, Status::Updated);
    assert_eq!(r.guidance_root, root_alias);
    assert_eq!(check(&root_alias).project_dir, root_alias);
}
