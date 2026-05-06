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
        .env("FISSION_BACKGROUND_TEST", "1")
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

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/text-lab/text_lab_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&screenshot_dir).ok();

    client
        .screenshot(&format!("{}/01_initial.png", screenshot_dir))
        .expect("initial screenshot");
    client.assert_text_visible("Combobox wrapper").unwrap();

    // The inline combobox is the third field on the page in the default 800x600 viewport.
    client.tap(200.0, 420.0).expect("focus combobox");
    client.type_text("alice").expect("type combobox query");
    client.pump().expect("pump query");
    client.wait(300).expect("wait for popup");
    let open_path = format!("{}/02_popup_open.png", screenshot_dir);
    client.screenshot(&open_path).expect("popup screenshot");
    client
        .assert_text_visible("alice@example.com")
        .expect("combobox suggestions should appear after typing");

    client
        .tap_text("alice@example.com")
        .expect("select suggestion");
    client.wait(300).expect("wait after selection");
    let after_path = format!("{}/03_after_selection.png", screenshot_dir);
    client
        .screenshot(&after_path)
        .expect("post-selection screenshot");
    client
        .tap_text("Open modal text flow")
        .expect("underlying controls should remain clickable after popup selection");
    client
        .wait(400)
        .expect("wait for modal after popup dismissal");
    client
        .assert_text_visible("Text Lab Modal")
        .expect("popup should dismiss cleanly so the next interaction can succeed");

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn modal_remains_visible_while_typing_and_apply_stays_reachable() {
    let control_port = reserve_control_port();
    let mut child = launch_text_lab(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("text-lab did not start");
    client.wait(1_500).expect("initial wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/text-lab/text_lab_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&screenshot_dir).ok();

    client.tap_text("Open modal text flow").expect("open modal");
    client.wait(400).expect("wait for modal");
    client
        .screenshot(&format!("{}/04_modal_open.png", screenshot_dir))
        .expect("modal-open screenshot");
    client.assert_text_visible("Apply").expect("apply visible");

    // Focus the modal's "To *" field and type.
    client.tap(180.0, 155.0).expect("focus modal To field");
    client.type_text("alice").expect("type in modal");
    client.pump().expect("pump after typing");
    client.wait(300).expect("wait after typing");
    client
        .screenshot(&format!("{}/05_modal_typed.png", screenshot_dir))
        .expect("modal-typed screenshot");

    client.assert_text_visible("Apply").expect(
        "modal should remain visible and keep Apply reachable while typing into a nested text field",
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn modal_apply_clears_recipient_suggestion_overlay() {
    let control_port = reserve_control_port();
    let mut child = launch_text_lab(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("text-lab did not start");
    client.wait(1_500).expect("initial wait");

    client.tap_text("Open modal text flow").expect("open modal");
    client.wait(400).expect("wait for modal");
    client.tap(180.0, 155.0).expect("focus modal To field");
    client.type_text("alice").expect("type in modal");
    client.pump().expect("pump after typing");
    client.wait(300).expect("wait after typing");
    client.tap_text("Apply").expect("apply modal");
    client.wait(400).expect("wait after apply");

    client
        .assert_text_not_visible("alice@example.com")
        .expect("recipient suggestion overlay text should be torn down when the modal closes");

    client.quit().expect("quit");
    let _ = child.wait();
}
