//! Internal serving sessions shared by the existing command authorities.

use crate::{BuildOptions, WebCargoOptions};
use anyhow::{bail, Context, Result};
use fission_command_core::Target;
use fission_command_process::StartupContext;
use fission_command_site::serving::{normalize_mount, ServerOptions};

pub use fission_command_site::serving::{ServeOptions, ServingEvent, ServingSession};

/// Builds via the existing authority, then returns an owned listener after
/// required local assets are verified. Browser readiness belongs to testing.
pub fn build_and_serve(
    build: BuildOptions,
    web_cargo: WebCargoOptions,
    mut server: ServerOptions,
    startup: &StartupContext,
    test_control: bool,
) -> Result<ServingSession> {
    let target = build
        .target
        .context("serving requires an explicit browser target")?;
    if !matches!(target, Target::Web | Target::Site) {
        bail!("file serving requires Web or Static site");
    }
    if test_control && target != Target::Web {
        bail!("test control requires Web");
    }
    server.mount = normalize_mount(&server.mount)?;
    server.spa = target == Target::Web;
    let project_dir = build.project_dir.clone();
    startup.run(|| crate::build_app_internal(build, web_cargo, test_control))?;
    let root = if target == Target::Web {
        project_dir.join("platforms/web")
    } else {
        fission_command_site::output_dir(&project_dir)?
    };
    ServingSession::start(root, &server, startup)
}
