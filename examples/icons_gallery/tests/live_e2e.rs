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

fn launch_icons_gallery(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_icons_gallery")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_icons-gallery"))
        .unwrap_or_else(|_| "target/debug/icons_gallery".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("failed to launch icons_gallery")
}

#[test]
#[ignore]
fn scrolling_changes_the_visible_window() {
    let control_port = reserve_control_port();
    let mut child = launch_icons_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("icons gallery did not start");
    client.wait(1_500).expect("initial wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/icons_gallery/icons_gallery_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&screenshot_dir).ok();
    let before_path = format!("{}/01_before_scroll.png", screenshot_dir);
    let after_path = format!("{}/02_after_scroll.png", screenshot_dir);

    client
        .screenshot(&before_path)
        .expect("screenshot before scroll");

    for _ in 0..4 {
        client.scroll(400.0, 300.0, 0.0, 180.0).expect("scroll");
        client.pump().expect("pump after scroll");
        client.wait(200).expect("wait after scroll");
    }

    client
        .screenshot(&after_path)
        .expect("screenshot after scroll");

    let before = std::fs::read(&before_path).expect("read before screenshot");
    let after = std::fs::read(&after_path).expect("read after screenshot");
    assert_ne!(
        before, after,
        "scrolling should change the rendered screenshot"
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

fn count_dark_pixels(path: &str, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    let img = image::open(path).expect("open screenshot").to_rgba8();
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = img.get_pixel(x, y).0;
            if px[0] < 235 || px[1] < 235 || px[2] < 235 {
                count += 1;
            }
        }
    }
    count
}

fn first_visible_icon_label(client: &LiveTestClient) -> String {
    client
        .get_text()
        .expect("get_text")
        .into_iter()
        .filter(|item| item.text.contains('/'))
        .map(|item| item.text)
        .next()
        .expect("at least one visible icon label")
}

#[test]
#[ignore]
fn large_scroll_does_not_blank_the_visible_list() {
    let control_port = reserve_control_port();
    let mut child = launch_icons_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("icons gallery did not start");
    client.wait(1_500).expect("initial wait");

    let screenshot_dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/icons_gallery/icons_gallery_live",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&screenshot_dir).ok();
    let after_path = format!("{}/03_after_large_scroll.png", screenshot_dir);

    for _ in 0..3 {
        client.scroll(400.0, 300.0, 0.0, 400.0).expect("scroll");
        client.pump().expect("pump after scroll");
        client.wait(200).expect("wait after scroll");
    }

    client
        .screenshot(&after_path)
        .expect("screenshot after large scroll");

    let dark_pixels = count_dark_pixels(&after_path, 20, 130, 780, 580);
    assert!(
        dark_pixels > 3_000,
        "list viewport should still contain painted rows after large scroll; dark pixel count was {}",
        dark_pixels
    );

    client.quit().expect("quit");
    let _ = child.wait();
}

#[test]
#[ignore]
fn large_scroll_changes_the_first_visible_icon_label() {
    let control_port = reserve_control_port();
    let mut child = launch_icons_gallery(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("icons gallery did not start");
    client.wait(1_500).expect("initial wait");

    let before = first_visible_icon_label(&client);
    for _ in 0..4 {
        client.scroll(400.0, 300.0, 0.0, 400.0).expect("scroll");
        client.pump().expect("pump after scroll");
        client.wait(200).expect("wait after scroll");
    }
    let after = first_visible_icon_label(&client);
    assert_ne!(
        before, after,
        "large scrolling should change the first visible icon row"
    );

    client.quit().expect("quit");
    let _ = child.wait();
}
