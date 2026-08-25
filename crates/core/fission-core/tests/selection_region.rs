use fission_core::env::{
    Clipboard, ContextMenuState, Env, GestureState, InteractionStateMap, RuntimeState,
    ScrollStateMap, SelectableTextStateMap, TextEditStateMap,
};
use fission_core::event::{
    EditingCommand, InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent, PointerId,
    PointerKind, MOD_SHIFT,
};
use fission_core::input::selectable_text::SelectableTextController;
use fission_core::input::{ControllerContext, InputController, TextEditingConvention};
use fission_core::internal::{lower_widget, InternalLoweringCx};
use fission_core::ui::{
    Column, SelectionPlatformStyle, SelectionRegion, SelectionRegionControls, Text, TextContent,
    Widget,
};
use fission_core::{
    Runtime, RuntimeEffect, SelectionRegionCommand, SelectionRegionController, TextAffinity,
    TextPosition, TextRegionPosition, TextRegionSelection,
};
use fission_ir::{
    CoreIR, FlexDirection, LayoutOp, Op, Role, SelectionRegionSemantics, Semantics, WidgetId,
};
use fission_layout::{
    LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LineMetric,
    TextMeasurer,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct TestClipboard(Mutex<String>);

impl Clipboard for TestClipboard {
    fn get_text(&self) -> Option<String> {
        Some(self.0.lock().unwrap().clone())
    }

    fn set_text(&self, text: &str) {
        *self.0.lock().unwrap() = text.into();
    }
}

struct FixedMeasurer;

impl TextMeasurer for FixedMeasurer {
    fn measure(&self, text: &str, _font_size: f32, _width: Option<f32>) -> (f32, f32) {
        (text.len() as f32 * 10.0, 20.0)
    }

    fn hit_test(&self, text: &str, _font_size: f32, _width: Option<f32>, x: f32, _y: f32) -> usize {
        ((x.max(0.0) / 10.0).floor() as usize).min(text.len())
    }

    fn get_caret_position(
        &self,
        text: &str,
        _font_size: f32,
        _width: Option<f32>,
        caret_index: usize,
    ) -> (f32, f32) {
        (caret_index.min(text.len()) as f32 * 10.0, 0.0)
    }

    fn get_line_metrics(
        &self,
        text: &str,
        _font_size: f32,
        _width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![LineMetric {
            start_index: 0,
            end_index: text.len(),
            baseline: 15.0,
            height: 20.0,
            width: text.len() as f32 * 10.0,
        }]
    }
}

fn selectable(value: &str) -> Semantics {
    Semantics {
        role: Role::Text,
        value: Some(value.into()),
        focusable: true,
        read_only: true,
        selectable_text: true,
        context_menu: true,
        ..Semantics::default()
    }
}

fn region_tree() -> (CoreIR, LayoutSnapshot, WidgetId, WidgetId, WidgetId) {
    let region = WidgetId::explicit("selection.region");
    let first = WidgetId::explicit("selection.first");
    let second = WidgetId::explicit("selection.second");
    let mut ir = CoreIR::new();
    ir.add_node(first, Op::Semantics(selectable("hello")), vec![]);
    ir.add_node(second, Op::Semantics(selectable("world")), vec![]);
    ir.add_node(
        region,
        Op::Semantics(Semantics {
            role: Role::Text,
            value: Some("hello\nworld".into()),
            focusable: true,
            read_only: true,
            context_menu: true,
            selection_region: Some(SelectionRegionSemantics {
                excluded: false,
                separator: "\n".into(),
            }),
            ..Semantics::default()
        }),
        vec![first, second],
    );
    ir.set_root(region);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 40.0));
    for (id, rect) in [
        (region, LayoutRect::new(0.0, 0.0, 100.0, 40.0)),
        (first, LayoutRect::new(0.0, 0.0, 100.0, 20.0)),
        (second, LayoutRect::new(0.0, 20.0, 100.0, 20.0)),
    ] {
        layout.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect,
                content_size: rect.size,
            },
        );
    }
    (ir, layout, region, first, second)
}

fn region_widget(
    region: WidgetId,
    first: WidgetId,
    second: WidgetId,
    platform_style: SelectionPlatformStyle,
) -> Widget {
    SelectionRegion::new(Column {
        children: vec![
            Text {
                id: Some(first),
                content: TextContent::from("hello"),
                selectable: true,
                ..Default::default()
            }
            .into(),
            Text {
                id: Some(second),
                content: TextContent::from("world"),
                selectable: true,
                ..Default::default()
            }
            .into(),
        ],
        ..Default::default()
    })
    .controller(SelectionRegionController::new(region))
    .controls(SelectionRegionControls {
        platform_style,
        ..SelectionRegionControls::default()
    })
    .into()
}

#[allow(clippy::too_many_arguments)]
fn context<'a>(
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    selectable_text: &'a mut SelectableTextStateMap,
    interaction: &'a mut InteractionStateMap,
    context_menu: &'a mut ContextMenuState,
    scroll: &'a mut ScrollStateMap,
    text_edit: &'a mut TextEditStateMap,
    gesture: &'a mut GestureState,
    clipboard: &'a Arc<dyn Clipboard>,
    measurer: &'a Arc<dyn TextMeasurer>,
    current_time: u64,
) -> ControllerContext<'a> {
    ControllerContext {
        ir,
        layout,
        text_edit,
        selectable_text,
        context_menu,
        interaction,
        scroll,
        viewport: Box::leak(Box::new(Default::default())),
        gesture,
        editing_convention: TextEditingConvention::Standard,
        current_time,
        clipboard: Some(clipboard),
        measurer: Some(measurer),
        dispatched_actions: Vec::new(),
    }
}

fn pointer(kind: PointerKind, point: LayoutPoint, down: bool) -> InputEvent {
    let pointer_id = if kind == PointerKind::Mouse {
        PointerId::MOUSE
    } else {
        PointerId::contact(1)
    };
    if down {
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id,
            kind,
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        })
    } else {
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id,
            kind,
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        })
    }
}

#[test]
fn region_lowering_preserves_document_order_and_excludes_nested_subtrees() {
    let region_id = WidgetId::explicit("article");
    let first = WidgetId::explicit("first");
    let hidden = WidgetId::explicit("hidden");
    let last = WidgetId::explicit("last");
    let tree = SelectionRegion::new(Column {
        children: vec![
            Text {
                id: Some(first),
                content: TextContent::from("alpha"),
                selectable: true,
                ..Default::default()
            }
            .into(),
            SelectionRegion::exclude(Text {
                id: Some(hidden),
                content: TextContent::from("secret"),
                selectable: true,
                ..Default::default()
            })
            .into(),
            Text {
                id: Some(last),
                content: TextContent::from("omega"),
                selectable: true,
                ..Default::default()
            }
            .into(),
        ],
        ..Default::default()
    })
    .controller(SelectionRegionController::new(region_id));

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let root = lower_widget(&Widget::from(tree), &mut cx);
    cx.ir.set_root(root);
    let semantics = match &cx.ir.nodes[&region_id].op {
        Op::Semantics(semantics) => semantics,
        _ => panic!("selection region must lower to semantics"),
    };
    assert_eq!(semantics.value.as_deref(), Some("alpha\nomega"));
    assert!(semantics.selection_region.is_some());
}

#[test]
fn controller_selects_directionally_and_select_all_materializes_each_node() {
    let (ir, _, region, first, second) = region_tree();
    let controller = SelectionRegionController::new(region);
    let mut runtime = RuntimeState::default();
    let reverse = TextRegionSelection {
        base: TextRegionPosition::at(second, TextPosition::at_end("world")),
        extent: TextRegionPosition::at(first, TextPosition::from_utf8("hello", 2).unwrap()),
        affinity: TextAffinity::Upstream,
    };
    controller
        .apply(&mut runtime, &ir, SelectionRegionCommand::Select(reverse))
        .unwrap();
    assert_eq!(controller.selection(&runtime), Some(reverse));
    assert_eq!(runtime.selectable_text.selection_range(first), Some((2, 5)));
    assert_eq!(
        runtime.selectable_text.selection_range(second),
        Some((0, 5))
    );

    controller
        .apply(&mut runtime, &ir, SelectionRegionCommand::SelectAll)
        .unwrap();
    assert_eq!(runtime.selectable_text.selection_range(first), Some((0, 5)));
    assert_eq!(
        runtime.selectable_text.selection_range(second),
        Some((0, 5))
    );

    controller
        .apply(&mut runtime, &ir, SelectionRegionCommand::Clear)
        .unwrap();
    assert_eq!(controller.selection(&runtime), None);
    assert!(runtime.selectable_text.get(first).is_none());
    assert!(runtime.selectable_text.get(second).is_none());
}

#[test]
fn selectable_text_is_an_implicit_one_node_region_for_the_controller() {
    let (ir, _, _, first, _) = region_tree();
    let controller = SelectionRegionController::new(first);
    let mut runtime = RuntimeState::default();

    controller
        .apply(&mut runtime, &ir, SelectionRegionCommand::SelectAll)
        .unwrap();

    assert_eq!(runtime.selectable_text.selection_range(first), Some((0, 5)));
    let selection = controller.selection(&runtime).expect("implicit selection");
    assert_eq!(selection.base.node_id, first);
    assert_eq!(selection.extent.node_id, first);
}

#[test]
fn retained_selection_is_cleared_when_an_endpoint_unmounts() {
    let (mut ir, _, region, first, second) = region_tree();
    let controller = SelectionRegionController::new(region);
    let mut runtime = Runtime::default();
    controller
        .apply(
            &mut runtime.runtime_state,
            &ir,
            SelectionRegionCommand::SelectAll,
        )
        .unwrap();
    ir.nodes.get_mut(&region).unwrap().children = vec![first];
    ir.nodes.remove(&second);
    runtime.reconcile_ir(&ir);
    assert_eq!(controller.selection(&runtime.runtime_state), None);
    assert!(runtime.runtime_state.selectable_text.get(second).is_none());
}

#[test]
fn retained_selection_survives_a_reordered_tree_by_stable_node_identity() {
    let (mut ir, _, region, first, second) = region_tree();
    let controller = SelectionRegionController::new(region);
    let mut runtime = Runtime::default();
    let selection = TextRegionSelection {
        base: TextRegionPosition::at(first, TextPosition::from_utf8("hello", 1).unwrap()),
        extent: TextRegionPosition::at(second, TextPosition::from_utf8("world", 3).unwrap()),
        affinity: TextAffinity::Downstream,
    };
    controller
        .apply(
            &mut runtime.runtime_state,
            &ir,
            SelectionRegionCommand::Select(selection),
        )
        .unwrap();

    ir.nodes.get_mut(&region).unwrap().children = vec![second, first];
    runtime.reconcile_ir(&ir);

    assert_eq!(
        controller.selection(&runtime.runtime_state),
        Some(selection)
    );
    assert_eq!(
        runtime.runtime_state.selectable_text.selection_range(first),
        Some((0, 1))
    );
    assert_eq!(
        runtime
            .runtime_state
            .selectable_text
            .selection_range(second),
        Some((3, 5))
    );
}

#[test]
fn runtime_effect_applies_a_reducer_requested_selection_after_lowering() {
    let (ir, layout, region, first, second) = region_tree();
    let controller = SelectionRegionController::new(region);
    let mut runtime = Runtime::default();

    assert!(
        runtime.queue_runtime_effect(RuntimeEffect::SelectionRegion {
            region_id: controller.id(),
            command: SelectionRegionCommand::SelectAll,
        })
    );
    assert!(runtime.post_layout_hook(&ir, &layout));

    assert_eq!(
        runtime.runtime_state.selectable_text.selection_range(first),
        Some((0, 5))
    );
    assert_eq!(
        runtime
            .runtime_state
            .selectable_text
            .selection_range(second),
        Some((0, 5))
    );
}

#[test]
fn drag_crosses_nodes_and_copy_uses_the_region_separator() {
    let (ir, layout, region, first, second) = region_tree();
    let mut selectable_text = SelectableTextStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut context_menu = ContextMenuState::default();
    let mut scroll = ScrollStateMap::default();
    let mut text_edit = TextEditStateMap::default();
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(TestClipboard::default());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(FixedMeasurer);
    let mut ctx = context(
        &ir,
        &layout,
        &mut selectable_text,
        &mut interaction,
        &mut context_menu,
        &mut scroll,
        &mut text_edit,
        &mut gesture,
        &clipboard,
        &measurer,
        100,
    );
    let mut controller = SelectableTextController;
    assert!(controller.handle_event(
        &mut ctx,
        &pointer(PointerKind::Mouse, LayoutPoint::new(20.0, 5.0), true),
    ));
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Move {
            pointer_id: PointerId::MOUSE,
            kind: PointerKind::Mouse,
            point: LayoutPoint::new(30.0, 25.0),
            modifiers: 0,
        }),
    ));
    assert!(controller.handle_event(
        &mut ctx,
        &pointer(PointerKind::Mouse, LayoutPoint::new(30.0, 25.0), false),
    ));
    assert_eq!(ctx.selectable_text.selection_range(first), Some((2, 5)));
    assert_eq!(ctx.selectable_text.selection_range(second), Some((0, 3)));
    assert!(controller.handle_event(&mut ctx, &InputEvent::Editing(EditingCommand::Copy)));
    assert_eq!(clipboard.get_text().as_deref(), Some("llo\nwor"));
    assert_eq!(ctx.interaction.focused, Some(region));

    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Right,
            modifiers: MOD_SHIFT,
        }),
    ));
    assert_eq!(ctx.selectable_text.selection_range(second), Some((0, 4)));
}

#[test]
fn rebuilt_region_exposes_one_directional_accessibility_selection() {
    let region_id = WidgetId::explicit("accessible.article");
    let first = WidgetId::explicit("accessible.first");
    let second = WidgetId::explicit("accessible.second");
    let tree = SelectionRegion::new(Column {
        children: vec![
            Text {
                id: Some(first),
                content: TextContent::from("alpha"),
                selectable: true,
                ..Default::default()
            }
            .into(),
            Text {
                id: Some(second),
                content: TextContent::from("omega"),
                selectable: true,
                ..Default::default()
            }
            .into(),
        ],
        ..Default::default()
    })
    .controller(SelectionRegionController::new(region_id));
    let env = Env::default();
    let mut runtime = RuntimeState::default();
    let mut first_lower = InternalLoweringCx::new(&env, &runtime, None, None);
    let root = lower_widget(&Widget::from(tree.clone()), &mut first_lower);
    first_lower.ir.set_root(root);
    let first_ir = first_lower.ir.clone();
    drop(first_lower);
    SelectionRegionController::new(region_id)
        .apply(
            &mut runtime,
            &first_ir,
            SelectionRegionCommand::Select(TextRegionSelection {
                base: TextRegionPosition::new(first, "alpha", 2).unwrap(),
                extent: TextRegionPosition::new(second, "omega", 3).unwrap(),
                affinity: TextAffinity::Downstream,
            }),
        )
        .unwrap();

    let mut rebuilt = InternalLoweringCx::new(&env, &runtime, None, None);
    let root = lower_widget(&Widget::from(tree), &mut rebuilt);
    rebuilt.ir.set_root(root);
    let semantics = match &rebuilt.ir.nodes[&region_id].op {
        Op::Semantics(semantics) => semantics,
        _ => panic!("selection region must lower to semantics"),
    };
    assert_eq!(semantics.value.as_deref(), Some("alpha\nomega"));
    assert_eq!(semantics.text_selection, Some((2, 9)));
}

#[test]
fn double_and_triple_click_select_word_then_paragraph() {
    let (ir, layout, region, _, _) = region_tree();
    let mut selectable_text = SelectableTextStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut context_menu = ContextMenuState::default();
    let mut scroll = ScrollStateMap::default();
    let mut text_edit = TextEditStateMap::default();
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(TestClipboard::default());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(FixedMeasurer);
    let mut controller = SelectableTextController;
    for (index, now) in [100, 200, 300].into_iter().enumerate() {
        let mut ctx = context(
            &ir,
            &layout,
            &mut selectable_text,
            &mut interaction,
            &mut context_menu,
            &mut scroll,
            &mut text_edit,
            &mut gesture,
            &clipboard,
            &measurer,
            now,
        );
        let point = LayoutPoint::new(20.0, 5.0);
        assert!(controller.handle_event(&mut ctx, &pointer(PointerKind::Mouse, point, true)));
        assert!(controller.handle_event(&mut ctx, &pointer(PointerKind::Mouse, point, false)));
        let selection = ctx.selectable_text.region_selection(region).unwrap();
        if index == 1 {
            assert_eq!(selection.base.offset.utf8_offset(), 0);
            assert_eq!(selection.extent.offset.utf8_offset(), 5);
        } else if index == 2 {
            assert_eq!(selection.base.offset.utf8_offset(), 0);
            assert_eq!(selection.extent.offset.utf8_offset(), 5);
        }
    }
}

#[test]
fn touch_long_press_selects_a_word_and_opens_the_region_menu() {
    let (ir, layout, region, _, _) = region_tree();
    let mut selectable_text = SelectableTextStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut context_menu = ContextMenuState::default();
    let mut scroll = ScrollStateMap::default();
    let mut text_edit = TextEditStateMap::default();
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(TestClipboard::default());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(FixedMeasurer);
    let mut controller = SelectableTextController;
    let point = LayoutPoint::new(20.0, 5.0);
    {
        let mut ctx = context(
            &ir,
            &layout,
            &mut selectable_text,
            &mut interaction,
            &mut context_menu,
            &mut scroll,
            &mut text_edit,
            &mut gesture,
            &clipboard,
            &measurer,
            0,
        );
        assert!(controller.handle_event(&mut ctx, &pointer(PointerKind::Touch, point, true)));
    }
    {
        let mut ctx = context(
            &ir,
            &layout,
            &mut selectable_text,
            &mut interaction,
            &mut context_menu,
            &mut scroll,
            &mut text_edit,
            &mut gesture,
            &clipboard,
            &measurer,
            600,
        );
        assert!(controller.handle_event(&mut ctx, &pointer(PointerKind::Touch, point, false)));
        assert_eq!(ctx.context_menu.owner, Some(region));
        let selection = ctx.selectable_text.region_selection(region).unwrap();
        assert_eq!(selection.base.offset.utf8_offset(), 0);
        assert_eq!(selection.extent.offset.utf8_offset(), 5);
    }
}

#[test]
fn adaptive_touch_affordances_observe_slop_and_render_handles_and_magnifier() {
    let (ir, layout, region, first, second) = region_tree();
    let mut selectable_text = SelectableTextStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut context_menu = ContextMenuState::default();
    let mut scroll = ScrollStateMap::default();
    let mut text_edit = TextEditStateMap::default();
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(TestClipboard::default());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(FixedMeasurer);
    let mut ctx = context(
        &ir,
        &layout,
        &mut selectable_text,
        &mut interaction,
        &mut context_menu,
        &mut scroll,
        &mut text_edit,
        &mut gesture,
        &clipboard,
        &measurer,
        100,
    );
    let mut controller = SelectableTextController;
    assert!(controller.handle_event(
        &mut ctx,
        &pointer(PointerKind::Touch, LayoutPoint::new(10.0, 5.0), true),
    ));
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Move {
            pointer_id: PointerId::contact(1),
            kind: PointerKind::Touch,
            point: LayoutPoint::new(15.0, 5.0),
            modifiers: 0,
        }),
    ));
    assert!(ctx.selectable_text.selection_range(first).is_none());
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Move {
            pointer_id: PointerId::contact(1),
            kind: PointerKind::Touch,
            point: LayoutPoint::new(30.0, 25.0),
            modifiers: 0,
        }),
    ));
    assert_eq!(ctx.selectable_text.selection_range(first), Some((1, 5)));
    assert_eq!(ctx.selectable_text.selection_range(second), Some((0, 3)));
    drop(ctx);

    let runtime = RuntimeState {
        selectable_text: selectable_text.clone(),
        context_menu: context_menu.clone(),
        interaction: interaction.clone(),
        ..RuntimeState::default()
    };
    let env = Env::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, Some(&layout));
    lower_widget(
        &region_widget(region, first, second, SelectionPlatformStyle::Adaptive),
        &mut cx,
    );
    assert!(cx
        .ir
        .nodes
        .contains_key(&WidgetId::derived(region.as_u128(), &[0x5E1E, 1])));
    assert!(cx
        .ir
        .nodes
        .contains_key(&WidgetId::derived(region.as_u128(), &[0x5E1E, 2])));
    assert!(cx
        .ir
        .nodes
        .contains_key(&WidgetId::derived(region.as_u128(), &[0x5E1E, 3])));
    assert!(matches!(
        &cx.ir.nodes[&WidgetId::derived(region.as_u128(), &[0x5E1E, 11])].op,
        Op::Layout(LayoutOp::Positioned {
            left: Some(left),
            top: Some(top),
            ..
        }) if (*left - 3.0).abs() < 0.01 && (*top - 13.0).abs() < 0.01
    ));
    assert!(matches!(
        &cx.ir.nodes[&WidgetId::derived(region.as_u128(), &[0x5E1E, 12])].op,
        Op::Layout(LayoutOp::Positioned {
            left: Some(left),
            top: Some(top),
            ..
        }) if (*left - 23.0).abs() < 0.01 && (*top - 33.0).abs() < 0.01
    ));

    let mut desktop_cx = InternalLoweringCx::new(&env, &runtime, None, Some(&layout));
    lower_widget(
        &region_widget(region, first, second, SelectionPlatformStyle::Desktop),
        &mut desktop_cx,
    );
    assert!(!desktop_cx
        .ir
        .nodes
        .contains_key(&WidgetId::derived(region.as_u128(), &[0x5E1E, 1])));
    assert!(!desktop_cx
        .ir
        .nodes
        .contains_key(&WidgetId::derived(region.as_u128(), &[0x5E1E, 3])));
}

#[test]
fn desktop_secondary_click_selects_a_word_and_opens_the_toolbar() {
    let (ir, layout, region, first, _) = region_tree();
    let mut selectable_text = SelectableTextStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut context_menu = ContextMenuState::default();
    let mut scroll = ScrollStateMap::default();
    let mut text_edit = TextEditStateMap::default();
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(TestClipboard::default());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(FixedMeasurer);
    let mut ctx = context(
        &ir,
        &layout,
        &mut selectable_text,
        &mut interaction,
        &mut context_menu,
        &mut scroll,
        &mut text_edit,
        &mut gesture,
        &clipboard,
        &measurer,
        100,
    );
    let mut controller = SelectableTextController;
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Down {
            pointer_id: PointerId::MOUSE,
            kind: PointerKind::Mouse,
            point: LayoutPoint::new(20.0, 5.0),
            button: PointerButton::Secondary,
            modifiers: 0,
        }),
    ));
    assert_eq!(ctx.selectable_text.selection_range(first), Some((0, 5)));
    assert_eq!(ctx.context_menu.owner, Some(region));
    assert_eq!(ctx.interaction.focused, Some(region));
}

#[test]
fn selection_drag_near_a_scroll_edge_advances_the_nearest_viewport() {
    let (mut ir, mut layout, region, _, _) = region_tree();
    let scroll_id = WidgetId::explicit("selection.scroll");
    ir.add_node(
        scroll_id,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: false,
            width: Some(100.0),
            height: Some(40.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![region],
    );
    ir.set_root(scroll_id);
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 40.0),
            content_size: LayoutSize::new(100.0, 200.0),
        },
    );
    let mut selectable_text = SelectableTextStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut context_menu = ContextMenuState::default();
    let mut scroll = ScrollStateMap::default();
    let mut text_edit = TextEditStateMap::default();
    let mut gesture = GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(TestClipboard::default());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(FixedMeasurer);
    let mut ctx = context(
        &ir,
        &layout,
        &mut selectable_text,
        &mut interaction,
        &mut context_menu,
        &mut scroll,
        &mut text_edit,
        &mut gesture,
        &clipboard,
        &measurer,
        100,
    );
    let mut controller = SelectableTextController;
    assert!(controller.handle_event(
        &mut ctx,
        &pointer(PointerKind::Mouse, LayoutPoint::new(10.0, 5.0), true),
    ));
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Move {
            pointer_id: PointerId::MOUSE,
            kind: PointerKind::Mouse,
            point: LayoutPoint::new(30.0, 39.0),
            modifiers: 0,
        }),
    ));
    assert_eq!(ctx.scroll.get_offset(scroll_id), 18.0);
}
