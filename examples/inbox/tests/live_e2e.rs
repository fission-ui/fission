/// Live E2E test for the Inbox example.
///
/// Launches the real inbox binary with test control, takes screenshots
/// of each section, scrolls, clicks, and verifies behavior.
///
/// Run: cargo test -p inbox --test live_e2e -- --ignored --nocapture
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

fn launch_inbox(control_port: u16) -> Child {
    let bin =
        std::env::var("CARGO_BIN_EXE_inbox").unwrap_or_else(|_| "target/debug/inbox".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .spawn()
        .expect("failed to launch inbox")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR")
        .unwrap_or_else(|_| "test_screenshots/inbox_e2e".into());
    std::fs::create_dir_all(&dir).ok();
    dir
}

#[test]
#[ignore]
fn inbox_initial_render() {
    let control_port = reserve_control_port();
    let mut child = launch_inbox(control_port);
    let client = LiveTestClient::connect(control_port);
    client.wait_for_ready(20_000).expect("inbox did not start");
    client.wait(2000).expect("wait for render");

    let dir = screenshot_dir();

    // Screenshot initial state
    client
        .screenshot(&format!("{}/01_initial.png", dir))
        .expect("screenshot");

    // Get all text
    let texts = client.get_text().expect("get_text");
    let all: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
    println!("Found {} text items", texts.len());
    for t in &texts[..texts.len().min(30)] {
        println!(
            "  [{:.0},{:.0} {:.0}x{:.0}] \"{}\"",
            t.x, t.y, t.width, t.height, t.text
        );
    }

    // Verify key UI elements rendered
    let has = |needle: &str| all.iter().any(|t| t.contains(needle));
    assert!(has("Inbox"), "Inbox label missing");

    // Get semantics tree
    let tree = client.get_tree().expect("get_tree");
    println!("\nSemantics tree: {} nodes", tree.len());
    let buttons: Vec<_> = tree.iter().filter(|n| n.role == "Button").collect();
    let text_inputs: Vec<_> = tree.iter().filter(|n| n.role == "TextInput").collect();
    println!("  Buttons: {}", buttons.len());
    println!("  TextInputs: {}", text_inputs.len());

    client.quit().expect("quit");
    let _ = child.wait();
    println!("\nScreenshots saved to {}/", dir);
}

#[test]
#[ignore]
fn inbox_scroll_and_interact() {
    let control_port = reserve_control_port();
    let mut child = launch_inbox(control_port);
    let client = LiveTestClient::connect(control_port);
    client.wait_for_ready(20_000).expect("inbox did not start");
    client.wait(2000).expect("wait");

    let dir = screenshot_dir();

    // Initial
    client
        .screenshot(&format!("{}/02_before_scroll.png", dir))
        .expect("screenshot");

    // Scroll the email list area (center of window)
    for _i in 0..3 {
        client.scroll(400.0, 400.0, 0.0, 100.0).expect("scroll");
        client.wait(300).expect("wait");
    }
    client
        .screenshot(&format!("{}/03_after_scroll.png", dir))
        .expect("screenshot");

    // Try clicking "Compose" button
    let result = client.tap_text("Compose");
    match result {
        Ok(()) => {
            client.wait(500).expect("wait");
            client
                .screenshot(&format!("{}/04_after_compose.png", dir))
                .expect("screenshot");
            println!("Tapped Compose successfully");
        }
        Err(e) => {
            println!("Could not tap Compose: {}", e);
        }
    }

    // Try clicking a folder
    let result = client.tap_text("Starred");
    match result {
        Ok(()) => {
            client.wait(500).expect("wait");
            client
                .screenshot(&format!("{}/05_starred.png", dir))
                .expect("screenshot");
            println!("Tapped Starred successfully");
        }
        Err(e) => {
            println!("Could not tap Starred: {}", e);
        }
    }

    // Try clicking Settings
    let result = client.tap_text("Settings");
    match result {
        Ok(()) => {
            client.wait(500).expect("wait");
            client
                .screenshot(&format!("{}/06_settings.png", dir))
                .expect("screenshot");
            println!("Opened Settings");
        }
        Err(e) => {
            println!("Could not tap Settings: {}", e);
        }
    }

    client.quit().expect("quit");
    let _ = child.wait();
    println!("\nAll screenshots saved to {}/", dir);
}

#[test]
#[ignore]
fn compose_recipient_typing_shows_suggestions() {
    let control_port = reserve_control_port();
    let mut child = launch_inbox(control_port);
    let client = LiveTestClient::connect(control_port);
    client.wait_for_ready(20_000).expect("inbox did not start");
    client.wait(1_500).expect("wait");

    let dir = screenshot_dir();
    client.tap_text("Compose").expect("open compose");
    client.wait(500).expect("wait for compose");
    client
        .screenshot(&format!("{}/07_compose_open.png", dir))
        .expect("compose open screenshot");

    // Focus the top recipient field and type a known suggestion prefix.
    client.tap(160.0, 122.0).expect("focus recipient field");
    client.type_text("alice").expect("type recipient query");
    client.pump().expect("pump after typing");
    client.wait(400).expect("wait after typing");
    client
        .screenshot(&format!("{}/08_compose_suggestions.png", dir))
        .expect("compose suggestion screenshot");

    client.assert_text_visible("alice@example.com").expect(
        "typing in the compose recipient field should show the inline suggestion popup",
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn settings_modal_layout_has_readable_text_rows() {
    let control_port = reserve_control_port();
    let mut child = launch_inbox(control_port);
    let client = LiveTestClient::connect(control_port);
    client.wait_for_ready(20_000).expect("inbox did not start");
    client.wait(1_500).expect("wait");

    let dir = screenshot_dir();
    client.tap_text("Settings").expect("open settings");
    client.wait(700).expect("wait for settings");
    client
        .screenshot(&format!("{}/10_settings_layout.png", dir))
        .expect("settings screenshot");

    let texts = client.get_text().expect("get_text after settings");
    for label in ["General", "Appearance", "Theme"] {
        let item = texts
            .iter()
            .find(|item| item.text == label)
            .unwrap_or_else(|| panic!("expected settings label '{label}' to be visible"));
        assert!(
            item.height >= 10.0,
            "settings label '{}' should occupy a readable row height, got {:?}",
            label,
            item
        );
    }

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn compose_schedule_time_is_zero_padded() {
    let control_port = reserve_control_port();
    let mut child = launch_inbox(control_port);
    let client = LiveTestClient::connect(control_port);
    client.wait_for_ready(20_000).expect("inbox did not start");
    client.wait(1_500).expect("wait");

    client.tap_text("Compose").expect("open compose");
    client.wait(700).expect("wait for compose");

    let d = screenshot_dir();
    client
        .screenshot(&format!("{}/07_compose_time_padded.png", d))
        .expect("compose screenshot");

    client
        .assert_text_visible("09")
        .expect("compose time picker should display a zero-padded hour");
    client
        .assert_text_visible("00")
        .expect("compose time picker should display a zero-padded minute");

    client.quit().expect("quit");
    let _ = child.wait();
}
