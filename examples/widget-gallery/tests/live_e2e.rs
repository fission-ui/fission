/// Live E2E test for the widget gallery.
///
/// This test launches the real widget-gallery binary with the test control
/// channel enabled, then uses the LiveTestClient to interact with it and
/// take screenshots.
///
/// Run with: cargo test -p widget-gallery --test live_e2e -- --ignored
/// (ignored by default because it requires a display and launches a window)
use fission_test_driver::LiveTestClient;
use std::net::TcpListener;
use std::process::{Child, Command};

fn reserve_control_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral test port")
        .local_addr()
        .expect("read ephemeral test port")
        .port()
}

fn launch_gallery(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_widget-gallery")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_widget_gallery"))
        .unwrap_or_else(|_| "target/debug/widget-gallery".to_string());
    let child = Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("failed to launch widget-gallery");
    child
}

#[test]
#[ignore] // requires display + real window
fn gallery_live_screenshot_all_sections() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);

    // Wait for app to be ready
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start in time");

    // Wait for first frame to render
    client.wait(1000).expect("wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/widget-gallery/live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
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
        client.scroll(400.0, 300.0, 0.0, 150.0).expect("scroll");
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

#[test]
#[ignore]
fn scrolling_changes_the_visible_gallery_window() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);

    client
        .wait_for_ready(15_000)
        .expect("gallery did not start in time");
    client.wait(1_000).expect("wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/widget-gallery/live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&screenshot_dir).ok();
    let before = format!("{}/03_before_scroll_assert.png", screenshot_dir);
    let after = format!("{}/04_after_scroll_assert.png", screenshot_dir);

    client.screenshot(&before).expect("before screenshot");
    for _ in 0..3 {
        client.scroll(400.0, 300.0, 0.0, 180.0).expect("scroll");
        client.pump().expect("pump after scroll");
        client.wait(200).expect("wait after scroll");
    }
    client.screenshot(&after).expect("after screenshot");

    let before_img = image::open(&before)
        .expect("open before screenshot")
        .to_rgba8();
    let after_img = image::open(&after)
        .expect("open after screenshot")
        .to_rgba8();

    // This pixel sits inside the prominent "Filled" pill in the top viewport.
    // After several scroll steps, that control should no longer occupy the same
    // screen position.
    let before_px = before_img.get_pixel(60, 455).0;
    let after_px = after_img.get_pixel(60, 455).0;
    assert_ne!(
        before_px, after_px,
        "scrolling should move the top controls out of place; sampled pixel stayed unchanged at {:?}",
        before_px
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn initial_surface_uses_a_light_page_background() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);

    client
        .wait_for_ready(15_000)
        .expect("gallery did not start in time");
    client.wait(1_000).expect("wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/widget-gallery/live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&screenshot_dir).ok();
    let path = format!("{}/05_light_background.png", screenshot_dir);
    client.screenshot(&path).expect("screenshot");

    let img = image::open(&path).expect("open screenshot").to_rgba8();
    let px = img.get_pixel(8, 8).0;
    assert!(
        px[0] > 230 && px[1] > 230 && px[2] > 230,
        "default light-theme examples should not clear to a dark page background; sampled pixel was {:?}",
        px
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn typing_into_the_visible_text_input_updates_the_field() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);

    client
        .wait_for_ready(15_000)
        .expect("gallery did not start in time");
    client.wait(1_000).expect("wait");

    client.tap(150.0, 505.0).expect("focus input");
    client.type_text("hello").expect("type text");
    client.pump().expect("pump after typing");
    client.wait(300).expect("wait after typing");

    client
        .assert_text_visible("hello")
        .expect("typed text should be visible in the input field");

    client.quit().expect("quit");
    let _ = child.wait();
}
