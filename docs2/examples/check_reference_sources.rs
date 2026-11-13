//! Check chart or widget reference examples and their named shared components.
//! Unresolved cross-page assemblies are reported, never silently counted as checked.
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn widget_imports(code: &str) -> Vec<(String, Vec<String>)> {
    let mut imports = Vec::new();
    let mut rest = code;
    while let Some((_, after)) = rest.split_once("use crate::widgets::") {
        let Some((statement, next)) = after.split_once(';') else {
            break;
        };
        if let Some((module, names)) = statement.split_once("::") {
            let names = names
                .trim()
                .trim_matches(['{', '}'])
                .split(',')
                .filter_map(|name| name.split_whitespace().next().map(str::to_owned))
                .collect();
            imports.push((module.to_owned(), names));
        }
        rest = next;
    }
    imports
}

fn declares(code: &str, name: &str) -> bool {
    let code = code.split_whitespace().collect::<Vec<_>>().join(" ");
    code.contains(&format!("pub {name}<"))
        || code.contains(&format!("pub struct {name} "))
        || code.contains(&format!("pub enum {name} "))
        || code.contains(&format!("pub fn {name}("))
}

fn collect(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "mdx") {
            files.push(path);
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = project.parent().context("docs2 has no workspace parent")?;
    let section = std::env::args().nth(1).unwrap_or_else(|| "charts".into());
    if !matches!(section.as_str(), "charts" | "widgets") {
        bail!("choose charts (the default) or widgets");
    }
    let target = PathBuf::from(
        std::env::var_os("CARGO_TARGET_DIR").context("set the approved shared CARGO_TARGET_DIR")?,
    );
    let stage = target.join(format!("reference-source-audit-{section}"));
    fs::create_dir_all(stage.join("src"))?;
    let mut files = Vec::new();
    collect(
        &project.join("content/reference").join(&section),
        &mut files,
    )?;
    files.sort();
    let mut modules = String::from("#![allow(dead_code, unused_imports, unused_variables)]\n");
    let mut report = String::from("page\tmodule\tstatus\n");
    let mut checked = 0;
    let mut skipped = 0;
    let mut contextual_fences = 0;
    let mut pages = Vec::new();
    for (index, path) in files.iter().enumerate() {
        let source = fs::read_to_string(path)?;
        let mut blocks = Vec::new();
        let mut rest = source.as_str();
        while let Some((_, after)) = rest.split_once("```rust\n") {
            let (code, after) = after
                .split_once("\n```")
                .with_context(|| format!("unclosed Rust example: {}", path.display()))?;
            if section == "widgets" && !code.contains("fission_component!") {
                contextual_fences += 1;
                rest = after;
                continue;
            }
            if code.lines().any(|line| line.starts_with("let ")) {
                // Setup snippets are statements intended for a function body.
                // Keep each such fence isolated rather than moving its variables
                // into unrelated component examples from the same page.
                blocks.push(format!(
                    "fn setup_example_{}() {{\n{code}\n}}",
                    blocks.len()
                ));
            } else {
                blocks.push(code.to_owned());
            }
            rest = after;
        }
        if blocks.is_empty() {
            if section == "widgets" && source.contains("```rust\n") {
                report.push_str(&format!(
                    "{}\t-\tcontextual fragments; not compiled\n",
                    path.strip_prefix(&project)?.display()
                ));
                skipped += 1;
            }
            continue;
        }
        let code = blocks.join("\n\n");
        pages.push((index, path, code));
    }
    let mut aliases = BTreeMap::<String, usize>::new();
    for (index, path, code) in &pages {
        let relative = path.strip_prefix(&project)?.display();
        // Widget references also contain deliberately contextual fragments.
        // Do not invent an AppState, action or build context to make them pass.
        let contextual_widget = section == "widgets"
            && (!code.contains("fission_component!")
                || code.contains("fn setup_example_")
                || (code.contains("AppState") && !code.contains("struct AppState")));
        let imports = widget_imports(code);
        let mut resolved = true;
        for (module, names) in &imports {
            let candidates = pages
                .iter()
                .filter(|(candidate, _, source)| {
                    candidate != index && names.iter().all(|name| declares(source, name))
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                resolved = false;
                eprintln!("Explicit assembly unresolved: {relative}: widgets::{module} ({names:?}), {} definition candidates", candidates.len());
            } else if aliases
                .insert(module.clone(), candidates[0].0)
                .is_some_and(|previous| previous != candidates[0].0)
            {
                bail!("conflicting widget module authority for {module}");
            }
        }
        if contextual_widget
            || !resolved
            || (code.contains("crate::") && imports.is_empty())
            || code.contains("super::")
            || code.contains("// ...")
            || code.contains("todo!()")
        {
            report.push_str(&format!(
                "{relative}\t-\trequires explicit cross-page or fragment assembly\n"
            ));
            skipped += 1;
            continue;
        }
        let module = format!("page_{index:03}");
        // Preserve the authored source; diagnostics identify its page module.
        fs::write(stage.join("src").join(format!("{module}.rs")), code)?;
        modules.push_str(&format!("mod {module};\n"));
        report.push_str(&format!(
            "{relative}\t{module}\tselected fences compiled only if the command succeeds\n"
        ));
        checked += 1;
    }
    modules.push_str("pub mod widgets {\n");
    for (name, index) in aliases {
        modules.push_str(&format!(
            "pub mod {name} {{ pub use crate::page_{index:03}::*; }}\n"
        ));
    }
    modules.push_str("}\n");
    fs::write(stage.join("src/lib.rs"), modules)?;
    fs::write(stage.join("assembly.tsv"), report)?;
    let features = if section == "widgets" {
        "\"charts\", \"three-d\", \"interactive-canvas\", \"terminal-widget\""
    } else {
        "\"charts\", \"three-d\""
    };
    let mut manifest = format!(
        "[package]\nname = \"docs2-reference-source-audit-{section}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\nfission = {{ path = {:?}, default-features = false, features = [{features}] }}\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\nanyhow = \"1\"\nchrono = \"0.4\"\n",
        root.join("crates/authoring/fission"));
    let workspace = fs::read_to_string(root.join("Cargo.toml"))?;
    manifest.push_str(
        &workspace[workspace
            .find("[profile.dev]")
            .context("missing profiles")?..],
    );
    fs::write(stage.join("Cargo.toml"), manifest)?;
    fs::copy(root.join("Cargo.lock"), stage.join("Cargo.lock"))?;
    eprintln!(
        "Checking selected complete examples from {checked} reference pages; {skipped} need manual assembly; {contextual_fences} contextual fences excluded. Map: {}",
        stage.join("assembly.tsv").display()
    );
    let status = Command::new("cargo")
        .args(["check", "--offline", "--lib", "--manifest-path"])
        .arg(stage.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .status()?;
    if !status.success() {
        bail!("reference source check failed; inspect the page-to-module map");
    }
    Ok(())
}
