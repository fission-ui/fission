use fission_test_driver::LiveTestClient;
use image::GenericImageView;
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
    let bin = std::env::var("CARGO_BIN_EXE_animation-gallery")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_animation_gallery"))
        .unwrap_or_else(|_| "target/debug/animation-gallery".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("failed to launch animation-gallery")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/animation-gallery/animation_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn assert_png_dimensions(path: &str, expected_width: u32, expected_height: u32) {
    let img = image::open(path).expect("open screenshot");
    let (width, height) = img.dimensions();
    assert_eq!(
        (width, height),
        (expected_width, expected_height),
        "unexpected screenshot dimensions for {path}"
    );
}

fn count_non_background_pixels(path: &str, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    let img = image::open(path).expect("open screenshot").to_rgba8();
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = img.get_pixel(x, y).0;
            if px[0] < 245 || px[1] < 245 || px[2] < 245 {
                count += 1;
            }
        }
    }
    count
}

#[test]
#[ignore]
fn animation_gallery_live_transitions_and_resize() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1000).expect("initial wait");

    let dir = screenshot_dir();

    client
        .screenshot(&format!("{}/01_initial.png", dir))
        .expect("initial screenshot");
    client.assert_text_visible("Animation Gallery").unwrap();
    client.assert_text_visible("Toggle scene").unwrap();
    client.assert_text_visible("Toggle custom pulse").unwrap();
    client.assert_text_visible("Scroll translation").unwrap();

    client.tap_text("Toggle scene").expect("toggle scene");
    client.wait(400).expect("wait after scene toggle");
    client
        .screenshot(&format!("{}/02_scene_toggled.png", dir))
        .expect("scene toggled screenshot");

    client
        .tap_text("Toggle custom pulse")
        .expect("toggle custom pulse");
    client.wait(400).expect("wait after custom toggle");
    client
        .screenshot(&format!("{}/03_custom_paused.png", dir))
        .expect("custom paused screenshot");

    client.simulate_resize(1280, 900).expect("simulate resize");
    client.pump().expect("pump after resize");
    client.wait(300).expect("wait after resize");
    let resized_path = format!("{}/04_resized.png", dir);
    client
        .screenshot(&resized_path)
        .expect("resized screenshot");
    assert_png_dimensions(&resized_path, 1280, 900);

    let tree = client.get_tree().expect("get_tree");
    println!("Animation gallery semantics nodes: {}", tree.len());
    assert!(tree.len() >= 2, "expected animation gallery semantics");

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_initial_cards_are_painted() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    let path = format!("{}/05_initial_cards.png", dir);
    client.screenshot(&path).expect("initial screenshot");

    let opacity_pixels = count_non_background_pixels(&path, 40, 250, 180, 360);
    assert!(
        opacity_pixels > 500,
        "opacity card should have visible painted content at time zero; non-background pixels={}",
        opacity_pixels
    );

    let translate_pixels = count_non_background_pixels(&path, 400, 250, 700, 360);
    assert!(
        translate_pixels > 500,
        "translate card should have visible painted content at time zero; non-background pixels={}",
        translate_pixels
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_paused_custom_pulse_keeps_a_visible_frame() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    client.tap_text("Toggle scene").expect("toggle scene");
    client.wait(400).expect("wait after scene toggle");
    client
        .tap_text("Toggle custom pulse")
        .expect("toggle custom pulse");
    client.wait(400).expect("wait after custom toggle");

    let path = format!("{}/06_custom_paused_visible.png", dir);
    client.screenshot(&path).expect("paused screenshot");
    let pulse_pixels = count_non_background_pixels(&path, 40, 250, 520, 360);
    assert!(
        pulse_pixels > 500,
        "paused custom pulse should retain visible content instead of blanking the card; non-background pixels={}",
        pulse_pixels
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn resized_surface_does_not_fall_back_to_a_dark_clear_band() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    client.simulate_resize(1280, 900).expect("simulate resize");
    client.pump().expect("pump after resize");
    client.wait(300).expect("wait after resize");

    let path = format!("{}/07_resize_clear_band.png", dir);
    client.screenshot(&path).expect("resized screenshot");

    let img = image::open(&path).expect("open screenshot").to_rgba8();
    let px = img.get_pixel(100, 850).0;
    assert!(
        px[0] > 230 && px[1] > 230 && px[2] > 230,
        "resized light-theme surface should not expose a dark compositor clear band; sampled pixel was {:?}",
        px
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn wide_resize_uses_the_available_horizontal_space() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    client.simulate_resize(1280, 900).expect("simulate resize");
    client.pump().expect("pump after resize");
    client.wait(300).expect("wait after resize");

    let path = format!("{}/08_wide_space_usage.png", dir);
    client.screenshot(&path).expect("wide resize screenshot");

    let right_half_pixels = count_non_background_pixels(&path, 860, 170, 1230, 520);
    assert!(
        right_half_pixels > 2_500,
        "wide animation gallery layouts should use the available horizontal space; right-half non-background pixels={}",
        right_half_pixels
    );

    client.quit().expect("quit");
    let _ = child.wait();
}
