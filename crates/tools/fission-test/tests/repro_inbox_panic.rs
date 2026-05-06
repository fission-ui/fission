use anyhow::Result;
use fission_core::ui::{Button, Container, Grid, GridItem, Node, Text, TextContent};
use fission_core::{op::GridTrack, BuildCtx, View, Widget};
use fission_test::TestHarness;
use fission_widgets::{LazyColumn, VStack};
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
struct AppState {}
impl fission_core::action::AppState for AppState {}

struct Root;
impl Widget<AppState> for Root {
    fn build(&self, _ctx: &mut BuildCtx<AppState>, _view: &View<AppState>) -> Node {
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
                GridItem::new(
                    Container::new(
                        VStack {
                            spacing: Some(10.0),
                            children: vec![Text {
                                content: TextContent::Literal("Sidebar".into()),
                                ..Default::default()
                            }
                            .into()],
                        }
                        .build(_ctx, _view),
                    )
                    .into_node(),
                )
                .cell(1, 1)
                .into(),
                // List (LazyColumn)
                GridItem::new(
                    LazyColumn {
                        id: None,
                        children: Arc::new(
                            (0..50)
                                .map(|i| {
                                    Button {
                                        child: Some(Box::new(
                                            Text {
                                                content: TextContent::Literal(format!(
                                                    "Item {}",
                                                    i
                                                )),
                                                ..Default::default()
                                            }
                                            .into(),
                                        )),
                                        ..Default::default()
                                    }
                                    .into()
                                })
                                .collect(),
                        ),
                        item_height: 40.0,
                    }
                    .into(),
                )
                .cell(1, 2)
                .into(),
                // Detail (Container)
                GridItem::new(
                    Container::new(
                        Text {
                            content: TextContent::Literal("Detail".into()),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into_node(),
                )
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
    let mut h = TestHarness::new(AppState::default()).with_root_widget(Root);
    h.pump()?;
    h.pump()?;
    Ok(())
}
