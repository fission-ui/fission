use anyhow::Result;
use std::path::Path;

fn main() -> Result<()> {
    fission_docs2::tutorials::build_all(Path::new(env!("CARGO_MANIFEST_DIR")))
}
