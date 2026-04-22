/// Live E2E test for the widget gallery.
///
/// This test launches the real widget-gallery binary with the test control
/// channel enabled, then uses the LiveTestClient to interact with it and
/// take screenshots.
///
/// Run with: cargo test -p widget-gallery --test live_e2e -- --ignored
/// (ignored by default because it requires a display and launches a window)

use fission_test_driver::LiveTestClient;
use std::process::{Child, Command};
use std::time::Duration;

const CONTROL_PORT: u16 = 9876;

fn launch_gallery() -> Child {
    let child = Command::new("cargo")
        .args(["run", "-p", "widget-gallery"])
        .env("FISSION_TEST_CONTROL_PORT", CONTROL_PORT.to_string())
        .spawn()
        .expect("failed to launch widget-gallery");
    child
}

#[test]
#[ignore] // requires display + real window
fn gallery_live_screenshot_all_sections() {
    let mut child = launch_gallery();
    let client = LiveTestClient::connect(CONTROL_PORT);

    // Wait for app to be ready
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start in time");

    // Wait for first frame to render
    client.wait(1000).expect("wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR")
        .unwrap_or_else(|_| "test_screenshots/live".into());
    std::fs::create_dir_all(&screenshot_dir).ok();

    // Take initial screenshot
    client
        .screenshot(&format!("{}/01_initial.png", screenshot_dir))
        .expect("screenshot");

    // Get the semantics tree
    let tree = client.get_tree().expect("get_tree");
    println!("Semantics tree has {} nodes", tree.len());
    assert!(tree.len() > 10, "expected many semantic nodes");

    // Scroll down and take more screenshots
    for i in 0..5 {
        client
            .scroll(400.0, 300.0, 0.0, 150.0)
            .expect("scroll");
        client.wait(200).expect("wait");
        client
            .screenshot(&format!("{}/02_scroll_{}.png", screenshot_dir, i))
            .expect("screenshot");
    }

    // Click the "Open Modal" button area (approximate position)
    // In a real test we'd use tap_text or get_text to find coordinates
    // client.tap_text("Open Modal").expect("tap");
    // client.wait(500).expect("wait");
    // client.screenshot(&format!("{}/03_modal.png", screenshot_dir)).expect("screenshot");

    // Quit the app
    client.quit().expect("quit");
    let _ = child.wait();

    println!("Screenshots saved to {}/", screenshot_dir);
}
