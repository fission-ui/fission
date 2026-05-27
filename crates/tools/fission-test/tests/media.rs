use fission_core::ui::Image;
use fission_core::AppState;
use fission_ir::op::{ImageAlignment, ImageFit};
use fission_render::DisplayOp;
use fission_test::TestHarness;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DummyState;
impl AppState for DummyState {}

#[test]
fn test_image_render_op() {
    let mut harness = TestHarness::new(DummyState);

    harness = harness.with_root_widget(Image::asset("test.png").size(100.0, 100.0));

    harness.pump().expect("Pump failed");

    let dl = harness.get_last_display_list().expect("No display list");

    let mut found_image = false;
    for op in dl.ops {
        if let DisplayOp::DrawImage { request, rect, .. } = op {
            if request.source.local_path() == Some("test.png")
                && rect.width() == 100.0
                && rect.height() == 100.0
            {
                found_image = true;
            }
        }
    }

    assert!(found_image, "DrawImage op not found or incorrect");
}

#[test]
fn test_network_image_render_op_preserves_request_and_fit() {
    let mut harness = TestHarness::new(DummyState);

    harness = harness.with_root_widget(
        Image::network("https://cdn.example.com/product.webp")
            .size(220.0, 160.0)
            .cache_size(440, 320)
            .semantic_label("Product thumbnail")
            .fit(ImageFit::Cover)
            .alignment(ImageAlignment::TopCenter),
    );

    harness.pump().expect("Pump failed");

    let dl = harness.get_last_display_list().expect("No display list");
    let image = dl.ops.into_iter().find_map(|op| match op {
        DisplayOp::DrawImage {
            request,
            rect,
            fit,
            alignment,
            ..
        } => Some((request, rect, fit, alignment)),
        _ => None,
    });

    let Some((request, rect, fit, alignment)) = image else {
        panic!("DrawImage op not found for network image");
    };
    assert_eq!(
        request.source.network_url(),
        Some("https://cdn.example.com/product.webp")
    );
    assert_eq!(request.cache_width, Some(440));
    assert_eq!(request.cache_height, Some(320));
    assert_eq!(request.semantic_label.as_deref(), Some("Product thumbnail"));
    assert_eq!(rect.width(), 220.0);
    assert_eq!(rect.height(), 160.0);
    assert_eq!(fit, fission_render::ImageFit::Cover);
    assert_eq!(alignment, ImageAlignment::TopCenter);
}

#[test]
fn test_svg_image_render_op() {
    let mut harness = TestHarness::new(DummyState);

    harness = harness.with_root_widget(
        Image::svg_text(
            r#"<svg viewBox="0 0 20 10" xmlns="http://www.w3.org/2000/svg"><rect x="1" y="1" width="18" height="8" rx="2"/></svg>"#,
        )
        .size(120.0, 60.0),
    );

    harness.pump().expect("Pump failed");

    let dl = harness.get_last_display_list().expect("No display list");

    let mut found_svg = false;
    for op in dl.ops {
        if let DisplayOp::DrawSvg {
            content, bounds, ..
        } = op
        {
            if content.contains("<svg") && bounds.width() == 120.0 && bounds.height() == 60.0 {
                found_svg = true;
            }
        }
    }

    assert!(found_svg, "DrawSvg op not found or incorrect for svg image");
}
