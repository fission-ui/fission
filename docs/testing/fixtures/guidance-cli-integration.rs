use fission_command_core::report::{CommandResult, Outcome};
use std::{fs, process::Command};

#[test]
fn combined_init_returns_one_envelope_with_every_instruction_path() {
    for json in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join("AGENTS.md"), "root policy").unwrap();
        fs::write(root.join("AGENTS.fission.md"), "root Fission policy").unwrap();
        let app = root.join("apps/nested");
        fs::create_dir_all(&app).unwrap();
        fs::write(root.join("apps/AGENTS.md"), "intermediate policy").unwrap();
        fs::write(app.join("AGENTS.md"), "nested policy").unwrap();
        fs::write(app.join("AGENTS.fission.md"), "nested Fission policy").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_fission"));
        command.args(["init", app.to_str().unwrap(), "--name", "combined_fixture"]);
        if json {
            command.arg("--json");
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let expected = [
            "AGENTS.md",
            "AGENTS.fission.md",
            ".fission/AGENTS.md",
            "apps/AGENTS.md",
            "apps/nested/AGENTS.md",
            "apps/nested/AGENTS.fission.md",
        ];
        if json {
            // Parsing all of stdout rejects prose before or after the envelope.
            let result: CommandResult = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(result.schema, "fission.cli-result.v1");
            let Outcome::Success { data } = result.outcome else {
                panic!("expected success")
            };
            for path in expected {
                assert!(
                    data.instructions.contains(&root.join(path)),
                    "missing {path}"
                );
            }
            let installation = data.guidance.unwrap();
            assert!(installation.shared_reference.unwrap().is_file());
            assert!(installation.web_router.unwrap().is_file());
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            for path in expected {
                assert!(
                    text.contains(root.join(path).to_str().unwrap()),
                    "missing {path}"
                );
            }
        }
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            "root policy"
        );
        assert_eq!(
            fs::read_to_string(app.join("AGENTS.fission.md")).unwrap(),
            "nested Fission policy"
        );
        assert!(app.join("fission.toml").is_file());
    }
}
