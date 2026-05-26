use fission_core::{
    hit_test::hit_test_with_scroll, ActionEnvelope, ActionId, AppState, InputEvent, LayoutPoint,
    PointerButton, PointerEvent, Runtime,
};
use fission_ir::op::{Color, Fill, LayoutOp, PaintOp};
use fission_ir::{ActionEntry, CoreIR, NodeId, Op, Semantics};
use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize, LayoutSnapshot};

fn geometry(rect: LayoutRect) -> LayoutNodeGeometry {
    LayoutNodeGeometry {
        content_size: rect.size,
        rect,
    }
}

#[derive(Debug, Default)]
struct BackdropState {
    dismissals: usize,
}

impl AppState for BackdropState {}

const DISMISS_ACTION_ID: ActionId = ActionId::from_u128(1);

fn dismiss_backdrop(
    state: &mut BackdropState,
    _action: &ActionEnvelope,
    _target: NodeId,
) -> anyhow::Result<()> {
    state.dismissals += 1;
    Ok(())
}

fn backdrop_scene() -> (CoreIR, LayoutSnapshot, NodeId, NodeId) {
    let root_id = NodeId::explicit("root");
    let backdrop_id = NodeId::explicit("backdrop_semantics");
    let backdrop_paint_id = NodeId::explicit("backdrop_paint");
    let panel_id = NodeId::explicit("panel");
    let panel_paint_id = NodeId::explicit("panel_paint");

    let mut backdrop_semantics = Semantics::default();
    backdrop_semantics.actions.entries.push(ActionEntry {
        trigger: fission_ir::semantics::ActionTrigger::Default,
        action_id: DISMISS_ACTION_ID.as_u128(),
        payload_data: Some(Vec::new()),
    });

    let mut ir = CoreIR::new();
    ir.add_node(
        backdrop_paint_id,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color::BLACK)),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        backdrop_id,
        Op::Semantics(backdrop_semantics),
        vec![backdrop_paint_id],
    );
    ir.add_node(
        panel_paint_id,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color::WHITE)),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        panel_id,
        Op::Layout(LayoutOp::Box {
            width: Some(300.0),
            height: Some(600.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![panel_paint_id],
    );
    ir.add_node(
        root_id,
        Op::Layout(LayoutOp::ZStack),
        vec![backdrop_id, panel_id],
    );
    ir.set_root(root_id);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout
        .nodes
        .insert(root_id, geometry(LayoutRect::new(0.0, 0.0, 800.0, 600.0)));
    layout.nodes.insert(
        backdrop_id,
        geometry(LayoutRect::new(0.0, 0.0, 800.0, 600.0)),
    );
    layout.nodes.insert(
        backdrop_paint_id,
        geometry(LayoutRect::new(0.0, 0.0, 800.0, 600.0)),
    );
    layout
        .nodes
        .insert(panel_id, geometry(LayoutRect::new(0.0, 0.0, 300.0, 600.0)));
    layout.nodes.insert(
        panel_paint_id,
        geometry(LayoutRect::new(0.0, 0.0, 300.0, 600.0)),
    );

    (ir, layout, backdrop_paint_id, panel_paint_id)
}

#[test]
fn painted_foreground_blocks_backdrop_hit_testing() {
    let (ir, layout, backdrop_paint_id, panel_paint_id) = backdrop_scene();
    let runtime = Runtime::default();

    let inside_panel = hit_test_with_scroll(
        &ir,
        &layout,
        &runtime.runtime_state.scroll,
        LayoutPoint::new(150.0, 40.0),
    );
    assert_eq!(inside_panel, Some(panel_paint_id));

    let on_backdrop = hit_test_with_scroll(
        &ir,
        &layout,
        &runtime.runtime_state.scroll,
        LayoutPoint::new(790.0, 40.0),
    );
    assert_eq!(on_backdrop, Some(backdrop_paint_id));
}

#[test]
fn paint_blocking_preserves_runtime_dispatch_for_visible_backdrop() -> anyhow::Result<()> {
    let (ir, layout, _backdrop_paint_id, _panel_paint_id) = backdrop_scene();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(BackdropState::default()))?;
    runtime.register_reducer::<BackdropState>(DISMISS_ACTION_ID, dismiss_backdrop)?;

    let tap = |runtime: &mut Runtime, point: LayoutPoint| -> anyhow::Result<()> {
        runtime.handle_input(
            InputEvent::Pointer(PointerEvent::Down {
                point,
                button: PointerButton::Primary,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )?;
        runtime.handle_input(
            InputEvent::Pointer(PointerEvent::Up {
                point,
                button: PointerButton::Primary,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
    };

    tap(&mut runtime, LayoutPoint::new(150.0, 40.0))?;
    let state = runtime
        .get_app_state::<BackdropState>()
        .expect("backdrop state");
    assert_eq!(
        state.dismissals, 0,
        "a painted foreground panel should not dispatch the backdrop action"
    );

    tap(&mut runtime, LayoutPoint::new(790.0, 40.0))?;
    let state = runtime
        .get_app_state::<BackdropState>()
        .expect("backdrop state");
    assert_eq!(
        state.dismissals, 1,
        "a tap on the exposed backdrop should still dispatch the backdrop action"
    );

    Ok(())
}
