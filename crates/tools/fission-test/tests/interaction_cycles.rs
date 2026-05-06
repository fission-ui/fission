use anyhow::Result;
use fission_core::ui::{Node, TextInput};
use fission_core::{InputEvent, LayoutPoint, View, Widget};
use fission_ir::Role;
use fission_test::{detect_ir_cycle, TestHarness};

#[derive(Debug, Default, Clone)]
struct AppState {
    _text: String,
    checked: bool,
}
impl fission_core::action::AppState for AppState {}

#[test]
fn text_input_focus_has_no_ir_cycles() -> Result<()> {
    struct Root;
    impl Widget<AppState> for Root {
        fn build(
            &self,
            _ctx: &mut fission_core::BuildCtx<AppState>,
            _view: &View<AppState>,
        ) -> Node {
            TextInput {
                value: String::new(),
                placeholder: Some("type".into()),
                width: Some(200.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into()
        }
    }

    let mut h = TestHarness::new(AppState::default()).with_root_widget(Root);
    h.pump()?;

    // Find TextInput semantics node rect center
    let ir = h.last_ir.as_ref().unwrap();
    let mut text_node = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::TextInput {
                text_node = Some(*id);
                break;
            }
        }
    }
    let id = text_node.expect("TextInput semantics not found");
    let rect = h.last_snapshot.as_ref().unwrap().get_node_rect(id).unwrap();
    let center = LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );

    h.send_event(InputEvent::Pointer(
        fission_core::event::PointerEvent::Down {
            point: center,
            button: fission_core::event::PointerButton::Primary,
        },
    ))?;
    h.pump()?;

    let ir2 = h.last_ir.as_ref().unwrap();
    assert!(
        detect_ir_cycle(ir2).is_none(),
        "IR contains cycle after focusing TextInput"
    );
    Ok(())
}

#[test]
fn checkbox_toggle_has_no_ir_cycles() -> Result<()> {
    use fission_core::view::Widget;
    use fission_core::{BuildCtx, View};
    use fission_widgets::Checkbox;

    use fission_core::event::{PointerButton, PointerEvent};
    use serde::{Deserialize, Serialize};

    #[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    struct Toggle;

    fn on_toggle(state: &mut AppState, _a: Toggle) {
        state.checked = !state.checked;
    }

    struct Root;
    impl Widget<AppState> for Root {
        fn build(
            &self,
            ctx: &mut BuildCtx<AppState>,
            view: &View<AppState>,
        ) -> fission_core::ui::Node {
            Checkbox {
                checked: view.state.checked,
                on_toggle: Some(ctx.bind(Toggle, on_toggle as fn(&mut AppState, Toggle))),
                label: Some("check".into()),
                ..Default::default()
            }
            .into()
        }
    }

    let mut h = TestHarness::new(AppState::default()).with_root_widget(Root);
    h.pump()?;

    // Locate checkbox semantics and click it (down + up)
    let ir = h.last_ir.as_ref().unwrap();
    let mut cb_node = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::Checkbox {
                cb_node = Some(*id);
                break;
            }
        }
    }
    let id = cb_node.expect("Checkbox semantics not found");
    let rect = h.last_snapshot.as_ref().unwrap().get_node_rect(id).unwrap();
    let center = LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );

    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Down {
        point: center,
        button: PointerButton::Primary,
    }))?;
    h.pump()?;
    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Up {
        point: center,
        button: PointerButton::Primary,
    }))?;
    h.pump()?;

    let ir2 = h.last_ir.as_ref().unwrap();
    if let Some(cycle) = fission_test::detect_ir_cycle(ir2) {
        panic!("IR cycle after checkbox toggle: {:?}", cycle);
    }
    Ok(())
}
