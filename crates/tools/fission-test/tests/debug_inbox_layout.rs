use anyhow::Result;
use fission_core::ui::{Button, Grid, GridItem, Text, TextContent, TextInput, Widget};
use fission_core::{op::GridTrack, WidgetId};
use fission_test::TestHarness;
use fission_widgets::{HStack, VStack};

#[derive(Debug, Default, Clone)]
struct GlobalState {}
impl fission_core::action::GlobalState for GlobalState {}

#[derive(Clone)]
struct HeaderRepro;
impl From<HeaderRepro> for Widget {
    fn from(_component: HeaderRepro) -> Self {
        let (_ctx, _view) = fission_core::build::current::<GlobalState>();
        Grid {
            columns: vec![
                GridTrack::Points(220.0),
                GridTrack::Points(380.0),
                GridTrack::Fr(1.0),
            ],
            rows: vec![GridTrack::Fr(1.0)],
            children: vec![
                // Col 2 Content
                GridItem::new(VStack {
                    spacing: Some(0.0),
                    children: vec![
                        // Header
                        HStack {
                            spacing: Some(8.0),
                            children: vec![
                                TextInput {
                                    width: Some(200.0),
                                    ..Default::default()
                                }
                                .into(),
                                // Popover logic simulated
                                // Anchor button
                                Button {
                                    id: Some(WidgetId::explicit("filter_btn")),
                                    child: Some(
                                        Text {
                                            content: TextContent::Literal("Filter".into()),
                                            ..Default::default()
                                        }
                                        .into(),
                                    ),
                                    ..Default::default()
                                }
                                .into(),
                            ],
                        }
                        .into(),
                    ],
                })
                .cell(1, 2)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
#[test]
fn test_inbox_header_layout_coords() -> Result<()> {
    let mut h = TestHarness::new(GlobalState::default()).with_root_widget(HeaderRepro);
    h.pump()?; // Build + Layout

    let filter_btn_id: WidgetId = WidgetId::explicit("filter_btn").into();

    if let Some(snapshot) = &h.last_snapshot {
        if let Some(geom) = snapshot.get_node_geometry(filter_btn_id) {
            println!("Filter Button Rect: {:?}", geom.rect);
            // Expected: 220 (Col 1) + 200 (Input) + 8 (Gap) = 428?
            // Gap spacing is accounted for in the row layout.
            // If Input is 200.
            // Button is at X=428?
            // Let's assert it is reasonable.
            assert!(
                geom.rect.x() > 400.0 && geom.rect.x() < 500.0,
                "Filter button X {} should be around 428",
                geom.rect.x()
            );
        } else {
            panic!("Filter button not found in layout snapshot");
        }
    } else {
        panic!("No snapshot");
    }

    Ok(())
}
