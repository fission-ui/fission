use anyhow::{ensure, Context, Result};
use fission_test_driver::{Bounds, BrowserTestOptions, LiveTestClient, SelectorQuery};
use image::RgbaImage;
use serde_json::Value;

const BACKGROUND: [u8; 3] = [17, 29, 44];
const PATH_FILL: [u8; 3] = [0, 180, 216];
const PATH_STROKE: [u8; 3] = [247, 251, 255];
const CHANNEL_TOLERANCE: u8 = 12;

fn browser_url() -> Option<String> {
    std::env::var("FISSION_WEB_SMOKE_URL").ok()
}

fn color_is_near(pixel: [u8; 4], expected: [u8; 3]) -> bool {
    pixel[3] >= 245
        && pixel[..3]
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= CHANNEL_TOLERANCE)
}

fn pixel_at(image: &RgbaImage, x: f32, y: f32) -> Result<[u8; 4]> {
    ensure!(
        x.is_finite() && y.is_finite(),
        "pixel coordinate must be finite"
    );
    ensure!(x >= 0.0 && y >= 0.0, "pixel coordinate must be nonnegative");
    let x = x.floor() as u32;
    let y = y.floor() as u32;
    ensure!(
        x < image.width() && y < image.height(),
        "pixel coordinate ({x}, {y}) lies outside {}x{} capture",
        image.width(),
        image.height()
    );
    Ok(image.get_pixel(x, y).0)
}

fn assert_color_at(
    image: &RgbaImage,
    x: f32,
    y: f32,
    expected: [u8; 3],
    description: &str,
) -> Result<()> {
    let actual = pixel_at(image, x, y)?;
    ensure!(
        color_is_near(actual, expected),
        "{description} at ({x:.1}, {y:.1}) was {actual:?}, expected RGB {expected:?} within {CHANNEL_TOLERANCE}"
    );
    Ok(())
}

fn pixels_near(image: &RgbaImage, bounds: Bounds, expected: [u8; 3]) -> usize {
    let left = bounds.x.max(0.0).floor() as u32;
    let top = bounds.y.max(0.0).floor() as u32;
    let right = (bounds.x + bounds.width)
        .ceil()
        .max(0.0)
        .min(image.width() as f32) as u32;
    let bottom = (bounds.y + bounds.height)
        .ceil()
        .max(0.0)
        .min(image.height() as f32) as u32;

    (top..bottom)
        .flat_map(|y| (left..right).map(move |x| image.get_pixel(x, y).0))
        .filter(|pixel| color_is_near(*pixel, expected))
        .count()
}

fn assert_renderer_selection(client: &LiveTestClient) -> Result<String> {
    let report = client
        .browser_report()
        .context("launched browser omitted its renderer report")?;
    let renderer = report
        .renderer
        .context("browser renderer report omitted the active renderer")?;
    let info = client.browser_evaluate_json("globalThis.__FISSION_RENDERER_INFO")?;
    let requested = info
        .get("requested")
        .and_then(Value::as_str)
        .context("renderer diagnostics omitted the requested renderer")?;
    let active = info
        .get("active")
        .and_then(Value::as_str)
        .context("renderer diagnostics omitted the active renderer")?;
    ensure!(active == renderer, "renderer reports disagree: {info}");

    match requested {
        "canvas2d-software" => ensure!(
            active == "canvas2d-software",
            "forced software request selected {active}: {info}"
        ),
        "webgpu-vello" => ensure!(
            active == "webgpu-vello",
            "explicit WebGPU request selected {active}: {info}"
        ),
        "auto" if active == "webgpu-vello" => {}
        "auto" if active == "canvas2d-software" => {
            let fallback = info
                .get("fallback_reason")
                .and_then(Value::as_str)
                .context("automatic software selection omitted its WebGPU fallback reason")?;
            ensure!(
                fallback.starts_with("webgpu_vello_init_failed:"),
                "automatic software selection reported an unexpected fallback: {info}"
            );
        }
        _ => ensure!(false, "unexpected renderer selection: {info}"),
    }

    Ok(renderer)
}

#[test]
fn scene_path_composites_in_selected_web_renderer() -> Result<()> {
    let Some(url) = browser_url() else {
        eprintln!("set FISSION_WEB_SMOKE_URL to run the Scene2D path browser test");
        return Ok(());
    };
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())?;
    client.pause_animations()?;
    client.pump()?;
    let renderer = assert_renderer_selection(&client)?;

    let path =
        client.resolve_selector(SelectorQuery::semantic_identifier("web-smoke.scene-path"))?;
    let bounds = path
        .visible_bounds
        .context("Scene2D path sample is outside the browser viewport")?;
    ensure!(
        (bounds.width - 160.0).abs() <= 0.5 && (bounds.height - 96.0).abs() <= 0.5,
        "Scene2D path bounds changed unexpectedly under {renderer}: {bounds:?}"
    );

    let png = client.capture_screenshot_png()?;
    let image = image::load_from_memory(&png)
        .context("decode Scene2D path browser capture")?
        .to_rgba8();

    assert_color_at(
        &image,
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.86,
        PATH_FILL,
        "deep path fill",
    )?;
    assert_color_at(
        &image,
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.08,
        BACKGROUND,
        "unfilled area above the curve",
    )?;
    assert_color_at(
        &image,
        bounds.x - 6.0,
        bounds.y + bounds.height * 0.86,
        BACKGROUND,
        "overflowing path geometry outside its hard clip",
    )?;

    let fill_pixels = pixels_near(&image, bounds, PATH_FILL);
    let stroke_pixels = pixels_near(&image, bounds, PATH_STROKE);
    let background_pixels = pixels_near(&image, bounds, BACKGROUND);
    ensure!(
        fill_pixels > 2_000,
        "{renderer} did not composite the broad Scene2D path fill: fill={fill_pixels}, stroke={stroke_pixels}, background={background_pixels}"
    );
    ensure!(
        stroke_pixels > 80,
        "{renderer} did not composite the Scene2D path stroke: fill={fill_pixels}, stroke={stroke_pixels}, background={background_pixels}"
    );
    ensure!(
        background_pixels > 500,
        "{renderer} painted the path as its whole rectangular bound: fill={fill_pixels}, stroke={stroke_pixels}, background={background_pixels}"
    );
    Ok(())
}
