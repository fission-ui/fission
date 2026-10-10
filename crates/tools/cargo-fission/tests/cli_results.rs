use fission_command_core::report::{CommandResult, Data, ErrorCode, Outcome, Step};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fission"))
        .current_dir(cwd)
        .args(args)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap()
}

fn result(output: &Output) -> CommandResult {
    let value: CommandResult = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one result: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(value.schema, "fission.cli-result.v1");
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    assert!(value.project_dir.as_ref().unwrap().is_absolute());
    assert_eq!(
        output.status.success(),
        matches!(value.outcome, Outcome::Success { .. })
    );
    value
}

fn success(output: &Output) -> Data {
    match result(output).outcome {
        Outcome::Success { data } => data,
        Outcome::Failure { error } => panic!("unexpected failure: {error:?}"),
    }
}

fn failure(output: &Output, code: ErrorCode) -> Vec<Step> {
    assert_ne!(output.status.code(), Some(0));
    match result(output).outcome {
        Outcome::Failure { error } => {
            assert_eq!(error.code, code);
            assert!(error.diagnostics.len() <= 8192);
            assert!(!error.recovery.is_empty());
            error.recovery
        }
        _ => panic!("expected failure"),
    }
}

fn follow(step: &Step) -> Output {
    assert_eq!(step.invocation.program, "fission");
    let argv = step
        .invocation
        .argv
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    cli(&argv, &step.invocation.cwd)
}

fn init(root: &Path) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let data = success(&cli(
        &[
            "init",
            root.to_str().unwrap(),
            "--name",
            "website-fixture",
            "--local-path",
            repo.to_str().unwrap(),
            "--json",
        ],
        root.parent().unwrap(),
    ));
    // Read immediately after initialization, before target/source edits.
    assert!(!data.instructions.is_empty());
    for path in data.instructions {
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("Fission App Guidelines"));
        assert!(text.contains("fission.toml"));
    }
}

fn static_fixture(root: &Path) {
    init(root);
    success(&cli(&["add-target", "static-site", "--json"], root));
}

#[test]
fn fresh_content_site_uses_real_report_artifacts_and_preserves_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("a website with spaces");
    static_fixture(&root);
    let config = fs::read(root.join("fission.toml")).unwrap();
    let routes = success(&cli(&["site", "routes", "--json"], &root));
    assert!(!routes.targets[0].routes.is_empty());
    assert!(routes.targets[0].artifact_dir.is_none());
    assert!(routes.targets[0].artifacts.is_empty());
    let check = success(&cli(&["site", "check", "--json"], &root));
    assert!(check.targets[0].artifact_dir.is_none());
    assert!(check.targets[0].artifacts.is_empty());
    assert!(!check.targets[0]
        .planned_output_dir
        .as_ref()
        .unwrap()
        .exists());
    let built = success(&cli(&["build", "--target", "static-site", "--json"], &root));
    assert!(built.targets[0].artifact_dir.as_ref().unwrap().is_dir());
    assert!(built.targets[0].artifacts.iter().all(|path| path.is_file()));
    assert!(built.targets[0]
        .routes
        .iter()
        .all(|route| route.output.is_file()));
    assert!(built.next_steps.is_empty());
    let direct = success(&cli(&["site", "build", "--json"], &root));
    assert_eq!(
        built.targets[0].artifact_dir,
        direct.targets[0].artifact_dir
    );
    assert_eq!(config, fs::read(root.join("fission.toml")).unwrap());
    let human = cli(&["site", "build"], &root);
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).starts_with("Built "));
    assert!(serde_json::from_slice::<serde_json::Value>(&human.stdout).is_err());
}

#[test]
fn missing_project_and_unconfigured_target_have_executable_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("not created yet");
    let output = cli(
        &[
            "site",
            "build",
            "--project-dir",
            root.to_str().unwrap(),
            "--json",
        ],
        temp.path(),
    );
    let recovery = failure(&output, ErrorCode::ProjectNotFound);
    let registered = success(&follow(&recovery[0]));
    for path in registered.instructions {
        fs::read_to_string(path).unwrap();
    }
    let output = cli(&["build", "--target", "static-site", "--json"], &root);
    let recovery = failure(&output, ErrorCode::TargetNotConfigured);
    success(&follow(&recovery[0]));
    success(&follow(recovery.last().unwrap()));
    fs::remove_file(root.join("platforms/site/README.md")).unwrap();
    let recovery = failure(
        &cli(&["build", "--target", "static-site", "--json"], &root),
        ErrorCode::ScaffoldMissing,
    );
    success(&follow(&recovery[0]));
    success(&follow(recovery.last().unwrap()));
}

#[test]
fn configuration_failure_is_typed_then_recovers_after_repair() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("config failure");
    static_fixture(&root);
    let path = root.join("fission.toml");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("{original}\n[site]\nout_dir = 123\n")).unwrap();
    let recovery = failure(
        &cli(&["site", "check", "--json"], &root),
        ErrorCode::InvalidConfiguration,
    );
    fs::write(&path, &original).unwrap();
    success(&follow(recovery.last().unwrap()));
    fs::write(&path, "not valid = [\nsecret = 'do-not-echo-me'\n").unwrap();
    let output = cli(&["site", "build", "--json"], &root);
    failure(&output, ErrorCode::InvalidConfiguration);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("do-not-echo-me"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("do-not-echo-me"));
}

#[test]
fn parsed_target_errors_and_clap_errors_have_distinct_exit_contracts() {
    let temp = tempfile::tempdir().unwrap();
    failure(
        &cli(&["add-target", "--json"], temp.path()),
        ErrorCode::InvalidTarget,
    );
    for args in [
        vec!["build", "--target", "linux", "--json"],
        vec!["build", "--json"],
        vec!["add-target", "ios", "--json"],
    ] {
        failure(&cli(&args, temp.path()), ErrorCode::ProjectNotFound);
    }
    for args in [
        vec!["build", "--target", "invalid", "--json"],
        vec!["init", "--json"],
        vec!["site", "build", "--wat", "--json"],
        vec!["doctor", "--json"],
    ] {
        let output = cli(&args, temp.path());
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn nested_init_reports_ancestor_instructions_and_preserves_custom_guidance() {
    let temp = tempfile::tempdir().unwrap();
    let canonical_root = temp.path().canonicalize().unwrap();
    fs::create_dir(temp.path().join(".git")).unwrap();
    fs::write(temp.path().join("AGENTS.md"), "Custom project instructions").unwrap();
    let root = temp.path().join("nested project");
    let data = success(&cli(
        &["init", root.to_str().unwrap(), "--json"],
        temp.path(),
    ));
    assert!(data
        .instructions
        .contains(&canonical_root.join("AGENTS.md")));
    assert!(data
        .instructions
        .contains(&canonical_root.join("AGENTS.fission.md")));
    assert_eq!(
        fs::read_to_string(temp.path().join("AGENTS.md")).unwrap(),
        "Custom project instructions"
    );
    for path in data.instructions {
        fs::read_to_string(path).unwrap();
    }
}

fn programmatic_fixture(root: &Path) {
    static_fixture(root);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let config = fs::read_to_string(root.join("fission.toml")).unwrap();
    // Distinct executable names keep concurrent fixtures from replacing each
    // other's Cargo binary in the shared dependency cache.
    let binary_name = root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    fs::write(
        root.join("fission.toml"),
        format!("{config}\n[site]\nentry = 'src/main.rs'\nout_dir = 'output with spaces'\n"),
    )
    .unwrap();
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "website-fixture"
version = "0.1.0"
edition = "2021"
autobins = false
[[bin]]
name = {:?}
path = "src/main.rs"
[lib]
path = "src/empty.rs"
[dependencies]
anyhow = "1"
fission-shell-site = {{ path = {:?} }}
[profile.dev]
debug = "line-tables-only"
[profile.dev.package."*"]
debug = false
opt-level = 1
"#,
            binary_name,
            repo.join("crates/shell/fission-shell-site")
        ),
    )
    .unwrap();
    fs::write(root.join("src/empty.rs"), "").unwrap();
}

#[test]
fn real_compiler_failure_then_programmatic_build_with_noisy_stdout_recovers() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("compiled site with spaces");
    programmatic_fixture(&root);
    fs::write(
        root.join("src/main.rs"),
        "fn main() { let _: u32 = \"compile failure\"; }\n",
    )
    .unwrap();
    let output = cli(&["site", "build", "--json"], &root);
    let recovery = failure(&output, ErrorCode::CompileFailed);
    assert!(String::from_utf8_lossy(&output.stdout).contains("E0308"));
    let human_failure = cli(&["site", "build"], &root);
    assert_eq!(human_failure.status.code(), output.status.code());
    assert!(human_failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&human_failure.stderr).contains("Compilation failed"));
    assert!(String::from_utf8_lossy(&human_failure.stderr).contains("E0308"));
    fs::write(
        root.join("src/main.rs"),
        r#"fn main() -> anyhow::Result<()> {
        println!("{}", "noise from site application ".repeat(2000));
        if let Ok(secret) = std::env::var("FISSION_TEST_API_TOKEN") { println!("{secret}"); }
        fission_shell_site::build_from_cli(fission_shell_site::FissionSite::new())
    }
"#,
    )
    .unwrap();
    let step = recovery.last().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fission"))
        .current_dir(&step.invocation.cwd)
        .args(&step.invocation.argv)
        .env("CARGO_NET_OFFLINE", "true")
        .env(
            "FISSION_TEST_API_TOKEN",
            "inherited-secret-must-be-redacted",
        )
        .output()
        .unwrap();
    let data = success(&output);
    assert!(data.targets[0]
        .artifact_dir
        .as_ref()
        .unwrap()
        .ends_with("output with spaces"));
    assert!(data.targets[0].artifacts.iter().all(|path| path.is_file()));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("inherited-secret-must-be-redacted"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("inherited-secret-must-be-redacted"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("[REDACTED]"));
    let routes = success(&cli(&["site", "routes", "--json"], &root));
    assert!(!routes.targets[0].routes.is_empty());
    assert!(routes.targets[0].artifact_dir.is_none());
    let human_build = cli(&["site", "build"], &root);
    assert!(human_build.status.success());
    assert!(String::from_utf8_lossy(&human_build.stdout).starts_with("Built "));
    assert!(!String::from_utf8_lossy(&human_build.stdout).contains("noise from site application"));
    let human_routes = cli(&["site", "routes"], &root);
    assert!(human_routes.status.success());
    for route in &routes.targets[0].routes {
        assert!(String::from_utf8_lossy(&human_routes.stdout).contains(&route.path));
        assert!(String::from_utf8_lossy(&human_routes.stdout).contains(&route.title));
    }
    let check = success(&cli(&["site", "check", "--json"], &root));
    let human_check = cli(&["site", "check"], &root);
    assert!(human_check.status.success());
    assert!(
        String::from_utf8_lossy(&human_check.stdout).contains(&format!(
            "Checked {} static route(s)",
            check.targets[0].routes.len()
        ))
    );
}

#[test]
fn missing_cargo_is_toolchain_failure_after_argument_parsing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("missing tool site");
    programmatic_fixture(&root);
    let output = Command::new(env!("CARGO_BIN_EXE_fission"))
        .args(["site", "build", "--json"])
        .current_dir(&root)
        .env("PATH", temp.path().join("no executables"))
        .output()
        .unwrap();
    let recovery = failure(&output, ErrorCode::MissingToolchain);
    assert_eq!(
        recovery.last().unwrap().invocation.argv,
        ["site", "build", "--json"]
    );
}

#[test]
fn an_uninstalled_selected_rust_toolchain_has_typed_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("selected toolchain");
    programmatic_fixture(&root);
    fs::write(root.join("src/main.rs"), "fn main() -> anyhow::Result<()> { fission_shell_site::build_from_cli(fission_shell_site::FissionSite::new()) }\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fission"))
        .current_dir(&root)
        .args(["site", "build", "--json"])
        .env("RUSTUP_TOOLCHAIN", "fission-uninstalled-toolchain-fixture")
        .output()
        .unwrap();
    let recovery = failure(&output, ErrorCode::MissingToolchain);
    // Select the installed default toolchain again, then execute the exact retry.
    success(&follow(recovery.last().unwrap()));
}

#[test]
fn invalid_cargo_configuration_does_not_echo_source_or_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("invalid Cargo config");
    programmatic_fixture(&root);
    fs::write(root.join("Cargo.toml"), "[package]\nname = 'website-fixture'\nsecret = 'credential-in-configuration'\nversion = [oops\n").unwrap();
    let output = cli(&["site", "build", "--json"], &root);
    failure(&output, ErrorCode::InvalidConfiguration);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("credential-in-configuration"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("credential-in-configuration"));
}

#[test]
fn established_readiness_json_retains_its_original_shape() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("legacy JSON");
    static_fixture(&root);
    let output = cli(
        &[
            "readiness",
            "package",
            "--target",
            "static-site",
            "--format",
            "static",
            "--json",
        ],
        &root,
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("checks").is_some());
    assert!(value.get("status").is_some());
    assert!(value.get("schema").is_none());
    assert!(value.get("outcome").is_none());
}

#[test]
fn setup_never_reports_directories_as_successful_file_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("setup artifact paths");
    fs::create_dir_all(root.join("Cargo.toml")).unwrap();
    let output = cli(
        &[
            "init",
            root.to_str().unwrap(),
            "--name",
            "setup-fixture",
            "--json",
        ],
        temp.path(),
    );
    let recovery = failure(&output, ErrorCode::ArtifactMissing);
    // Read generated guidance immediately after the partially completed init.
    fs::read_to_string(root.join("AGENTS.md")).unwrap();
    fs::remove_dir(root.join("Cargo.toml")).unwrap();
    success(&follow(recovery.last().unwrap()));
    fs::create_dir_all(root.join("platforms/site/README.md")).unwrap();
    let recovery = failure(
        &cli(&["add-target", "static-site", "--json"], &root),
        ErrorCode::ArtifactMissing,
    );
    fs::remove_dir(root.join("platforms/site/README.md")).unwrap();
    let data = success(&follow(recovery.last().unwrap()));
    assert!(data.artifacts.iter().all(|path| path.is_file()));
}

#[cfg(unix)]
#[test]
fn non_utf8_execution_paths_remain_valid_and_json_fallback_is_complete() {
    use std::os::unix::ffi::OsStringExt;
    let temp = tempfile::tempdir().unwrap();
    let human_root = temp
        .path()
        .join(std::ffi::OsString::from_vec(b"human-\xff".to_vec()));
    let json_root = temp
        .path()
        .join(std::ffi::OsString::from_vec(b"json-\xff".to_vec()));
    let invoke = |root: &Path, json: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fission"));
        command
            .current_dir(temp.path())
            .args(["init", "--name", "path-parity"])
            .arg(root);
        if json {
            command.arg("--json");
        }
        command.output().unwrap()
    };
    let supports_non_utf8 = match fs::create_dir(&human_root) {
        Ok(()) => true,
        // Some filesystems (including macOS APFS) reject these bytes themselves.
        Err(error) if matches!(error.raw_os_error(), Some(84 | 92)) => false,
        Err(error) => panic!("cannot probe filesystem path support: {error}"),
    };
    let human = invoke(&human_root, false);
    let output = invoke(&json_root, true);
    assert_eq!(output.status.code(), Some(1));
    let value: CommandResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.schema, "fission.cli-result.v1");
    assert!(value.project_dir.is_none());
    assert!(value.state.is_none());
    assert!(
        matches!(value.outcome, Outcome::Failure { error } if error.code == ErrorCode::ReportUnavailable)
    );
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    if !supports_non_utf8 {
        assert_eq!(human.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&human.stderr)
                .contains("Project files could not be read or updated"),
            "the filesystem failure must not be replaced with UTF-8 validation"
        );
        assert!(!json_root.exists());
        return;
    }
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    assert!(
        json_root.join("fission.toml").is_file(),
        "execution must not depend on serialization"
    );
    assert_eq!(snapshot(&human_root), snapshot(&json_root));
    let human = cli(&["init", "cwd-human", "--name", "cwd-parity"], &human_root);
    assert!(human.status.success());
    let output = cli(
        &["init", "cwd-json", "--name", "cwd-parity", "--json"],
        &human_root,
    );
    let value: CommandResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.project_dir.is_none() && value.state.is_none());
    assert!(
        matches!(value.outcome, Outcome::Failure { error } if error.code == ErrorCode::ReportUnavailable)
    );
    assert_eq!(
        snapshot(&human_root.join("cwd-human")),
        snapshot(&human_root.join("cwd-json"))
    );
}

#[test]
#[ignore = "requires installed wasm-pack and wasm32 Rust toolchain; run explicitly for release validation"]
fn real_web_wasm_build_reports_existing_hosting_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("real Web fixture with spaces");
    init(&root);
    success(&cli(&["add-target", "web", "--json"], &root));
    let original = fs::read(root.join("fission.toml")).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_fission"));
    command
        .current_dir(&root)
        .args(["build", "--target", "web", "--json"])
        .env("CARGO_NET_OFFLINE", "true");
    if let Some(target_dir) = std::env::var_os("FISSION_WEBSITE_WASM_TARGET_DIR") {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    let mut human_command = Command::new(env!("CARGO_BIN_EXE_fission"));
    human_command
        .current_dir(&root)
        .args(["build", "--target", "web"])
        .env("CARGO_NET_OFFLINE", "true");
    if let Some(target_dir) = std::env::var_os("FISSION_WEBSITE_WASM_TARGET_DIR") {
        human_command.env("CARGO_TARGET_DIR", target_dir);
    }
    let human = human_command.output().unwrap();
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    let output = command.output().unwrap();
    assert_eq!(human.status.code(), output.status.code());
    let data = success(&output);
    assert!(data.targets[0]
        .artifact_dir
        .as_ref()
        .unwrap()
        .ends_with("platforms/web"));
    assert!(data.targets[0]
        .artifact_dir
        .as_ref()
        .unwrap()
        .join("index.html")
        .is_file());
    let wasm = data.targets[0]
        .artifacts
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap();
    assert_eq!(&fs::read(wasm).unwrap()[..4], b"\0asm");
    assert!(data.targets[0].artifacts.iter().all(|path| path.is_file()));
    for path in &data.targets[0].artifacts {
        assert!(String::from_utf8_lossy(&human.stdout).contains(path.to_str().unwrap()));
    }
    assert_eq!(original, fs::read(root.join("fission.toml")).unwrap());
}

fn snapshot(root: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn visit(
        root: &Path,
        dir: &Path,
        result: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path
                .file_name()
                .is_some_and(|name| name == "target" || name == ".git")
            {
                continue;
            }
            if path.is_dir() {
                visit(root, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = std::collections::BTreeMap::new();
    visit(root, root, &mut result);
    result
}

#[test]
fn setup_modes_have_identical_side_effects_for_every_existing_target() {
    let temp = tempfile::tempdir().unwrap();
    let human_root = temp.path().join("human");
    let json_root = temp.path().join("json");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let human = cli(
        &[
            "init",
            human_root.to_str().unwrap(),
            "--name",
            "mode-parity",
            "--local-path",
            repo.to_str().unwrap(),
        ],
        temp.path(),
    );
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    let json = cli(
        &[
            "init",
            json_root.to_str().unwrap(),
            "--name",
            "mode-parity",
            "--local-path",
            repo.to_str().unwrap(),
            "--json",
        ],
        temp.path(),
    );
    let report = result(&json);
    assert_eq!(human.status.code(), json.status.code());
    assert_eq!(report.state.unwrap().configured_targets.len(), 3);
    let Outcome::Success { data } = report.outcome else {
        panic!("init failed");
    };
    assert!(
        data.next_steps.is_empty(),
        "init must not choose subsequent target intent"
    );
    assert_eq!(snapshot(&human_root), snapshot(&json_root));
    for path in data.instructions {
        fs::read_to_string(path).unwrap();
    }
    let args = [
        "add-target",
        "web",
        "static-site",
        "terminal",
        "ssr",
        "ios",
        "android",
        "linux",
        "macos",
        "windows",
    ];
    let human = cli(&args, &human_root);
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let json = cli(&json_args, &json_root);
    assert_eq!(human.status.code(), json.status.code());
    let data = success(&json);
    assert!(data.artifacts.iter().all(|path| path.is_file()));
    assert_eq!(snapshot(&human_root), snapshot(&json_root));
    // Existing user files survive re-initialization and scaffold repair in both modes.
    for root in [&human_root, &json_root] {
        fs::write(root.join("AGENTS.md"), "custom guidance").unwrap();
        fs::write(root.join("platforms/terminal/README.md"), "custom scaffold").unwrap();
    }
    assert!(cli(&["add-target", "terminal"], &human_root)
        .status
        .success());
    success(&cli(&["add-target", "terminal", "--json"], &json_root));
    assert_eq!(snapshot(&human_root), snapshot(&json_root));
    assert_eq!(
        fs::read_to_string(json_root.join("AGENTS.md")).unwrap(),
        "custom guidance"
    );
    assert_eq!(
        fs::read_to_string(json_root.join("platforms/terminal/README.md")).unwrap(),
        "custom scaffold"
    );
}

fn assert_failure_parity(args: &[&str], root: &Path, code: ErrorCode) {
    let before = snapshot(root);
    let human = cli(args, root);
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let json = cli(&json_args, root);
    assert_eq!(human.status.code(), json.status.code());
    assert!(human.stdout.is_empty());
    let Outcome::Failure { error } = result(&json).outcome else {
        panic!("expected failure");
    };
    assert_eq!(error.code, code);
    assert!(String::from_utf8_lossy(&human.stderr).contains(&error.message));
    assert_eq!(
        snapshot(root),
        before,
        "validation must have identical side effects"
    );
}

#[test]
fn generic_validation_errors_and_default_selection_have_mode_parity() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("generic validation");
    init(&root);
    success(&cli(&["add-target", "terminal", "--json"], &root));
    assert_failure_parity(&["add-target"], &root, ErrorCode::InvalidTarget);
    assert_failure_parity(
        &["build", "--target", "terminal", "--features", "fixtures"],
        &root,
        ErrorCode::InvalidTarget,
    );
    assert_failure_parity(
        &["build", "--target", "terminal", "--variant", "app-store"],
        &root,
        ErrorCode::InvalidTarget,
    );
    assert_failure_parity(
        &["build", "--target", "ios"],
        &root,
        ErrorCode::TargetNotConfigured,
    );
    fs::remove_file(root.join("platforms/terminal/README.md")).unwrap();
    assert_failure_parity(
        &["build", "--target", "terminal"],
        &root,
        ErrorCode::ScaffoldMissing,
    );
    fs::write(
        root.join("fission.toml"),
        "not valid = [\nsecret = 'parity-secret'\n",
    )
    .unwrap();
    assert_failure_parity(&["build"], &root, ErrorCode::InvalidConfiguration);
    assert_failure_parity(
        &["add-target", "ios"],
        &root,
        ErrorCode::InvalidConfiguration,
    );
    assert_failure_parity(&["site", "check"], &root, ErrorCode::InvalidConfiguration);
}

fn native_fixture(root: &Path) {
    init(root);
    success(&cli(&["add-target", "terminal", "--json"], root));
    let name = root.file_name().unwrap().to_str().unwrap();
    fs::write(root.join("Cargo.toml"), format!("[package]\nname = {name:?}\nversion = '0.1.0'\nedition = '2021'\n[profile.dev]\ndebug = 'line-tables-only'\n")).unwrap();
    fs::write(root.join("src/lib.rs"), "").unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"existing target works\"); }\n",
    )
    .unwrap();
}

#[test]
fn existing_terminal_and_host_builds_share_artifacts_and_failures_in_both_modes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("native-mode-parity");
    native_fixture(&root);
    let config = fs::read(root.join("fission.toml")).unwrap();
    for args in [vec!["build", "--target", "terminal"], vec!["build"]] {
        let human = cli(&args, &root);
        assert!(
            human.status.success(),
            "{}",
            String::from_utf8_lossy(&human.stderr)
        );
        let mut json_args = args.clone();
        json_args.push("--json");
        let output = cli(&json_args, &root);
        let report = result(&output);
        assert_eq!(human.status.code(), output.status.code());
        let Outcome::Success { data } = report.outcome else {
            panic!("build failed");
        };
        let target = &data.targets[0];
        if args.len() == 1 {
            let host = if cfg!(target_os = "macos") {
                fission_command_core::Target::Macos
            } else if cfg!(target_os = "windows") {
                fission_command_core::Target::Windows
            } else {
                fission_command_core::Target::Linux
            };
            assert_eq!(report.selected_targets, [host]);
            assert_eq!(target.target, host);
        } else {
            assert_eq!(target.target, fission_command_core::Target::Terminal);
        }
        assert_eq!(target.artifacts.len(), 1);
        let artifact = &target.artifacts[0];
        assert!(artifact.is_file());
        assert!(String::from_utf8_lossy(&human.stdout).contains(artifact.to_str().unwrap()));
        let run = Command::new(artifact).output().unwrap();
        assert!(run.status.success());
        assert_eq!(
            String::from_utf8_lossy(&run.stdout).trim(),
            "existing target works"
        );
    }
    assert_eq!(config, fs::read(root.join("fission.toml")).unwrap());
    fs::write(
        root.join("src/main.rs"),
        "fn main() { let _: u32 = \"wrong type\"; }\n",
    )
    .unwrap();
    assert_failure_parity(
        &["build", "--target", "terminal"],
        &root,
        ErrorCode::CompileFailed,
    );
}

#[cfg(unix)]
#[test]
fn android_build_modes_execute_the_existing_script_once_and_verify_its_artifact() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("android-mode-parity");
    init(&root);
    success(&cli(&["add-target", "android", "--json"], &root));
    let script = root.join("platforms/android/package-apk.sh");
    fs::write(&script, "#!/bin/sh\nprintf 'run\\n' >> build-count\nmkdir -p apk-output\nprintf 'apk' > apk-output/app.apk\nprintf '%s/apk-output/app.apk\\n' \"$PWD\"\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let config = fs::read(root.join("fission.toml")).unwrap();
    let human = cli(&["build", "--target", "android", "--release"], &root);
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("build-count")).unwrap(),
        "run\n"
    );
    let json = cli(
        &["build", "--target", "android", "--release", "--json"],
        &root,
    );
    let data = success(&json);
    assert_eq!(
        fs::read_to_string(root.join("build-count")).unwrap(),
        "run\nrun\n"
    );
    assert_eq!(human.status.code(), json.status.code());
    assert_eq!(
        data.targets[0].target,
        fission_command_core::Target::Android
    );
    assert_eq!(
        data.targets[0].artifacts,
        [root.join("apk-output/app.apk").canonicalize().unwrap()]
    );
    assert!(String::from_utf8_lossy(&human.stdout)
        .contains(data.targets[0].artifacts[0].to_str().unwrap()));
    assert_eq!(config, fs::read(root.join("fission.toml")).unwrap());
}

#[test]
fn direct_site_modes_do_not_require_generic_target_registration_or_scaffolds() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("direct site parity");
    static_fixture(&root);
    let config = fs::read_to_string(root.join("fission.toml")).unwrap();
    fs::write(
        root.join("fission.toml"),
        config
            .replace("\"static-site\",", "")
            .replace(", \"static-site\"", ""),
    )
    .unwrap();
    assert!(!fission_command_core::read_project_config(&root)
        .unwrap()
        .targets
        .contains(&fission_command_core::Target::Site));
    fs::remove_file(root.join("platforms/site/README.md")).unwrap();
    for operation in ["routes", "check", "build"] {
        let human = cli(&["site", operation], &root);
        let json = cli(&["site", operation, "--json"], &root);
        assert_eq!(human.status.code(), json.status.code());
        let data = success(&json);
        for route in &data.targets[0].routes {
            assert!(
                String::from_utf8_lossy(&human.stdout).contains(&route.path)
                    || operation == "check"
            );
        }
    }
}
