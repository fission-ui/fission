use anyhow::Result;
use fission_core::event::{PointerButton, PointerEvent};
use fission_core::ui::TextInput;
use fission_core::{with_reducer, AppState, BuildCtx, View, Widget};
use fission_ir::NodeId;
use fission_test::TestHarness;
use fission_widgets::Modal;

#[derive(Debug, Default, Clone)]
struct State {
    modal_open: bool,
}
impl AppState for State {}

#[fission_macros::fission_reducer(Dismiss)]
fn dismiss(state: &mut State) {
    state.modal_open = false;
}

#[test]
fn clicking_text_input_inside_modal_sets_focus() -> Result<()> {
    let subject_id = NodeId::explicit("subject_input");

    struct Root {
        subject_id: NodeId,
    }
    impl Widget<State> for Root {
        fn build(
            &self,
            ctx: &mut BuildCtx<State>,
            view: &View<State>,
        ) -> impl fission_core::IntoWidget<State> {
            fission_core::AnyWidget::from_node({
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
                            placeholder: Some(fission_core::ui::TextContent::Literal(
                                "Subject".into(),
                            )),
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
                    on_dismiss: Some(with_reducer!(ctx, Dismiss, dismiss)),
                    actions: vec![],
                    width: Some(420.0),
                }
                .build_node(ctx, view)
            })
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
        modifiers: 0,
    }))?;
    h.pump()?;

    assert_eq!(
        h.runtime.runtime_state.interaction.focused,
        Some(subject_id),
        "expected subject TextInput to become focused on click"
    );

    Ok(())
}
