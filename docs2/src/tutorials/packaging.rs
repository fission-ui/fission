//! Package already-compiled tutorial Wasm without starting Cargo a second time.

use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct PackagingTools {
    bindgen: PathBuf,
    bindgen_version: String,
    optimizer: Option<(PathBuf, String)>,
}

impl PackagingTools {
    pub(super) fn discover(workspace: &Path, optimize: bool) -> Result<Self> {
        let lock_path = workspace.join("Cargo.lock");
        let lock = fs::read_to_string(&lock_path)
            .with_context(|| format!("read {} to select wasm-bindgen", lock_path.display()))?;
        let required = locked_bindgen_version(&lock)?;
        let caches = cache_roots();
        let bindgen_candidates = configured_candidates("WASM_BINDGEN", "wasm-bindgen", &caches);
        let mut observed = Vec::new();
        let mut selected = None;
        for path in bindgen_candidates {
            if let Ok(version) = tool_version(&path) {
                if version.split_whitespace().nth(1) == Some(required.as_str()) {
                    selected = Some((path, version));
                    break;
                }
                observed.push(version);
            }
        }
        let (bindgen, bindgen_version) = selected.with_context(|| {
            format!(
                "tutorial packaging requires wasm-bindgen-cli {required}, matching Cargo.lock; \
                 install it with `cargo install wasm-bindgen-cli --version {required} --locked` \
                 and make its binary available on PATH, or set WASM_PACK_CACHE to an existing \
                 wasm-pack tool cache (detected versions: {})",
                if observed.is_empty() {
                    "none".to_owned()
                } else {
                    observed.join(", ")
                }
            )
        })?;
        let optimizer = if optimize {
            Some(
                configured_candidates("WASM_OPT", "wasm-opt", &caches)
                    .into_iter()
                    .find_map(|path| tool_version(&path).ok().map(|version| (path, version)))
                    .context(
                        "optional tutorial optimisation requires wasm-opt; install Binaryen \
                         and add wasm-opt to PATH, or set WASM_PACK_CACHE to an existing tool cache",
                    )?,
            )
        } else {
            None
        };
        Ok(Self {
            bindgen,
            bindgen_version,
            optimizer,
        })
    }

    /// Binding and optimisation are separate cached operations. Enabling the
    /// optional optimiser must not force another Rust build or binding pass.
    pub(super) fn binding_context(&self) -> String {
        format!("tutorial-bindings-v1;target=web;{}", self.bindgen_version)
    }

    pub(super) fn optimization_context(&self) -> Option<String> {
        self.optimizer.as_ref().map(|(_, version)| {
            format!("tutorial-optimization-v1;{version};args=-Oz,--all-features;cores=1")
        })
    }

    pub(super) fn package(&self, input_wasm: &Path, output_pkg: &Path, module: &str) -> Result<()> {
        if !input_wasm.is_file() {
            bail!(
                "compiled tutorial artifact is missing: {}; compilation must finish before packaging",
                input_wasm.display()
            );
        }
        fs::create_dir_all(output_pkg)
            .with_context(|| format!("create tutorial package {}", output_pkg.display()))?;
        let status = Command::new(&self.bindgen)
            .arg(input_wasm)
            .args(["--target", "web", "--out-dir"])
            .arg(output_pkg)
            .args(["--out-name", module])
            .status()
            .with_context(|| format!("launch {}", self.bindgen.display()))?;
        if !status.success() {
            bail!("wasm-bindgen failed to package tutorial {module} ({status})");
        }
        let javascript = output_pkg.join(format!("{module}.js"));
        let wasm = output_pkg.join(format!("{module}_bg.wasm"));
        for output in [&javascript, &wasm] {
            if !output.is_file() || fs::metadata(output)?.len() == 0 {
                bail!(
                    "tutorial package output is missing or empty: {}",
                    output.display()
                );
            }
        }
        Ok(())
    }

    pub(super) fn optimize_package(
        &self,
        baseline_pkg: &Path,
        output_pkg: &Path,
        module: &str,
    ) -> Result<()> {
        let (optimizer, _) = self
            .optimizer
            .as_ref()
            .context("tutorial optimisation was not enabled during tool discovery")?;
        if baseline_pkg == output_pkg {
            bail!("optimisation must use a separate package directory to preserve its baseline");
        }
        copy_package(baseline_pkg, output_pkg)?;
        let wasm = output_pkg.join(format!("{module}_bg.wasm"));
        // Never overwrite the baseline package, nor replace this copy until
        // Binaryen has succeeded and produced a nonempty artifact.
        let optimized = output_pkg.join(format!("{module}_bg.optimized.wasm"));
        let status = Command::new(optimizer)
            .arg(&wasm)
            .args(["-Oz", "--all-features", "-o"])
            .arg(&optimized)
            .env("BINARYEN_CORES", "1")
            .status()
            .with_context(|| format!("launch {}", optimizer.display()))?;
        if !status.success() {
            bail!("optional wasm-opt failed for tutorial {module} ({status})");
        }
        if !optimized.is_file() || fs::metadata(&optimized)?.len() == 0 {
            bail!("wasm-opt produced no usable output for tutorial {module}");
        }
        fs::rename(&optimized, &wasm)
            .with_context(|| format!("install optimised tutorial {module}"))?;
        Ok(())
    }
}

fn copy_package(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let output = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_package(&entry.path(), &output)?;
        } else {
            fs::copy(entry.path(), output)?;
        }
    }
    Ok(())
}

pub(super) fn required_bindgen_version(workspace: &Path) -> Result<String> {
    locked_bindgen_version(&fs::read_to_string(workspace.join("Cargo.lock"))?)
}

fn locked_bindgen_version(lock: &str) -> Result<String> {
    let mut versions = BTreeSet::new();
    for package in lock.split("[[package]]").skip(1) {
        let mut name = None;
        let mut version = None;
        for line in package.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"');
            match key.trim() {
                "name" => name = Some(value),
                "version" => version = Some(value),
                _ => {}
            }
        }
        if name == Some("wasm-bindgen") {
            versions.insert(version.context("wasm-bindgen has no version in Cargo.lock")?);
        }
    }
    if versions.len() != 1 {
        bail!("expected exactly one wasm-bindgen version in Cargo.lock, found {versions:?}");
    }
    Ok(versions.into_iter().next().unwrap().to_owned())
}

fn tool_version(path: &Path) -> Result<String> {
    let output = Command::new(path).arg("--version").output()?;
    if !output.status.success() {
        bail!("{} --version failed", path.display());
    }
    let version = String::from_utf8(output.stdout)?.trim().to_owned();
    if version.is_empty() {
        bail!("{} returned an empty version", path.display());
    }
    Ok(version)
}

fn cache_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(cache) = std::env::var_os("WASM_PACK_CACHE") {
        roots.push(PathBuf::from(cache));
    }
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        roots.push(PathBuf::from(cache).join(".wasm-pack"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(Path::new(&home).join(".cache/.wasm-pack"));
        roots.push(Path::new(&home).join("Library/Caches/.wasm-pack"));
    }
    if let Some(cache) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(cache).join(".wasm-pack"));
    }
    roots
}

fn configured_candidates(variable: &str, tool: &str, caches: &[PathBuf]) -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os(variable) {
        vec![PathBuf::from(path)]
    } else {
        tool_candidates(tool, caches)
    }
}

fn tool_candidates(tool: &str, caches: &[PathBuf]) -> Vec<PathBuf> {
    let executable = format!("{tool}{}", std::env::consts::EXE_SUFFIX);
    let mut candidates = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join(&executable))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for root in caches {
        candidates.push(root.join(&executable));
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        let mut directories = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&format!("{tool}-")))
            })
            .collect::<Vec<_>>();
        directories.sort();
        for directory in directories {
            candidates.push(directory.join(&executable));
            candidates.push(directory.join("bin").join(&executable));
        }
    }
    let mut seen = BTreeSet::new();
    candidates.retain(|path| path.is_file() && seen.insert(path.clone()));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_version_selects_the_exact_package_not_its_related_crates() {
        let lock = r#"
version = 4
[[package]]
name = "wasm-bindgen-cli"
version = "0.2.130"
[[package]]
name = "wasm-bindgen"
version = "0.2.126"
[[package]]
name = "wasm-bindgen-shared"
version = "0.2.126"
"#;
        assert_eq!(locked_bindgen_version(lock).unwrap(), "0.2.126");
        assert!(locked_bindgen_version("").is_err());
        assert!(locked_bindgen_version(&format!(
            "{lock}\n[[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.130\"\n"
        ))
        .is_err());
    }

    #[test]
    fn optimizer_policy_is_part_of_the_package_cache_context() {
        let mut tools = PackagingTools {
            bindgen: PathBuf::from("wasm-bindgen"),
            bindgen_version: "wasm-bindgen 0.2.126".into(),
            optimizer: None,
        };
        let baseline = tools.binding_context();
        assert!(tools.optimization_context().is_none());
        tools.optimizer = Some((PathBuf::from("wasm-opt"), "wasm-opt version 117".into()));
        assert_eq!(baseline, tools.binding_context());
        assert!(tools
            .optimization_context()
            .unwrap()
            .contains("wasm-opt version 117"));
        tools.bindgen_version = "wasm-bindgen 0.2.130".into();
        assert_ne!(baseline, tools.binding_context());
    }
}
