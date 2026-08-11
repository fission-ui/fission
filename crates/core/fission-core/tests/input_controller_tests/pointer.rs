use super::*;

#[test]
fn test_single_line_auto_scroll_with_rich_text_uses_local_coordinates() {
    let input_id = WidgetId::derived(10, &[0]);
    let scroll_id = WidgetId::derived(10, &[1]);
    let text_id = WidgetId::derived(10, &[2]);
    let value = "012345678901234567890123456789";
    let ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(240.0, 64.0, 100.0, 24.0),
            content_size: LayoutSize::new(320.0, 24.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, value.len(), Some(value.len()));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let event = InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('!'),
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    assert!(
        ctx.scroll.get_offset(scroll_id) > 0.0,
        "single-line inputs should scroll horizontally to keep caret visible"
    );
}

#[test]
fn test_single_line_auto_scroll_respects_scroll_padding() {
    let input_id = WidgetId::derived(210, &[0]);
    let scroll_id = WidgetId::derived(210, &[1]);
    let text_id = WidgetId::derived(210, &[2]);
    let value = "012345678901234567890123456789";
    let mut ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);
    if let Op::Semantics(semantics) = &mut ir.nodes.get_mut(&input_id).expect("input").op {
        semantics.scroll_padding = Some([18.0, 24.0, 2.0, 3.0]);
    }

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(240.0, 64.0, 100.0, 24.0),
            content_size: LayoutSize::new(320.0, 24.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, value.len(), Some(value.len()));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let event = InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('!'),
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    assert!(
        ctx.scroll.get_offset(scroll_id) >= 24.0,
        "right scroll padding should leave extra room past the caret"
    );
}

#[test]
fn test_drag_start_behavior_down_skips_pointer_slop() {
    let input_id = WidgetId::derived(211, &[0]);
    let scroll_id = WidgetId::derived(211, &[1]);
    let text_id = WidgetId::derived(211, &[2]);
    let value = "abcdef";
    let mut ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);
    attach_text_input_runtime_config(
        &mut ir,
        input_id,
        TextInputRuntimeConfig {
            drag_start_behavior: DragStartBehavior::Down,
            undo_controller: None,
            restoration_id: None,
            spell_check_configuration: None,
        },
    );

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(FineTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 0, Some(0));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );

    let down = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(200.0, 44.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &down));
    assert!(ctx.interaction.is_pressed(input_id));
    assert_eq!(
        ctx.interaction.last_down_point,
        Some(LayoutPoint::new(200.0, 44.0))
    );

    let drag = InputEvent::Pointer(PointerEvent::Move {
        point: LayoutPoint::new(201.0, 44.0),
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &drag));

    let state = ctx.text_edit.get(input_id).expect("text state");
    assert_eq!(state.caret, 1);
}

#[test]
fn test_multiline_pointer_hit_test_applies_vertical_scroll_offset() {
    let input_id = WidgetId::derived(214, &[0]);
    let scroll_id = WidgetId::derived(214, &[1]);
    let text_id = WidgetId::derived(214, &[2]);
    let value = "aa\nbb\ncc";
    let ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, true);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 80.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    scroll.set_offset(scroll_id, 20.0);

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );

    let down = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(205.0, 40.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &down));

    let caret = ctx.text_edit.get(input_id).expect("text state").caret;
    assert!(
        caret >= 3,
        "vertical scroll offset should hit the second visible line, got {caret}"
    );
}

#[test]
fn test_restoration_id_restores_local_edit_state() {
    let first_id = WidgetId::derived(212, &[0]);
    let second_id = WidgetId::derived(212, &[1]);
    let mut text_edit = TextEditStateMap::default();

    text_edit.sync_from_runtime(first_id, "", Some("search-box"), Some(8));
    let first_state = text_edit.get_mut_or_default(first_id);
    let restored = first_state.apply_edit(0..0, "restored", 8, 8);
    assert_eq!(restored, "restored");
    text_edit.persist_restoration(first_id, Some("search-box"));

    text_edit.sync_from_runtime(second_id, "", Some("search-box"), Some(8));
    let second_state = text_edit.get(second_id).expect("restored state");
    assert_eq!(second_state.committed_text(), "restored");
    assert_eq!(second_state.caret, 8);
    assert_eq!(second_state.anchor, 8);
}

#[test]
fn test_undo_controller_capacity_limits_history_depth() {
    let input_id = WidgetId::derived(213, &[0]);
    let scroll_id = WidgetId::derived(213, &[1]);
    let text_id = WidgetId::derived(213, &[2]);
    let mut ir = create_rich_text_input_tree(input_id, scroll_id, text_id, "", false);
    attach_text_input_runtime_config(
        &mut ir,
        input_id,
        TextInputRuntimeConfig {
            drag_start_behavior: DragStartBehavior::Start,
            undo_controller: Some(TextUndoController { capacity: 1 }),
            restoration_id: None,
            spell_check_configuration: None,
        },
    );

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 0, Some(0));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );

    for ch in ['a', 'b', 'c'] {
        let event = InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Char(ch),
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
    }
    assert_eq!(
        ctx.text_edit
            .get(input_id)
            .expect("text state")
            .committed_text(),
        "abc"
    );

    let undo = InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('z'),
        modifiers: primary_shortcut_modifier(),
    });
    assert!(controller.handle_event(&mut ctx, &undo));
    assert_eq!(
        ctx.text_edit
            .get(input_id)
            .expect("text state")
            .committed_text(),
        "ab"
    );

    assert!(controller.handle_event(&mut ctx, &undo));
    assert_eq!(
        ctx.text_edit
            .get(input_id)
            .expect("text state")
            .committed_text(),
        "ab"
    );
}

#[test]
fn test_pointer_hit_test_handles_draw_rich_text_single_line() {
    let input_id = WidgetId::derived(11, &[0]);
    let scroll_id = WidgetId::derived(11, &[1]);
    let text_id = WidgetId::derived(11, &[2]);
    let value = "abcdefghij";
    let ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 0, Some(0));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let event = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(260.0, 44.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    let caret = ctx.text_edit.get(input_id).map(|s| s.caret).unwrap_or(0);
    assert!(caret >= 5, "caret should move based on pointer hit-test");
}

#[test]
fn test_shift_click_extends_selection_from_existing_anchor() {
    let input_id = WidgetId::derived(28, &[0]);
    let scroll_id = WidgetId::derived(28, &[1]);
    let text_id = WidgetId::derived(28, &[2]);
    let value = "abcdefghij";
    let ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 2, Some(2));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let event = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(280.0, 44.0),
        button: PointerButton::Primary,
        modifiers: MOD_SHIFT,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    let state = ctx.text_edit.get(input_id).unwrap();
    assert_eq!(state.anchor, 2);
    assert!(
        state.caret >= 7,
        "shift-click should extend selection to the clicked caret"
    );
}

#[test]
fn test_secondary_click_shows_text_toolbar_affordance() {
    let input_id = WidgetId::derived(31, &[0]);
    let scroll_id = WidgetId::derived(31, &[1]);
    let text_id = WidgetId::derived(31, &[2]);
    let value = "abcdefghij";
    let ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 4, Some(4));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let event = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(248.0, 50.0),
        button: PointerButton::Secondary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    let affordances = &ctx.text_edit.get(input_id).expect("text state").affordances;
    assert!(affordances.toolbar_visible);
    assert_eq!(
        affordances.toolbar_anchor,
        Some(LayoutPoint::new(48.0, 10.0))
    );
    assert!(!affordances.magnifier_visible);
}

#[test]
fn test_pointer_down_outside_focused_input_clears_text_affordances() {
    let input_id = WidgetId::derived(34, &[0]);
    let scroll_id = WidgetId::derived(34, &[1]);
    let text_id = WidgetId::derived(34, &[2]);
    let value = "abcdefghij";
    let mut ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);
    if let Some(node) = ir.nodes.get_mut(&input_id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::TapOutside,
                action_id: 999,
                payload_data: None,
            });
        }
    }

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    let state = text_edit.get_mut_or_default(input_id);
    state.caret = 8;
    state.anchor = 2;
    state.affordances.toolbar_visible = true;
    state.affordances.toolbar_anchor = Some(LayoutPoint::new(48.0, 10.0));
    state.affordances.selection_start_handle = Some(LayoutPoint::new(20.0, 16.0));
    state.affordances.selection_end_handle = Some(LayoutPoint::new(80.0, 16.0));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let outside_click = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(20.0, 20.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(!controller.handle_event(&mut ctx, &outside_click));

    let payload: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    assert_eq!(ctx.dispatched_actions[0].0, input_id);
    assert_eq!(
        ctx.dispatched_actions[0].1.id,
        fission_core::ActionId::from_u128(999)
    );
    assert_eq!(payload, value);

    let affordances = &ctx.text_edit.get(input_id).expect("text state").affordances;
    assert!(!affordances.toolbar_visible);
    assert!(affordances.toolbar_anchor.is_none());
    assert!(affordances.selection_start_handle.is_none());
    assert!(affordances.selection_end_handle.is_none());
    assert!(affordances.caret_handle.is_none());
}

#[test]
fn test_toolbar_copy_button_click_uses_derived_node_id() {
    let input_id = WidgetId::derived(32, &[0]);
    let scroll_id = WidgetId::derived(32, &[1]);
    let text_id = WidgetId::derived(32, &[2]);
    let value = "abcdefghij";
    let mut ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );
    let copy_button_id = test_text_input_toolbar_button_id(input_id, TextContextMenuAction::Copy);
    attach_focusable_overlay_node(
        &mut ir,
        &mut layout,
        input_id,
        copy_button_id,
        LayoutRect::new(205.0, 4.0, 56.0, 28.0),
    );

    let clipboard_impl = Arc::new(MockClipboard::new());
    let clipboard: Arc<dyn Clipboard> = clipboard_impl.clone();
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 5, Some(2));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let event = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(220.0, 16.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));
    assert_eq!(clipboard_impl.get_text().as_deref(), Some("cde"));
}

#[test]
fn test_selection_handle_drag_updates_selection_and_toolbar_lifecycle() {
    let input_id = WidgetId::derived(33, &[0]);
    let scroll_id = WidgetId::derived(33, &[1]);
    let text_id = WidgetId::derived(33, &[2]);
    let value = "abcdefghij";
    let mut ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );
    let start_handle_id = test_text_input_selection_handle_id(
        input_id,
        fission_core::env::TextSelectionHandleKind::Start,
    );
    attach_focusable_overlay_node(
        &mut ir,
        &mut layout,
        input_id,
        start_handle_id,
        LayoutRect::new(215.0, 46.0, 14.0, 14.0),
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 8, Some(2));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );

    let down = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(220.0, 50.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &down));
    let affordances = &ctx.text_edit.get(input_id).expect("text state").affordances;
    assert_eq!(
        affordances.active_handle,
        Some(fission_core::env::TextSelectionHandleKind::Start)
    );
    assert!(affordances.magnifier_visible);
    assert!(!affordances.toolbar_visible);

    let drag = InputEvent::Pointer(PointerEvent::Move {
        point: LayoutPoint::new(212.0, 50.0),
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &drag));
    let state = ctx.text_edit.get(input_id).expect("text state");
    assert_eq!(state.anchor, 1);
    assert_eq!(state.caret, 8);
    assert!(state.affordances.magnifier_visible);
    assert_eq!(
        state.affordances.magnifier_anchor,
        state.affordances.selection_start_handle
    );

    let up = InputEvent::Pointer(PointerEvent::Up {
        point: LayoutPoint::new(212.0, 50.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &up));
    let affordances = &ctx.text_edit.get(input_id).expect("text state").affordances;
    assert_eq!(affordances.active_handle, None);
    assert!(!affordances.magnifier_visible);
    assert!(affordances.toolbar_visible);
}

#[test]
fn test_masked_pointer_hit_testing_maps_back_to_source_offsets() {
    let input_id = WidgetId::derived(30, &[0]);
    let scroll_id = WidgetId::derived(30, &[1]);
    let text_id = WidgetId::derived(30, &[2]);
    let value = "aé😊b";
    let mut ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, false);
    set_masked(&mut ir, input_id, true);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(200.0, 40.0, 120.0, 24.0),
            content_size: LayoutSize::new(120.0, 24.0),
        },
    );
    layout.nodes.insert(
        input_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(180.0, 30.0, 180.0, 44.0),
            content_size: LayoutSize::new(180.0, 44.0),
        },
    );

    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(input_id));
    text_edit.set_caret(input_id, 0, Some(0));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
    );
    let event = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(225.0, 44.0),
        button: PointerButton::Primary,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    let caret = ctx.text_edit.get(input_id).map(|s| s.caret).unwrap_or(0);
    assert_eq!(
        caret, 3,
        "masked hit testing should map back to the second grapheme boundary"
    );
}
