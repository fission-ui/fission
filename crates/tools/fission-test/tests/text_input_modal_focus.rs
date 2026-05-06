use anyhow::Result;
use fission_core::event::{PointerButton, PointerEvent};
use fission_core::ui::{Node, TextInput};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_ir::NodeId;
use fission_test::TestHarness;
use fission_widgets::Modal;

#[derive(Debug, Default, Clone)]
struct State {
    modal_open: bool,
}
impl AppState for State {}

#[derive(
    fission_macros::Action, serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq,
)]
struct Dismiss;

#[test]
fn clicking_text_input_inside_modal_sets_focus() -> Result<()> {
    let subject_id = NodeId::explicit("subject_input");

    fn dismiss(state: &mut State, _: Dismiss) {
        state.modal_open = false;
    }

    struct Root {
        subject_id: NodeId,
    }
    impl Widget<State> for Root {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            let content = fission_widgets::VStack {
                spacing: Some(8.0),
                children: vec![
                    TextInput {
                        id: Some(NodeId::explicit("to_input")),
                        value: "a@b.com".into(),
                        placeholder: Some(fission_core::ui::TextContent::Literal("To".into())),
                        width: Some(300.0),
                        ..Default::default()
                    }
                    .into_node(),
                    TextInput {
                        id: Some(self.subject_id),
                        value: "Hello".into(),
                        placeholder: Some(fission_core::ui::TextContent::Literal("Subject".into())),
                        width: Some(300.0),
                        ..Default::default()
                    }
                    .into_node(),
                ],
            }
            .into_node();

            Modal {
                id: fission_core::WidgetNodeId::explicit("modal"),
                title: "Compose".into(),
                content: Box::new(content),
                is_open: view.state.modal_open,
                on_dismiss: Some(ctx.bind(Dismiss, dismiss as fn(&mut State, Dismiss))),
                actions: vec![],
                width: Some(420.0),
            }
            .build(ctx, view)
        }
    }

    let mut h = TestHarness::new(State { modal_open: true }).with_root_widget(Root { subject_id });
    h.pump()?;

    let rect = h
        .last_snapshot
        .as_ref()
        .unwrap()
        .get_node_rect(subject_id)
        .expect("subject TextInput rect");
    let center = fission_core::LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );

    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Down {
        point: center,
        button: PointerButton::Primary,
    }))?;
    h.pump()?;

    assert_eq!(
        h.runtime.runtime_state.interaction.focused,
        Some(subject_id),
        "expected subject TextInput to become focused on click"
    );

    Ok(())
}
