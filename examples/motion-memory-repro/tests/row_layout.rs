//! Layout regression: each row grows to fit its wrapped description.

use fission::layout::LayoutSize;
use fission::render::LayoutRect;
use fission_ir::CoreIR;
use fission_test::TestHarness;
use motion_memory_repro::MotionMemoryReproApp;

const DESCRIPTION: &str = "Repeated scroll content to reproduce retained renderer memory.";
const ROW_WIDTH: f32 = 352.0;

fn row_rect_for(ir: &CoreIR, h: &TestHarness<()>, text_id: fission::core::WidgetId) -> LayoutRect {
    let snapshot = h.last_snapshot.as_ref().unwrap();
    let mut current = ir.nodes.get(&text_id).and_then(|n| n.parent);
    while let Some(id) = current {
        if let Some(rect) = snapshot.get_node_rect(id) {
            if (rect.size.width - ROW_WIDTH).abs() < 0.5 {
                return rect;
            }
        }
        current = ir.nodes.get(&id).and_then(|n| n.parent);
    }
    panic!("description should sit inside a {ROW_WIDTH}px row");
}

#[test]
fn row_descriptions_end_inside_their_row() {
    std::env::set_var("FISSION_REPRO_SCENARIO", "plain");
    std::env::set_var("FISSION_REPRO_ROWS", "4");
    let app = MotionMemoryReproApp::from_env().expect("plain scenario needs no fixtures");
    let mut h = TestHarness::new(()).with_root_widget(app);
    h.env.viewport_size = LayoutSize::new(800.0, 600.0);
    h.pump().expect("repro should build and lay out");

    let ir = h.last_ir.clone().expect("pump should produce IR");
    let snapshot = h.last_snapshot.as_ref().unwrap();
    let descriptions: Vec<_> = ir
        .nodes
        .iter()
        .filter(|(_, node)| node.op.text().as_deref() == Some(DESCRIPTION))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(descriptions.len(), 4, "every row should render its description");

    for id in descriptions {
        let text = snapshot.get_node_rect(id).expect("description laid out");
        let row = row_rect_for(&ir, &h, id);
        let text_bottom = text.origin.y + text.size.height;
        let row_bottom = row.origin.y + row.size.height;
        assert!(
            text.size.height > 0.0 && text_bottom <= row_bottom + 0.5,
            "description {text:?} runs past its row {row:?}"
        );
    }
}
