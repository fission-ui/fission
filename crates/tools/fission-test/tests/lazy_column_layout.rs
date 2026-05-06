use anyhow::Result;
use fission_core::ui::{Container, Node, Text, TextContent};
use fission_core::{BuildCtx, View, Widget};
use fission_test::TestHarness;
use fission_widgets::LazyColumn;
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
struct AppState {}
impl fission_core::action::AppState for AppState {}

struct Root;
impl Widget<AppState> for Root {
    fn build(&self, _ctx: &mut BuildCtx<AppState>, _view: &View<AppState>) -> Node {
        let mut children = Vec::new();
        for i in 0..5 {
            children.push(
                Container::new(
                    Text {
                        content: TextContent::Literal(format!("Item {}", i)),
                        ..Default::default()
                    }
                    .into(),
                )
                .height(50.0) // Explicit height
                .into_node(),
            );
        }

        LazyColumn {
            id: None,
            children: Arc::new(children),
            item_height: 50.0,
        }
        .into()
    }
}

#[test]
fn test_lazy_column_vertical_stacking() -> Result<()> {
    let mut h = TestHarness::new(AppState::default()).with_root_widget(Root);
    h.pump()?;

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    // Find the text items
    let mut items = Vec::new();
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) = &node.op {
            if text.starts_with("Item") {
                if let Some(geom) = snap.get_node_geometry(*id) {
                    items.push((text.clone(), geom.rect));
                }
            }
        }
    }

    items.sort_by_key(|(t, _)| t.clone());

    for i in 0..items.len() - 1 {
        let (t1, r1) = &items[i];
        let (t2, r2) = &items[i + 1];
        println!("{} Y: {}, {} Y: {}", t1, r1.y(), t2, r2.y());

        // Item 1 should be below Item 0
        if r2.y() < r1.y() + 40.0 {
            // Allow some overlap if padding? No, explicit height 50.
            // If Item 1 Y < Item 0 Y, that's definitely wrong.
            // If Item 1 Y < Item 0 Bottom, they overlap.
            if r2.y() < r1.bottom() {
                panic!("Overlap detected: {} at {:?} vs {} at {:?}", t1, r1, t2, r2);
            }
        }
    }

    Ok(())
}
