use anyhow::Result;
use fission_core::ui::{Row, Text, TextContent, Widget};
use fission_render::DisplayOp;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct GlobalState;
impl fission_core::action::GlobalState for GlobalState {}

#[derive(Clone)]
struct OrderRow;
impl From<OrderRow> for Widget {
    fn from(_component: OrderRow) -> Self {
        let (_ctx, _view) = fission_core::build::current::<GlobalState>();
        Row {
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
        }
        .into()
    }
}
#[test]
fn row_children_order_preserved_in_display_list() -> Result<()> {
    let mut h = TestHarness::new(GlobalState::default()).with_root_widget(OrderRow);
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
