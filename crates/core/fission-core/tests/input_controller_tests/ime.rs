use super::*;

#[test]
fn test_ime_preedit_tracks_cursor_without_dispatching_change() {
    let node_id = WidgetId::derived(40, &[0]);
    let ir = create_text_node(node_id, "hello world", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(300.0, 40.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, "hello ".len(), Some("hello ".len()));

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
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Ime(ImeEvent::Preedit {
            text: "世界".to_string(),
            cursor: Some((0, "世界".len())),
        }),
    ));

    let state = ctx.text_edit.get(node_id).expect("text input state");
    assert_eq!(state.committed_text(), "hello world");
    assert_eq!(
        state.preedit.as_ref().map(|preedit| preedit.text.as_str()),
        Some("世界")
    );
    assert_eq!(
        state.preedit.as_ref().and_then(|preedit| preedit.cursor),
        Some((0, "世界".len()))
    );
    assert_eq!(
        state.display_text(),
        (
            "hello 世界world".to_string(),
            Some(("hello ".len(), "hello 世界".len())),
        )
    );
    assert_eq!(
        state.display_preedit_cursor_range(),
        Some(("hello ".len(), "hello 世界".len())),
    );
    assert!(ctx.dispatched_actions.is_empty());
}

#[test]
fn pending_text_edit_accepts_transformed_model_value_on_sync() {
    let node_id = WidgetId::derived(40, &[3]);
    let mut text_edit = TextEditStateMap::default();
    text_edit.sync_from_runtime(node_id, "First item\n", None, None);
    text_edit.get_mut_or_default(node_id).apply_edit(
        "First item".len().."First item".len(),
        "Second item",
        "First itemSecond item".len(),
        "First itemSecond item".len(),
    );

    text_edit.sync_from_runtime(node_id, "First item\nSecond item", None, None);

    let state = text_edit.get(node_id).expect("text input state");
    assert_eq!(state.committed_text(), "First item\nSecond item");
    assert!(!state.pending_model_sync);
    assert_eq!(state.caret, "First item\nSecond item".len());
    assert_eq!(state.anchor, "First item\nSecond item".len());
}

#[test]
fn runtime_updates_ime_cursor_area_from_text_caret() {
    let input_id = WidgetId::derived(45, &[0]);
    let scroll_id = WidgetId::derived(45, &[1]);
    let text_id = WidgetId::derived(45, &[2]);
    let ir = create_rich_text_input_tree(input_id, scroll_id, text_id, "hello", false);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(300.0, 100.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(50.0, 20.0, 40.0, 60.0),
            content_size: LayoutSize::new(120.0, 60.0),
        },
    );

    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);
    let ime_handler = Arc::new(RecordingImeHandler::default());
    let mut runtime = Runtime::default()
        .with_measurer(measurer)
        .with_ime_handler(ime_handler.clone());

    runtime
        .runtime_state
        .interaction
        .set_focused(Some(input_id));
    runtime
        .runtime_state
        .text_edit
        .set_caret(input_id, "hello".len(), Some("hello".len()));

    runtime
        .handle_input(
            InputEvent::Ime(ImeEvent::Preedit {
                text: "!".to_string(),
                cursor: Some((1, 1)),
            }),
            &ir,
            &layout,
        )
        .expect("IME preedit should be handled");

    assert_eq!(ime_handler.allowed.lock().unwrap().last(), Some(&true));
    let cursor_area = *ime_handler
        .cursor_areas
        .lock()
        .unwrap()
        .last()
        .expect("cursor area should be updated");
    assert_eq!(cursor_area, LayoutRect::new(85.0, 36.0, 2.0, 20.0));
}

#[test]
fn test_ime_cancel_clears_preedit_without_committing() {
    let node_id = WidgetId::derived(41, &[0]);
    let ir = create_text_node(node_id, "hello", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(300.0, 40.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, "hello".len(), Some("hello".len()));
    text_edit
        .get_mut_or_default(node_id)
        .set_preedit("!".to_string(), Some((0, 1)));

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
    assert!(controller.handle_event(&mut ctx, &InputEvent::Ime(ImeEvent::Cancel)));

    let state = ctx.text_edit.get(node_id).expect("text input state");
    assert_eq!(state.committed_text(), "hello");
    assert!(state.preedit.is_none());
    assert!(ctx.dispatched_actions.is_empty());
}

#[test]
fn test_ime_commit_replaces_preedit_range_and_dispatches_change() {
    let node_id = WidgetId::derived(42, &[0]);
    let ir = create_text_node(node_id, "hello world", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(300.0, 40.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, "hello ".len(), Some("hello world".len()));

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
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Ime(ImeEvent::Preedit {
            text: "世界".to_string(),
            cursor: Some((0, "世界".len())),
        }),
    ));
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Ime(ImeEvent::Commit {
            text: "世界".to_string(),
        }),
    ));

    let state = ctx.text_edit.get(node_id).expect("text input state");
    assert_eq!(state.committed_text(), "hello 世界");
    assert!(state.preedit.is_none());
    assert_eq!(state.caret, "hello 世界".len());
    assert_eq!(state.anchor, "hello 世界".len());
    assert_eq!(ctx.dispatched_actions.len(), 1);
    let (target, envelope, input) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    assert_eq!(envelope.payload, b"null");
    let change = input.text_change().expect("text change input");
    assert_eq!(change.node_id, node_id);
    assert_eq!(change.new_text, "hello 世界");
    assert_eq!(change.new_caret, "hello 世界".len());
    assert_eq!(change.new_anchor, "hello 世界".len());
}

#[test]
fn test_ime_preedit_cursor_is_clamped_to_character_boundaries() {
    let node_id = WidgetId::derived(43, &[0]);
    let ir = create_text_node(node_id, "", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(300.0, 40.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));

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
    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Ime(ImeEvent::Preedit {
            text: "éx".to_string(),
            cursor: Some((1, "éx".len())),
        }),
    ));

    let state = ctx.text_edit.get(node_id).expect("text input state");
    assert_eq!(
        state.preedit.as_ref().and_then(|preedit| preedit.cursor),
        Some((0, "éx".len())),
    );
}
