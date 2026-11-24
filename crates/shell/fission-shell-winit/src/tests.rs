#[cfg(feature = "scene3d")]
use super::scene3d_hardware_adapter_supported;
use super::{
    animation_redraw_interval, build_window_attributes, clamp_copy_extent_to_texture,
    classify_web_text_value, collect_semantic_records, collect_startup_deep_links_from,
    cursor_icon_for, downscale_rgba_box, enqueue_bounded, fission_key_down_event,
    focused_text_input_id, handle_fill_text_selector, handle_scroll_into_view_selector,
    layout_size_to_image_dimensions, logical_viewport_to_physical_size,
    logical_viewport_to_render_target_size, magnification_scale_factor, map_test_pointer_id,
    map_test_pointer_kind, map_test_pointer_phase, map_test_scroll_delta_mode, map_touch_phase,
    native_window_size_for_logical_viewport, normalize_scale_factor, normalize_winit_scroll_delta,
    physical_position_to_layout_point, physical_size_to_layout_size, preferred_native_present_mode,
    preferred_surface_alpha_mode, preferred_web_surface_alpha_mode, prepare_capture_redraw,
    present_frame_with_winit_coordination, rect_visible_in_scroll_ancestors,
    repeating_animation_redraw_interval, resize_is_unsettled, resolve_build_viewport,
    resolve_selector_record, runtime_timer_wake_at, scroll_build_dependency_offsets,
    scroll_build_dependency_offsets_changed, should_defer_webgpu_redraw,
    should_present_startup_clear_frame, surface_acquire_recovery,
    sync_tracked_target_texture_size_to_surface, take_one, test_pump_can_ack_current_frame,
    text_input_blink_enabled, texture_plans_fit_device_limits, visual_rect_for_node,
    web_effective_render_scale, web_live_test_event_loop_wait_until,
    window_insets_from_safe_area_frames, windows_shell_execute_succeeded, windows_wide,
    BrowserDefaults, ControlFlow, FrameScheduleState, Instant, LiveResizeController,
    RedrawRequestTiming, SurfaceAcquireRecovery, TestPumpState, WebGpuQueueBackpressure,
    WindowViewportState,
};
use crate::pipeline::{software_frame_needs_raster, CompositorTexturePlan};
use crate::InvalidationSet;
use fission_core::authoring::BuildCtx;
use fission_core::event::{InputEvent, KeyCode, KeyEvent};
use fission_core::{
    Action as FissionAction, ActionId as FissionActionId, ActionRegistry, DeepLinkConfig,
    GlobalState, MotionPropertyId, PointerId, PointerKind, PointerPhase, ReducerContext,
    ResourceKey, Runtime, RuntimeResourceDeclaration, RuntimeResourceKind, ScrollDeltaMode,
    TextEditSource, TextValuePhase, TimerResource, UpdateTextInput, WidgetId,
};
use fission_core::{ActiveMotion, MotionEasing, MotionStateMap, MotionValue, ScrollStateMap};
use fission_ir::semantics::{ActionTrigger, MouseCursor};
use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, Role, Semantics};
use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize, LayoutSnapshot};
use fission_test_driver::{TestPointerKind, TestPointerPhase, TestResponse, TestScrollDeltaMode};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use wgpu::PresentMode;

#[derive(Debug, Default, Clone)]
struct TimerWakeState {
    ticks: u32,
}

impl GlobalState for TimerWakeState {}

#[test]
fn live_test_scroll_into_view_reveals_a_semantics_wrapper_without_layout_geometry() {
    let root = WidgetId::explicit("root");
    let scroll = WidgetId::explicit("document-scroll");
    let horizontal_scroll = WidgetId::explicit("document-horizontal-scroll");
    let page = WidgetId::explicit("page-frame");
    let semantic_page = WidgetId::explicit("page-semantic-region");
    let page_content = WidgetId::explicit("page-content");
    let box_op = || {
        Op::Layout(LayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        })
    };

    let mut ir = CoreIR::new();
    ir.add_node(page_content, box_op(), Vec::new());
    ir.add_node(
        semantic_page,
        Op::Semantics(Semantics {
            role: Role::Text,
            identifier: Some("document.page.2".into()),
            value: Some("Page 2/2".into()),
            ..Default::default()
        }),
        vec![page_content],
    );
    ir.add_node(page, box_op(), vec![semantic_page]);
    ir.add_node(
        horizontal_scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Row,
            show_scrollbar: true,
            width: Some(100.0),
            height: Some(100.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![page],
    );
    ir.add_node(
        scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: Some(100.0),
            height: Some(100.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![horizontal_scroll],
    );
    ir.add_node(root, box_op(), vec![scroll]);
    ir.set_root(root);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    for (id, rect, content_size) in [
        (
            root,
            LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            LayoutSize::new(100.0, 100.0),
        ),
        (
            scroll,
            LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            LayoutSize::new(100.0, 500.0),
        ),
        (
            horizontal_scroll,
            LayoutRect::new(0.0, 240.0, 100.0, 100.0),
            LayoutSize::new(400.0, 100.0),
        ),
        (
            page,
            LayoutRect::new(0.0, 250.0, 100.0, 60.0),
            LayoutSize::new(100.0, 60.0),
        ),
        (
            page_content,
            LayoutRect::new(10.0, 260.0, 80.0, 40.0),
            LayoutSize::new(80.0, 40.0),
        ),
    ] {
        snapshot
            .nodes
            .insert(id, LayoutNodeGeometry { rect, content_size });
    }

    let mut pipeline = crate::Pipeline::new();
    pipeline.prev_ir = Some(ir.clone());
    pipeline.last_snapshot = Some(snapshot.clone());
    let query =
        fission_test_driver::SelectorQuery::semantic_identifier("document.page.2").include_hidden();
    let mut runtime = Runtime::default();

    let (response, needs_scroll) =
        handle_scroll_into_view_selector(&query, &mut runtime, &pipeline);
    assert!(matches!(
        response,
        fission_test_driver::TestResponse::SelectorResolved { .. }
    ));
    assert!(needs_scroll);
    assert!(runtime.post_layout_hook(&ir, &snapshot));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 208.0);
    assert_eq!(
        runtime.runtime_state.scroll.get_offset(horizontal_scroll),
        0.0
    );

    let visible = resolve_selector_record(&pipeline, &runtime.runtime_state.scroll, &query)
        .expect("semantic page remains selectable after scrolling");
    assert_eq!(
        visible.node.visibility,
        fission_test_driver::VisibilityState::FullyVisible
    );
}

#[test]
fn only_changed_scroll_build_dependencies_request_tree_rebuilds() {
    let tracked_scroll = WidgetId::from_u128(80_001);
    let ordinary_scroll = WidgetId::from_u128(80_002);
    let mut ir = CoreIR::new();
    ir.scroll_build_dependencies.insert(tracked_scroll);

    let mut scroll = ScrollStateMap::default();
    let before = scroll_build_dependency_offsets(&ir, &scroll);

    scroll.set_offset(ordinary_scroll, 24.0);
    assert!(!scroll_build_dependency_offsets_changed(&before, &scroll));

    scroll.set_offset(tracked_scroll, 24.0);
    assert!(scroll_build_dependency_offsets_changed(&before, &scroll));
}

#[fission_macros::fission_reducer(ShellTimerWakeFired)]
fn shell_timer_wake_fired(state: &mut TimerWakeState, _ctx: &mut ReducerContext<TimerWakeState>) {
    state.ticks += 1;
}

#[test]
fn immediate_timer_schedules_a_wake_when_the_shell_would_be_idle() {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(TimerWakeState::default()))
        .expect("register timer wake state");

    let mut ctx = BuildCtx::new();
    let on_tick = fission_core::with_reducer!(ctx, ShellTimerWakeFired, shell_timer_wake_fired);
    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);

    let timer = TimerResource::new(
        ResourceKey::new("shell-idle-wake"),
        Duration::from_millis(1),
        (),
    )
    .immediate()
    .on_tick(on_tick);
    runtime
        .reconcile_resources(vec![RuntimeResourceDeclaration {
            key: "shell-idle-wake".into(),
            deps: timer.deps.clone(),
            policy: timer.policy,
            kind: RuntimeResourceKind::Timer(timer),
        }])
        .expect("register immediate timer");

    let last_frame_time = Instant::now();
    assert_eq!(
        runtime_timer_wake_at(&runtime, last_frame_time),
        Some(last_frame_time)
    );
}

#[test]
fn software_renderer_keeps_the_caret_static_unless_blink_is_explicitly_enabled() {
    assert!(!text_input_blink_enabled(None, true));
    assert!(text_input_blink_enabled(None, false));
    assert!(text_input_blink_enabled(Some("1"), true));
    assert!(text_input_blink_enabled(Some("true"), true));
    assert!(!text_input_blink_enabled(Some("0"), false));
    assert!(!text_input_blink_enabled(Some("false"), false));
}

#[test]
fn live_test_pump_reuses_the_current_frame_when_idle() {
    let idle = TestPumpState {
        has_current_frame: true,
        ..Default::default()
    };
    assert!(test_pump_can_ack_current_frame(idle));

    let mut pending = idle;
    pending.invalidations.mark_build();
    assert!(!test_pump_can_ack_current_frame(pending));

    let mut pending = idle;
    pending.resize_unsettled = true;
    assert!(!test_pump_can_ack_current_frame(pending));

    let mut pending = idle;
    pending.active_animation = true;
    assert!(!test_pump_can_ack_current_frame(pending));

    let mut pending = idle;
    pending.redraw_pending = true;
    assert!(!test_pump_can_ack_current_frame(pending));

    let mut pending = idle;
    pending.pending_screenshot = true;
    assert!(!test_pump_can_ack_current_frame(pending));

    assert!(!test_pump_can_ack_current_frame(TestPumpState::default()));
}

#[test]
fn key_down_preserves_produced_text_independently_of_logical_key() {
    let event = fission_key_down_event(KeyCode::Space, 0, Some("t"));

    assert_eq!(
        event,
        InputEvent::Keyboard(KeyEvent::DownWithText {
            key_code: KeyCode::Space,
            modifiers: 0,
            text: "t".to_owned(),
        })
    );
}

#[test]
fn key_down_preserves_multi_scalar_produced_text_once() {
    let event = fission_key_down_event(KeyCode::Char('e'), 0, Some("e\u{301}"));

    assert_eq!(
        event,
        InputEvent::Keyboard(KeyEvent::DownWithText {
            key_code: KeyCode::Char('e'),
            modifiers: 0,
            text: "e\u{301}".to_owned(),
        })
    );
}

#[test]
fn key_down_keeps_framework_navigation_keys_out_of_the_text_path() {
    for (key_code, produced_text) in [
        (KeyCode::Tab, "\t"),
        (KeyCode::Enter, "\r"),
        (KeyCode::Escape, "\u{1b}"),
        (KeyCode::Space, " "),
        (KeyCode::Left, "ignored"),
    ] {
        assert_eq!(
            fission_key_down_event(key_code.clone(), 0, Some(produced_text)),
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers: 0,
            })
        );
    }
}
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{MouseScrollDelta, TouchPhase};
use winit::window::CursorIcon;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LiveTextAction(String);

impl FissionAction for LiveTextAction {
    fn static_id() -> FissionActionId {
        FissionActionId::from_name("winit_live_test::LiveTextAction")
    }
}

#[derive(Debug, Default)]
struct LiveTextState {
    edit: Option<(String, UpdateTextInput)>,
}

impl GlobalState for LiveTextState {}

fn record_live_text(
    state: &mut LiveTextState,
    action: LiveTextAction,
    context: &mut ReducerContext<LiveTextState>,
) {
    state.edit = Some((
        action.0,
        context
            .input
            .text_change()
            .expect("live text input")
            .clone(),
    ));
}

fn live_text_runtime() -> Runtime {
    let mut runtime = Runtime::default().with_global_state(LiveTextState::default());
    let mut registry = ActionRegistry::<LiveTextState>::new();
    registry.register(
        record_live_text
            as fn(&mut LiveTextState, LiveTextAction, &mut ReducerContext<LiveTextState>),
    );
    runtime.absorb_registry(registry);
    runtime
}

fn live_text_pipeline(payload: Option<Vec<u8>>) -> (crate::Pipeline, WidgetId) {
    let target = WidgetId::explicit("live-text-input");
    let mut ir = CoreIR::new();
    let actions = payload
        .map(|payload_data| fission_ir::ActionSet {
            entries: vec![fission_ir::ActionEntry {
                trigger: ActionTrigger::TextChanged,
                action_id: LiveTextAction::static_id().as_u128(),
                payload_data: Some(payload_data),
            }],
        })
        .unwrap_or_default();
    ir.add_node(
        target,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            identifier: Some("live.text".into()),
            value: Some(String::new()),
            actions,
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.set_root(target);
    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 80.0));
    snapshot.nodes.insert(
        target,
        LayoutNodeGeometry {
            rect: LayoutRect::new(12.0, 12.0, 240.0, 40.0),
            content_size: LayoutSize::new(240.0, 40.0),
        },
    );
    let mut pipeline = crate::Pipeline::new();
    pipeline.prev_ir = Some(ir);
    pipeline.last_snapshot = Some(snapshot);
    (pipeline, target)
}

#[test]
fn initial_window_maximization_is_opt_in() {
    let default_attributes = build_window_attributes(
        "Fission",
        false,
        true,
        false,
        false,
        None,
        BrowserDefaults::NONE,
        None,
    )
    .unwrap();
    let maximized_attributes = build_window_attributes(
        "Fission",
        true,
        true,
        false,
        false,
        None,
        BrowserDefaults::NONE,
        None,
    )
    .unwrap();

    assert!(!default_attributes.maximized);
    assert!(maximized_attributes.maximized);
}

#[test]
fn windows_shell_arguments_are_utf16_nul_terminated() {
    let value = "https://example.com/projects/café";
    let encoded = windows_wide(value).expect("valid shell argument");

    assert_eq!(encoded.last(), Some(&0));
    assert_eq!(
        &encoded[..encoded.len() - 1],
        value.encode_utf16().collect::<Vec<_>>()
    );
    assert!(windows_wide("https://example.com/\0truncated").is_err());
}

#[test]
fn windows_shell_execute_status_uses_documented_success_boundary() {
    assert!(!windows_shell_execute_succeeded(0));
    assert!(!windows_shell_execute_succeeded(32));
    assert!(windows_shell_execute_succeeded(33));
}

#[test]
fn surface_alpha_mode_always_comes_from_the_supported_set() {
    use wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};

    assert_eq!(
        preferred_surface_alpha_mode(&[Opaque, Inherit]),
        Opaque,
        "opaque is the preferred portable fallback"
    );
    assert_eq!(
        preferred_surface_alpha_mode(&[Inherit]),
        Inherit,
        "the first advertised mode is used when preferred modes are absent"
    );
    assert_eq!(
        preferred_surface_alpha_mode(&[Opaque, PreMultiplied]),
        PreMultiplied
    );
    assert_eq!(
        preferred_surface_alpha_mode(&[Opaque, PostMultiplied]),
        PostMultiplied
    );
    assert_eq!(preferred_surface_alpha_mode(&[]), Opaque);
}

#[test]
fn web_surface_prefers_opaque_composition() {
    use wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};

    assert_eq!(
        preferred_web_surface_alpha_mode(&[PreMultiplied, Opaque, PostMultiplied]),
        Opaque
    );
    assert_eq!(
        preferred_web_surface_alpha_mode(&[PostMultiplied, PreMultiplied]),
        PreMultiplied
    );
    assert_eq!(preferred_web_surface_alpha_mode(&[Inherit]), Inherit);
    assert_eq!(preferred_web_surface_alpha_mode(&[]), Opaque);
}

#[test]
fn recoverable_surface_acquisition_errors_never_crash_the_app() {
    use super::SurfaceAcquireError;

    assert_eq!(
        surface_acquire_recovery(&SurfaceAcquireError::Lost),
        SurfaceAcquireRecovery::Reconfigure
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceAcquireError::Outdated),
        SurfaceAcquireRecovery::Reconfigure
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceAcquireError::Timeout),
        SurfaceAcquireRecovery::Retry
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceAcquireError::Occluded),
        SurfaceAcquireRecovery::Retry
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceAcquireError::Validation),
        SurfaceAcquireRecovery::Reconfigure
    );
}

#[test]
fn semantic_cursor_icons_map_to_winit_icons() {
    assert_eq!(cursor_icon_for(MouseCursor::Default), CursorIcon::Default);
    assert_eq!(cursor_icon_for(MouseCursor::Pointer), CursorIcon::Pointer);
    assert_eq!(cursor_icon_for(MouseCursor::Text), CursorIcon::Text);
    assert_eq!(
        cursor_icon_for(MouseCursor::NotAllowed),
        CursorIcon::NotAllowed
    );
    assert_eq!(
        cursor_icon_for(MouseCursor::VerticalText),
        CursorIcon::VerticalText
    );
}

#[test]
fn winit_scroll_delta_normalizes_to_positive_down_and_right() {
    assert_eq!(
        normalize_winit_scroll_delta(&MouseScrollDelta::LineDelta(-1.0, -2.0), 1.0),
        (50.0, 100.0, ScrollDeltaMode::Line)
    );
    assert_eq!(
        normalize_winit_scroll_delta(
            &MouseScrollDelta::PixelDelta(PhysicalPosition::new(-20.0, -40.0)),
            2.0,
        ),
        (10.0, 20.0, ScrollDeltaMode::Pixel)
    );
}

#[test]
fn pointer_signal_phases_and_test_vocabulary_map_without_loss() {
    assert_eq!(map_touch_phase(TouchPhase::Started), PointerPhase::Started);
    assert_eq!(
        map_touch_phase(TouchPhase::Cancelled),
        PointerPhase::Cancelled
    );
    assert_eq!(
        map_test_pointer_kind(TestPointerKind::Touch),
        PointerKind::Touch
    );
    assert_eq!(
        map_test_pointer_kind(TestPointerKind::Stylus),
        PointerKind::Stylus
    );
    assert_eq!(
        map_test_pointer_id(0, TestPointerKind::Mouse),
        PointerId::MOUSE
    );
    assert_ne!(
        map_test_pointer_id(0, TestPointerKind::Touch),
        PointerId::MOUSE
    );
    assert_eq!(
        map_test_pointer_phase(TestPointerPhase::Ended),
        PointerPhase::Ended
    );
    assert_eq!(
        map_test_scroll_delta_mode(TestScrollDeltaMode::Line),
        ScrollDeltaMode::Line
    );
}

#[test]
fn native_magnification_is_converted_to_a_safe_multiplicative_factor() {
    assert_eq!(magnification_scale_factor(0.25), 1.25);
    assert_eq!(magnification_scale_factor(-0.25), 0.75);
    assert_eq!(magnification_scale_factor(f64::NAN), 1.0);
    assert_eq!(magnification_scale_factor(-2.0), 1.0);
}

#[test]
fn physical_input_position_maps_into_layout_space() {
    let point = physical_position_to_layout_point(
        PhysicalPosition::new(240.0, 360.0),
        2.0,
        PhysicalPosition::new(0, 0),
    );
    assert_eq!(point, fission_render::LayoutPoint::new(120.0, 180.0));
}

#[test]
fn physical_input_position_subtracts_content_origin_before_scaling() {
    let point = physical_position_to_layout_point(
        PhysicalPosition::new(240.0, 460.0),
        2.0,
        PhysicalPosition::new(0, 100),
    );
    assert_eq!(point, fission_render::LayoutPoint::new(120.0, 180.0));
}

#[test]
fn safe_area_frames_convert_to_logical_window_insets() {
    let insets = window_insets_from_safe_area_frames(
        PhysicalPosition::new(0, 177),
        PhysicalPosition::new(0, 0),
        PhysicalSize::new(1206, 2343),
        PhysicalSize::new(1206, 2622),
        3.0,
    );

    assert_eq!(insets.left, 0.0);
    assert_eq!(insets.right, 0.0);
    assert_eq!(insets.top, 59.0);
    assert_eq!(insets.bottom, 34.0);
}

#[test]
fn visual_rect_subtracts_ancestor_scroll_offset() {
    let scroll = WidgetId::from_u128(1);
    let child = WidgetId::from_u128(2);
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Paint(fission_ir::PaintOp::DrawRect {
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            corner_radii: None,
            border_sides: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![child],
    );
    ir.set_root(scroll);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    snapshot.nodes.insert(
        scroll,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 400.0),
        },
    );
    snapshot.nodes.insert(
        child,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 150.0, 80.0, 20.0),
            content_size: LayoutSize::new(80.0, 20.0),
        },
    );
    let mut scroll_map = ScrollStateMap::default();
    scroll_map.set_offset(scroll, 120.0);

    let visual = visual_rect_for_node(&ir, &snapshot, &scroll_map, child).unwrap();
    assert_eq!(visual, LayoutRect::new(0.0, 30.0, 80.0, 20.0));
    assert!(rect_visible_in_scroll_ancestors(
        &ir,
        &snapshot,
        &scroll_map,
        child,
        visual
    ));
}

#[test]
fn repeating_animation_uses_reduced_frame_rate() {
    let min_frame = Duration::from_millis(16);
    let repeat_frame = Duration::from_millis(66);
    assert_eq!(
        animation_redraw_interval(false, Some(repeat_frame), false, min_frame),
        Some(repeat_frame)
    );
}

#[test]
fn finite_animation_keeps_full_frame_rate() {
    let min_frame = Duration::from_millis(16);
    assert_eq!(
        animation_redraw_interval(true, None, false, min_frame),
        Some(min_frame)
    );
    assert_eq!(
        animation_redraw_interval(false, None, true, min_frame),
        Some(min_frame)
    );
}

#[test]
fn idle_video_does_not_force_full_frame_rate() {
    let min_frame = Duration::from_millis(16);
    let repeat_frame = Duration::from_millis(66);
    assert_eq!(
        animation_redraw_interval(false, Some(repeat_frame), false, min_frame),
        Some(repeat_frame)
    );
}

#[test]
fn unchanged_software_frames_reuse_the_valid_render_target() {
    assert!(!software_frame_needs_raster(
        true,
        InvalidationSet::default(),
        0,
        false,
    ));
    assert!(software_frame_needs_raster(
        false,
        InvalidationSet::default(),
        0,
        false,
    ));

    let mut invalidations = InvalidationSet::default();
    invalidations.mark_composite();
    assert!(software_frame_needs_raster(true, invalidations, 0, false,));
    assert!(software_frame_needs_raster(
        true,
        InvalidationSet::default(),
        1,
        false,
    ));
    assert!(software_frame_needs_raster(
        true,
        InvalidationSet::default(),
        0,
        true,
    ));
}

#[test]
fn no_repeat_interval_means_no_idle_animation_redraw() {
    let min_frame = Duration::from_millis(16);
    assert_eq!(
        animation_redraw_interval(false, None, false, min_frame),
        None
    );
}

#[test]
fn redraw_requests_are_coalesced_until_the_frame_starts() {
    let mut schedule = FrameScheduleState::new(false);
    let mut redraw_requests = 0;
    let now = Instant::now();
    let min_frame = Duration::from_millis(16);

    assert_eq!(
        schedule.redraw_timing(now, now, min_frame),
        RedrawRequestTiming::DeferredUntil(now + min_frame)
    );

    assert!(schedule.request_redraw(|| redraw_requests += 1));
    assert!(!schedule.request_redraw(|| redraw_requests += 1));
    assert_eq!(redraw_requests, 1);
    assert_eq!(
        schedule.redraw_timing(now + min_frame, now, min_frame),
        RedrawRequestTiming::AlreadyScheduled
    );

    schedule.redraw_started();
    assert_eq!(
        schedule.redraw_timing(now + min_frame, now, min_frame),
        RedrawRequestTiming::Ready
    );
    assert!(schedule.request_redraw(|| redraw_requests += 1));
    assert_eq!(redraw_requests, 2);
}

#[test]
fn scheduled_redraw_waits_for_its_event_instead_of_rearming_a_timer() {
    let mut schedule = FrameScheduleState::new(false);
    let deadline = Instant::now() + Duration::from_millis(16);

    assert_eq!(
        schedule.wait_for_redraw_or_deadline(deadline),
        ControlFlow::WaitUntil(deadline)
    );
    assert!(schedule.mark_redraw_scheduled());
    assert_eq!(
        schedule.wait_for_redraw_or_deadline(deadline),
        ControlFlow::Wait
    );

    schedule.redraw_started();
    assert_eq!(
        schedule.wait_for_redraw_or_deadline(deadline),
        ControlFlow::WaitUntil(deadline)
    );
}

#[test]
fn web_live_test_event_loop_uses_a_short_timer_deadline() {
    let now = Instant::now();
    assert_eq!(
        web_live_test_event_loop_wait_until(now),
        ControlFlow::WaitUntil(now + Duration::from_millis(16))
    );
}

#[test]
fn queued_capture_survives_accepted_but_undelivered_proxy_event() {
    fn proxy_send_event() -> Result<(), ()> {
        Ok(())
    }

    let proxy_send_result = proxy_send_event();
    assert!(proxy_send_result.is_ok(), "the proxy accepts the event");
    let delivered_proxy_callbacks = 0;
    assert_eq!(delivered_proxy_callbacks, 0, "the event callback is lost");
    let mut capture_handler_calls = 0;

    let mut pending = VecDeque::new();
    let (first_response_tx, first_response_rx) = mpsc::channel();
    let (second_response_tx, second_response_rx) = mpsc::channel();
    assert!(enqueue_bounded(&mut pending, first_response_tx, 2).is_ok());
    assert!(enqueue_bounded(&mut pending, second_response_tx, 2).is_ok());

    // Model the regular LiveTest event-loop pass. The proxy callback never
    // arrives, but the bounded queue hands one request to the renderer at
    // a time and preserves later captures while the slot is busy.
    let first_capture = take_one(&mut pending).expect("first queued capture");
    capture_handler_calls += 1;
    let second_capture = take_one(&mut pending).expect("second queued capture");
    assert!(
        enqueue_bounded(&mut pending, second_capture, 2).is_ok(),
        "a busy renderer slot requeues the next capture"
    );

    first_capture
        .send(TestResponse::Screenshot {
            png_base64: "cG5n".into(),
            width: 16,
            height: 9,
        })
        .expect("first capture response receiver remains connected");
    let second_capture = take_one(&mut pending).expect("requeued capture");
    capture_handler_calls += 1;
    second_capture
        .send(TestResponse::Screenshot {
            png_base64: "cG5n".into(),
            width: 16,
            height: 9,
        })
        .expect("second capture response receiver remains connected");
    assert!(
        take_one(&mut pending).is_none(),
        "captures drain exactly once"
    );
    assert_eq!(capture_handler_calls, 2, "each request is handled once");

    for response_rx in [first_response_rx, second_response_rx] {
        match response_rx.try_recv().expect("screenshot response") {
            TestResponse::Screenshot {
                png_base64,
                width,
                height,
            } => {
                assert_eq!(png_base64, "cG5n");
                assert_eq!((width, height), (16, 9));
            }
            other => panic!("unexpected capture response: {other:?}"),
        }
    }

    let (overflow_tx, overflow_rx) = mpsc::channel();
    let rejected =
        enqueue_bounded(&mut pending, overflow_tx, 0).expect_err("queue capacity is enforced");
    drop(rejected);
    assert!(matches!(
        overflow_rx.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn web_test_poll_markers_are_classified_as_high_frequency() {
    assert!(super::web_test_diagnostic_is_poll_stage(430));
    assert!(super::web_test_diagnostic_is_poll_stage(432));
    assert!(super::web_test_diagnostic_is_poll_stage(435));
    assert!(!super::web_test_diagnostic_is_poll_stage(417));
}

#[test]
fn webgpu_redraw_is_deferred_only_for_outstanding_gpu_work() {
    assert!(!should_defer_webgpu_redraw(false, true));
    assert!(!should_defer_webgpu_redraw(true, false));
    assert!(should_defer_webgpu_redraw(true, true));
}

#[test]
fn webgpu_queue_completion_wakes_once_for_coalesced_redraws() {
    let state = WebGpuQueueBackpressure::default();
    assert!(!state.complete_work(), "no deferred redraw means no wake");

    state.mark_submitted();
    state.defer_redraw();
    state.defer_redraw();
    assert!(state.is_work_in_flight());
    assert!(
        state.complete_work(),
        "one wake releases the coalesced redraw"
    );
    assert!(
        !state.complete_work(),
        "the deferred redraw is consumed once"
    );
    assert!(!state.is_work_in_flight());
}

#[test]
fn webgpu_queue_age_marker_ignores_completed_or_superseded_submissions() {
    let state = WebGpuQueueBackpressure::default();
    let first_generation = state.mark_submitted();
    assert!(state.is_submission_in_flight(first_generation));
    assert!(!state.complete_work());
    assert!(!state.is_submission_in_flight(first_generation));

    let second_generation = state.mark_submitted();
    assert_ne!(first_generation, second_generation);
    assert!(!state.is_submission_in_flight(first_generation));
    assert!(state.is_submission_in_flight(second_generation));
    state.mark_device_lost();
    assert!(!state.is_submission_in_flight(second_generation));
}

#[test]
fn webgpu_completion_can_enqueue_after_wake_cycle_waits_for_gpu() {
    let wake_gate = AtomicBool::new(true);
    let mut wake_dispatched_in_cycle = true;
    assert!(super::release_web_wake_gate_after_event_cycle(
        &mut wake_dispatched_in_cycle,
        &wake_gate,
    ));
    assert!(!wake_gate.load(Ordering::Acquire));
    assert!(!wake_dispatched_in_cycle);

    let state = WebGpuQueueBackpressure::default();
    state.mark_submitted();
    state.defer_redraw();
    assert!(
        state.complete_work(),
        "GPU completion needs a deferred wake"
    );
    assert!(
        !wake_gate.swap(true, Ordering::AcqRel),
        "the completion wake must not be suppressed as a duplicate"
    );
}

#[test]
fn webgpu_device_loss_clears_queue_and_surfaces_recovery_once() {
    let state = WebGpuQueueBackpressure::default();
    state.mark_submitted();
    state.defer_redraw();

    state.mark_device_lost();

    assert!(!state.is_work_in_flight());
    assert!(state.take_device_lost());
    assert!(!state.take_device_lost());
    assert!(
        !state.complete_work(),
        "device-loss recovery consumes the wake"
    );
}

#[test]
fn screenshot_redraw_restarts_a_stale_coalesced_frame() {
    let mut schedule = FrameScheduleState::new(false);
    let mut redraw_pending = true;
    assert!(schedule.mark_redraw_scheduled());

    prepare_capture_redraw(&mut redraw_pending, &mut schedule);

    assert!(!redraw_pending);
    assert!(!schedule.redraw_scheduled);
    assert!(schedule.mark_redraw_scheduled());
}

#[test]
fn repeat_animation_interval_uses_low_priority_hint() {
    let mut animation = MotionStateMap::default();
    animation.active.insert(
        (WidgetId::explicit("spinner"), MotionPropertyId::opacity()),
        ActiveMotion {
            target: WidgetId::explicit("spinner"),
            property: MotionPropertyId::opacity(),
            start_value: MotionValue::Scalar(0.3),
            end_value: MotionValue::Scalar(1.0),
            start_time: 0,
            duration: 600,
            repeat: true,
            frame_interval_ms: Some(166),
            easing: MotionEasing::Linear,
        },
    );
    assert_eq!(
        repeating_animation_redraw_interval(&animation, Duration::from_millis(66)),
        Some(Duration::from_millis(166))
    );
}

#[test]
fn repeat_animation_interval_chooses_fastest_active_repeat() {
    let mut animation = MotionStateMap {
        values: HashMap::new(),
        active: HashMap::new(),
        ..Default::default()
    };
    animation.active.insert(
        (WidgetId::explicit("slow"), MotionPropertyId::opacity()),
        ActiveMotion {
            target: WidgetId::explicit("slow"),
            property: MotionPropertyId::opacity(),
            start_value: MotionValue::Scalar(0.3),
            end_value: MotionValue::Scalar(1.0),
            start_time: 0,
            duration: 600,
            repeat: true,
            frame_interval_ms: Some(200),
            easing: MotionEasing::Linear,
        },
    );
    animation.active.insert(
        (WidgetId::explicit("fast"), MotionPropertyId::opacity()),
        ActiveMotion {
            target: WidgetId::explicit("fast"),
            property: MotionPropertyId::opacity(),
            start_value: MotionValue::Scalar(0.3),
            end_value: MotionValue::Scalar(1.0),
            start_time: 0,
            duration: 600,
            repeat: true,
            frame_interval_ms: Some(100),
            easing: MotionEasing::Linear,
        },
    );
    assert_eq!(
        repeating_animation_redraw_interval(&animation, Duration::from_millis(66)),
        Some(Duration::from_millis(100))
    );
}

#[test]
fn live_resize_reports_unsettled_until_deadline() {
    let settle = Duration::from_millis(90);
    let mut resize = LiveResizeController::new(settle);
    let now = std::time::Instant::now();
    resize.note_resize(now);

    assert!(resize.is_live(now + Duration::from_millis(30)));
    assert!(resize_is_unsettled(
        false,
        false,
        resize.is_live(now + Duration::from_millis(30))
    ));
    assert!(!resize.is_live(now + Duration::from_millis(95)));
}

#[test]
fn viewport_resize_forces_build_viewport_refresh() {
    let target = LayoutSize::new(1440.0, 900.0);
    let mut invalidations = InvalidationSet::default();

    let build_viewport = resolve_build_viewport(
        Some(LayoutSize::new(1024.0, 768.0)),
        target,
        true,
        &mut invalidations,
    );

    assert!(invalidations.build);
    assert_eq!(build_viewport, target);
}

#[test]
fn stable_viewport_preserves_existing_build_viewport() {
    let target = LayoutSize::new(1024.0, 768.0);
    let mut invalidations = InvalidationSet::default();

    let build_viewport = resolve_build_viewport(Some(target), target, true, &mut invalidations);

    assert!(!invalidations.build);
    assert_eq!(build_viewport, target);
}

#[test]
fn oversized_texture_plan_forces_scene_fallback() {
    let plans = vec![CompositorTexturePlan {
        key: 1,
        bounds: LayoutRect::new(0.0, 0.0, 320.0, 9000.0),
        scene: Some(fission_render::RenderScene::new(LayoutRect::new(
            0.0, 0.0, 320.0, 9000.0,
        ))),
        scene_cache_key: Some(1),
        content_key: 1,
        local_dynamic: false,
        composite_dynamic: false,
        opacity: 1.0,
        transform: None,
        transform_clip: false,
        clip: None,
        children: Vec::new(),
        source_layer_path: None,
    }];
    assert!(!texture_plans_fit_device_limits(&plans, 1.0, 8192));
}

#[test]
fn nested_texture_plans_must_all_fit_device_limits() {
    let child = CompositorTexturePlan {
        key: 2,
        bounds: LayoutRect::new(0.0, 0.0, 400.0, 8400.0),
        scene: Some(fission_render::RenderScene::new(LayoutRect::new(
            0.0, 0.0, 400.0, 8400.0,
        ))),
        scene_cache_key: Some(2),
        content_key: 2,
        local_dynamic: false,
        composite_dynamic: false,
        opacity: 1.0,
        transform: None,
        transform_clip: false,
        clip: None,
        children: Vec::new(),
        source_layer_path: None,
    };
    let plans = vec![CompositorTexturePlan {
        key: 1,
        bounds: LayoutRect::new(0.0, 0.0, 800.0, 600.0),
        scene: None,
        scene_cache_key: None,
        content_key: 3,
        local_dynamic: false,
        composite_dynamic: false,
        opacity: 1.0,
        transform: None,
        transform_clip: false,
        clip: None,
        children: vec![child],
        source_layer_path: None,
    }];
    assert!(!texture_plans_fit_device_limits(&plans, 1.0, 8192));
}

#[test]
fn screenshot_dimensions_follow_logical_viewport() {
    let dims = layout_size_to_image_dimensions(fission_layout::LayoutSize::new(1600.0, 1200.0));
    assert_eq!(dims, (1600, 1200));

    let rounded = layout_size_to_image_dimensions(fission_layout::LayoutSize::new(999.6, 700.4));
    assert_eq!(rounded, (1000, 700));
}

#[test]
fn linux_wayland_defers_startup_clear_to_the_first_redraw() {
    assert!(!should_present_startup_clear_frame(true));
    assert!(should_present_startup_clear_frame(false));
}

#[test]
fn linux_wayland_uses_mailbox_only_when_the_surface_supports_it() {
    let supported = [
        PresentMode::Fifo,
        PresentMode::Mailbox,
        PresentMode::Immediate,
    ];

    assert_eq!(
        preferred_native_present_mode(&supported, true),
        PresentMode::Mailbox
    );
    assert_eq!(
        preferred_native_present_mode(&supported, false),
        PresentMode::AutoVsync
    );
    assert_eq!(
        preferred_native_present_mode(&[PresentMode::Fifo], true),
        PresentMode::AutoVsync
    );
}

#[test]
fn non_wayland_native_surface_presentation_notifies_winit_before_commit() {
    let operations = RefCell::new(Vec::new());

    present_frame_with_winit_coordination(
        false,
        || operations.borrow_mut().push("pre_present_notify"),
        || operations.borrow_mut().push("commit_surface_frame"),
    );

    assert_eq!(
        operations.into_inner(),
        vec!["pre_present_notify", "commit_surface_frame"]
    );
}

#[test]
fn linux_wayland_surface_presentation_does_not_install_a_frame_callback() {
    let operations = RefCell::new(Vec::new());

    present_frame_with_winit_coordination(
        true,
        || operations.borrow_mut().push("pre_present_notify"),
        || operations.borrow_mut().push("commit_surface_frame"),
    );

    assert_eq!(operations.into_inner(), vec!["commit_surface_frame"]);
}

#[test]
fn simulated_resize_uses_physical_render_target_size() {
    let dims = logical_viewport_to_render_target_size(
        fission_layout::LayoutSize::new(1600.0, 1200.0),
        2.0,
    );
    assert_eq!(dims, (3200, 2400));

    let fractional =
        logical_viewport_to_render_target_size(fission_layout::LayoutSize::new(430.0, 900.0), 1.5);
    assert_eq!(fractional, (645, 1350));
}

#[test]
fn physical_viewport_maps_to_logical_size_with_scale_factor() {
    let logical = physical_size_to_layout_size(PhysicalSize::new(1728, 1117), 1.5);
    assert_eq!(logical.width, 1152.0);
    assert!((logical.height - 744.6667).abs() < 0.001);
}

#[test]
fn scale_factor_change_preserves_logical_viewport_until_resize_arrives() {
    let viewport = WindowViewportState {
        physical_size: PhysicalSize::new(1600, 1200),
        scale_factor: 1.0,
    }
    .with_scale_factor(2.0);

    assert_eq!(viewport.physical_size, PhysicalSize::new(3200, 2400));
    assert_eq!(
        viewport.logical_size(),
        fission_layout::LayoutSize::new(1600.0, 1200.0)
    );
}

#[test]
fn resized_event_overrides_scale_factor_prediction_authoritatively() {
    let viewport = WindowViewportState {
        physical_size: PhysicalSize::new(1600, 1200),
        scale_factor: 1.0,
    }
    .with_scale_factor(1.5)
    .with_physical_size(PhysicalSize::new(2412, 1809));

    assert_eq!(viewport.physical_size, PhysicalSize::new(2412, 1809));
    assert_eq!(
        viewport.logical_size(),
        fission_layout::LayoutSize::new(1608.0, 1206.0)
    );
}

#[test]
fn fractional_logical_viewports_round_up_for_render_targets() {
    let physical =
        logical_viewport_to_physical_size(fission_layout::LayoutSize::new(430.2, 900.1), 1.5);
    assert_eq!(physical, PhysicalSize::new(646, 1351));
}

#[test]
fn web_complete_text_values_preserve_platform_source_and_phase() {
    assert_eq!(
        classify_web_text_value("insertFromComposition"),
        (TextEditSource::Ime, TextValuePhase::CompositionCommitted)
    );
    assert_eq!(
        classify_web_text_value("insertReplacementText"),
        (TextEditSource::Autocorrect, TextValuePhase::Committed)
    );
    assert_eq!(
        classify_web_text_value("insertFromAutoFill"),
        (TextEditSource::Autofill, TextValuePhase::Committed)
    );
}

#[test]
fn web_render_scale_preserves_normal_retina_viewports() {
    let scale = web_effective_render_scale(fission_layout::LayoutSize::new(1440.0, 900.0), 2.0);
    assert_eq!(scale, 2.0);
}

#[test]
fn web_render_scale_bounds_ultrawide_retina_backing_store() {
    let logical = fission_layout::LayoutSize::new(3440.0, 1440.0);
    let scale = web_effective_render_scale(logical, 2.0);
    let physical = logical_viewport_to_physical_size(logical, scale);

    assert!(scale > 1.0 && scale < 2.0);
    assert!((physical.width as u64) * (physical.height as u64) <= 8_400_000);
    assert!((physical.width as f64 / scale - 3440.0).abs() < 1.0);
    assert!((physical.height as f64 / scale - 1440.0).abs() < 1.0);
}

#[test]
fn scale_factor_prediction_never_undershoots_fractional_viewports() {
    let initial = WindowViewportState {
        physical_size: PhysicalSize::new(1728, 1117),
        scale_factor: 1.5,
    };
    let predicted = initial.with_scale_factor(2.0);

    assert_eq!(predicted.physical_size, PhysicalSize::new(2304, 1490));
    assert!(predicted.logical_size().width >= initial.logical_size().width);
    assert!(predicted.logical_size().height >= initial.logical_size().height);
}

#[test]
fn logical_resize_updates_native_viewport_prediction() {
    let initial = WindowViewportState {
        physical_size: PhysicalSize::new(800, 632),
        scale_factor: 2.0,
    };
    let resized = initial.with_logical_size(fission_layout::LayoutSize::new(1600.0, 1200.0));

    assert_eq!(resized.physical_size, PhysicalSize::new(3200, 2400));
    assert_eq!(
        resized.logical_size(),
        fission_layout::LayoutSize::new(1600.0, 1200.0)
    );
}

#[test]
fn logical_resize_requests_logical_window_dimensions() {
    let requested =
        native_window_size_for_logical_viewport(fission_layout::LayoutSize::new(1600.0, 2200.0));

    assert_eq!(requested.width, 1600.0);
    assert_eq!(requested.height, 2200.0);
}

#[test]
fn invalid_scale_factors_fall_back_to_unit_scale() {
    assert_eq!(normalize_scale_factor(0.0), 1.0);
    assert_eq!(normalize_scale_factor(-2.0), 1.0);
    assert_eq!(normalize_scale_factor(f64::NAN), 1.0);
    assert_eq!(normalize_scale_factor(f64::INFINITY), 1.0);
    assert_eq!(normalize_scale_factor(1.5), 1.5);
}

#[test]
fn invalid_scale_factor_does_not_shrink_viewport_math() {
    let logical = physical_size_to_layout_size(PhysicalSize::new(1600, 1200), 0.0);
    assert_eq!(logical, fission_layout::LayoutSize::new(1600.0, 1200.0));

    let render_target = logical_viewport_to_render_target_size(
        fission_layout::LayoutSize::new(1600.0, 1200.0),
        0.0,
    );
    assert_eq!(render_target, (1600, 1200));
}

#[test]
fn surface_resize_resets_custom_target_texture_tracking() {
    let mut tracked_target_texture_size = (1600, 1200);

    sync_tracked_target_texture_size_to_surface(
        &mut tracked_target_texture_size,
        PhysicalSize::new(1055, 791),
    );

    assert_eq!(tracked_target_texture_size, (1055, 791));
    assert_ne!(
        tracked_target_texture_size,
        logical_viewport_to_render_target_size(
            fission_layout::LayoutSize::new(1600.0, 1200.0),
            1.0,
        )
    );
}

#[test]
fn resize_settle_signal_tracks_real_resize_state() {
    assert!(resize_is_unsettled(true, false, false));
    assert!(resize_is_unsettled(false, true, false));
    assert!(resize_is_unsettled(false, false, true));
    assert!(!resize_is_unsettled(false, false, false));
}

#[cfg(feature = "scene3d")]
#[test]
fn scene3d_refuses_software_adapters() {
    assert!(!scene3d_hardware_adapter_supported(wgpu::DeviceType::Cpu));
    assert!(scene3d_hardware_adapter_supported(
        wgpu::DeviceType::IntegratedGpu
    ));
    assert!(scene3d_hardware_adapter_supported(
        wgpu::DeviceType::DiscreteGpu
    ));
}

#[test]
fn screenshot_copy_extent_never_exceeds_texture_bounds() {
    assert_eq!(
        clamp_copy_extent_to_texture(1600, 1200, 1055, 791),
        (1055, 791)
    );
    assert_eq!(clamp_copy_extent_to_texture(0, 0, 1055, 791), (1, 1));
    assert_eq!(
        clamp_copy_extent_to_texture(640, 480, 1055, 791),
        (640, 480)
    );
}

#[test]
fn integer_downscale_uses_fast_box_path() {
    let rgba = vec![
        10, 20, 30, 255, 30, 40, 50, 255, 50, 60, 70, 255, 70, 80, 90, 255,
    ];
    let downscaled = downscale_rgba_box(&rgba, 2, 2, 1, 1).expect("downscale");
    assert_eq!(downscaled, vec![40, 50, 60, 255]);
}

#[test]
fn startup_deep_link_collection_filters_to_declared_config() {
    let config = DeepLinkConfig::new()
        .scheme("fission")
        .domain("example.com")
        .path_prefix("/tasks");

    let links = collect_startup_deep_links_from(
        &config,
        vec![
            "--ignored".to_string(),
            "fission://open/tasks/1".to_string(),
            "other://open/tasks/1".to_string(),
        ],
        vec!["https://example.com/tasks/2?source=email".to_string()],
    );

    assert_eq!(links.len(), 2);
    assert_eq!(links[0].url, "https://example.com/tasks/2?source=email");
    assert!(links[0].cold_start);
    assert_eq!(links[1].url, "fission://open/tasks/1");
    assert!(links[1].cold_start);
}

mod semantics;
