//! Setup authorities: execute once and return verified facts for either renderer.
use crate::{
    report::{self, Data, ErrorCode, Failure, Result},
    Target,
};
use std::path::{Path, PathBuf};

pub fn init_project(
    root: &Path,
    name: Option<String>,
    app_id: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<Data> {
    let root = report::absolute(root)?;
    super::init_project_files(&root, name, app_id, local_path)
        .map_err(|error| report::operation_failure(&error))?;
    let artifacts =
        report::verified_files(vec![root.join("fission.toml"), root.join("Cargo.toml")])?;
    let instructions_root = super::find_git_root(&root).unwrap_or_else(|| root.clone());
    let mut instructions = ["AGENTS.md", "AGENTS.fission.md"]
        .into_iter()
        .map(|name| instructions_root.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    if root != instructions_root && root.join("AGENTS.md").is_file() {
        instructions.push(root.join("AGENTS.md"));
    }
    Ok(Data {
        artifacts,
        instructions,
        ..Data::default()
    })
}

pub fn add_targets(root: &Path, targets: &[Target]) -> Result<Data> {
    let root = report::absolute(root)?;
    if targets.is_empty() {
        return Err(Failure::new(
            ErrorCode::InvalidTarget,
            "Select at least one target.",
        ));
    }
    report::load_project(&root)?;
    super::add_target_files(&root, targets).map_err(|error| report::operation_failure(&error))?;
    let mut artifacts = vec![root.join("fission.toml")];
    artifacts.extend(
        targets
            .iter()
            .map(|target| root.join(target.scaffold_relative_path())),
    );
    Ok(Data {
        artifacts: report::verified_files(artifacts)?,
        ..Data::default()
    })
}
