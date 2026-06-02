use anyhow::Result;
use fission_core::event::{PointerButton, PointerEvent};
use fission_core::ui::{TextInput, Widget};
use fission_core::{with_reducer, GlobalState, WidgetId};
use fission_test::TestHarness;
use fission_widgets::Modal;

#[derive(Debug, Default, Clone)]
struct State {
    modal_open: bool,
}
impl GlobalState for State {}

#[fission_macros::fission_reducer(Dismiss)]
fn dismiss(state: &mut State) {
    state.modal_open = false;
}

#[test]
fn clicking_text_input_inside_modal_sets_focus() -> Result<()> {
    let subject_widget_id = WidgetId::explicit("subject_input");
    let subject_id: WidgetId = subject_widget_id.into();

    #[derive(Clone)]
    struct Root {
        subject_id: WidgetId,
    }
    impl From<Root> for Widget {
        fn from(component: Root) -> Self {
            let (ctx, view) = fission_core::build::current::<State>();
            let content = fission_widgets::VStack {
                spacing: Some(8.0),
                children: vec![
                    TextInput {
                        id: Some(WidgetId::explicit("to_input")),
                        value: "a@b.com".into(),
                        placeholder: Some(fission_core::ui::TextContent::Literal("To".into())),
                        width: Some(300.0),
                        ..Default::default()
                    }
                    .into(),
                    TextInput {
                        id: Some(component.subject_id),
                        value: "Hello".into(),
                        placeholder: Some(fission_core::ui::TextContent::Literal("Subject".into())),
                        width: Some(300.0),
                        ..Default::default()
                    }
                    .into(),
                ],
            }
            .into();

            Modal {
                id: fission_core::WidgetId::explicit("modal"),
                title: "Compose".into(),
                content: content,
                is_open: view.state().modal_open,
                on_dismiss: Some(with_reducer!(ctx, Dismiss, dismiss)),
                actions: vec![],
                width: Some(420.0),
            }
            .into()
        }
    }
    let mut h = TestHarness::new(State { modal_open: true }).with_root_widget(Root {
        subject_id: subject_widget_id,
    });
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
