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

fn launch_counter(control_port: u16) -> Child {
    let bin =
        std::env::var("CARGO_BIN_EXE_counter").unwrap_or_else(|_| "target/debug/counter".into());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("failed to launch counter")
}

#[test]
#[ignore]
fn show_modal_visibly_dims_the_background() {
    let control_port = reserve_control_port();
    let mut child = launch_counter(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("counter did not start");
    client.wait(1_500).expect("initial wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/counter/counter_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&screenshot_dir).ok();

    client.tap_text("Show Modal").expect("show modal");
    client.wait(400).expect("wait after show modal");
    client
        .assert_text_visible("Hide Modal")
        .expect("button state should toggle after opening the modal");

    let path = format!("{}/01_modal_visible.png", screenshot_dir);
    client.screenshot(&path).expect("modal screenshot");
    let img = image::open(&path).expect("open screenshot").to_rgba8();
    let px = img.get_pixel(780, 20).0;
    assert!(
        px[0] < 220 && px[1] < 220 && px[2] < 220,
        "opening the modal should dim the background outside the modal; sampled pixel was {:?}",
        px
    );

    client.quit().expect("quit");
    let _ = child.wait();
}
