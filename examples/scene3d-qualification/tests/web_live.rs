use std::collections::HashSet;
use std::path::{Path, PathBuf};

use fission::game::{Game, GameTime};
use fission::scene3d::{RenderCapabilities3D, Scene3DProcessor, Vec2};
use fission_test_driver::{BrowserTestOptions, LiveTestClient, SemanticNode, TestPointerKind};
use scene3d_qualification::{HarborGame, BEACON_NODE};

fn browser() -> LiveTestClient {
    let url = std::env::var("FISSION_SCENE3D_WEB_URL")
        .expect("set FISSION_SCENE3D_WEB_URL to the running qualification app");
    LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())
        .expect("launch qualification browser")
}

fn screenshot_path(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.artifacts/screenshots/examples/scene3d-qualification");
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
    assert!(visible > 10_000, "3D viewport did not render enough pixels");
    assert!(
        buckets.len() > 8,
        "3D viewport is blank or a compatibility surface: {buckets:?}"
    );
}

fn beacon_viewport_point() -> Vec2 {
    let scene = HarborGame::new().present(GameTime::default());
    let prepared = Scene3DProcessor::new().prepare(&scene, RenderCapabilities3D::default());
    (0..54)
        .flat_map(|row| (0..96).map(move |column| (column, row)))
        .find_map(|(column, row)| {
            let point = Vec2::new(column as f32 * 10.0 + 5.0, row as f32 * 10.0 + 5.0);
            prepared
                .pick_viewport(point)
                .is_some_and(|hit| hit.node == BEACON_NODE)
                .then_some(point)
        })
        .expect("beacon is visible in the initial viewport")
}

#[test]
#[ignore = "requires a running Web qualification app and Chrome WebGPU"]
fn touch_picking_and_keyboard_complete_the_real_web_game() {
    let client = browser();
    client
        .wait_for_text("Objective: select the beacon", 30_000)
        .expect("game becomes ready");
    let viewport = semantic_node(&client, "scene3d:3001");
    let point = beacon_viewport_point();
    let x = viewport.logical_bounds.x + point.x;
    let y = viewport.logical_bounds.y + point.y;
    client
        .pointer_down(1, TestPointerKind::Touch, x, y, 0)
        .expect("touch beacon");
    client
        .pointer_up(1, TestPointerKind::Touch, x, y, 0)
        .expect("release beacon");
    client.pump().expect("pump beacon selection");
    client
        .wait_for_text("Beacon selected", 10_000)
        .expect("coordinate picking selects the beacon");

    for (key, count) in [("ArrowRight", 5), ("ArrowUp", 18), ("ArrowLeft", 5)] {
        for _ in 0..count {
            client.press_key(key, 0).expect("drive game with keyboard");
        }
    }
    client
        .wait_for_text("Beacon reached — qualification complete", 10_000)
        .expect("keyboard run completes");

    let viewport = semantic_node(&client, "scene3d:3001");
    let screenshot = screenshot_path("complete-touch-keyboard-run");
    client
        .screenshot(screenshot.to_str().unwrap())
        .expect("capture completed game");
    assert_varied_pixels(&screenshot, &viewport);
}
