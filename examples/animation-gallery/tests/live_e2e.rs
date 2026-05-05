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
        .spawn()
        .expect("failed to launch animation-gallery")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR")
        .unwrap_or_else(|_| "test_screenshots/animation_live".into());
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
