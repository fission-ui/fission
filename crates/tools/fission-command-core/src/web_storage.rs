use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::{write_binary_file_with_policy, FissionProject, PlatformCapability, WritePolicy};

const WEB_TARGET: &str = "wasm32-unknown-unknown";

/// Validates and prepares the generated host support required by Web SQLite.
///
/// `fission.toml` is the capability authority. Cargo metadata is inspected only
/// to reject a dependency graph that has independently enabled the Rust
/// provider without declaring its required browser host assets.
pub fn prepare_web_target(root: &Path, project: &FissionProject) -> Result<()> {
    let web_sqlite_enabled = resolved_web_sqlite_enabled(root)?;
    prepare_web_target_with_feature_state(root, project, web_sqlite_enabled)
}

pub(crate) fn write_web_sqlite_assets(root: &Path, write_policy: WritePolicy) -> Result<()> {
    for (name, contents) in [
        ("sqlite3.mjs", fission_store_sqlite::SQLITE_WEB_MODULE),
        ("sqlite3.wasm", fission_store_sqlite::SQLITE_WEB_WASM),
        (
            "sqlite3-opfs-async-proxy.js",
            fission_store_sqlite::SQLITE_WEB_OPFS_ASYNC_PROXY,
        ),
        (
            "fission-sqlite.mjs",
            fission_store_sqlite::SQLITE_WEB_BRIDGE,
        ),
        (
            "fission-sqlite-worker.mjs",
            fission_store_sqlite::SQLITE_WEB_WORKER,
        ),
        ("NOTICE.txt", fission_store_sqlite::SQLITE_WEB_NOTICE),
    ] {
        write_binary_file_with_policy(
            &root.join("platforms/web/sqlite").join(name),
            contents,
            write_policy,
        )?;
    }
    Ok(())
}

pub(crate) fn enable_web_sqlite_scaffold(root: &Path, application_id: &str) -> Result<()> {
    // These files are framework-owned generated assets. Refreshing them is
    // necessary when Fission upgrades SQLite or fixes its host protocol.
    write_web_sqlite_assets(root, WritePolicy::Overwrite)?;
    let path = root.join("platforms/web/bootstrap.mjs");
    let mut bootstrap =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let install = format!(
        "installFissionSqlite({{ appId: {} }});",
        serde_json::to_string(application_id).context("failed to encode the Web application ID")?
    );
    if bootstrap.contains("installFissionSqlite(") {
        if bootstrap.contains("installFissionSqlite();") {
            bootstrap = bootstrap.replace("installFissionSqlite();", &install);
            fs::write(&path, bootstrap)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        return Ok(());
    }
    let init = "await init();";
    bootstrap.find(init).ok_or_else(|| {
        anyhow::anyhow!(
            "cannot enable Web SQLite because {} has no `{init}` call",
            path.display()
        )
    })?;
    let mut updated = bootstrap;
    if !updated.contains("./sqlite/fission-sqlite.mjs") {
        let import_end = updated.find('\n').ok_or_else(|| {
            anyhow::anyhow!(
                "cannot enable Web SQLite because {} has no import section",
                path.display()
            )
        })?;
        let sqlite_import =
            "import { installFissionSqlite } from \"./sqlite/fission-sqlite.mjs\";\n";
        updated.insert_str(import_end + 1, sqlite_import);
    }
    let init_start = updated
        .find(init)
        .expect("the validated init call remains after inserting an import");
    updated.insert_str(init_start, &format!("{install}\n"));
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn prepare_web_target_with_feature_state(
    root: &Path,
    project: &FissionProject,
    web_sqlite_enabled: bool,
) -> Result<()> {
    let storage_declared = project.capabilities.contains(&PlatformCapability::Storage);

    if web_sqlite_enabled && !storage_declared {
        bail!(
            "Web SQLite is enabled in the Cargo dependency graph, but `fission.toml` does not declare the `storage` capability.\n\nRun:\n\n    fission add-capability storage --project-dir {}\n\nCompilation was not started.",
            root.display()
        );
    }

    if storage_declared && !web_sqlite_enabled {
        bail!(
            "`fission.toml` declares the `storage` capability, but the Web SQLite provider is not enabled in the Cargo dependency graph.\n\nRun:\n\n    fission add-capability storage --project-dir {}\n\nCompilation was not started.",
            root.display()
        );
    }

    if storage_declared {
        enable_web_sqlite_scaffold(root, &project.app.app_id)?;
        validate_web_sqlite_assets(root)?;
    }

    Ok(())
}

fn validate_web_sqlite_assets(root: &Path) -> Result<()> {
    let sqlite_root = root.join("platforms/web/sqlite");
    for name in [
        "sqlite3.mjs",
        "sqlite3.wasm",
        "sqlite3-opfs-async-proxy.js",
        "fission-sqlite.mjs",
        "fission-sqlite-worker.mjs",
        "NOTICE.txt",
    ] {
        let path = sqlite_root.join(name);
        if !path.is_file() {
            bail!("Web SQLite host asset is missing: {}", path.display());
        }
    }

    let bootstrap_path = root.join("platforms/web/bootstrap.mjs");
    let bootstrap = fs::read_to_string(&bootstrap_path)
        .with_context(|| format!("failed to read {}", bootstrap_path.display()))?;
    let install = bootstrap.find("installFissionSqlite(").with_context(|| {
        format!(
            "{} does not install the Fission SQLite bridge",
            bootstrap_path.display()
        )
    })?;
    let start = bootstrap.find("await init();").with_context(|| {
        format!(
            "{} does not start the generated Web application",
            bootstrap_path.display()
        )
    })?;
    if install > start {
        bail!(
            "{} installs the Fission SQLite bridge after starting the Web application",
            bootstrap_path.display()
        );
    }
    if !bootstrap.contains("./sqlite/fission-sqlite.mjs") {
        bail!(
            "{} does not import the Fission SQLite bridge",
            bootstrap_path.display()
        );
    }
    Ok(())
}

fn resolved_web_sqlite_enabled(root: &Path) -> Result<bool> {
    let manifest_path = root.join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--filter-platform")
        .arg(WEB_TARGET)
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(root)
        .output()
        .context("failed to inspect the Web Cargo dependency graph")?;
    if !output.status.success() {
        std::io::stderr().write_all(&output.stderr).ok();
        bail!("cargo metadata failed with {}", output.status);
    }

    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .context("failed to parse the Web Cargo dependency graph")?;
    let manifest_path = manifest_path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", manifest_path.display()))?;
    Ok(metadata.web_sqlite_enabled_for(&manifest_path))
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    resolve: Option<CargoResolve>,
}

impl CargoMetadata {
    fn web_sqlite_enabled_for(&self, manifest_path: &Path) -> bool {
        let Some(resolve) = &self.resolve else {
            return false;
        };
        let Some(root_id) = self.packages.iter().find_map(|package| {
            paths_refer_to_same_file(&package.manifest_path, manifest_path)
                .then_some(package.id.as_str())
        }) else {
            return false;
        };

        let packages = self
            .packages
            .iter()
            .map(|package| (package.id.as_str(), package))
            .collect::<HashMap<_, _>>();
        let nodes = resolve
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let mut pending = vec![root_id];
        let mut visited = HashSet::new();

        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(node) = nodes.get(id) else {
                continue;
            };
            if packages.get(id).is_some_and(|package| {
                package.name == "fission-store-sqlite"
                    && node.features.iter().any(|feature| feature == "web")
            }) {
                return true;
            }
            pending.extend(node.dependencies.iter().map(String::as_str));
        }

        false
    }
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CargoResolve {
    nodes: Vec<CargoNode>,
}

#[derive(Debug, Deserialize)]
struct CargoNode {
    id: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    features: Vec<String>,
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{AppConfig, NativeConfig, Target};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fission-web-storage-{name}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    fn project(storage: bool) -> FissionProject {
        FissionProject {
            app: AppConfig {
                name: "fixture".into(),
                app_id: "dev.fission.fixture".into(),
                splash: None,
            },
            targets: BTreeSet::from([Target::Web]),
            capabilities: storage
                .then_some(PlatformCapability::Storage)
                .into_iter()
                .collect(),
            native: NativeConfig::default(),
        }
    }

    fn web_scaffold(root: &Path) {
        fs::create_dir_all(root.join("platforms/web")).unwrap();
        fs::write(
            root.join("platforms/web/bootstrap.mjs"),
            "import init from \"./pkg/app.js\";\nawait init();\n",
        )
        .unwrap();
    }

    #[test]
    fn enabled_provider_without_declared_capability_fails_before_assets_are_written() {
        let root = TestDir::new("missing-capability");
        web_scaffold(root.path());

        let error = prepare_web_target_with_feature_state(root.path(), &project(false), true)
            .unwrap_err()
            .to_string();

        assert!(error.contains("does not declare the `storage` capability"));
        assert!(error.contains("Compilation was not started"));
        assert!(!root.path().join("platforms/web/sqlite").exists());
    }

    #[test]
    fn declared_capability_without_provider_fails_before_compilation() {
        let root = TestDir::new("missing-provider");
        web_scaffold(root.path());

        let error = prepare_web_target_with_feature_state(root.path(), &project(true), false)
            .unwrap_err()
            .to_string();

        assert!(error.contains("provider is not enabled"));
        assert!(error.contains("Compilation was not started"));
    }

    #[test]
    fn declared_storage_repairs_assets_and_installs_bridge_before_app_start() {
        let root = TestDir::new("repair-assets");
        web_scaffold(root.path());
        fs::create_dir_all(root.path().join("platforms/web/sqlite")).unwrap();
        fs::write(
            root.path().join("platforms/web/sqlite/fission-sqlite.mjs"),
            "stale generated bridge",
        )
        .unwrap();

        prepare_web_target_with_feature_state(root.path(), &project(true), true).unwrap();

        let bootstrap =
            fs::read_to_string(root.path().join("platforms/web/bootstrap.mjs")).unwrap();
        assert!(bootstrap.contains("./sqlite/fission-sqlite.mjs"));
        assert!(
            bootstrap
                .find("installFissionSqlite({ appId: \"dev.fission.fixture\" });")
                .unwrap()
                < bootstrap.find("await init();").unwrap()
        );
        assert_eq!(
            fs::read(root.path().join("platforms/web/sqlite/fission-sqlite.mjs")).unwrap(),
            fission_store_sqlite::SQLITE_WEB_BRIDGE
        );
        assert!(root
            .path()
            .join("platforms/web/sqlite/sqlite3.wasm")
            .is_file());
        assert!(root
            .path()
            .join("platforms/web/sqlite/sqlite3-opfs-async-proxy.js")
            .is_file());
    }

    #[test]
    fn declared_storage_repairs_a_bootstrap_with_only_the_sqlite_import() {
        let root = TestDir::new("repair-partial-bootstrap");
        fs::create_dir_all(root.path().join("platforms/web")).unwrap();
        fs::write(
            root.path().join("platforms/web/bootstrap.mjs"),
            "import init from \"./pkg/app.js\";\nimport { installFissionSqlite } from \"./sqlite/fission-sqlite.mjs\";\nawait init();\n",
        )
        .unwrap();

        prepare_web_target_with_feature_state(root.path(), &project(true), true).unwrap();

        let bootstrap =
            fs::read_to_string(root.path().join("platforms/web/bootstrap.mjs")).unwrap();
        assert_eq!(bootstrap.matches("./sqlite/fission-sqlite.mjs").count(), 1);
        assert!(bootstrap.contains("installFissionSqlite({ appId: \"dev.fission.fixture\" });"));
    }

    #[test]
    fn dependency_detection_ignores_unreachable_workspace_packages() {
        let root = TestDir::new("unreachable-provider");
        let app_manifest = root.path().join("Cargo.toml");
        fs::write(&app_manifest, "[package]\nname='app'\nversion='0.1.0'\n").unwrap();
        let sqlite_manifest = root.path().join("sqlite.toml");
        fs::write(&sqlite_manifest, "").unwrap();
        let metadata = CargoMetadata {
            packages: vec![
                CargoPackage {
                    id: "app".into(),
                    name: "app".into(),
                    manifest_path: app_manifest.clone(),
                },
                CargoPackage {
                    id: "sqlite".into(),
                    name: "fission-store-sqlite".into(),
                    manifest_path: sqlite_manifest,
                },
            ],
            resolve: Some(CargoResolve {
                nodes: vec![
                    CargoNode {
                        id: "app".into(),
                        dependencies: Vec::new(),
                        features: Vec::new(),
                    },
                    CargoNode {
                        id: "sqlite".into(),
                        dependencies: Vec::new(),
                        features: vec!["web".into()],
                    },
                ],
            }),
        };

        assert!(!metadata.web_sqlite_enabled_for(&app_manifest));
    }

    #[test]
    fn dependency_detection_finds_reachable_web_provider() {
        let root = TestDir::new("reachable-provider");
        let app_manifest = root.path().join("Cargo.toml");
        fs::write(&app_manifest, "[package]\nname='app'\nversion='0.1.0'\n").unwrap();
        let sqlite_manifest = root.path().join("sqlite.toml");
        fs::write(&sqlite_manifest, "").unwrap();
        let metadata = CargoMetadata {
            packages: vec![
                CargoPackage {
                    id: "app".into(),
                    name: "app".into(),
                    manifest_path: app_manifest.clone(),
                },
                CargoPackage {
                    id: "sqlite".into(),
                    name: "fission-store-sqlite".into(),
                    manifest_path: sqlite_manifest,
                },
            ],
            resolve: Some(CargoResolve {
                nodes: vec![
                    CargoNode {
                        id: "app".into(),
                        dependencies: vec!["sqlite".into()],
                        features: Vec::new(),
                    },
                    CargoNode {
                        id: "sqlite".into(),
                        dependencies: Vec::new(),
                        features: vec!["web".into()],
                    },
                ],
            }),
        };

        assert!(metadata.web_sqlite_enabled_for(&app_manifest));
    }

    #[test]
    fn cargo_metadata_detects_a_real_web_sqlite_dependency() {
        let root = TestDir::new("cargo-metadata-provider");
        let sqlite_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/fission-store-sqlite")
            .canonicalize()
            .unwrap();
        fs::write(
            root.path().join("Cargo.toml"),
            format!(
                "[package]\nname = \"web-storage-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nfission-store-sqlite = {{ path = {:?}, features = [\"web\"] }}\n",
                sqlite_path.to_string_lossy()
            ),
        )
        .unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();

        assert!(resolved_web_sqlite_enabled(root.path()).unwrap());
    }
}
