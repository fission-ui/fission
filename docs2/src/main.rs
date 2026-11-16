use anyhow::{Context, Result};
use fission::site::build_from_cli;
use fission_docs2::{site_app, tutorials};
use std::path::Path;

fn main() -> Result<()> {
    tutorials::build_all(Path::new(env!("CARGO_MANIFEST_DIR")))
        .context("the goal apps did not compile; the documentation site was not built")?;
    build_from_cli(site_app()?)
}
