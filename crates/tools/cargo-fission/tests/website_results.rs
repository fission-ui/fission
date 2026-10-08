use fission_command_core::website::{CommandResult, Data, ErrorCode, Outcome, Step};
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
    assert_eq!(value.schema, "fission.website-result.v1");
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
    assert!(!routes.routes.is_empty());
    assert!(routes.artifact_dir.is_none());
    assert!(routes.artifacts.is_empty());
    let check = success(&cli(&["site", "check", "--json"], &root));
    assert!(check.artifact_dir.is_none());
    assert!(check.artifacts.is_empty());
    assert!(!check.planned_output_dir.unwrap().exists());
    let built = success(&cli(&["build", "--target", "static-site", "--json"], &root));
    assert!(built.artifact_dir.as_ref().unwrap().is_dir());
    assert!(built.artifacts.iter().all(|path| path.is_file()));
    assert!(built.routes.iter().all(|route| route.output.is_file()));
    success(&follow(&built.next_steps[0]));
    let direct = success(&cli(&["site", "build", "--json"], &root));
    assert_eq!(built.artifact_dir, direct.artifact_dir);
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
        &cli(&["site", "routes", "--json"], &root),
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
    for args in [
        vec!["build", "--target", "linux", "--json"],
        vec!["build", "--json"],
        vec!["add-target", "--json"],
        vec!["add-target", "ios", "--json"],
    ] {
        failure(&cli(&args, temp.path()), ErrorCode::InvalidTarget);
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
    assert!(data.artifact_dir.unwrap().ends_with("output with spaces"));
    assert!(data.artifacts.iter().all(|path| path.is_file()));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("inherited-secret-must-be-redacted"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("inherited-secret-must-be-redacted"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("[REDACTED]"));
    let routes = success(&cli(&["site", "routes", "--json"], &root));
    assert!(!routes.routes.is_empty());
    assert!(routes.artifact_dir.is_none());
    success(&cli(&["site", "check", "--json"], &root));
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
    assert_eq!(recovery[0].invocation.argv[0], "doctor");
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

#[cfg(unix)]
#[test]
fn non_utf8_project_paths_emit_one_failure_without_partial_json() {
    use std::os::unix::ffi::OsStringExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp
        .path()
        .join(std::ffi::OsString::from_vec(b"bad-path-\xff".to_vec()));
    let output = Command::new(env!("CARGO_BIN_EXE_fission"))
        .current_dir(temp.path())
        .args(["init", "--json"])
        .arg(&root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: CommandResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.project_dir.is_none());
    assert!(
        matches!(value.outcome, Outcome::Failure { error } if error.code == ErrorCode::InvalidConfiguration)
    );
    assert!(!root.exists());
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
    let output = command.output().unwrap();
    let data = success(&output);
    assert!(data
        .artifact_dir
        .as_ref()
        .unwrap()
        .ends_with("platforms/web"));
    assert!(data.artifact_dir.unwrap().join("index.html").is_file());
    let wasm = data
        .artifacts
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap();
    assert_eq!(&fs::read(wasm).unwrap()[..4], b"\0asm");
    assert!(data.artifacts.iter().all(|path| path.is_file()));
    assert_eq!(original, fs::read(root.join("fission.toml")).unwrap());
}
