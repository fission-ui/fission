use super::*;
use fission_core::{hit_test::HitTargetPolicy, LayoutNodeGeometry, LayoutRect, LayoutSize};
use fission_ir::{CoreNode, Semantics, StructuralOp};

#[test]
fn native_control_text_preserves_focus_traversal_and_button_activation() {
    use fission_core::{InputEvent, KeyCode, KeyEvent};
    for (code, text) in [
        (KeyCode::Tab, "\t"),
        (KeyCode::Enter, "\r"),
        (KeyCode::Escape, "\u{1b}"),
        (KeyCode::Backspace, "\u{8}"),
        (KeyCode::Space, " "),
    ] {
        assert!(matches!(
            crate::fission_key_down_event(code.clone(), 0, Some(text)),
            InputEvent::Keyboard(KeyEvent::Down { key_code, modifiers: 0 }) if key_code == code
        ));
    }
    // Keyboard layout output remains authoritative for actual text input.
    assert!(matches!(
        crate::fission_key_down_event(KeyCode::Space, 0, Some("t")),
        InputEvent::Keyboard(KeyEvent::DownWithText { text, .. }) if text == "t"
    ));
}

#[test]
fn borderless_resize_uses_the_correct_edge_without_capturing_content() {
    assert_eq!(
        resize_direction(LayoutPoint::new(1.0, 1.0), 800.0, 600.0),
        Some(ResizeDirection::NorthWest)
    );
    assert_eq!(
        resize_direction(LayoutPoint::new(799.0, 599.0), 800.0, 600.0),
        Some(ResizeDirection::SouthEast)
    );
    assert_eq!(
        resize_direction(LayoutPoint::new(799.0, 200.0), 800.0, 600.0),
        Some(ResizeDirection::East)
    );
    assert_eq!(
        resize_direction(LayoutPoint::new(20.0, 30.0), 800.0, 600.0),
        None
    );
    assert_eq!(
        resize_direction(LayoutPoint::new(-1.0, 30.0), 800.0, 600.0),
        None
    );
}

fn drag_tree(child: Semantics) -> (CoreIR, WidgetId) {
    let region = WidgetId::explicit("window.drag");
    let control = WidgetId::explicit("window.child");
    let paint = WidgetId::explicit("window.paint");
    let mut ir = CoreIR::default();
    for (id, op, children, parent) in [
        (
            region,
            Op::Semantics(Semantics {
                window_drag_region: true,
                ..Default::default()
            }),
            vec![control],
            None,
        ),
        (control, Op::Semantics(child), vec![paint], Some(region)),
        (
            paint,
            Op::Structural(StructuralOp::Group { stable_hash: 0 }),
            vec![],
            Some(control),
        ),
    ] {
        ir.nodes.insert(
            id,
            CoreNode {
                id,
                op,
                children,
                parent,
                hash: 0,
                composite: Default::default(),
            },
        );
    }
    ir.root = Some(region);
    (ir, paint)
}

#[test]
fn painted_child_resolves_to_ancestor_drag_surface() {
    let (ir, target) = drag_tree(Semantics::default());
    assert!(is_drag_target(&ir, target));
    assert!(!is_drag_target(&ir, WidgetId::explicit("missing")));
}

#[test]
fn drag_preflight_respects_the_active_pointer_minimum_target() {
    let (ir, paint) = drag_tree(Semantics {
        focusable: true,
        ..Default::default()
    });
    let region = ir.root.unwrap();
    let control = ir.nodes[&paint].parent.unwrap();
    let mut layout = LayoutSnapshot::new(LayoutSize::new(300.0, 60.0));
    for (id, rect) in [
        (region, LayoutRect::new(0.0, 0.0, 300.0, 60.0)),
        (control, LayoutRect::new(100.0, 20.0, 16.0, 16.0)),
        (paint, LayoutRect::new(100.0, 20.0, 16.0, 16.0)),
    ] {
        layout.nodes.insert(
            id,
            LayoutNodeGeometry {
                content_size: rect.size,
                rect,
            },
        );
    }
    let mut runtime = Runtime::default();
    runtime.set_hit_target_policy(HitTargetPolicy {
        pointer: 40.0,
        touch: 64.0,
    });

    // Outside both the drawn control and a hard-coded 24px target, but inside
    // its configured 40px mouse target. Exact preflight used to start a drag.
    let expanded_target = LayoutPoint::new(90.0, 28.0);
    assert_eq!(
        runtime.hit_test(expanded_target, &ir, &layout),
        Some(region)
    );
    assert!(!is_drag_surface_at(&runtime, &ir, &layout, expanded_target));
    assert!(is_drag_surface_at(
        &runtime,
        &ir,
        &layout,
        LayoutPoint::new(80.0, 28.0),
    ));

    // Changing the pointer policy takes effect immediately; touch expansion
    // must not enlarge mouse targets during native drag preflight.
    runtime.set_hit_target_policy(HitTargetPolicy {
        pointer: 0.0,
        touch: 64.0,
    });
    assert!(is_drag_surface_at(&runtime, &ir, &layout, expanded_target));
    assert!(!is_drag_surface_at(
        &runtime,
        &ir,
        &layout,
        LayoutPoint::new(108.0, 28.0),
    ));
}

#[test]
fn controls_and_selection_do_not_start_native_drag_even_when_disabled() {
    for semantics in [
        Semantics {
            focusable: true,
            ..Default::default()
        },
        Semantics {
            focusable: true,
            disabled: true,
            ..Default::default()
        },
        Semantics {
            text_editable: true,
            ..Default::default()
        },
        Semantics {
            selectable_text: true,
            ..Default::default()
        },
        Semantics {
            draggable: true,
            ..Default::default()
        },
        Semantics {
            scrollable_y: true,
            ..Default::default()
        },
        Semantics {
            context_menu: true,
            ..Default::default()
        },
    ] {
        let (ir, target) = drag_tree(semantics);
        assert!(!is_drag_target(&ir, target));
    }
}

#[test]
fn ordinary_content_and_disabled_drag_surface_do_not_move_the_window() {
    let (mut ir, target) = drag_tree(Semantics::default());
    let root = ir.root.unwrap();
    if let Op::Semantics(semantics) = &mut ir.nodes.get_mut(&root).unwrap().op {
        semantics.disabled = true;
    }
    assert!(!is_drag_target(&ir, target));
    if let Op::Semantics(semantics) = &mut ir.nodes.get_mut(&root).unwrap().op {
        semantics.disabled = false;
        semantics.window_drag_region = false;
    }
    assert!(!is_drag_target(&ir, target));
}

#[test]
fn double_click_requires_time_and_position_and_does_not_triple_toggle() {
    let now = Instant::now();
    let point = LayoutPoint::new(20.0, 30.0);
    let mut state = WindowDragState::default();
    assert!(!state.double_press(now, point));
    assert!(state.double_press(now + Duration::from_millis(100), point));
    assert!(!state.double_press(now + Duration::from_millis(200), point));
    assert!(!state.double_press(now + Duration::from_secs(1), point));
    assert!(!state.double_press(
        now + Duration::from_millis(1100),
        LayoutPoint::new(40.0, 30.0)
    ));
}

#[test]
fn shutdown_cancels_queued_operations() {
    let (completion, mut response) = oneshot::channel();
    let controls = WindowControls {
        pending: Arc::new(Mutex::new(vec![PendingCommand {
            command: WindowCommand::Maximize,
            completion,
        }])),
    };
    let weak = Arc::downgrade(&controls.pending);
    drop(controls);
    assert!(weak.upgrade().is_none());
    assert!(response.try_recv().is_err());
}

#[test]
fn decorations_are_opt_in_to_disable() {
    #[derive(Debug, Default)]
    struct State;
    impl fission_core::GlobalState for State {}
    let app = crate::WinitApp::<State, _>::new(fission_core::ui::Spacer::default());
    assert!(app.decorations);
    assert!(!app.with_decorations(false).decorations);
    let attributes = crate::build_window_attributes(
        "Custom",
        false,
        false,
        false,
        false,
        None,
        crate::BrowserDefaults::NONE,
        None,
    )
    .unwrap();
    assert!(!attributes.decorations);
}
