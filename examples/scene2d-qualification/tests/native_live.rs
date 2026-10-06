use std::collections::HashSet;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use fission_test_driver::{LiveTestClient, SemanticNode};

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn finish(mut self, client: &LiveTestClient) {
        client.quit().expect("quit qualification app");
        let status = self.0.take().expect("qualification child").wait();
        assert!(
            status.expect("wait for qualification app").success(),
            "qualification app exited unsuccessfully"
        );
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

fn reserve_control_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral test port")
        .local_addr()
        .expect("read ephemeral test port")
        .port()
}

fn launch() -> (ChildGuard, LiveTestClient) {
    let control_port = reserve_control_port();
    let child = Command::new(env!("CARGO_BIN_EXE_scene2d-qualification"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("launch 2D qualification app");
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(30_000)
        .expect("2D qualification app becomes ready");
    (ChildGuard(Some(child)), client)
}

fn screenshot_path() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.artifacts/screenshots/examples/scene2d-qualification");
    std::fs::create_dir_all(&directory).expect("create screenshot directory");
    directory.join(format!("complete-native-{}.png", std::env::consts::OS))
}

fn semantic_node(client: &LiveTestClient, identifier: &str) -> SemanticNode {
    client
        .get_tree()
        .expect("read semantic tree")
        .into_iter()
        .find(|node| node.identifier.as_deref() == Some(identifier))
        .unwrap_or_else(|| panic!("missing semantic node {identifier}"))
}

fn assert_varied_pixels(path: &Path, node: &SemanticNode) {
    let image = image::open(path).expect("open screenshot").to_rgba8();
    let bounds = node.visible_bounds.unwrap_or(node.logical_bounds);
    let x0 = bounds.x.max(0.0).floor() as u32;
    let y0 = bounds.y.max(0.0).floor() as u32;
    let x1 = (bounds.x + bounds.width).ceil().max(0.0) as u32;
    let y1 = (bounds.y + bounds.height).ceil().max(0.0) as u32;
    let mut buckets = HashSet::new();
    let mut visible = 0usize;
    for y in y0..y1.min(image.height()) {
        for x in x0..x1.min(image.width()) {
            let [r, g, b, a] = image.get_pixel(x, y).0;
            if a > 0 {
                visible += 1;
                buckets.insert((r / 16, g / 16, b / 16));
            }
        }
    }
    assert!(visible > 10_000, "2D viewport did not render enough pixels");
    assert!(
        buckets.len() > 8,
        "2D viewport is blank or a compatibility surface: {buckets:?}"
    );
}

#[test]
#[ignore = "requires a graphical desktop session"]
fn keyboard_completes_the_native_game_and_renders_scene_pixels() {
    let (child, client) = launch();
    client
        .wait_for_text("Guide the scout", 30_000)
        .expect("game becomes ready");

    for (key, count) in [("ArrowUp", 8), ("ArrowRight", 40), ("ArrowDown", 8)] {
        for _ in 0..count {
            client.press_key(key, 0).expect("drive game with keyboard");
        }
    }
    client
        .wait_for_text("Success — beacon secured.", 10_000)
        .expect("native keyboard run completes");

    let viewport = semantic_node(&client, "scene2d-qualification.viewport");
    let screenshot = screenshot_path();
    client
        .screenshot(screenshot.to_str().expect("UTF-8 screenshot path"))
        .expect("capture completed native game");
    assert_varied_pixels(&screenshot, &viewport);
    child.finish(&client);
}
