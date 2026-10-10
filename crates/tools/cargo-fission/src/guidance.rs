//! Guidance rendering at the CLI boundary; authorities return typed facts only.
use fission_command_core::guidance::{GuidanceInstallation, GuidanceResult};

pub(crate) fn print_result(result: &GuidanceResult, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string(result)?);
    } else {
        println!(
            "Guidance {:?}: CLI {}, API {}, guidance {}, schema {}",
            result.status,
            result.bundled.cli_version,
            result.bundled.framework_api_version,
            result.bundled.guidance_version,
            result.schema_version
        );
        println!(
            "Framework: {:?}: {}",
            result.framework_dependency.compatibility, result.framework_dependency.message
        );
        println!("Guidance root: {}", result.guidance_root.display());
        for file in &result.files {
            println!(
                "{:?}: {}",
                file.health,
                result.guidance_root.join(&file.path).display()
            );
        }
        for finding in &result.findings {
            println!("{}: {}", finding.path.display(), finding.message);
        }
        for recovery in &result.recovery {
            println!("{}\nargv: {:?}", recovery.reason, recovery.argv);
        }
    }
    Ok(())
}

pub(crate) fn print_installation(installation: &GuidanceInstallation) {
    let result = &installation.guidance;
    println!("Fission guidance: {:?}", result.status);
    for path in &result.instruction_paths {
        println!("Read instructions: {}", path.display());
    }
    if let Some(path) = &installation.shared_reference {
        println!("Shared app rules: {}", path.display());
    }
    if let Some(path) = &installation.web_router {
        println!("Web router: {}", path.display());
    } else {
        eprintln!("Web router unavailable; resolve guidance findings with fission skills check --project-dir {}", result.project_dir.display());
    }
    for finding in result.findings.iter().filter(|f| f.blocks_update) {
        eprintln!(
            "Guidance conflict at {}: {}",
            finding.path.display(),
            finding.message
        );
    }
}
