use super::*;
use image::ImageEncoder;

fn png(width: u32, height: u32) -> String {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &vec![128; width as usize * height as usize * 4],
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn scaled_options() -> BrowserTestOptions {
    let mut options = BrowserTestOptions::new("https://example.com").fission_canvas();
    options.viewport_width = 3;
    options.viewport_height = 4;
    options.device_scale_factor = 2.0;
    options
}

#[test]
fn renderer_png_must_use_physical_dimensions_at_device_scale_two() {
    assert!(validate_renderer_screenshot(&png(6, 8), 6, 8, &scaled_options()).is_ok());
    let error = validate_renderer_screenshot(&png(3, 4), 3, 4, &scaled_options()).unwrap_err();
    assert!(error
        .to_string()
        .contains("renderer declared 3x4, expected 6x8"));
}

#[test]
fn renderer_png_decoded_dimensions_must_match_its_declared_physical_dimensions() {
    let error = validate_renderer_screenshot(&png(3, 4), 6, 8, &scaled_options()).unwrap_err();
    assert!(error
        .to_string()
        .contains("decoded renderer PNG is 3x4, expected 6x8"));
}

#[test]
fn renderer_png_with_correct_declared_dimensions_still_must_decode() {
    let data = base64::engine::general_purpose::STANDARD.encode(b"not a PNG");
    let error = validate_renderer_screenshot(&data, 6, 8, &scaled_options()).unwrap_err();
    assert!(error.to_string().contains("invalid renderer PNG"));
}

#[test]
fn only_the_explicit_canvas2d_capture_request_uses_browser_host_capture() {
    assert!(renderer_capture_needs_browser_host(
        "renderer_screenshot_unavailable: use browser-host capture with the Canvas2D renderer"
    ));
    for message in [
        "renderer_screenshot_unavailable: WebGPU readback timed out after 5000ms; no replacement image was generated",
        "renderer_screenshot_unavailable: WebGPU buffer map failed for 780x1800 readback",
        "renderer_screenshot_unavailable: WebGPU screenshot queue completion failed",
        "renderer_screenshot_unavailable: unknown renderer failure",
        "unrelated command failure",
    ] {
        assert!(!renderer_capture_needs_browser_host(message), "must propagate: {message}");
    }
}
