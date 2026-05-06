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

fn launch_chart_gallery(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_chart-gallery")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_chart_gallery"))
        .unwrap_or_else(|_| "target/debug/chart-gallery".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("failed to launch chart-gallery")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/chart-gallery/chart_gallery_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&dir).ok();
    dir
}

#[test]
#[ignore]
fn sidebar_scroll_reaches_lower_entries() {
    let control_port = reserve_control_port();
    let mut child = launch_chart_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(20_000)
        .expect("chart-gallery did not start");
    client.wait(1_500).expect("initial wait");

    let dir = screenshot_dir();
    let before = format!("{}/01_sidebar_before_scroll.png", dir);
    let after = format!("{}/02_sidebar_after_scroll.png", dir);
    client.screenshot(&before).expect("before screenshot");

    for _ in 0..4 {
        client
            .scroll(120.0, 420.0, 0.0, 180.0)
            .expect("sidebar scroll");
        client.pump().expect("pump after scroll");
        client.wait(200).expect("wait after scroll");
    }
    client.screenshot(&after).expect("after screenshot");

    client
        .assert_text_visible("Liquidfill")
        .expect("scrolling the sidebar should reveal lower navigation entries");

    client.quit().expect("quit");
    let _ = child.wait();
}
