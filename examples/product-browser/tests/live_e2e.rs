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

fn launch_product_browser(control_port: u16) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_product-browser")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_product_browser"))
        .unwrap_or_else(|_| "target/debug/product-browser".to_string());
    Command::new(bin)
        .env("FISSION_TEST_CONTROL_PORT", control_port.to_string())
        .env("FISSION_BACKGROUND_TEST", "1")
        .spawn()
        .expect("failed to launch product-browser")
}

fn screenshot_dir() -> String {
    let dir = std::env::var("FISSION_SCREENSHOT_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/screenshots/examples/product-browser/live_e2e",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    std::fs::create_dir_all(&dir).expect("create screenshot dir");
    dir
}

fn assert_region_has_visible_image_pixels(path: &str, region: (u32, u32, u32, u32)) {
    let img = image::open(path).expect("open screenshot").to_rgba8();
    let (x, y, width, height) = region;
    assert!(
        x + width <= img.width() && y + height <= img.height(),
        "region {region:?} outside screenshot {}x{}",
        img.width(),
        img.height()
    );

    let mut visible_pixels = 0usize;
    for py in y..(y + height) {
        for px in x..(x + width) {
            let pixel = img.get_pixel(px, py).0;
            let [r, g, b, a] = pixel;
            if a > 0 && (r < 245 || g < 245 || b < 245) {
                visible_pixels += 1;
            }
        }
    }

    assert!(
        visible_pixels > 500,
        "expected visible non-white image pixels in {region:?}, found {visible_pixels}; screenshot={path}"
    );
}

#[test]
#[ignore]
fn product_browser_displays_remote_product_images() {
    let control_port = reserve_control_port();
    let mut child = launch_product_browser(control_port);
    let client = LiveTestClient::connect(control_port);
    client
        .wait_for_ready(15_000)
        .expect("product-browser did not start");
    client
        .simulate_resize(1200, 800)
        .expect("resize product-browser");
    client.wait(8_000).expect("wait for products and images");
    client.pump().expect("pump after async image loads");
    client
        .assert_text_visible("30 shown")
        .expect("product data should load before image assertion");
    client
        .assert_text_visible("Essence Mascara Lash Princess")
        .expect("first product should be visible");

    let path = format!("{}/product_browser_remote_images.png", screenshot_dir());
    client
        .screenshot(&path)
        .expect("capture product screenshot");

    assert_region_has_visible_image_pixels(&path, (280, 128, 220, 160));
    assert_region_has_visible_image_pixels(&path, (556, 128, 220, 160));
    assert_region_has_visible_image_pixels(&path, (856, 130, 280, 220));

    client.quit().expect("quit");
    let _ = child.wait();
}
