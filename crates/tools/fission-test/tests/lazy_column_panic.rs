use anyhow::Result;
use fission_core::ui::{Text, TextContent, Widget};
use fission_test::TestHarness;
use fission_widgets::LazyColumn;

#[derive(Debug, Default, Clone)]
struct GlobalState {}
impl fission_core::action::GlobalState for GlobalState {}

#[derive(Clone)]
struct Root;
impl From<Root> for Widget {
    fn from(_component: Root) -> Self {
        let (_ctx, _view) = fission_core::build::current::<GlobalState>();
        let mut children = Vec::new();
        for i in 0..10 {
            children.push(
                Text {
                    content: TextContent::Literal(format!("Item {}", i)),
                    ..Default::default()
                }
                .into(),
            );
        }

        LazyColumn {
            id: None,
            children,
            item_height: 20.0,
        }
        .into()
    }
}
#[test]
fn test_lazy_column_no_panic() -> Result<()> {
    let mut h = TestHarness::new(GlobalState::default()).with_root_widget(Root);
    // Pump a few frames to trigger layout updates and verifications
    h.pump()?;
    h.pump()?;
    Ok(())
}
