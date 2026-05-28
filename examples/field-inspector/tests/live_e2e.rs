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

fn launch_field_inspector(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_field-inspector")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_field_inspector"))
        .unwrap_or_else(|_| "target/debug/field-inspector".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .env("FISSION_FIELD_INSPECTOR_DEMO_HOSTS", "1")
        .spawn()
        .expect("failed to launch field-inspector")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/field-inspector/live_e2e",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&dir).expect("create screenshot dir");
    dir
}

#[test]
#[ignore]
fn field_inspector_runs_capability_workflow_smoke() {
    let control_port = reserve_control_port();
    let mut child = launch_field_inspector(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("field-inspector did not start");
    client
        .simulate_resize(1360, 900)
        .expect("resize field-inspector");
    client
        .assert_text_visible("Capability-driven field service")
        .expect("hero should be visible");
    client
        .tap_text("Start inspection")
        .expect("start inspection");
    client.wait(1_000).expect("wait for capability effects");
    client.pump().expect("pump after capability effects");
    client
        .assert_text_visible("Capability readiness")
        .expect("readiness panel should stay visible");
    client
        .assert_text_visible("51.50740")
        .expect("geolocation capability result should be rendered");
    client
        .assert_text_visible("1 nearby device(s)")
        .expect("bluetooth capability result should be rendered");

    let path = format!("{}/field_inspector_overview.png", screenshot_dir());
    client
        .screenshot(&path)
        .expect("capture field inspector screenshot");

    client
        .simulate_resize(390, 844)
        .expect("resize field-inspector to phone viewport");
    client.wait(500).expect("wait for compact relayout");
    client.pump().expect("pump after compact relayout");
    client
        .assert_text_visible("Demo memory mode")
        .expect("provider mode should stay visible in compact layout");
    let path = format!("{}/field_inspector_phone.png", screenshot_dir());
    client
        .screenshot(&path)
        .expect("capture compact field inspector screenshot");

    client.quit().expect("quit");
    let _ = child.wait();
}
