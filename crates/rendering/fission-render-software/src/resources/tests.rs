use super::*;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Cursor;
#[cfg(not(target_arch = "wasm32"))]
use std::net::TcpListener;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

use fission_render::{DisplayList, DisplayOp, ImageFit, RenderScene, TextRun};

use crate::SoftwareRenderer;

struct SingleLineMeasurer;

impl TextMeasurer for SingleLineMeasurer {
    fn measure(&self, text: &str, font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (text.len() as f32 * font_size, font_size * 1.2)
    }

    fn get_line_metrics(
        &self,
        text: &str,
        font_size: f32,
        _available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![LineMetric {
            start_index: 0,
            end_index: text.len(),
            baseline: font_size,
            height: font_size * 1.2,
            width: text.len() as f32 * font_size,
        }]
    }
}

#[test]
fn normalized_gradient_point_maps_to_painted_bounds() {
    let point = normalized_fill_point(
        fission_render::LayoutRect::new(20.0, 40.0, 200.0, 100.0),
        (0.25, 0.75),
    );
    assert_eq!(point, Point::from_xy(70.0, 115.0));
}

#[test]
fn software_text_does_not_rewrap_pipeline_single_line_layouts() {
    let bounds = fission_render::LayoutRect::new(0.0, 0.0, 24.0, 24.0);
    let mut display_list = DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, 260.0, 80.0));
    display_list.push(DisplayOp::DrawText {
        text: "Secure local storage required".into(),
        position: bounds.origin,
        size: 20.0,
        color: RenderColor {
            r: 20,
            g: 40,
            b: 60,
            a: 255,
        },
        bounds,
        node_id: None,
        underline: false,
        wrap: true,
        caret_index: None,
        caret_color: None,
        caret_width: None,
        caret_height: None,
        caret_radius: None,
        paragraph_style: None,
    });

    let pixels = SoftwareRenderer::render_with_text_measurer(
        &RenderScene::from_display_list(display_list),
        260,
        80,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
        Arc::new(SingleLineMeasurer),
    )
    .expect("render pipeline-shaped single line");
    let row_has_ink = |y: usize| {
        pixels[y * 260 * 4..(y + 1) * 260 * 4]
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0)
    };

    assert!(
        (0..24).any(|y| row_has_ink(y)),
        "the first line should be painted"
    );
    assert!(
        !(30..80).any(|y| row_has_ink(y)),
        "fontdue must not add lines below the pipeline's one-line bounds"
    );
}

#[test]
fn software_rich_text_does_not_rewrap_pipeline_single_line_layouts() {
    let bounds = fission_render::LayoutRect::new(0.0, 0.0, 24.0, 20.0);
    let mut display_list = DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, 220.0, 70.0));
    display_list.push(DisplayOp::DrawRichText {
        runs: vec![TextRun {
            text: "Enable secure storage".into(),
            style: fission_render::TextStyle {
                font_size: 14.0,
                color: RenderColor {
                    r: 20,
                    g: 40,
                    b: 60,
                    a: 255,
                },
                underline: false,
                font_family: None,
                locale: None,
                font_weight: 600,
                font_style: fission_ir::op::FontStyle::Normal,
                line_height: None,
                letter_spacing: 0.0,
                background_color: None,
            },
        }],
        position: bounds.origin,
        bounds,
        node_id: None,
        wrap: true,
        caret_index: None,
        caret_color: None,
        caret_width: None,
        caret_height: None,
        caret_radius: None,
        paragraph_style: None,
        annotations: Vec::new(),
    });

    let pixels = SoftwareRenderer::render_with_text_measurer(
        &RenderScene::from_display_list(display_list),
        220,
        70,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
        Arc::new(SingleLineMeasurer),
    )
    .expect("render pipeline-shaped rich-text line");
    let row_has_ink = |y: usize| {
        pixels[y * 220 * 4..(y + 1) * 220 * 4]
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0)
    };

    assert!((0..20).any(|y| row_has_ink(y)));
    assert!(
        !(26..70).any(|y| row_has_ink(y)),
        "fontdue must not rewrap a pipeline-shaped rich-text label"
    );
}

#[test]
fn software_rich_text_paints_requested_run_backgrounds() {
    let bounds = fission_render::LayoutRect::new(2.0, 2.0, 40.0, 24.0);
    let mut display_list = DisplayList::new(bounds);
    display_list.push(DisplayOp::DrawRichText {
        runs: vec![TextRun {
            text: "A".into(),
            style: fission_render::TextStyle {
                font_size: 18.0,
                color: RenderColor {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                },
                underline: false,
                font_family: None,
                locale: None,
                font_weight: 400,
                font_style: fission_ir::op::FontStyle::Normal,
                line_height: None,
                letter_spacing: 0.0,
                background_color: Some(RenderColor {
                    r: 230,
                    g: 20,
                    b: 30,
                    a: 255,
                }),
            },
        }],
        position: bounds.origin,
        bounds,
        node_id: None,
        wrap: false,
        caret_index: None,
        caret_color: None,
        caret_width: None,
        caret_height: None,
        caret_radius: None,
        paragraph_style: None,
        annotations: Vec::new(),
    });

    let pixels = SoftwareRenderer::render(
        &RenderScene::from_display_list(display_list),
        48,
        32,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
    )
    .expect("render rich-text background");

    assert!(pixels
        .chunks_exact(4)
        .any(|pixel| { pixel[0] > 200 && pixel[1] < 50 && pixel[2] < 50 && pixel[3] > 200 }));
}

#[test]
fn scaled_svg_keeps_its_display_list_origin() {
    let bounds = fission_render::LayoutRect::new(10.0, 12.0, 20.0, 20.0);
    let mut display_list = DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, 64.0, 64.0));
    display_list.push(DisplayOp::Save);
    display_list.push(DisplayOp::Translate(fission_render::LayoutPoint::new(
        5.0, 7.0,
    )));
    display_list.push(DisplayOp::DrawSvg {
        content: r#"<svg viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10"/></svg>"#
            .into(),
        fill: Some(Fill::Solid(RenderColor {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        })),
        stroke: None,
        bounds,
        node_id: None,
    });
    display_list.push(DisplayOp::Restore);

    let pixels = SoftwareRenderer::render(
        &RenderScene::from_display_list(display_list),
        64,
        64,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
    )
    .expect("render translated and scaled SVG");
    let pixel_at = |x: usize, y: usize| {
        let offset = (y * 64 + x) * 4;
        &pixels[offset..offset + 4]
    };

    assert_eq!(pixel_at(16, 20), &[255, 0, 0, 255]);
    assert_eq!(
        pixel_at(40, 45),
        &[0, 0, 0, 0],
        "the SVG origin must not be scaled a second time"
    );
}

#[test]
fn malformed_path_data_is_an_explicit_render_failure() {
    let bounds = fission_render::LayoutRect::new(0.0, 0.0, 20.0, 20.0);
    let mut display_list = DisplayList::new(bounds);
    display_list.push(DisplayOp::DrawPath {
        path: "not valid SVG path data".into(),
        fill: Some(Fill::Solid(RenderColor {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        })),
        stroke: None,
        bounds,
        node_id: None,
    });

    let error = SoftwareRenderer::render(
        &RenderScene::from_display_list(display_list),
        20,
        20,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("invalid software-renderer path data"));
}

#[test]
fn unsupported_svg_elements_are_an_explicit_render_failure() {
    let bounds = fission_render::LayoutRect::new(0.0, 0.0, 20.0, 20.0);
    let mut display_list = DisplayList::new(bounds);
    display_list.push(DisplayOp::DrawSvg {
        content: r#"<svg viewBox="0 0 20 20"><circle cx="10" cy="10" r="8"/></svg>"#.into(),
        fill: Some(Fill::Solid(RenderColor {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        })),
        stroke: None,
        bounds,
        node_id: None,
    });

    let error = SoftwareRenderer::render(
        &RenderScene::from_display_list(display_list),
        20,
        20,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("unsupported SVG element <circle>"));
}

#[cfg(not(target_arch = "wasm32"))]
fn tiny_png() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 128, 255, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode png");
    bytes.into_inner()
}

#[test]
fn decoded_images_are_converted_to_tiny_skia_premultiplied_rgba() {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1,
        1,
        image::Rgba([200, 100, 50, 128]),
    ));
    let pixmap = decode_dynamic_image(image, None, None).expect("decode one pixel");

    assert_eq!(pixmap.data(), &[100, 50, 25, 128]);
}

#[cfg(not(target_arch = "wasm32"))]
fn solid_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba(rgba));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode png");
    bytes.into_inner()
}

#[cfg(not(target_arch = "wasm32"))]
fn centered_mark_png(width: u32, height: u32) -> Vec<u8> {
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([8, 8, 12, 255]));
    let mark_x0 = width / 3;
    let mark_x1 = width - mark_x0;
    let mark_y0 = height / 3;
    let mark_y1 = height - mark_y0;
    for y in mark_y0..mark_y1 {
        for x in mark_x0..mark_x1 {
            image.put_pixel(x, y, image::Rgba([245, 245, 245, 255]));
        }
    }
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode centered mark png");
    bytes.into_inner()
}

#[cfg(not(target_arch = "wasm32"))]
fn serve_once(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test image server");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0_u8; 1024];
        let _ = std::io::Read::read(&mut stream, &mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        let _ = std::io::Write::write_all(&mut stream, &body);
        let _ = std::io::Write::flush(&mut stream);
    });
    url
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn memory_image_load_populates_cache_off_thread() {
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: tiny_png(),
            mime_type: Some("image/png".into()),
        },
        cache_width: Some(1),
        cache_height: Some(1),
        ..Default::default()
    };
    let key = request.stable_cache_key();
    image_cache().invalidate(&key);
    image_cache().run_pending_tasks();
    let before = image_cache_generation();

    spawn_image_load(key.clone(), request);

    let deadline = Instant::now() + Duration::from_secs(2);
    while image_cache_generation() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }

    let Some(ImageCacheEntry::Ready(image)) = image_cache().get(&key) else {
        panic!("expected decoded image in cache");
    };
    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 1);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn network_image_fetch_decodes_png_response() {
    let url = serve_once(tiny_png());
    let image = fetch_network_image(&url, Vec::new(), Some(1), Some(1))
        .expect("fetch and decode test image");

    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 1);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn cached_image_request_paints_visible_pixels() {
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: tiny_png(),
            mime_type: Some("image/png".into()),
        },
        cache_width: Some(1),
        cache_height: Some(1),
        ..Default::default()
    };
    let key = request.stable_cache_key();
    image_cache().invalidate(&key);
    image_cache().run_pending_tasks();
    let before = image_cache_generation();
    spawn_image_load(key.clone(), request.clone());

    let deadline = Instant::now() + Duration::from_secs(2);
    while image_cache_generation() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }

    let rect = fission_render::LayoutRect::new(0.0, 0.0, 4.0, 4.0);
    let mut display_list = DisplayList::new(rect);
    display_list.push(DisplayOp::DrawImage {
        rect,
        request,
        fit: ImageFit::Fill,
        alignment: ImageAlignment::Center,
        bounds: rect,
        node_id: None,
    });
    let scene = RenderScene::from_display_list(display_list);
    let transparent = RenderColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    let pixels = SoftwareRenderer::render(&scene, 4, 4, transparent, 1.0)
        .expect("render software image scene");

    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && (pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0)),
        "expected image draw to produce visible non-transparent pixels"
    );
}

#[test]
fn failed_image_state_is_an_explicit_render_failure() {
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: vec![0, 1, 2, 3],
            mime_type: Some("image/png".into()),
        },
        cache_width: Some(4),
        cache_height: Some(4),
        ..Default::default()
    };
    let key = request.stable_cache_key();
    image_cache().insert(key, ImageCacheEntry::Failed);
    let bounds = fission_render::LayoutRect::new(0.0, 0.0, 4.0, 4.0);
    let mut display_list = DisplayList::new(bounds);
    display_list.push(DisplayOp::DrawImage {
        rect: bounds,
        request,
        fit: ImageFit::Fill,
        alignment: ImageAlignment::Center,
        bounds,
        node_id: None,
    });

    let error = SoftwareRenderer::render(
        &RenderScene::from_display_list(display_list),
        4,
        4,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("could not load image resource"));
}

#[test]
fn high_dpi_render_uses_device_space_without_logical_upscale() {
    let bounds = fission_render::LayoutRect::new(0.0, 0.0, 10.0, 10.0);
    let rect = fission_render::LayoutRect::new(1.0, 1.0, 2.0, 2.0);
    let mut display_list = DisplayList::new(bounds);
    display_list.push(DisplayOp::DrawRect {
        rect,
        fill: Some(Fill::Solid(RenderColor {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        })),
        stroke: None,
        corner_radius: 0.0,
        shadow: None,
        bounds: rect,
        node_id: None,
    });
    let scene = RenderScene::from_display_list(display_list);
    let transparent = RenderColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    let pixels = SoftwareRenderer::render(&scene, 20, 20, transparent, 2.0)
        .expect("render high-DPI software scene");

    let pixel_at = |x: usize, y: usize| {
        let start = (y * 20 + x) * 4;
        &pixels[start..start + 4]
    };
    assert_eq!(pixel_at(0, 0), &[0, 0, 0, 0]);
    assert_eq!(pixel_at(3, 3), &[255, 0, 0, 255]);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn cover_image_draw_is_clipped_to_destination_rect() {
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: solid_png(4, 2, [255, 0, 0, 255]),
            mime_type: Some("image/png".into()),
        },
        cache_width: Some(4),
        cache_height: Some(2),
        ..Default::default()
    };
    let key = request.stable_cache_key();
    image_cache().invalidate(&key);
    image_cache().run_pending_tasks();
    let before = image_cache_generation();
    spawn_image_load(key.clone(), request.clone());

    let deadline = Instant::now() + Duration::from_secs(2);
    while image_cache_generation() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }

    let bounds = fission_render::LayoutRect::new(0.0, 0.0, 10.0, 10.0);
    let rect = fission_render::LayoutRect::new(4.0, 4.0, 2.0, 2.0);
    let mut display_list = DisplayList::new(bounds);
    display_list.push(DisplayOp::DrawImage {
        rect,
        request,
        fit: ImageFit::Cover,
        alignment: ImageAlignment::Center,
        bounds: rect,
        node_id: None,
    });
    let scene = RenderScene::from_display_list(display_list);
    let transparent = RenderColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    let pixels = SoftwareRenderer::render(&scene, 10, 10, transparent, 1.0)
        .expect("render clipped cover image");

    let pixel_at = |x: usize, y: usize| {
        let start = (y * 10 + x) * 4;
        &pixels[start..start + 4]
    };

    assert_eq!(pixel_at(3, 4), &[0, 0, 0, 0]);
    assert_eq!(pixel_at(6, 4), &[0, 0, 0, 0]);
    assert!(
        pixel_at(4, 4)[3] > 0 && pixel_at(4, 4)[0] > 0,
        "expected destination rect to contain image pixels"
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn scaled_memory_image_paints_center_pixels() {
    let rect = fission_render::LayoutRect::new(40.0, 161.55, 144.0, 144.0);
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: centered_mark_png(256, 256),
            mime_type: Some("image/png".into()),
        },
        ..Default::default()
    };
    let request = image_request_with_default_cache_size(&request, rect);
    let key = request.stable_cache_key();
    image_cache().invalidate(&key);
    image_cache().run_pending_tasks();
    let before = image_cache_generation();
    spawn_image_load(key.clone(), request.clone());

    let deadline = Instant::now() + Duration::from_secs(2);
    while image_cache_generation() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut display_list =
        DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, 320.0, 480.0));
    display_list.push(DisplayOp::DrawImage {
        rect,
        request,
        fit: ImageFit::Contain,
        alignment: ImageAlignment::Center,
        bounds: rect,
        node_id: None,
    });
    let scene = RenderScene::from_display_list(display_list);
    let pixels = SoftwareRenderer::render(
        &scene,
        960,
        1440,
        RenderColor {
            r: 34,
            g: 39,
            b: 52,
            a: 255,
        },
        3.0,
    )
    .expect("render scale probe");

    let mut bright_pixels = 0;
    for y in 660..735 {
        for x in 300..375 {
            let offset = ((y * 960 + x) * 4) as usize;
            let r = pixels[offset];
            let g = pixels[offset + 1];
            let b = pixels[offset + 2];
            if r > 200 && g > 200 && b > 200 {
                bright_pixels += 1;
            }
        }
    }
    assert!(
        bright_pixels > 500,
        "expected scaled image center to remain visible, found {bright_pixels} bright pixels"
    );
}

#[test]
fn box_shadow_blurs_beyond_the_source_bounds() {
    let rect = fission_render::LayoutRect::new(8.0, 8.0, 4.0, 4.0);
    let mut display_list = DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, 20.0, 20.0));
    display_list.push(DisplayOp::DrawRect {
        rect,
        fill: None,
        stroke: None,
        corner_radius: 2.0,
        shadow: Some(fission_render::BoxShadow {
            color: RenderColor {
                r: 0,
                g: 0,
                b: 0,
                a: 220,
            },
            blur_radius: 6.0,
            spread_radius: 1.0,
            offset: (0.0, 0.0),
            inset: false,
        }),
        bounds: rect,
        node_id: None,
    });
    let pixels = SoftwareRenderer::render(
        &RenderScene::from_display_list(display_list),
        20,
        20,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
    )
    .expect("render blurred shadow");
    let alpha_at = |x: usize, y: usize| pixels[(y * 20 + x) * 4 + 3];

    assert!(alpha_at(10, 10) > alpha_at(3, 3));
    assert!(alpha_at(6, 10) > 0, "blur should extend beyond the source");
}

#[test]
fn inset_box_shadow_stays_inside_and_darkens_the_edge() {
    let rect = fission_render::LayoutRect::new(4.0, 4.0, 12.0, 12.0);
    let mut display_list = DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, 20.0, 20.0));
    display_list.push(DisplayOp::DrawRect {
        rect,
        fill: None,
        stroke: None,
        corner_radius: 2.0,
        shadow: Some(fission_render::BoxShadow {
            color: RenderColor {
                r: 0,
                g: 0,
                b: 0,
                a: 240,
            },
            blur_radius: 4.0,
            spread_radius: 1.0,
            offset: (0.0, 0.0),
            inset: true,
        }),
        bounds: rect,
        node_id: None,
    });
    let pixels = SoftwareRenderer::render(
        &RenderScene::from_display_list(display_list),
        20,
        20,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        1.0,
    )
    .expect("render inset shadow");
    let alpha_at = |x: usize, y: usize| pixels[(y * 20 + x) * 4 + 3];

    assert_eq!(alpha_at(2, 10), 0, "inset shadow must not escape the box");
    assert!(
        alpha_at(5, 10) > alpha_at(10, 10),
        "inset shadow should be strongest near the edge"
    );
}
