use arboard::Clipboard;
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

fn launch_terminal(control_port: u16) -> Child {
    let bin =
        std::env::var("CARGO_BIN_EXE_terminal").unwrap_or_else(|_| "target/debug/terminal".into());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .spawn()
        .expect("failed to launch terminal example")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR")
        .unwrap_or_else(|_| "test_screenshots/terminal_live".into());
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn focus_terminal(client: &LiveTestClient) {
    client.tap(120.0, 88.0).expect("focus terminal");
    client.wait(250).expect("wait after focus");
}

#[test]
#[ignore]
fn terminal_executes_commands_pastes_and_copies_selection() {
    let control_port = reserve_control_port();
    let mut child = launch_terminal(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("terminal did not start");
    client.wait(1_500).expect("initial wait");
    focus_terminal(&client);

    client
        .type_text("printf 'FISSION_TERMINAL_OK\\n'")
        .expect("type printf");
    client.press_key("Enter", 0).expect("run printf");
    client.wait(500).expect("wait for output");
    client
        .assert_text_visible("FISSION_TERMINAL_OK")
        .expect("terminal output should be visible");

    let mut clipboard = Clipboard::new().expect("clipboard available");
    clipboard
        .set_text("printf 'FISSION_PASTE_OK\\n'")
        .expect("seed clipboard");
    client.press_key("V", 4).expect("paste clipboard command");
    client.wait(500).expect("wait after paste");
    client
        .assert_text_visible("FISSION_PASTE_OK")
        .expect("pasted command output should be visible");

    client
        .type_text("printf 'COPY_ME_FROM_TERMINAL\\n'")
        .expect("type copy target command");
    client.press_key("Enter", 0).expect("run copy target");
    client.wait(500).expect("wait for copy target");
    client
        .assert_text_visible("COPY_ME_FROM_TERMINAL")
        .expect("copy target should be visible");

    let target = client
        .get_text()
        .expect("read visible text")
        .into_iter()
        .find(|item| item.text.contains("COPY_ME_FROM_TERMINAL"))
        .expect("copy target text item");
    let y = target.y + target.height * 0.5;
    client
        .drag(target.x + 2.0, y, target.x + target.width - 2.0, y, 10)
        .expect("drag terminal selection");
    client.press_key("C", 4).expect("copy terminal selection");
    client.wait(250).expect("wait after copy");

    let copied = clipboard.get_text().expect("read clipboard text");
    assert!(
        copied.contains("COPY_ME_FROM_TERMINAL"),
        "terminal selection copy should place the selected text on the clipboard, got: {copied:?}"
    );

    let screenshot_dir = screenshot_dir();
    client
        .screenshot(&format!("{}/01_terminal_commands_copy.png", screenshot_dir))
        .expect("terminal screenshot");

    client.quit().expect("quit terminal example");
    let _ = child.wait();
}

#[test]
#[ignore]
fn terminal_alt_screen_switches_the_visible_surface() {
    let control_port = reserve_control_port();
    let mut child = launch_terminal(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("terminal did not start");
    client.wait(1_500).expect("initial wait");
    focus_terminal(&client);

    client
        .type_text("printf $'\\e[?1049hALT SCREEN ACTIVE\\r\\n'; sleep 1; printf $'\\e[?1049l'")
        .expect("type alt-screen command");
    client
        .press_key("Enter", 0)
        .expect("run alt-screen command");
    client.wait(350).expect("wait for alt-screen enter");
    client.pump().expect("pump alt-screen visible frame");
    client
        .assert_text_visible("ALT SCREEN ACTIVE")
        .expect("alternate screen content should be visible while active");

    let screenshot_dir = screenshot_dir();
    client
        .screenshot(&format!("{}/02_alt_screen_active.png", screenshot_dir))
        .expect("active alt-screen screenshot");

    client.wait(1_200).expect("wait for alt-screen exit");
    client.pump().expect("pump after alt-screen exit");
    client
        .type_text("printf 'AFTER_ALT_SCREEN\\n'")
        .expect("type post alt-screen command");
    client
        .press_key("Enter", 0)
        .expect("run post alt-screen command");
    client
        .wait(400)
        .expect("wait after post alt-screen command");
    client
        .assert_text_visible("AFTER_ALT_SCREEN")
        .expect("terminal should remain interactive after leaving the alternate screen");

    client
        .screenshot(&format!("{}/03_alt_screen_restored.png", screenshot_dir))
        .expect("restored terminal screenshot");

    client.quit().expect("quit terminal example");
    let _ = child.wait();
}
