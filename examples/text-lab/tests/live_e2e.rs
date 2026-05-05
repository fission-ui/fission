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

fn launch_text_lab(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_text-lab")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_text_lab"))
        .unwrap_or_else(|_| "target/debug/text-lab".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .spawn()
        .expect("failed to launch text-lab")
}

#[test]
#[ignore]
fn combobox_popup_appears_and_dismisses_after_selection() {
    let control_port = reserve_control_port();
    let mut child = launch_text_lab(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("text-lab did not start");
    client.wait(1_500).expect("initial wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR")
        .unwrap_or_else(|_| "test_screenshots/text_lab_live".into());
    std::fs::create_dir_all(&screenshot_dir).ok();

    client
        .screenshot(&format!("{}/01_initial.png", screenshot_dir))
        .expect("initial screenshot");
    client.assert_text_visible("Combobox wrapper").unwrap();

    // The inline combobox is the third field on the page in the default 800x600 viewport.
    client.tap(260.0, 300.0).expect("focus combobox");
    client.type_text("alice").expect("type combobox query");
    client.pump().expect("pump query");
    client.wait(300).expect("wait for popup");
    client
        .screenshot(&format!("{}/02_popup_open.png", screenshot_dir))
        .expect("popup screenshot");
    client
        .assert_text_visible("alice@example.com")
        .expect("combobox suggestions should appear after typing");

    client
        .tap_text("alice@example.com")
        .expect("select suggestion");
    client.wait(300).expect("wait after selection");
    client
        .screenshot(&format!("{}/03_after_selection.png", screenshot_dir))
        .expect("post-selection screenshot");

    client
        .assert_text_not_visible("alice@example.com")
        .expect("popup should dismiss after selection");

    client.quit().expect("quit");
    let _ = child.wait();
}
