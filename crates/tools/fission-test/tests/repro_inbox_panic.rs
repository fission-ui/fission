use anyhow::Result;
use fission_core::op::GridTrack;
use fission_core::ui::{Button, Container, Grid, GridItem, Text, TextContent, Widget};
use fission_test::TestHarness;
use fission_widgets::{LazyColumn, VStack};

#[derive(Debug, Default, Clone)]
struct GlobalState {}
impl fission_core::action::GlobalState for GlobalState {}

#[derive(Clone)]
struct Root;
impl From<Root> for Widget {
    fn from(_component: Root) -> Self {
        let (_ctx, _view) = fission_core::build::current::<GlobalState>();
        // Mimic InboxApp Grid structure
        Grid {
            columns: vec![
                GridTrack::Points(200.0),
                GridTrack::Points(300.0),
                GridTrack::Fr(1.0),
            ],
            rows: vec![GridTrack::Fr(1.0)],
            children: vec![
                // Sidebar (VStack)
                GridItem::new(Container::new(VStack {
                    spacing: Some(10.0),
                    children: vec![Text {
                        content: TextContent::Literal("Sidebar".into()),
                        ..Default::default()
                    }
                    .into()],
                }))
                .cell(1, 1)
                .into(),
                // List (LazyColumn)
                GridItem::new(LazyColumn {
                    id: None,
                    children: (0..50)
                        .map(|i| {
                            Button {
                                child: Some(
                                    Text {
                                        content: TextContent::Literal(format!("Item {}", i)),
                                        ..Default::default()
                                    }
                                    .into(),
                                ),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect(),
                    item_height: 40.0,
                })
                .cell(1, 2)
                .into(),
                // Detail (Container)
                GridItem::new(Container::new(Text {
                    content: TextContent::Literal("Detail".into()),
                    ..Default::default()
                }))
                .cell(1, 3)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
#[test]
fn test_inbox_structure_panic() -> Result<()> {
    let mut h = TestHarness::new(GlobalState::default()).with_root_widget(Root);
    h.pump()?;
    h.pump()?;
    Ok(())
}
