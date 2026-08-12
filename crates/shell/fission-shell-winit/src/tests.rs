#[cfg(not(target_arch = "wasm32"))]
use super::should_auto_select_native_skia_raster;
use super::wgpu::PresentMode;
use super::{
    animation_redraw_interval, build_window_attributes, clamp_copy_extent_to_texture,
    collect_semantic_records, collect_startup_deep_links_from, cursor_icon_for, downscale_rgba_box,
    layout_size_to_image_dimensions, logical_viewport_to_physical_size,
    logical_viewport_to_render_target_size, native_window_size_for_logical_viewport,
    normalize_scale_factor, normalize_winit_scroll_delta, physical_position_to_layout_point,
    physical_size_to_layout_size, preferred_native_present_mode, preferred_surface_alpha_mode,
    present_frame_with_winit_coordination, rect_visible_in_scroll_ancestors,
    repeating_animation_redraw_interval, resize_is_unsettled, resolve_build_viewport,
    resolve_selector_record, rgba_screenshot, should_present_startup_clear_frame,
    surface_acquire_recovery, sync_tracked_target_texture_size_to_surface,
    texture_plans_fit_device_limits, visual_rect_for_node, window_insets_from_safe_area_frames,
    windows_shell_execute_succeeded, windows_wide, LiveResizeController, SurfaceAcquireRecovery,
    WindowViewportState, WinitPresenter,
};
use crate::pipeline::CompositorTexturePlan;
use crate::renderer_diagnostics::RendererRequest;
use crate::InvalidationSet;
use fission_core::{ActiveMotion, MotionEasing, MotionStateMap, MotionValue, ScrollStateMap};
use fission_core::{DeepLinkConfig, MotionPropertyId, WidgetId};
use fission_ir::semantics::MouseCursor;
use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, Role, Semantics};
use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize, LayoutSnapshot};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::MouseScrollDelta;
use winit::window::CursorIcon;

#[test]
fn native_presenter_starts_detached() {
    let presenter = WinitPresenter::detached();
    assert!(!presenter.is_attached());
}

#[test]
fn initial_window_maximization_is_opt_in() {
    let default_attributes =
        build_window_attributes("Fission", false, false, false, None, None).unwrap();
    let maximized_attributes =
        build_window_attributes("Fission", true, false, false, None, None).unwrap();

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
    use super::wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};

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

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn windows_auto_uses_skia_raster_for_cpu_and_warp_adapters() {
    use super::wgpu::DeviceType::{Cpu, IntegratedGpu};

    assert!(should_auto_select_native_skia_raster(
        RendererRequest::Auto,
        true,
        Cpu,
        "Microsoft Basic Render Driver"
    ));
    assert!(should_auto_select_native_skia_raster(
        RendererRequest::Auto,
        true,
        IntegratedGpu,
        "Microsoft Direct3D12 (WARP)"
    ));
    assert!(should_auto_select_native_skia_raster(
        RendererRequest::Auto,
        true,
        IntegratedGpu,
        "Microsoft Basic Render Driver"
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn skia_software_auto_selection_preserves_hardware_and_explicit_choices() {
    use super::wgpu::DeviceType::{Cpu, IntegratedGpu};

    assert!(!should_auto_select_native_skia_raster(
        RendererRequest::Auto,
        false,
        Cpu,
        "Microsoft Basic Render Driver"
    ));
    assert!(!should_auto_select_native_skia_raster(
        RendererRequest::Auto,
        true,
        IntegratedGpu,
        "Qualcomm Adreno X1"
    ));
    for request in [
        RendererRequest::NativeVelloGpu,
        RendererRequest::NativeVelloCpu,
        RendererRequest::NativeSoftware,
        RendererRequest::NativeSkiaRaster,
        RendererRequest::NativeSkiaGanesh,
    ] {
        assert!(!should_auto_select_native_skia_raster(
            request,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn vello_cpu_override_never_changes_an_explicit_skia_request() {
    assert_eq!(
        apply_cpu_vello_override(RendererRequest::NativeSkiaRaster, true),
        RendererRequest::NativeSkiaRaster
    );
    assert_eq!(
        apply_cpu_vello_override(RendererRequest::NativeSkiaGanesh, true),
        RendererRequest::NativeSkiaGanesh
    );
    assert_eq!(
        apply_cpu_vello_override(RendererRequest::NativeSoftware, true),
        RendererRequest::NativeSoftware
    );
    assert_eq!(
        apply_cpu_vello_override(RendererRequest::NativeVelloGpu, true),
        RendererRequest::NativeVelloCpu
    );
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "skia")))]
#[test]
fn explicit_skia_request_fails_before_surface_initialization_when_feature_is_absent() {
    let error = require_compiled_native_renderer(RendererRequest::NativeSkiaRaster).unwrap_err();

    assert_eq!(error.request, RendererRequest::NativeSkiaRaster);
    assert!(error.details.contains("`skia` Cargo feature"));

    let ganesh = require_compiled_native_renderer(RendererRequest::NativeSkiaGanesh).unwrap_err();
    assert_eq!(ganesh.request, RendererRequest::NativeSkiaGanesh);
    assert!(ganesh.details.contains("`skia` Cargo feature"));

    let software = require_compiled_native_renderer(RendererRequest::NativeSoftware).unwrap_err();
    assert_eq!(software.request, RendererRequest::NativeSoftware);
    assert!(software.details.contains("`skia` Cargo feature"));
}

#[cfg(all(not(target_arch = "wasm32"), feature = "skia"))]
#[test]
fn native_software_alias_requires_and_accepts_the_skia_profile() {
    require_compiled_native_renderer(RendererRequest::NativeSoftware).unwrap();
    assert!(RendererRequest::NativeSoftware.uses_skia_raster());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn direct_ganesh_is_the_only_native_request_that_skips_wgpu_initialization() {
    assert!(!native_request_requires_wgpu(
        RendererRequest::NativeSkiaGanesh
    ));
    assert!(native_request_requires_wgpu(
        RendererRequest::NativeSkiaRaster
    ));
    assert!(native_request_requires_wgpu(RendererRequest::Auto));
    assert!(native_renderer_supports_capture(
        RendererRequest::NativeSkiaGanesh
    ));
    assert!(native_renderer_supports_capture(
        RendererRequest::NativeSkiaRaster
    ));
}

#[cfg(all(
    feature = "skia",
    feature = "three-d",
    any(
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "android"
    )
))]
#[test]
fn direct_ganesh_rejects_three_d_builds_at_selection_time() {
    let error = require_compiled_native_renderer(RendererRequest::NativeSkiaGanesh).unwrap_err();
    assert!(error.details.contains("3D interoperability"));
}

#[test]
fn recoverable_surface_acquisition_errors_never_crash_the_app() {
    use super::wgpu::SurfaceError;

    assert_eq!(
        surface_acquire_recovery(&SurfaceError::Lost),
        SurfaceAcquireRecovery::Reconfigure
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceError::Outdated),
        SurfaceAcquireRecovery::Reconfigure
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceError::Timeout),
        SurfaceAcquireRecovery::Retry
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceError::Other),
        SurfaceAcquireRecovery::Retry
    );
    assert_eq!(
        surface_acquire_recovery(&SurfaceError::OutOfMemory),
        SurfaceAcquireRecovery::Exit
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
        (50.0, 100.0)
    );
    assert_eq!(
        normalize_winit_scroll_delta(
            &MouseScrollDelta::PixelDelta(PhysicalPosition::new(-20.0, -40.0)),
            2.0,
        ),
        (10.0, 20.0)
    );
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
fn get_tree_metadata_keeps_identifier_separate_and_masks_value() {
    let input = WidgetId::from_u128(10);
    let mut ir = CoreIR::new();
    ir.add_node(
        input,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            label: Some("Password".into()),
            identifier: Some("account.password".into()),
            value: Some("hunter2".into()),
            focusable: true,
            masked: true,
            text_selection: Some((1, 3)),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.set_root(input);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
    snapshot.nodes.insert(
        input,
        LayoutNodeGeometry {
            rect: LayoutRect::new(20.0, 30.0, 160.0, 32.0),
            content_size: LayoutSize::new(160.0, 32.0),
        },
    );

    let records = collect_semantic_records(&ir, &snapshot, &ScrollStateMap::default());
    assert_eq!(records.len(), 1);
    let node = &records[0].node;
    assert_eq!(node.identifier.as_deref(), Some("account.password"));
    assert_eq!(node.label.as_deref(), Some("Password"));
    assert_eq!(node.value, None);
    assert!(node.value_present);
    assert!(node.masked);
    assert_eq!(node.text_selection, Some((1, 3)));
    assert_eq!(
        node.visibility,
        fission_test_driver::VisibilityState::FullyVisible
    );
}

#[test]
fn selector_can_scope_duplicate_labels() {
    let shell = WidgetId::from_u128(20);
    let app = WidgetId::from_u128(21);
    let shell_open = WidgetId::from_u128(22);
    let app_open = WidgetId::from_u128(23);

    let mut ir = CoreIR::new();
    ir.add_node(
        shell_open,
        Op::Semantics(Semantics {
            role: Role::Button,
            label: Some("Open".into()),
            identifier: Some("shell.open".into()),
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        app_open,
        Op::Semantics(Semantics {
            role: Role::Button,
            label: Some("Open".into()),
            identifier: Some("app.open".into()),
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        app,
        Op::Semantics(Semantics {
            role: Role::Generic,
            identifier: Some("mounted.app".into()),
            ..Semantics::default()
        }),
        vec![app_open],
    );
    ir.add_node(
        shell,
        Op::Semantics(Semantics {
            role: Role::Generic,
            identifier: Some("shell".into()),
            ..Semantics::default()
        }),
        vec![shell_open, app],
    );
    ir.set_root(shell);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
    for (id, y) in [
        (shell, 0.0),
        (app, 80.0),
        (shell_open, 8.0),
        (app_open, 88.0),
    ] {
        snapshot.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, y, 120.0, 40.0),
                content_size: LayoutSize::new(120.0, 40.0),
            },
        );
    }

    let mut pipeline = crate::Pipeline::new();
    pipeline.prev_ir = Some(ir);
    pipeline.last_snapshot = Some(snapshot);

    let query = fission_test_driver::SelectorQuery::label("Open").scoped(
        fission_test_driver::SelectorQuery::semantic_identifier("mounted.app"),
    );
    let record = resolve_selector_record(&pipeline, &ScrollStateMap::default(), &query)
        .expect("scoped selector should resolve");
    assert_eq!(record.node.identifier.as_deref(), Some("app.open"));
}

#[test]
fn hidden_selector_prefers_the_responsive_branch_participating_in_layout() {
    let root = WidgetId::from_u128(30);
    let active = WidgetId::from_u128(31);
    let inactive = WidgetId::from_u128(32);
    let mut ir = CoreIR::new();
    for id in [active, inactive] {
        ir.add_node(
            id,
            Op::Semantics(Semantics {
                role: Role::Button,
                label: Some("Continue".into()),
                identifier: Some("tour.continue".into()),
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
    }
    ir.add_node(
        root,
        Op::Semantics(Semantics {
            role: Role::Generic,
            identifier: Some("root".into()),
            ..Semantics::default()
        }),
        vec![active, inactive],
    );
    ir.set_root(root);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
    snapshot.nodes.insert(
        root,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            content_size: LayoutSize::new(320.0, 240.0),
        },
    );
    snapshot.nodes.insert(
        active,
        LayoutNodeGeometry {
            rect: LayoutRect::new(20.0, 30.0, 120.0, 40.0),
            content_size: LayoutSize::new(120.0, 40.0),
        },
    );

    let mut pipeline = crate::Pipeline::new();
    pipeline.prev_ir = Some(ir);
    pipeline.last_snapshot = Some(snapshot);
    let query =
        fission_test_driver::SelectorQuery::semantic_identifier("tour.continue").include_hidden();
    let record = resolve_selector_record(&pipeline, &ScrollStateMap::default(), &query)
        .expect("the active responsive branch should disambiguate the selector");

    assert_eq!(record.id, active);
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
fn no_repeat_interval_means_no_idle_animation_redraw() {
    let min_frame = Duration::from_millis(16);
    assert_eq!(
        animation_redraw_interval(false, None, false, min_frame),
        None
    );
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
fn tightly_packed_rgba_capture_uses_the_standard_screenshot_response() {
    let response = rgba_screenshot(vec![10, 20, 30, 255, 40, 50, 60, 255], 2, 1, 2, 1, None);
    match response {
        fission_test_driver::TestResponse::Screenshot {
            png_base64,
            width,
            height,
        } => {
            assert!(!png_base64.is_empty());
            assert_eq!((width, height), (2, 1));
        }
        response => panic!("expected screenshot response, got {response:?}"),
    }

    assert!(matches!(
        rgba_screenshot(vec![0; 7], 2, 1, 2, 1, None),
        fission_test_driver::TestResponse::Error { .. }
    ));
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
