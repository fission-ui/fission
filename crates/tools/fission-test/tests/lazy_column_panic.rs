use anyhow::Result;
use fission_core::ui::{Node, Text, TextContent};
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
            children: Arc::new(children),
            item_height: 20.0,
        }
        .into()
    }
}

#[test]
fn test_lazy_column_no_panic() -> Result<()> {
    let mut h = TestHarness::new(AppState::default()).with_root_widget(Root);
    // Pump a few frames to trigger layout updates and verifications
    h.pump()?;
    h.pump()?;
    Ok(())
}
