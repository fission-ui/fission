use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "launches Chrome and the Fission server; run with --ignored for browser bridge E2E"]
fn browser_bridge_click_updates_cart_island() {
    let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = project_dir
        .parent()
        .and_then(Path::parent)
        .expect("example should live under examples/<name>");
    let script = project_dir.join("tests/browser_bridge_e2e.mjs");

    let status = Command::new("node")
        .arg(&script)
        .env("FISSION_PROJECT_DIR", project_dir)
        .current_dir(workspace)
        .status()
        .expect("failed to start JavaScript browser bridge E2E test");

    assert!(status.success(), "browser bridge E2E test failed: {status}");
}
