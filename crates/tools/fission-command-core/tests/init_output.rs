use std::{fs, process::Command};

#[test]
fn initialization_authorities_leave_stdout_to_the_caller() {
    for template in [false, true] {
        let root = std::env::temp_dir().join(format!(
            "fission-silent-init-{}-{template}",
            std::process::id()
        ));
        assert!(!root.exists());
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "initialization_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FISSION_INIT_OUTPUT_FIXTURE", &root)
            .env("FISSION_INIT_OUTPUT_TEMPLATE", template.to_string())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let authority_output = stdout
            .split_once("AUTHORITY_START\n")
            .unwrap()
            .1
            .split_once("AUTHORITY_END")
            .unwrap()
            .0;
        assert_eq!(
            authority_output, "",
            "initialization must return facts instead of rendering"
        );
        assert!(root.join("fission.toml").is_file());
        assert_eq!(root.join("WEBSITE.md").is_file(), template);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
#[ignore = "subprocess fixture for the output boundary regression"]
fn initialization_child() {
    let root = std::path::PathBuf::from(std::env::var_os("FISSION_INIT_OUTPUT_FIXTURE").unwrap());
    let template = std::env::var("FISSION_INIT_OUTPUT_TEMPLATE").unwrap() == "true";
    println!("AUTHORITY_START");
    fission_command_core::init_project_with_website_template(
        &root,
        Some("silent_fixture".into()),
        None,
        None,
        template,
    )
    .unwrap();
    println!("AUTHORITY_END");
}
