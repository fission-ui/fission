use fission_core::event::{InputEvent, PointerButton, PointerEvent};
use fission_core::{ActionEnvelope, ActionId, GlobalState, Runtime};
use fission_ir::semantics::ActionTrigger;
use fission_ir::WidgetId;
use fission_ir::{ActionEntry, ActionSet, CompositeStyle, CoreIR, CoreNode, Op, Role, Semantics};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};

#[derive(Default, Debug, Clone)]
struct SliderState {
    changes: usize,
    value: f32,
}

impl GlobalState for SliderState {}

#[test]
fn slider_pointer_up_does_not_replay_template_change_payload() -> anyhow::Result<()> {
    let action_id = ActionId::from_name("fission_core_test::SliderChanged");
    let slider_id = WidgetId::explicit("slider");
    let (ir, layout) = slider_tree(slider_id, action_id);
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(SliderState::default()))?;
    runtime.register_reducer::<SliderState>(action_id, record_slider_change)?;

    let point = LayoutPoint::new(150.0, 20.0);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    let state = runtime
        .get_app_state::<SliderState>()
        .expect("slider state");
    assert_eq!(state.changes, 1);
    assert_eq!(state.value, 25.0);
    Ok(())
}

#[test]
fn secondary_click_does_not_change_slider_value() -> anyhow::Result<()> {
    let action_id = ActionId::from_name("fission_core_test::SliderChanged");
    let slider_id = WidgetId::explicit("slider");
    let (ir, layout) = slider_tree(slider_id, action_id);
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(SliderState::default()))?;
    runtime.register_reducer::<SliderState>(action_id, record_slider_change)?;

    let point = LayoutPoint::new(150.0, 20.0);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Secondary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Secondary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;

    let state = runtime
        .get_app_state::<SliderState>()
        .expect("slider state");
    assert_eq!(state.changes, 0);
    assert_eq!(state.value, 0.0);
    Ok(())
}

fn record_slider_change(
    state: &mut SliderState,
    action: &ActionEnvelope,
    _target: WidgetId,
) -> anyhow::Result<()> {
    state.changes += 1;
    state.value = serde_json::from_slice(&action.payload)?;
    Ok(())
}

fn slider_tree(slider_id: WidgetId, action_id: ActionId) -> (CoreIR, LayoutSnapshot) {
    let mut ir = CoreIR::default();
    ir.root = Some(slider_id);
    ir.nodes.insert(
        slider_id,
        CoreNode {
            id: slider_id,
            op: Op::Semantics(Semantics {
                role: Role::Slider,
                focusable: true,
                draggable: true,
                min_value: Some(0.0),
                max_value: Some(100.0),
                current_value: Some(0.0),
                actions: ActionSet {
                    entries: vec![ActionEntry {
                        trigger: ActionTrigger::Change,
                        action_id: action_id.as_u128(),
                        payload_data: Some(serde_json::to_vec(&0.0).unwrap()),
                    }],
                },
                ..Default::default()
            }),
            composite: CompositeStyle::default(),
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );

    let mut layout = LayoutSnapshot::new(LayoutSize::new(300.0, 100.0));
    layout.nodes.insert(
        slider_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(100.0, 10.0, 200.0, 20.0),
            content_size: LayoutSize::new(200.0, 20.0),
        },
    );
    (ir, layout)
}
