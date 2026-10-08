use fission_command_core::guidance::{GuidanceResult, Status};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fission-skills-cli-{}-{} app",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join(".git")).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fission"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn skills(&self, operation: &str) -> (Output, GuidanceResult) {
        let output = self.cli(&[
            "skills",
            operation,
            "--project-dir",
            self.0.to_str().unwrap(),
            "--json",
        ]);
        let result =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"));
        (output, result)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn fresh_init_prints_actual_paths_and_installs_readable_router() {
    let f = Fixture::new();
    let output = f.cli(&["init", ".", "--name", "guidance_fixture"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let invoked = f.0.join(".");
    assert!(stdout.contains(invoked.join("AGENTS.md").to_str().unwrap()));
    assert!(stdout.contains(
        invoked
            .join(".fission/skills/fission-web/SKILL.md")
            .to_str()
            .unwrap()
    ));
    let entrypoint = fs::read_to_string(f.0.join("AGENTS.md")).unwrap();
    assert!(entrypoint.contains("[shared application rules](.fission/references/shared-app.md)"));
    assert!(f.0.join(".fission/references/shared-app.md").is_file());
    let config = fs::read(f.0.join("fission.toml")).unwrap();
    let (output, result) = f.skills("check");
    assert_eq!(output.status.code(), Some(1)); // Dependency has not been resolved.
    assert_eq!(result.status, Status::NeedsAttention);
    let (output, result) = f.skills("update");
    assert!(output.status.success());
    assert_eq!(result.status, Status::Updated);
    assert_eq!(fs::read(f.0.join("fission.toml")).unwrap(), config);
    assert!(f
        .cli(&["init", ".", "--name", "guidance_fixture"])
        .status
        .success());
    assert_eq!(fs::read(f.0.join("fission.toml")).unwrap(), config);
}

#[test]
fn json_and_exit_codes_distinguish_healthy_conflict_and_malformed_arguments() {
    let f = Fixture::new();
    assert!(f
        .cli(&["init", ".", "--name", "guidance_fixture"])
        .status
        .success());
    fs::write(f.0.join("Cargo.lock"), format!("version = 4\n[[package]]\nname = \"fission\"\nversion = \"{}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n", env!("CARGO_PKG_VERSION"))).unwrap();
    let (output, result) = f.skills("check");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(result.status, Status::Healthy);
    fs::write(f.0.join("AGENTS.md"), "user customized instructions").unwrap();
    let manifest = fs::read(f.0.join(".fission/guidance-manifest.json")).unwrap();
    let (output, result) = f.skills("update");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result.status, Status::Conflict);
    assert_eq!(
        fs::read_to_string(f.0.join("AGENTS.md")).unwrap(),
        "user customized instructions"
    );
    assert_eq!(
        fs::read(f.0.join(".fission/guidance-manifest.json")).unwrap(),
        manifest
    );
    for args in [
        vec!["skills", "check", "--project-dir", "--json"],
        vec!["skills", "nonsense", "--json"],
        vec!["skills", "update", "--unknown", "--json"],
        vec!["skills", "--json"],
    ] {
        let output = f.cli(&args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let result: GuidanceResult = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result.status, Status::InvalidArguments);
        assert_eq!(result.recovery[0].argv, ["fission", "skills", "--help"]);
    }
    assert!(f.cli(&["skills", "check", "--help"]).status.success());
}

#[test]
fn nested_init_preserves_all_custom_instruction_files_and_reports_fallback() {
    let f = Fixture::new();
    fs::write(f.0.join("AGENTS.md"), "root contributor policy").unwrap();
    fs::write(
        f.0.join("AGENTS.fission.md"),
        "root custom Fission instructions",
    )
    .unwrap();
    let nested = f.0.join("apps/nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("AGENTS.md"), "nested user policy").unwrap();
    fs::write(nested.join("AGENTS.fission.md"), "nested custom policy").unwrap();
    let output = f.cli(&[
        "init",
        nested.to_str().unwrap(),
        "--name",
        "nested_guidance_fixture",
    ]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(f.0.join(".fission/AGENTS.md").to_str().unwrap()));
    assert!(text.contains(nested.join("AGENTS.md").to_str().unwrap()));
    for (path, expected) in [
        (f.0.join("AGENTS.md"), "root contributor policy"),
        (
            f.0.join("AGENTS.fission.md"),
            "root custom Fission instructions",
        ),
        (nested.join("AGENTS.md"), "nested user policy"),
        (nested.join("AGENTS.fission.md"), "nested custom policy"),
    ] {
        assert_eq!(fs::read_to_string(path).unwrap(), expected);
    }
}

#[test]
fn init_reports_unavailable_router_when_legacy_customization_blocks_installation() {
    let f = Fixture::new();
    let instructions = "<!-- fission-cli-generated-agents:v1 -->\nuser edited instructions";
    fs::write(f.0.join("AGENTS.md"), instructions).unwrap();
    let output = f.cli(&["init", ".", "--name", "legacy_guidance_fixture"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.contains("Conflict"));
    assert!(!stdout.contains("Web router:"));
    assert!(stderr.contains("Web router unavailable"));
    assert_eq!(
        fs::read_to_string(f.0.join("AGENTS.md")).unwrap(),
        instructions
    );
    assert!(!f.0.join(".fission/skills/fission-web/SKILL.md").exists());
}
