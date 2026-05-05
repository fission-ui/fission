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

fn launch_icons_gallery(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_icons_gallery")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_icons-gallery"))
        .unwrap_or_else(|_| "target/debug/icons_gallery".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .spawn()
        .expect("failed to launch icons_gallery")
}

fn visible_text_signature(client: &LiveTestClient) -> Vec<(String, i32)> {
    client
        .get_text()
        .expect("get_text")
        .into_iter()
        .filter(|item| !item.text.trim().is_empty())
        .take(20)
        .map(|item| (item.text, item.y.round() as i32))
        .collect()
}

#[test]
#[ignore]
fn scrolling_changes_the_visible_window() {
    let control_port = reserve_control_port();
    let mut child = launch_icons_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("icons gallery did not start");
    client.wait(1_500).expect("initial wait");

    let before = visible_text_signature(&client);
    assert!(!before.is_empty(), "expected visible text before scroll");

    for _ in 0..4 {
        client.scroll(400.0, 300.0, 0.0, 180.0).expect("scroll");
        client.pump().expect("pump after scroll");
        client.wait(200).expect("wait after scroll");
    }

    let after = visible_text_signature(&client);
    assert_ne!(
        before, after,
        "scrolling should change visible text content or positions"
    );

    client.quit().expect("quit");
    let _ = child.wait();
}
