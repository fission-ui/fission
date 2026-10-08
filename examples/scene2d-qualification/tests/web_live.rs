use std::collections::HashSet;
use std::path::{Path, PathBuf};

use fission_test_driver::{
    BrowserTestOptions, LiveTestClient, SelectorQuery, SemanticNode, TestPointerKind,
};

fn browser() -> LiveTestClient {
    let url = std::env::var("FISSION_SCENE2D_WEB_URL")
        .expect("set FISSION_SCENE2D_WEB_URL to the running qualification app");
    LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())
        .expect("launch qualification browser")
}

fn screenshot_path(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.artifacts/screenshots/examples/scene2d-qualification");
    std::fs::create_dir_all(&directory).expect("create screenshot directory");
    directory.join(format!("{name}.png"))
}

fn semantic_node(client: &LiveTestClient, identifier: &str) -> SemanticNode {
    client
        .get_tree()
        .expect("read semantic tree")
        .into_iter()
        .find(|node| node.identifier.as_deref() == Some(identifier))
        .unwrap_or_else(|| panic!("missing semantic node {identifier}"))
}

fn touch(client: &LiveTestClient, node: &SemanticNode, pointer_id: u64) {
    let bounds = node.visible_bounds.unwrap_or(node.logical_bounds);
    let x = bounds.x + bounds.width * 0.5;
    let y = bounds.y + bounds.height * 0.5;
    client
        .pointer_down(pointer_id, TestPointerKind::Touch, x, y, 0)
        .expect("touch down");
    client
        .pointer_up(pointer_id, TestPointerKind::Touch, x, y, 0)
        .expect("touch up");
    client.pump().expect("pump touch result");
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
    assert!(
        visible > 10_000,
        "scene viewport did not render enough pixels"
    );
    assert!(
        buckets.len() > 8,
        "scene viewport is blank or a compatibility surface: {buckets:?}"
    );
}

#[test]
#[ignore = "requires a running Web qualification app and Chrome"]
fn touch_completes_the_real_web_game_and_renders_scene_pixels() {
    let client = browser();
    client
        .wait_for_text("Guide the scout", 30_000)
        .expect("game becomes ready");
    let up = semantic_node(&client, "scene2d:1:node:20");
    let right = semantic_node(&client, "scene2d:1:node:23");
    let down = semantic_node(&client, "scene2d:1:node:21");
    let mut pointer_id = 1;
    for (node, count) in [(&up, 8), (&right, 40), (&down, 8)] {
        for _ in 0..count {
            touch(&client, node, pointer_id);
            pointer_id += 1;
        }
    }
    client
        .wait_for_text("Success — beacon secured.", 10_000)
        .expect("touch run completes");

    let viewport = semantic_node(&client, "scene2d-qualification.viewport");
    let screenshot = screenshot_path("complete-touch-run");
    client
        .screenshot(screenshot.to_str().unwrap())
        .expect("capture completed game");
    assert_varied_pixels(&screenshot, &viewport);
}

#[test]
#[ignore = "requires a running Web qualification app and Chrome"]
fn keyboard_completes_the_real_web_game_and_renders_scene_pixels() {
    let client = browser();
    client
        .wait_for_text("Guide the scout", 30_000)
        .expect("game becomes ready");

    let viewport = semantic_node(&client, "scene2d-qualification.viewport");
    let initial = screenshot_path("initial-keyboard-run");
    client
        .screenshot(initial.to_str().unwrap())
        .expect("capture initial game");
    assert_varied_pixels(&initial, &viewport);
    client
        .focus_selector(SelectorQuery::semantic_identifier("beacon-run.game-input"))
        .expect("focus game input region");

    for (key, count) in [("Up", 8), ("Right", 40), ("Down", 8)] {
        for _ in 0..count {
            client.press_key(key, 0).expect("drive game with keyboard");
        }
    }
    client
        .wait_for_text("Success — beacon secured.", 10_000)
        .expect("keyboard run completes");

    let screenshot = screenshot_path("complete-keyboard-run");
    client
        .screenshot(screenshot.to_str().unwrap())
        .expect("capture completed game");
    assert_varied_pixels(&screenshot, &viewport);
}
