use anyhow::Result;
use fission_core::event::{PointerButton, PointerEvent};
use fission_core::ui::{Node, Text};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_test::TestHarness;
use fission_widgets::{Drawer, DrawerSide};

#[derive(Debug, Default, Clone)]
struct State {
    drawer_open: bool,
}
impl AppState for State {}

#[derive(
    fission_macros::Action, serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq,
)]
struct DismissDrawer;

#[test]
fn drawer_renders_content_and_backdrop_dismisses() -> Result<()> {
    fn dismiss(state: &mut State, _: DismissDrawer) {
        state.drawer_open = false;
    }

    struct Root;
    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            let content = Text::new("Drawer content").into_node();
            Drawer {
                id: fission_core::WidgetNodeId::explicit("drawer"),
                side: DrawerSide::Left,
                is_open: view.state.drawer_open,
                on_dismiss: Some(ctx.bind(DismissDrawer, dismiss as fn(&mut State, DismissDrawer))),
                content: Box::new(content),
                width: Some(300.0),
            }
            .build(ctx, view)
        }
    }

    let mut h = TestHarness::new(State { drawer_open: true }).with_root_widget(Root);
    h.pump()?;

    // Ensure content exists and is laid out.
    let ir = h.last_ir.as_ref().unwrap();
    let snap = h.last_snapshot.as_ref().unwrap();
    let mut found = false;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) = &node.op {
            if text == "Drawer content" {
                let r = snap.get_node_rect(*id).unwrap();
                assert!(
                    r.width() > 0.0 && r.height() > 0.0,
                    "drawer content has zero size"
                );
                found = true;
                break;
            }
        }
    }
    assert!(found, "drawer content text not found");

    // Click outside the drawer panel (right side) to hit backdrop.
    let outside = fission_core::LayoutPoint::new(790.0, 10.0);
    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Down {
        point: outside,
        button: PointerButton::Primary,
    }))?;
    h.pump()?;
    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Up {
        point: outside,
        button: PointerButton::Primary,
    }))?;
    h.pump()?;

    let state = h.runtime.get_app_state::<State>().unwrap();
    assert!(
        !state.drawer_open,
        "drawer should dismiss via backdrop click"
    );

    Ok(())
}
