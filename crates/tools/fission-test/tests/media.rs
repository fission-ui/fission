use fission_core::ui::Image;
use fission_core::AppState;
use fission_render::DisplayOp;
use fission_test::TestHarness;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DummyState;
impl AppState for DummyState {}

#[test]
fn test_image_render_op() {
    let mut harness = TestHarness::new(DummyState);

    harness = harness.with_root_widget(Image {
        source: "test.png".into(),
        width: Some(100.0),
        height: Some(100.0),
        ..Default::default()
    });

    harness.pump().expect("Pump failed");

    let dl = harness.get_last_display_list().expect("No display list");

    let mut found_image = false;
    for op in dl.ops {
        if let DisplayOp::DrawImage { source, rect, .. } = op {
            if source == "test.png" && rect.width() == 100.0 && rect.height() == 100.0 {
                found_image = true;
            }
        }
    }

    assert!(found_image, "DrawImage op not found or incorrect");
}

#[test]
fn test_svg_image_render_op() {
    let mut harness = TestHarness::new(DummyState);

    let svg_path =
        std::env::temp_dir().join(format!("fission-image-test-{}.svg", std::process::id()));
    let mut file = std::fs::File::create(&svg_path).expect("create temp svg");
    file.write_all(
        br#"<svg viewBox="0 0 20 10" xmlns="http://www.w3.org/2000/svg"><rect x="1" y="1" width="18" height="8" rx="2"/></svg>"#,
    )
    .expect("write temp svg");

    harness = harness.with_root_widget(Image {
        source: svg_path.to_string_lossy().to_string(),
        width: Some(120.0),
        height: Some(60.0),
        ..Default::default()
    });

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

    std::fs::remove_file(&svg_path).ok();

    assert!(found_svg, "DrawSvg op not found or incorrect for svg image");
}
