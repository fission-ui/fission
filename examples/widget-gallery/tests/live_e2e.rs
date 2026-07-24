/// Live E2E test for the widget gallery.
///
/// This test launches the real widget-gallery binary with the test control
/// channel enabled, then uses the LiveTestClient to interact with it and
/// take screenshots.
///
/// Run with: cargo test -p widget-gallery --test live_e2e -- --ignored
/// (ignored by default because it requires a display and launches a window)
use fission_test_driver::{LiveTestClient, SelectorQuery};
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
    let input_selector = SelectorQuery::semantic_identifier("gallery.button.filled");

    client.screenshot(&before).expect("before screenshot");
    let before_input = client
        .resolve_selector(input_selector.clone())
        .expect("resolve the initially visible input");
    let before_bounds = before_input
        .visible_bounds
        .expect("input should initially have visible bounds");
    for _ in 0..3 {
        client.scroll(400.0, 300.0, 0.0, 180.0).expect("scroll");
        client.pump().expect("pump after scroll");
        client.wait(200).expect("wait after scroll");
    }
    client.screenshot(&after).expect("after screenshot");
    let after_input = client
        .resolve_selector(input_selector.include_hidden())
        .expect("resolve input after scrolling");
    assert!(
        after_input.visible_bounds.is_none()
            || after_input
                .visible_bounds
                .is_some_and(|bounds| bounds.y < before_bounds.y - 100.0),
        "scrolling should move the input substantially upward or out of view: before={before_bounds:?}, after={:?}",
        after_input.visible_bounds
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

    client
        .fill_text_semantic_identifier("gallery.text_input", "hello")
        .expect("fill visible text input");
    client.wait(300).expect("wait after typing");

    client
        .assert_text_visible("hello")
        .expect("typed text should be visible in the input field");

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn slider_track_click_uses_the_full_visible_width() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    let selector = SelectorQuery::semantic_identifier("gallery.slider");

    client
        .wait_for_ready(15_000)
        .expect("gallery did not start in time");
    client
        .scroll_into_view(selector.clone().include_hidden())
        .expect("scroll slider into view");
    let slider = client
        .resolve_selector(selector.clone())
        .expect("resolve visible slider");
    let bounds = slider.visible_bounds.expect("visible slider bounds");
    assert!(
        bounds.width >= 200.0,
        "slider semantics must cover its track, got {bounds:?}"
    );

    client
        .tap(
            bounds.x + bounds.width * 0.75,
            bounds.y + bounds.height / 2.0,
        )
        .expect("click slider at 75 percent");
    client.wait(200).expect("wait after slider click");

    let updated = client
        .resolve_selector(selector)
        .expect("resolve updated slider");
    let value = updated
        .value
        .as_deref()
        .expect("slider value")
        .parse::<f32>()
        .expect("numeric slider value");
    assert!(
        (value - 75.0).abs() <= 1.0,
        "slider click should map to 75, got {value}"
    );

    client.quit().expect("quit");
    let _ = child.wait();
}
