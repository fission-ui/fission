use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use fission_core::internal::{CustomEventResult, CustomRenderObject, RenderObjectHolder};
use fission_core::{
    hit_test::hit_test_with_scroll, ActionEnvelope, ActionId, GlobalState, InputEvent, LayoutPoint,
    PointerButton, PointerEvent, Runtime,
};
use fission_ir::op::{Color, Fill, LayoutOp, PaintOp};
use fission_ir::{ActionEntry, CoreIR, Op, Semantics, StructuralOp, WidgetId};
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

impl GlobalState for BackdropState {}

const DISMISS_ACTION_ID: ActionId = ActionId::from_u128(1);

fn dismiss_backdrop(
    state: &mut BackdropState,
    _action: &ActionEnvelope,
    _target: WidgetId,
) -> anyhow::Result<()> {
    state.dismissals += 1;
    Ok(())
}

fn backdrop_scene() -> (CoreIR, LayoutSnapshot, WidgetId, WidgetId) {
    let root_id = WidgetId::explicit("root");
    let backdrop_id = WidgetId::explicit("backdrop_semantics");
    let backdrop_paint_id = WidgetId::explicit("backdrop_paint");
    let panel_id = WidgetId::explicit("panel");
    let panel_paint_id = WidgetId::explicit("panel_paint");

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

fn pointer_transparent_overlay_scene() -> (CoreIR, LayoutSnapshot, WidgetId) {
    let root_id = WidgetId::explicit("pointer_transparent_root");
    let target_id = WidgetId::explicit("pointer_transparent_target");
    let target_paint_id = WidgetId::explicit("pointer_transparent_target_paint");
    let marker_id = WidgetId::explicit("pointer_transparent_marker");
    let decoration_id = WidgetId::explicit("pointer_transparent_decoration");

    let mut target_semantics = Semantics::default();
    target_semantics.actions.entries.push(ActionEntry {
        trigger: fission_ir::semantics::ActionTrigger::Default,
        action_id: DISMISS_ACTION_ID.as_u128(),
        payload_data: Some(Vec::new()),
    });

    let mut ir = CoreIR::new();
    ir.add_node(
        target_paint_id,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color::WHITE)),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        target_id,
        Op::Semantics(target_semantics),
        vec![target_paint_id],
    );
    ir.add_node(
        decoration_id,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color::BLACK)),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        marker_id,
        Op::Structural(StructuralOp::PointerTransparent),
        vec![decoration_id],
    );
    ir.add_node(
        root_id,
        Op::Layout(LayoutOp::ZStack),
        vec![target_id, marker_id],
    );
    ir.set_root(root_id);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 80.0));
    for id in [
        root_id,
        target_id,
        target_paint_id,
        marker_id,
        decoration_id,
    ] {
        layout
            .nodes
            .insert(id, geometry(LayoutRect::new(0.0, 0.0, 100.0, 80.0)));
    }

    (ir, layout, target_paint_id)
}

#[derive(Debug)]
struct CountingRenderObject {
    handled_events: Arc<AtomicUsize>,
}

impl CustomRenderObject for CountingRenderObject {
    fn handle_event(
        &self,
        _node_id: WidgetId,
        _event: &InputEvent,
        _node_rect: LayoutRect,
    ) -> CustomEventResult {
        self.handled_events.fetch_add(1, Ordering::SeqCst);
        CustomEventResult::consumed()
    }
}

fn custom_render_fallback_scene(
    pointer_transparent: bool,
    handled_events: Arc<AtomicUsize>,
) -> (CoreIR, LayoutSnapshot) {
    let root_id = WidgetId::explicit(if pointer_transparent {
        "transparent_custom_root"
    } else {
        "ordinary_custom_root"
    });
    let target_id = WidgetId::derived(root_id.as_u128(), &[0]);
    let custom_id = WidgetId::derived(root_id.as_u128(), &[1]);
    let marker_id = WidgetId::derived(root_id.as_u128(), &[2]);

    let mut ir = CoreIR::new();
    ir.add_node(
        target_id,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color::WHITE)),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        custom_id,
        Op::Structural(StructuralOp::Group { stable_hash: 1 }),
        vec![],
    );
    let overlay_id = if pointer_transparent {
        ir.add_node(
            marker_id,
            Op::Structural(StructuralOp::PointerTransparent),
            vec![custom_id],
        );
        marker_id
    } else {
        custom_id
    };
    ir.add_node(
        root_id,
        Op::Layout(LayoutOp::ZStack),
        vec![target_id, overlay_id],
    );
    ir.set_root(root_id);
    let render_object: Arc<dyn CustomRenderObject> =
        Arc::new(CountingRenderObject { handled_events });
    let erased: fission_ir::AnyRenderObject = Arc::new(RenderObjectHolder(render_object));
    ir.custom_render_objects.insert(custom_id, erased);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 80.0));
    for id in [root_id, target_id, custom_id, marker_id] {
        layout
            .nodes
            .insert(id, geometry(LayoutRect::new(0.0, 0.0, 100.0, 80.0)));
    }
    (ir, layout)
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
fn pointer_transparent_subtree_passes_through_in_both_hit_test_apis() {
    let (ir, layout, target_paint_id) = pointer_transparent_overlay_scene();
    let runtime = Runtime::default();
    let point = LayoutPoint::new(40.0, 30.0);

    assert_eq!(
        hit_test_with_scroll(&ir, &layout, &runtime.runtime_state.scroll, point),
        Some(target_paint_id)
    );
    assert_eq!(runtime.hit_test(point, &ir, &layout), Some(target_paint_id));
}

#[test]
fn custom_render_fallback_respects_pointer_transparent_ancestors() -> anyhow::Result<()> {
    let control_count = Arc::new(AtomicUsize::new(0));
    let (control_ir, control_layout) =
        custom_render_fallback_scene(false, Arc::clone(&control_count));
    let mut runtime = Runtime::default();
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(40.0, 30.0),
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &control_ir,
        &control_layout,
    )?;
    assert_eq!(
        control_count.load(Ordering::SeqCst),
        1,
        "control custom render object should receive the pointer event"
    );

    let transparent_count = Arc::new(AtomicUsize::new(0));
    let (transparent_ir, transparent_layout) =
        custom_render_fallback_scene(true, Arc::clone(&transparent_count));
    let mut runtime = Runtime::default();
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(40.0, 30.0),
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &transparent_ir,
        &transparent_layout,
    )?;
    assert_eq!(
        transparent_count.load(Ordering::SeqCst),
        0,
        "global custom-render fallback must not bypass pointer transparency"
    );

    Ok(())
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
