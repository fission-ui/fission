use fission_test::TestHarness;
use fission_core::AppState;
use fission_core::ui::Image;
use fission_render::DisplayOp;
use serde::{Serialize, Deserialize};

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
