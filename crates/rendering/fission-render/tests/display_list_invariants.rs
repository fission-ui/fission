use fission_render::{
    Color, DisplayList, DisplayOp, LayoutRect, LayoutSize, LayoutPoint, Renderer,
    DISPLAY_LIST_VERSION
};
use anyhow::Result;
use serde_json;

// --- Dummy Renderer for testing trait implementation ---
pub struct DummyRenderer;

impl Renderer for DummyRenderer {
    fn render(&mut self, display_list: &DisplayList) -> Result<()> {
        // In a real scenario, this would execute drawing commands.
        // For now, we just print for verification.
        // println!("Rendering DisplayList (version: {}):
        // {:?}", display_list.version, display_list.ops);
        Ok(())
    }
}

#[test]
fn test_display_list_creation() {
    let bounds = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
    let dl = DisplayList::new(bounds);
    assert_eq!(dl.version, DISPLAY_LIST_VERSION);
    assert_eq!(dl.bounds, bounds);
    assert!(dl.ops.is_empty());
}

#[test]
fn test_display_op_serialization() {
    let op1 = DisplayOp::DrawRect {
        rect: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        fill: Some(fission_render::Fill { color: Color::BLUE }),
        stroke: None,
        bounds: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        node_id: None,
    };
    let op2 = DisplayOp::Translate(LayoutPoint { x: 5.0, y: 5.0 });

    let json1 = serde_json::to_string(&op1).unwrap();
    let deserialized1: DisplayOp = serde_json::from_str(&json1).unwrap();
    assert_eq!(op1, deserialized1);

    let json2 = serde_json::to_string(&op2).unwrap();
    let deserialized2: DisplayOp = serde_json::from_str(&json2).unwrap();
    assert_eq!(op2, deserialized2);
}

#[test]
fn test_display_list_ops_order() {
    let bounds = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
    let mut dl = DisplayList::new(bounds);

    let op1 = DisplayOp::Save;
    let op2 = DisplayOp::Translate(LayoutPoint { x: 10.0, y: 10.0 });
    let op3 = DisplayOp::Restore;

    dl.push(op1.clone());
    dl.push(op2.clone());
    dl.push(op3.clone());

    assert_eq!(dl.ops.len(), 3);
    assert_eq!(dl.ops[0], op1);
    assert_eq!(dl.ops[1], op2);
    assert_eq!(dl.ops[2], op3);
}

#[test]
fn test_renderer_trait_object() -> Result<()> {
    let mut renderer: Box<dyn Renderer> = Box::new(DummyRenderer);
    let bounds = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
    let dl = DisplayList::new(bounds);

    // Verify that the render method can be called through the trait object
    renderer.render(&dl)?;
    Ok(())
}

#[test]
fn test_display_list_serialization() {
    let bounds = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
    let mut dl = DisplayList::new(bounds);
    dl.push(DisplayOp::Save);
    dl.push(DisplayOp::DrawRect {
        rect: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        fill: Some(fission_render::Fill { color: Color::RED }),
        stroke: None,
        bounds: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        node_id: None,
    });
    dl.push(DisplayOp::Restore);

    let json = serde_json::to_string(&dl).unwrap();
    let deserialized: DisplayList = serde_json::from_str(&json).unwrap();

    assert_eq!(dl, deserialized);
}
