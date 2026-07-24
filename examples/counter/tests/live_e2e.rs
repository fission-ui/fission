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
fn counter_buttons_update_retained_local_state() {
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

    client
        .tap_semantic_identifier("counter.increment")
        .expect("first increment");
    client
        .tap_semantic_identifier("counter.increment")
        .expect("second increment");
    client
        .tap_semantic_identifier("counter.decrement")
        .expect("decrement");
    client.wait(200).expect("wait after counter actions");

    let visible_text = client.get_text().expect("read counter text");
    assert!(
        visible_text.iter().any(|item| item.text == "1"),
        "expected counter value 1, found {:?}",
        visible_text
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>()
    );

    let path = format!("{}/01_counter_value.png", screenshot_dir);
    client.screenshot(&path).expect("counter screenshot");

    client.quit().expect("quit");
    let _ = child.wait();
}
