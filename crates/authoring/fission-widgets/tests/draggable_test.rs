use fission_core::{AppState, BuildCtx, View, Widget, Node, ActionId, ActionEnvelope, Action, Handler, ReducerContext, Lower, LayoutEngine};
use fission_core::runtime::Runtime;
use fission_widgets::draggable::{Draggable, DragTarget};
use fission_widgets::VStack;
use fission_core::ui::widgets::button::Button;
use fission_core::ui::Container;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    dropped_data: Option<String>,
}
impl AppState for TestState {}

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct OnDrop;

fn handle_drop(state: &mut TestState, _action: OnDrop, ctx: &mut ReducerContext<TestState>) {
    if let Some(payload) = ctx.input.as_internal_drop() {
        state.dropped_data = Some(String::from_utf8(payload.to_vec()).unwrap());
    }
}

#[test]
fn test_internal_drag_drop_flow() {
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(TestState::default())).unwrap();
    
    let mut registry = fission_core::ActionRegistry::new();
    registry.register(handle_drop as Handler<TestState, OnDrop>);
    runtime.absorb_registry(registry);

    // Pass 1: Lower and Layout
    let env = fission_core::Env::default();
    
    // Build tree manually
    let mut build_ctx = BuildCtx::<TestState>::new();
    let state = TestState::default();
    let view = View::new(&state, &runtime.runtime_state, &env, None);

    let root = VStack {
        spacing: Some(10.0),
        children: vec![
            Draggable {
                payload: "hello".as_bytes().to_vec(),
                on_drag_start: None,
                on_drag_end: None,
                child: Box::new(Button { on_press: Some(ActionEnvelope { id: ActionId::from_u128(100), payload: vec![] }), ..Default::default() }.into_node()),
            }.build(&mut build_ctx, &view),
            
            DragTarget {
                on_drop: Some(ActionEnvelope { id: OnDrop::static_id(), payload: OnDrop.encode() }),
                child: Box::new(Container::default().width(100.0).height(100.0).bg(fission_core::op::Color::RED).into_node()),
            }.build(&mut build_ctx, &view),
        ]
    }.into_node();

    let mut cx = fission_core::LoweringContext::new(&env, &runtime.runtime_state, None, None);
    let root_id = root.lower(&mut cx);
    let mut ir = cx.ir;
    ir.root = Some(root_id);
    
    let env = fission_core::Env::default();
    let input_nodes = fission_core::lowering::build_layout_tree(&ir, &env);
    let mut layout_engine = LayoutEngine::new();
    layout_engine.rebuild(&input_nodes).unwrap();
    let snapshot = layout_engine.compute_layout(&input_nodes, root_id, fission_core::LayoutSize::new(1000.0, 1000.0), &|_| 0.0).unwrap();

    // Find Draggable and DragTarget positions
    let root_node = ir.nodes.get(&root_id).unwrap();
    let draggable_id = root_node.children[0];
    let drag_target_id = root_node.children[1];
    
    let draggable_rect = snapshot.get_node_rect(draggable_id).unwrap();
    let target_rect = snapshot.get_node_rect(drag_target_id).unwrap();
    
    // Simulate Down on Draggable (Center of button)
    let down_point = fission_core::LayoutPoint::new(draggable_rect.x() + draggable_rect.width() / 2.0, draggable_rect.y() + 5.0);
    runtime.handle_input(
        fission_core::InputEvent::Pointer(fission_core::PointerEvent::Down { point: down_point, button: fission_core::PointerButton::Primary }),
        &ir,
        &snapshot
    ).unwrap();
    
    // Simulate Move to Target
    let move_point = fission_core::LayoutPoint::new(target_rect.x() + target_rect.width() / 2.0, target_rect.y() + 5.0);
    runtime.handle_input(
        fission_core::InputEvent::Pointer(fission_core::PointerEvent::Move { point: move_point }),
        &ir,
        &snapshot
    ).unwrap();
    
    // Verify dragging state
    assert!(runtime.runtime_state.gesture.is_panning);
    assert_eq!(runtime.runtime_state.gesture.dragging_payload, Some("hello".as_bytes().to_vec()));

    // Simulate Up on Target
    runtime.handle_input(
        fission_core::InputEvent::Pointer(fission_core::PointerEvent::Up { point: move_point, button: fission_core::PointerButton::Primary }),
        &ir,
        &snapshot
    ).unwrap();
    
    // Assert State Updated
    let state = runtime.get_app_state::<TestState>().unwrap();
    assert_eq!(state.dropped_data, Some("hello".to_string()));
}
