use super::*;

pub(crate) fn scaffold(
    root: &Path,
    project: &FissionProject,
    local_path: Option<&Path>,
    policy: WritePolicy,
) -> Result<()> {
    for (path, source) in [
        ("src/lib.rs", include_str!("../assets/website/lib.rs")),
        ("src/state.rs", include_str!("../assets/website/state.rs")),
        ("src/page.rs", include_str!("../assets/website/page.rs")),
        (
            "src/navigation.rs",
            include_str!("../assets/website/navigation.rs"),
        ),
        ("src/footer.rs", include_str!("../assets/website/footer.rs")),
        ("src/design.rs", include_str!("../assets/website/design.rs")),
        ("build.rs", include_str!("../assets/website/build.rs")),
        (
            "design/dsp.json",
            include_str!("../assets/website/dsp.json"),
        ),
        (
            "design/tokens.json",
            include_str!("../assets/website/tokens.json"),
        ),
        ("i18n/en.json", include_str!("../assets/website/en.json")),
        ("i18n/es.json", include_str!("../assets/website/es.json")),
        ("WEBSITE.md", include_str!("../assets/website/WEBSITE.md")),
    ] {
        write_file_with_policy(&root.join(path), source, policy)?;
    }
    let library = project.app.name.replace('-', "_");
    write_file_with_policy(&root.join("README.md"), "# Your website\n\nA Fission website starter with home and about pages. Read [WEBSITE.md](WEBSITE.md) for local checks, preview, routing, and GitHub Pages preparation. Immediately read generated AGENTS.md or AGENTS.fission.md and its linked guidance before editing.\n", policy)?;
    write_file_with_policy(
        &root.join("src/main.rs"),
        &format!("#[cfg(not(target_arch = \"wasm32\"))]\nfn main() -> anyhow::Result<()> {{\n    {library}::build_site()\n}}\n\n#[cfg(target_arch = \"wasm32\")]\nfn main() {{}}\n"),
        policy,
    )?;
    configure_manifest(root, local_path)?;
    let path = root.join("fission.toml");
    let mut doc = fs::read_to_string(&path)?.parse::<DocumentMut>()?;
    doc["site"]["entry"] = value("src/main.rs");
    doc["site"]["title"] = value("Your website");
    doc["site"]["routes"] = value(Array::new());
    doc["site"]["asset_dirs"] = value(string_array(["assets"].into_iter()));
    // Site asset directories copy their contents into the output root.
    doc["site"]["favicon"] = value("app-icon.png");
    doc["site"]["search"]["enabled"] = value(false);
    write_file(&path, &doc.to_string())?;
    println!("Website ready. Read WEBSITE.md for checks, preview, routing, and GitHub Pages preparation. No website has been published.");
    Ok(())
}

fn configure_manifest(root: &Path, local_path: Option<&Path>) -> Result<()> {
    let path = root.join("Cargo.toml");
    let mut doc = fs::read_to_string(&path)?.parse::<DocumentMut>()?;
    doc["dependencies"]["serde_json"] = value("1");
    let mut dependency = InlineTable::new();
    if let Some(local) = local_path {
        dependency.insert(
            "path",
            Value::from(
                local
                    .join("crates/tools/fission-design-system-codegen")
                    .to_string_lossy()
                    .to_string(),
            ),
        );
    } else {
        dependency.insert("version", Value::from(CURRENT_VERSION));
    }
    doc["build-dependencies"]["fission-design-system-codegen"] =
        Item::Value(Value::InlineTable(dependency));
    let mut web_sys = InlineTable::new();
    web_sys.insert("version", Value::from("0.3"));
    web_sys.insert(
        "features",
        Value::Array(string_array(["Window", "Location"].into_iter())),
    );
    doc["target"]["cfg(target_arch = \"wasm32\")"]["dependencies"]["web-sys"] =
        Item::Value(Value::InlineTable(web_sys));
    write_file(&path, &doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn directory(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "fission-website-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn website_uses_existing_target_wiring_and_preserves_user_guidance() {
        let root = directory("guidance");
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(
            root.join("AGENTS.md"),
            "# Team instructions\nKeep my rules.\n",
        )
        .unwrap();
        init_project_with_website_template(&root, Some("website-test".into()), None, None, true)
            .unwrap();
        assert_eq!(
            read_project_config(&root).unwrap().targets,
            BTreeSet::from([Target::Site])
        );
        let cargo: toml::Value =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        assert_eq!(
            cargo["target"]["cfg(target_arch = \"wasm32\")"]["dependencies"]["web-sys"]["features"]
                .as_array()
                .unwrap(),
            &vec![toml::Value::from("Window"), toml::Value::from("Location")]
        );
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            "# Team instructions\nKeep my rules.\n"
        );
        let guidance = fs::read_to_string(root.join("AGENTS.fission.md")).unwrap();
        assert!(guidance.contains("WEBSITE.md"));
        assert!(guidance.contains("never generated HTML"));
        let readme = fs::read_to_string(root.join("README.md")).unwrap();
        assert!(readme.contains("WEBSITE.md"));
        assert!(!readme.contains("launch the desktop"));
        assert!(!root.join("platforms/macos").exists());
        assert!(!root.join("content/getting-started.md").exists());
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(root.join("fission.toml")).unwrap()).unwrap();
        assert_eq!(config["site"]["entry"].as_str(), Some("src/main.rs"));
        assert_eq!(config["site"]["search"]["enabled"].as_bool(), Some(false));
        assert!(root.join("design/tokens.json").exists());
        add_targets(&root, &[Target::Web]).unwrap();
        assert_eq!(
            read_project_config(&root).unwrap().targets,
            BTreeSet::from([Target::Site, Target::Web])
        );
        assert!(root.join("platforms/web/build-wasm.sh").exists());
        let host = fs::read_to_string(root.join("platforms/web/index.html")).unwrap();
        assert!(host.contains("href=\"assets/app-icon.png\""));
        assert!(host.contains("src=\"./bootstrap.mjs\""));
        assert!(root.join("platforms/web/assets/app-icon.png").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn website_does_not_replace_existing_source_or_user_fallback() {
        let root = directory("existing");
        fs::write(root.join("AGENTS.md"), "User instructions\n").unwrap();
        fs::write(root.join("README.md"), "User readme\n").unwrap();
        fs::write(
            root.join("AGENTS.fission.md"),
            "User fallback instructions\n",
        )
        .unwrap();
        init_project_with_website_template(&root, None, None, None, true).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("README.md")).unwrap(),
            "User readme\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.fission.md")).unwrap(),
            "User fallback instructions\n"
        );
        let before = fs::read_to_string(root.join("src/page.rs")).unwrap();
        assert!(init_project_with_website_template(&root, None, None, None, true).is_err());
        assert_eq!(
            fs::read_to_string(root.join("src/page.rs")).unwrap(),
            before
        );
        add_targets(&root, &[Target::Web]).unwrap();
        assert!(read_project_config(&root)
            .unwrap()
            .targets
            .contains(&Target::Web));
        assert_eq!(
            fs::read_to_string(root.join("src/page.rs")).unwrap(),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn website_rejects_existing_target_configuration_before_writing() {
        let root = directory("config");
        let manifest = "[targets]\nweb = true\n";
        fs::write(root.join("fission.toml"), manifest).unwrap();
        assert!(init_project_with_website_template(&root, None, None, None, true).is_err());
        assert_eq!(
            fs::read_to_string(root.join("fission.toml")).unwrap(),
            manifest
        );
        assert!(!root.join("src").exists());
        fs::remove_dir_all(root).unwrap();
    }

    // Opt-in because this compiles a generated application against the checkout,
    // rather than just checking template strings. CI/maintainers can run it with:
    // cargo test -p fission-command-core website_static_build -- --ignored
    #[test]
    #[ignore = "compiles and renders a fresh generated website"]
    fn website_static_build_has_mount_independent_routes_and_assets() {
        let root = directory("build");
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap();
        init_project_with_website_template(
            &root,
            Some("website-build-test".into()),
            None,
            Some(checkout.clone()),
            true,
        )
        .unwrap();
        let result = Command::new("cargo")
            .args(["run", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .args(["--", "build", "--project-dir"])
            .arg(&root)
            .env(
                "CARGO_TARGET_DIR",
                checkout.join("target/website-regression"),
            )
            .status()
            .unwrap();
        assert!(result.success());
        let output = root.join("target/fission/site");
        for route in ["", "about/"] {
            let html = fs::read_to_string(output.join(route).join("index.html")).unwrap();
            assert!(html.contains("Built with Fission"));
            assert!(html.contains("nav-home") && html.contains("nav-about"));
            for attribute in ["href=\"", "src=\""] {
                for rest in html.split(attribute).skip(1) {
                    let href = rest.split('"').next().unwrap();
                    if href.starts_with("https:")
                        || href.starts_with("http:")
                        || href.starts_with('#')
                        || href.starts_with("data:")
                    {
                        continue;
                    }
                    assert!(
                        !href.starts_with('/'),
                        "root-absolute asset/link breaks Pages: {href}"
                    );
                    for mount in ["/", "/repository-name/"] {
                        let mut parts: Vec<String> = format!("{mount}{route}")
                            .split('/')
                            .filter(|part| !part.is_empty())
                            .map(str::to_owned)
                            .collect();
                        for part in href
                            .split('/')
                            .filter(|part| !part.is_empty() && *part != ".")
                        {
                            if part == ".." {
                                parts.pop();
                            } else {
                                parts.push(part.to_owned());
                            }
                        }
                        if mount != "/" {
                            assert_eq!(parts.remove(0), "repository-name");
                        }
                        let path = output.join(parts.join("/"));
                        assert!(
                            path.is_file() || path.join("index.html").is_file(),
                            "missing {href} from {mount}{route}"
                        );
                    }
                }
            }
        }
        if std::env::var_os("FISSION_WEBSITE_KEEP_FIXTURE").is_some() {
            println!("Website fixture: {}", root.display());
        } else {
            fs::remove_dir_all(root).unwrap();
        }
    }
}
