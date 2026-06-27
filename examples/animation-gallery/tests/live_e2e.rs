use fission_test_driver::LiveTestClient;
use image::GenericImageView;
use std::net::TcpListener;
use std::process::{Child, Command};

fn reserve_control_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral test port")
        .local_addr()
        .expect("read ephemeral test port")
        .port()
}

fn launch_gallery(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_animation-gallery")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_animation_gallery"))
        .unwrap_or_else(|_| "target/debug/animation-gallery".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("failed to launch animation-gallery")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/animation-gallery/animation_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn assert_png_dimensions(path: &str, expected_width: u32, expected_height: u32) {
    let img = image::open(path).expect("open screenshot");
    let (width, height) = img.dimensions();
    assert_eq!(
        (width, height),
        (expected_width, expected_height),
        "unexpected screenshot dimensions for {path}"
    );
}

fn count_non_background_pixels(path: &str, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    let img = image::open(path).expect("open screenshot").to_rgba8();
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = img.get_pixel(x, y).0;
            if px[0] < 245 || px[1] < 245 || px[2] < 245 {
                count += 1;
            }
        }
    }
    count
}

fn assert_child_still_running(child: &mut Child, context: &str) {
    assert!(
        child.try_wait().expect("check child status").is_none(),
        "animation gallery process exited while exercising {context}"
    );
}

fn screenshot_slug(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn capture_example(client: &LiveTestClient, dir: &str, prefix: &str, label: &str) {
    let path = format!("{}/{}_{}.png", dir, prefix, screenshot_slug(label));
    client
        .screenshot(&path)
        .expect("capture example screenshot");
    let img = image::open(&path).expect("open example screenshot");
    let (width, height) = img.dimensions();
    assert!(
        width >= 640 && height >= 480,
        "example screenshot should capture a real viewport; got {width}x{height} for {path}"
    );
}

fn route_to_example(client: &LiveTestClient, label: &str, expected_text: &str) {
    let mut last_error = None;
    for _ in 0..6 {
        if let Err(error) = client.tap_text(label) {
            last_error = Some(error);
        } else {
            client.wait(250).expect("wait for route");
            client
                .scroll(900.0, 520.0, 0.0, -1_000.0)
                .expect("scroll route content to top");
            client.pump().expect("pump after route content scroll");
            client.wait(120).expect("wait after route content scroll");
            if client.assert_text_visible(expected_text).is_ok() {
                return;
            }
        }
        client
            .scroll(145.0, 650.0, 0.0, 260.0)
            .expect("scroll route list");
        client.wait(120).expect("wait after route-list scroll");
    }
    panic!(
        "open route {label}: {}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "route was not opened".into())
    );
}

fn use_workbench_viewport(client: &LiveTestClient) {
    client
        .simulate_resize(1280, 900)
        .expect("resize gallery for live example coverage");
    client.pump().expect("pump after live coverage resize");
    client.wait(250).expect("wait after live coverage resize");
}

fn exercise_widget_example(client: &LiveTestClient, child: &mut Child, dir: &str, label: &str) {
    route_to_example(client, label, "Controls");
    client.tap_text("Play").expect("play widget example");
    client.wait(350).expect("wait for widget motion");

    match label {
        "Modal" => {
            client.tap(32.0, 32.0).expect("tap modal backdrop");
        }
        "Drawer" => {
            client.tap(32.0, 32.0).expect("tap drawer backdrop");
        }
        "Popover" => {
            client.tap(32.0, 32.0).expect("tap popover backdrop");
        }
        "Tooltip" => {
            client.tap_text("Reset").expect("reset tooltip");
        }
        "Toast" => {
            client.tap_text("Reset").expect("reset toast");
        }
        "Accordion" => {
            client.assert_text_visible("Motion details").unwrap();
            client.assert_text_visible("Panel height, opacity").unwrap();
            client.tap_text("Reset").expect("reset accordion");
        }
        "Tabs" => {
            client.tap_text("IR").expect("select IR tab");
            client.wait(150).expect("wait for tab transition");
            client.assert_text_visible("Lowered MotionExpr").unwrap();
            client.tap_text("Reset").expect("reset tabs");
        }
        "Button" => {
            client.tap_text("Send").expect("press button demo");
            client.tap_text("Reset").expect("reset button");
        }
        "Checkbox" => {
            client.assert_text_visible("Accept motion terms").unwrap();
            client.tap_text("Reset").expect("reset checkbox");
        }
        "Switch" | "Sidebar" | "Carousel" => {
            client.tap_text("Reset").expect("reset widget");
        }
        _ => unreachable!("unlisted widget example {label}"),
    }

    client.pump().expect("pump after widget interaction");
    client.wait(200).expect("wait after widget close");
    capture_example(client, dir, "widget", label);
    assert_child_still_running(child, label);
}

#[test]
#[ignore]
fn animation_gallery_all_widget_examples_are_live_and_dismissable() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");
    use_workbench_viewport(&client);

    let dir = screenshot_dir();
    for label in [
        "Modal",
        "Drawer",
        "Popover",
        "Tooltip",
        "Toast",
        "Accordion",
        "Tabs",
        "Button",
        "Checkbox",
        "Switch",
        "Sidebar",
        "Carousel",
    ] {
        exercise_widget_example(&client, &mut child, &dir, label);
    }

    let tree = client.get_tree().expect("get semantics tree");
    assert!(
        tree.len() >= 10,
        "gallery should expose a useful semantics tree after widget exercises"
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_widget_composer_edits_current_widget_without_duplicate_dispatch() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");
    use_workbench_viewport(&client);

    let dir = screenshot_dir();
    route_to_example(&client, "Modal", "Controls");
    client.tap_text("Compose...").expect("open widget composer");
    client.wait(250).expect("wait for composer");
    client.assert_text_visible("Compose Modal").unwrap();
    client.assert_text_visible("3 atoms").unwrap();

    client.tap_text("FromLeft").expect("add one modal atom");
    client.wait(200).expect("wait after add atom");
    client.assert_text_visible("4 atoms").unwrap();
    client.assert_text_visible("4: FromLeft").unwrap();
    capture_example(&client, &dir, "composer", "modal_after_one_atom");

    client.tap_text("Undo Last").expect("undo one atom");
    client.wait(200).expect("wait after undo atom");
    client.assert_text_visible("3 atoms").unwrap();
    client
        .assert_text_not_visible("4: FromLeft")
        .expect("one add should be undone by one undo");

    client.tap_text("Done").expect("close composer");
    client.wait(200).expect("wait after close composer");
    client
        .tap_text("Play")
        .expect("play widget after composer close");
    client.wait(250).expect("wait for modal");
    client.assert_text_visible("Archive thread").unwrap();

    assert_child_still_running(&mut child, "widget composer");
    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_all_property_examples_are_live() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");
    use_workbench_viewport(&client);

    let dir = screenshot_dir();
    for label in [
        "Opacity",
        "Translate X/Y",
        "Scale",
        "Rotation",
        "Width / Height",
        "Background Color",
        "Border Color",
        "Corner Radius",
        "Clip / Reveal",
    ] {
        route_to_example(&client, label, "Notes");
        client.tap_text("150").expect("scrub property to midpoint");
        client.wait(150).expect("wait for property scrub");
        capture_example(&client, &dir, "property", label);
        assert_child_still_running(&mut child, label);
    }

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_composition_policy_and_diagnostics_examples_are_live() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");
    use_workbench_viewport(&client);

    let dir = screenshot_dir();

    route_to_example(&client, "Live Composer", "Composition Builder");
    client.tap_text("Clear").expect("clear composition");
    client.tap_text("Add FromLeft").expect("add first atom");
    client.tap_text("Add Fade").expect("add fade atom");
    client.tap_text("Add Scale").expect("add scale atom");
    client
        .assert_text_visible("3")
        .expect("composition builder should show added atom count");
    client
        .scroll(900.0, 520.0, 0.0, 420.0)
        .expect("scroll to composition playback");
    client.wait(120).expect("wait after composition scroll");
    client.tap_text("Play").expect("play composed modal");
    client.wait(250).expect("wait for composed modal");
    client.tap(32.0, 32.0).expect("close composed modal");
    capture_example(&client, &dir, "composition", "live_composer");

    for label in [
        "Additive Modal Motion",
        "Conflicting Motion",
        "Ordered Last-Wins",
    ] {
        route_to_example(&client, label, "Conflict Rule");
        if label != "Additive Modal Motion" {
            client
                .assert_text_visible("surface.translate_y from bottom")
                .unwrap();
        }
        client
            .scroll(900.0, 520.0, 0.0, 420.0)
            .expect("scroll to composition playback");
        client.wait(120).expect("wait after composition scroll");
        client.tap_text("Play").expect("play composition route");
        client.wait(250).expect("wait for composition route");
        client.tap(32.0, 32.0).expect("close composition modal");
        capture_example(&client, &dir, "composition", label);
        assert_child_still_running(&mut child, label);
    }

    for label in ["Full Motion", "Reduced Motion", "Disabled Motion"] {
        route_to_example(&client, label, "Motion Policy");
        client.tap_text("Play").expect("play policy demo");
        client.wait(200).expect("wait for policy demo");
        client
            .assert_text_visible("Policy is evaluating")
            .or_else(|_| client.assert_text_visible("Instant final state"))
            .unwrap();
        capture_example(&client, &dir, "policy", label);
        client.tap_text("Reset").expect("reset policy demo");
    }

    for label in [
        "Lowered MotionDeclaration",
        "Lowered MotionExpr",
        "Timeline Values",
        "Test Harness Examples",
    ] {
        route_to_example(&client, label, "Diagnostics");
        client
            .assert_text_visible("LiveTest pattern")
            .or_else(|_| client.assert_text_visible("Deterministic test"))
            .or_else(|_| client.assert_text_visible("Timeline samples"))
            .or_else(|_| client.assert_text_visible("MotionExpr graph"))
            .unwrap();
        capture_example(&client, &dir, "diagnostics", label);
        assert_child_still_running(&mut child, label);
    }

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_live_transitions_and_resize() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1000).expect("initial wait");

    let dir = screenshot_dir();

    client
        .screenshot(&format!("{}/01_initial.png", dir))
        .expect("initial screenshot");
    client.assert_text_visible("Animation Gallery").unwrap();
    client.assert_text_visible("Widgets").unwrap();
    client.assert_text_visible("Properties").unwrap();
    client.assert_text_visible("Motion Policy").unwrap();

    client.tap_text("Modal").expect("open modal page");
    client.wait(200).expect("wait for modal page");
    client.tap_text("Play").expect("play modal motion");
    client.wait(400).expect("wait after play");
    client
        .screenshot(&format!("{}/02_scene_toggled.png", dir))
        .expect("scene toggled screenshot");

    client.tap_text("Confirm").expect("close modal preview");
    client.wait(600).expect("wait after close");
    client
        .screenshot(&format!("{}/03_closed_modal.png", dir))
        .expect("closed modal screenshot");

    client.simulate_resize(1280, 900).expect("simulate resize");
    client.pump().expect("pump after resize");
    client.wait(300).expect("wait after resize");
    let resized_path = format!("{}/04_resized.png", dir);
    client
        .screenshot(&resized_path)
        .expect("resized screenshot");
    assert_png_dimensions(&resized_path, 1280, 900);

    let tree = client.get_tree().expect("get_tree");
    println!("Animation gallery semantics nodes: {}", tree.len());
    assert!(tree.len() >= 2, "expected animation gallery semantics");

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_initial_cards_are_painted() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    let path = format!("{}/05_initial_cards.png", dir);
    client.screenshot(&path).expect("initial screenshot");

    let opacity_pixels = count_non_background_pixels(&path, 40, 250, 180, 360);
    assert!(
        opacity_pixels > 500,
        "overview card should have visible painted content at time zero; non-background pixels={}",
        opacity_pixels
    );

    let translate_pixels = count_non_background_pixels(&path, 400, 250, 700, 360);
    assert!(
        translate_pixels > 500,
        "second overview card should have visible painted content at time zero; non-background pixels={}",
        translate_pixels
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_paused_composed_motion_keeps_a_visible_frame() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    client.tap_text("Modal").expect("open modal page");
    client.wait(200).expect("wait for modal page");
    client.tap_text("Play").expect("play modal motion");
    client.wait(400).expect("wait after play");
    client.tap_text("Confirm").expect("close modal preview");
    client.wait(600).expect("wait after close");
    let path = format!("{}/06_closed_modal_visible.png", dir);
    client.screenshot(&path).expect("paused screenshot");
    let pulse_pixels = count_non_background_pixels(&path, 40, 250, 520, 360);
    assert!(
        pulse_pixels > 500,
        "modal preview should retain visible content instead of blanking the panel; non-background pixels={}",
        pulse_pixels
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn animation_gallery_button_route_does_not_stack_overflow() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    client.tap_text("Button").expect("open button page");
    client.wait(300).expect("wait for button route");
    client.wait(300).expect("wait after button route render");
    client
        .screenshot(&format!("{}/09_button_route.png", dir))
        .expect("button route screenshot");

    assert!(
        child.try_wait().expect("check child status").is_none(),
        "button route should not abort the gallery process"
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn resized_surface_does_not_fall_back_to_a_dark_clear_band() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    client.simulate_resize(1280, 900).expect("simulate resize");
    client.pump().expect("pump after resize");
    client.wait(300).expect("wait after resize");

    let path = format!("{}/07_resize_clear_band.png", dir);
    client.screenshot(&path).expect("resized screenshot");

    let img = image::open(&path).expect("open screenshot").to_rgba8();
    let px = img.get_pixel(100, 850).0;
    assert!(
        px[0] > 230 && px[1] > 230 && px[2] > 230,
        "resized light-theme surface should not expose a dark compositor clear band; sampled pixel was {:?}",
        px
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn wide_resize_uses_the_available_horizontal_space() {
    let control_port = reserve_control_port();
    let mut child = launch_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("gallery did not start");
    client.wait(1_000).expect("initial wait");

    let dir = screenshot_dir();
    client.simulate_resize(1280, 900).expect("simulate resize");
    client.pump().expect("pump after resize");
    client.wait(300).expect("wait after resize");

    let path = format!("{}/08_wide_space_usage.png", dir);
    client.screenshot(&path).expect("wide resize screenshot");

    let right_half_pixels = count_non_background_pixels(&path, 860, 170, 1230, 520);
    assert!(
        right_half_pixels > 2_500,
        "wide animation gallery workbench should use the available horizontal space; right-half non-background pixels={}",
        right_half_pixels
    );

    client.quit().expect("quit");
    let _ = child.wait();
}
