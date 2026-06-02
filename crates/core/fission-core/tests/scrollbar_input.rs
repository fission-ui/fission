use fission_core::env::{
    Clipboard, GestureState, InteractionStateMap, ScrollStateMap, TextEditStateMap,
};
use fission_core::event::{InputEvent, PointerButton, PointerEvent};
use fission_core::input::gesture::GestureController;
use fission_core::input::{ControllerContext, InputController};
use fission_core::scrollbar::scrollbar_geometry_for_node;
use fission_core::Runtime;
use fission_ir::{CompositeStyle, CoreIR, CoreNode, FlexDirection, LayoutOp, Op, WidgetId};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};
use std::sync::Arc;

#[derive(Default)]
struct NoClipboard;

impl Clipboard for NoClipboard {
    fn get_text(&self) -> Option<String> {
        None
    }

    fn set_text(&self, _text: &str) {}
}

#[test]
fn dragging_scrollbar_thumb_updates_scroll_offset_directly() {
    let (ir, layout, scroll) = scroll_tree(FlexDirection::Column);

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll_map = ScrollStateMap::default();
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(NoClipboard);
    let mut controller = GestureController;

    let geometry =
        scrollbar_geometry_for_node(&ir, &layout, &scroll_map, scroll).expect("scrollbar");
    let down = LayoutPoint::new(
        geometry.thumb_rect.origin.x + 1.0,
        geometry.thumb_rect.origin.y + 4.0,
    );
    let move_to = LayoutPoint::new(down.x, geometry.rail_rect.bottom() - 1.0);

    let mut ctx = ControllerContext {
        ir: &ir,
        layout: &layout,
        text_edit: &mut text_edit,
        interaction: &mut interaction,
        scroll: &mut scroll_map,
        gesture: &mut gesture,
        clipboard: Some(&clipboard),
        measurer: None,
        dispatched_actions: Vec::new(),
    };

    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Down {
            point: down,
            button: PointerButton::Primary,
            modifiers: 0,
        })
    ));
    assert!(ctx.gesture.scrollbar_drag.is_some());

    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Move {
            point: move_to,
            modifiers: 0,
        })
    ));
    assert!(ctx.scroll.get_offset(scroll) > 0.0);

    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Up {
            point: move_to,
            button: PointerButton::Primary,
            modifiers: 0,
        })
    ));
    assert!(ctx.gesture.scrollbar_drag.is_none());
}

#[test]
fn dragging_nested_scrollbar_uses_visual_pointer_coordinates() {
    let parent = WidgetId::derived(82, &[0]);
    let child = WidgetId::derived(82, &[1]);
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Row,
            show_scrollbar: true,
            width: Some(100.0),
            height: Some(50.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![],
    );
    ir.add_node(
        parent,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: Some(120.0),
            height: Some(120.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![child],
    );
    ir.set_root(parent);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(120.0, 120.0));
    layout.nodes.insert(
        parent,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 120.0, 120.0),
            content_size: LayoutSize::new(120.0, 320.0),
        },
    );
    layout.nodes.insert(
        child,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 160.0, 100.0, 50.0),
            content_size: LayoutSize::new(300.0, 50.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll_map = ScrollStateMap::default();
    scroll_map.set_offset(parent, 100.0);
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(NoClipboard);
    let mut controller = GestureController;
    let mut ctx = ControllerContext {
        ir: &ir,
        layout: &layout,
        text_edit: &mut text_edit,
        interaction: &mut interaction,
        scroll: &mut scroll_map,
        gesture: &mut gesture,
        clipboard: Some(&clipboard),
        measurer: None,
        dispatched_actions: Vec::new(),
    };

    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Down {
            point: LayoutPoint::new(50.0, 104.0),
            button: PointerButton::Primary,
            modifiers: 0,
        })
    ));
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Move {
            point: LayoutPoint::new(98.0, 104.0),
            modifiers: 0,
        })
    ));

    assert!(
        ctx.scroll.get_offset(child) > 100.0,
        "nested horizontal scrollbar should drag in its own layout coordinates"
    );
    assert_eq!(ctx.scroll.get_offset(parent), 100.0);
}

#[test]
fn wheel_scroll_on_vertical_scrollbar_rail_updates_scroll_offset() {
    let (ir, layout, scroll) = scroll_tree(FlexDirection::Column);
    let mut runtime = Runtime::default();
    let geometry = scrollbar_geometry_for_node(&ir, &layout, &runtime.runtime_state.scroll, scroll)
        .expect("scrollbar");

    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Scroll {
                point: LayoutPoint::new(
                    geometry.rail_rect.origin.x + 1.0,
                    geometry.rail_rect.origin.y + 80.0,
                ),
                delta: LayoutPoint::new(0.0, 90.0),
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .expect("scroll rail wheel");

    assert!(runtime.runtime_state.scroll.get_offset(scroll) > 0.0);
}

#[test]
fn wheel_scroll_on_horizontal_scrollbar_rail_updates_scroll_offset() {
    let (ir, layout, scroll) = scroll_tree(FlexDirection::Row);
    let mut runtime = Runtime::default();
    let geometry = scrollbar_geometry_for_node(&ir, &layout, &runtime.runtime_state.scroll, scroll)
        .expect("scrollbar");

    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Scroll {
                point: LayoutPoint::new(
                    geometry.rail_rect.origin.x + 80.0,
                    geometry.rail_rect.origin.y + 1.0,
                ),
                delta: LayoutPoint::new(90.0, 0.0),
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .expect("scroll rail wheel");

    assert!(runtime.runtime_state.scroll.get_offset(scroll) > 0.0);
}

fn scroll_tree(direction: FlexDirection) -> (CoreIR, LayoutSnapshot, WidgetId) {
    let scroll = WidgetId::derived(81, &[1]);
    let mut ir = CoreIR::default();
    ir.nodes.insert(
        scroll,
        CoreNode {
            id: scroll,
            parent: None,
            children: Vec::new(),
            op: Op::Layout(LayoutOp::Scroll {
                direction,
                show_scrollbar: true,
                width: Some(100.0),
                height: Some(200.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            composite: CompositeStyle::default(),
            hash: 0,
        },
    );
    ir.set_root(scroll);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 200.0));
    let content_size = match direction {
        FlexDirection::Column => LayoutSize::new(100.0, 600.0),
        FlexDirection::Row => LayoutSize::new(500.0, 200.0),
    };
    layout.nodes.insert(
        scroll,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 200.0),
            content_size,
        },
    );

    (ir, layout, scroll)
}
