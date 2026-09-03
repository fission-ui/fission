use fission_core::event::{KeyCode, KeyEvent, PointerButton, PointerEvent, PointerId, PointerKind};
use fission_core::input::range_slider::{
    prepare_range_slider_change, RangeSliderChangeSource, RangeSliderController,
    RangeSliderControllerContext, RangeSliderStateMap, RangeSliderThumb,
};
use fission_core::internal::{build_layout_tree, lower_widget};
use fission_core::{
    ActionEnvelope, ActionId, ActionInput, Env, InputEvent, InteractionStateMap, RuntimeState,
    ScrollStateMap,
};
use fission_ir::{ActionTrigger, Op, Role, WidgetId};
use fission_layout::{LayoutEngine, LayoutPoint, LayoutSize};
use fission_widgets::RangeSlider;

fn callback() -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name("range.changed"),
        payload: br#"{"filter":"price"}"#.to_vec(),
    }
}

fn lower(range: RangeSlider) -> (fission_ir::CoreIR, fission_layout::LayoutSnapshot, WidgetId) {
    let expected_root = range.id.expect("test range has an explicit id");
    let widget = range.into();
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = fission_core::internal::InternalLoweringCx::new(&env, &runtime, None, None);
    let root = lower_widget(&widget, &mut cx);
    cx.ir.root = Some(root);
    let input = build_layout_tree(&cx.ir, &env);
    let mut engine = LayoutEngine::new();
    engine.rebuild(&input).unwrap();
    let snapshot = engine
        .compute_layout(&input, root, LayoutSize::new(400.0, 80.0), &|_| 0.0)
        .unwrap();
    assert_eq!(root, expected_root);
    (cx.ir, snapshot, root)
}

fn range() -> RangeSlider {
    RangeSlider {
        id: Some(WidgetId::explicit("price-range")),
        semantics_identifier: Some("filters.price".into()),
        start: 20.0,
        end: 80.0,
        min: 0.0,
        max: 100.0,
        step: Some(5.0),
        on_change: Some(callback()),
    }
}

#[test]
fn ordinary_instances_receive_distinct_implicit_control_ids() {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let view = fission_core::View::new(&(), &runtime, &env, None);
    let mut build_ctx = fission_core::internal::BuildCtx::<()>::new();
    let widgets = fission_core::build::enter(&mut build_ctx, &view, || {
        vec![RangeSlider::default().into(), RangeSlider::default().into()]
    });
    let mut cx = fission_core::internal::InternalLoweringCx::new(&env, &runtime, None, None);
    let first = lower_widget(&widgets[0], &mut cx);
    let second = lower_widget(&widgets[1], &mut cx);
    assert_ne!(first, second);
}

#[test]
fn lowering_exposes_one_range_with_two_stable_thumb_semantics() {
    let (ir, _, root) = lower(range());
    let sliders = ir
        .nodes
        .iter()
        .filter_map(|(id, node)| match &node.op {
            Op::Semantics(semantics) if semantics.role == Role::Slider => Some((*id, semantics)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sliders.len(), 2);

    let start = sliders
        .iter()
        .find(|(_, semantics)| semantics.identifier.as_deref() == Some("filters.price.start"))
        .expect("lower thumb semantics");
    let end = sliders
        .iter()
        .find(|(_, semantics)| semantics.identifier.as_deref() == Some("filters.price.end"))
        .expect("upper thumb semantics");
    assert_eq!(start.1.current_value, Some(20.0));
    assert_eq!(start.1.min_value, Some(0.0));
    assert_eq!(start.1.max_value, Some(80.0));
    assert_eq!(end.1.current_value, Some(80.0));
    assert_eq!(end.1.min_value, Some(20.0));
    assert_eq!(end.1.max_value, Some(100.0));
    for (_, semantics) in [&start, &end] {
        let change = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == ActionTrigger::Change)
            .expect("thumb change action");
        assert_eq!(
            change.payload_data.as_deref(),
            Some(callback().payload.as_slice())
        );
    }
    let (owner, _, _) = prepare_range_slider_change(
        &ir,
        start.0,
        30.0,
        RangeSliderChangeSource::Accessibility,
        0,
    )
    .expect("thumb belongs to range control");
    assert_eq!(owner, root);
}

#[test]
fn pointer_capture_updates_both_thumbs_and_prevents_crossing() {
    let (ir, snapshot, root) = lower(range());
    let rect = snapshot.get_node_rect(root).expect("range layout");
    let scroll = ScrollStateMap::default();
    let viewport = fission_core::ViewportStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut state = RangeSliderStateMap::default();

    let mut ctx = RangeSliderControllerContext {
        ir: &ir,
        layout: &snapshot,
        scroll: &scroll,
        viewport: &viewport,
        interaction: &mut interaction,
        state: &mut state,
        dispatched_actions: Vec::new(),
    };
    assert!(RangeSliderController.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Down {
            pointer_id: PointerId::MOUSE,
            kind: PointerKind::Mouse,
            point: LayoutPoint::new(
                rect.x() + rect.width() * 0.1,
                rect.y() + rect.height() / 2.0
            ),
            button: PointerButton::Primary,
            modifiers: 1,
        }),
    ));
    let (_, envelope, input) = ctx.dispatched_actions.last().expect("lower proposal");
    assert_eq!(envelope.payload, callback().payload);
    let change = input.range_slider_change().expect("typed range input");
    assert_eq!(change.active_thumb, RangeSliderThumb::Start);
    assert_eq!((change.start, change.end), (10.0, 80.0));
    assert_eq!(change.modifiers, 1);

    // The pointer is beyond the range bounds, but capture keeps routing it to
    // the lower thumb. Non-crossing policy clamps it to the upper value.
    assert!(RangeSliderController.handle_event(
        &mut ctx,
        &InputEvent::Pointer(PointerEvent::Move {
            pointer_id: PointerId::MOUSE,
            kind: PointerKind::Mouse,
            point: LayoutPoint::new(rect.right() + 200.0, rect.y()),
            modifiers: 0,
        }),
    ));
    let change = ctx
        .dispatched_actions
        .last()
        .and_then(|(_, _, input)| input.range_slider_change())
        .expect("captured proposal");
    assert_eq!((change.start, change.end), (80.0, 80.0));
}

#[test]
fn keyboard_and_accessibility_preserve_payload_and_use_typed_input() {
    let (ir, snapshot, _) = lower(range());
    let end_id = ir
        .nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Semantics(semantics)
                if semantics.identifier.as_deref() == Some("filters.price.end") =>
            {
                Some(*id)
            }
            _ => None,
        })
        .unwrap();
    let scroll = ScrollStateMap::default();
    let viewport = fission_core::ViewportStateMap::default();
    let mut interaction = InteractionStateMap::default();
    interaction.set_focused(Some(end_id));
    let mut state = RangeSliderStateMap::default();
    let mut ctx = RangeSliderControllerContext {
        ir: &ir,
        layout: &snapshot,
        scroll: &scroll,
        viewport: &viewport,
        interaction: &mut interaction,
        state: &mut state,
        dispatched_actions: Vec::new(),
    };
    assert!(RangeSliderController.handle_event(
        &mut ctx,
        &InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Right,
            modifiers: 4,
        }),
    ));
    let (_, envelope, input) = ctx.dispatched_actions.last().unwrap();
    assert_eq!(envelope.payload, callback().payload);
    let change = input.range_slider_change().unwrap();
    assert_eq!(change.active_thumb, RangeSliderThumb::End);
    assert_eq!((change.start, change.end), (20.0, 85.0));
    assert_eq!(change.source, RangeSliderChangeSource::Keyboard);
    assert_eq!(change.modifiers, 4);

    let (_, envelope, input) = prepare_range_slider_change(
        &ir,
        end_id,
        -100.0,
        RangeSliderChangeSource::Accessibility,
        0,
    )
    .unwrap();
    assert_eq!(envelope.payload, callback().payload);
    let change = match input.unscoped() {
        ActionInput::RangeSliderChanged(change) => change,
        other => panic!("expected range input, got {other:?}"),
    };
    assert_eq!(change.active_thumb, RangeSliderThumb::End);
    assert_eq!((change.start, change.end), (20.0, 20.0));
    assert_eq!(change.source, RangeSliderChangeSource::Accessibility);
}

#[test]
fn snapping_is_clamped_after_rounding_at_an_uneven_maximum() {
    let mut uneven = range();
    uneven.start = 0.0;
    uneven.end = 1.0;
    uneven.min = 0.0;
    uneven.max = 1.0;
    uneven.step = Some(0.6);
    let (ir, _, _) = lower(uneven);
    let end_id = ir
        .nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Semantics(semantics)
                if semantics.identifier.as_deref() == Some("filters.price.end") =>
            {
                Some(*id)
            }
            _ => None,
        })
        .unwrap();
    let (_, _, input) =
        prepare_range_slider_change(&ir, end_id, 1.0, RangeSliderChangeSource::Accessibility, 0)
            .unwrap();
    assert_eq!(input.range_slider_change().unwrap().end, 1.0);
}

#[test]
fn stable_hash_tracks_fractional_values_bounds_step_and_payload() {
    fn stable_hash(mut range: RangeSlider) -> u64 {
        range.id = Some(WidgetId::explicit("stable-range"));
        let (ir, _, root) = lower(range);
        match &ir.nodes.get(&root).unwrap().op {
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash }) => *stable_hash,
            other => panic!("expected custom structural root, got {other:?}"),
        }
    }

    let base = stable_hash(range());
    let mut fractional = range();
    fractional.start = 20.25;
    assert_ne!(base, stable_hash(fractional));
    let mut bounds = range();
    bounds.max = 200.0;
    assert_ne!(base, stable_hash(bounds));
    let mut step = range();
    step.step = Some(2.5);
    assert_ne!(base, stable_hash(step));
    let mut payload = range();
    payload.on_change.as_mut().unwrap().payload.push(9);
    assert_ne!(base, stable_hash(payload));
}
