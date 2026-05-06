use anyhow::Result;
use fission_core::ui::{Node, Row, Text, TextContent};
use fission_core::{BuildCtx, View, Widget};
use fission_render::DisplayOp;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct AppState;
impl fission_core::action::AppState for AppState {}

struct OrderRow;
impl Widget<AppState> for OrderRow {
    fn build(&self, _ctx: &mut BuildCtx<AppState>, _view: &View<AppState>) -> Node {
        Node::Row(Row {
            children: vec![
                Text {
                    content: TextContent::Literal("A".into()),
                    ..Default::default()
                }
                .into(),
                Text {
                    content: TextContent::Literal("B".into()),
                    ..Default::default()
                }
                .into(),
                Text {
                    content: TextContent::Literal("C".into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn row_children_order_preserved_in_display_list() -> Result<()> {
    let mut h = TestHarness::new(AppState::default()).with_root_widget(OrderRow);
    h.pump()?;
    let dl = h.get_last_display_list().expect("display list");
    // Collect DrawText ops in the order they appear
    let texts: Vec<String> = dl
        .ops
        .iter()
        .filter_map(|op| {
            if let DisplayOp::DrawText { text, .. } = op {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect();
    // We expect the first three DrawText ops to be A, B, C in order
    let prefix: Vec<String> = texts.into_iter().take(3).collect();
    assert_eq!(
        prefix,
        vec!["A".to_string(), "B".to_string(), "C".to_string()]
    );
    Ok(())
}
