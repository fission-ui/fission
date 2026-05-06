use anyhow::Result;
use fission_render::{DisplayList, DisplayOp, LayoutRect, RenderScene, Renderer};

// A mock renderer that captures what it was asked to render.
#[derive(Default)]
struct MockRenderer {
    captured_list: Option<DisplayList>,
}

impl Renderer for MockRenderer {
    fn render_scene(&mut self, scene: &RenderScene) -> Result<()> {
        self.captured_list = Some(scene.flatten());
        Ok(())
    }
}

#[test]
fn test_display_list_serialization() {
    let rect = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
    let op1 = DisplayOp::DrawRect {
        rect,
        fill: None,
        stroke: None,
        corner_radius: 0.0,
        shadow: None, // Added
        bounds: rect,
        node_id: None,
    };

    let mut list = DisplayList::new(rect);
    list.push(op1.clone());

    // Serialize
    let serialized = serde_json::to_string(&list).expect("Failed to serialize display list");

    // Deserialize
    let deserialized: DisplayList =
        serde_json::from_str(&serialized).expect("Failed to deserialize display list");

    assert_eq!(list, deserialized);
    assert_eq!(deserialized.ops.len(), 1);
    assert_eq!(deserialized.ops[0], op1);
}

#[test]
fn test_renderer_consumes_display_list() {
    let mut renderer = MockRenderer::default();
    let bounds = LayoutRect::new(0.0, 0.0, 800.0, 600.0);
    let mut dl = DisplayList::new(bounds);

    dl.push(DisplayOp::DrawRect {
        rect: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        fill: None,
        stroke: None,
        corner_radius: 0.0,
        shadow: None, // Added
        bounds: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        node_id: None,
    });

    renderer.render(&dl).expect("Render failed");

    assert!(renderer.captured_list.is_some());
    let captured = renderer.captured_list.unwrap();
    assert_eq!(captured.ops.len(), 1);
}
