use fission_core::ui::{SemanticsRegion, Spacer, WindowDragRegion};
use fission_core::{
    internal::lower_widget_to_ir, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, Runtime,
    Widget,
};
use fission_ir::{Op, WidgetId};
use fission_layout::LayoutNodeGeometry;

#[test]
fn drag_surface_lowers_without_stealing_child_focus_or_hit_testing() {
    let control_id = WidgetId::explicit("control");
    let widget: Widget = WindowDragRegion::new(SemanticsRegion {
        id: Some(control_id),
        focusable: Some(true),
        child: Some(Spacer::default().into()),
        ..Default::default()
    })
    .semantics_identifier("drag")
    .into();
    let ir = lower_widget_to_ir(&widget);
    let root = ir.root.unwrap();
    let Op::Semantics(region) = &ir.nodes[&root].op else {
        panic!("missing drag semantics");
    };
    assert!(region.window_drag_region);
    assert!(!region.focusable);
    assert_eq!(region.identifier.as_deref(), Some("drag"));
    let control = ir
        .nodes
        .values()
        .find(|node| node.id == control_id)
        .unwrap();
    assert!(matches!(&control.op, Op::Semantics(s) if s.focusable && !s.window_drag_region));

    let mut layout = LayoutSnapshot::new(LayoutSize::new(300.0, 60.0));
    for id in ir.nodes.keys() {
        let rect = if *id == root {
            LayoutRect::new(0.0, 0.0, 300.0, 60.0)
        } else {
            LayoutRect::new(100.0, 10.0, 100.0, 40.0)
        };
        layout.nodes.insert(
            *id,
            LayoutNodeGeometry {
                rect,
                content_size: rect.size,
            },
        );
    }
    let runtime = Runtime::default();
    assert_eq!(
        runtime.hit_test(LayoutPoint::new(20.0, 30.0), &ir, &layout),
        Some(root)
    );
    assert_eq!(
        runtime.hit_test(LayoutPoint::new(150.0, 30.0), &ir, &layout),
        Some(control_id)
    );
    assert_eq!(
        runtime.hit_test(LayoutPoint::new(350.0, 30.0), &ir, &layout),
        None
    );
}
