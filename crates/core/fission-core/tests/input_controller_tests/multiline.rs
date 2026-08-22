use super::*;

#[test]
fn test_multiline_enter_key() {
    let node_id = WidgetId::derived(1, &[0]);
    let initial_text = "Line One";
    let ir = create_text_node(node_id, initial_text, true); // Multiline
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0)); // Fixed width for wrapping and calc
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, initial_text.len(), Some(initial_text.len())); // Caret at end

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
        key_code: KeyCode::Enter,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    let (target, env, input) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    assert_eq!(env.payload, b"null");
    assert_eq!(input.text_change().unwrap().new_text, "Line One\n");
    assert_eq!(
        ctx.text_edit.get(node_id).unwrap().caret,
        "Line One\n".len()
    );
}

#[test]
#[ignore]
fn test_multiline_vertical_navigation_up_down() {
    let node_id = WidgetId::derived(1, &[0]);
    let initial_text = "First Line\nSecond Line\nThird Line";
    let ir = create_text_node(node_id, initial_text, true); // Multiline
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0)); // Fixed width for wrapping and calc
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    // Caret at end of Line Two
    text_edit.set_caret(
        node_id,
        "First Line\nSecond Line".len(),
        Some("First Line\nSecond Line".len()),
    );

    let mut controller = TextInputController;

    // Move Up
    {
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
            key_code: KeyCode::Up,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        // Expect caret to move to the same horizontal position on Line One
        let st = ctx.text_edit.get(node_id).unwrap();
        // Mock measurer based on fixed char width: "Line One".len() = 8
        assert_eq!(st.caret, "First Line".len());
        assert_eq!(st.anchor, "First Line".len());
    }

    // Move Down (from Line One to Line Two)
    {
        // Set caret to Line One end for consistent horizontal position
        text_edit.set_caret(node_id, "First Line".len(), Some("First Line".len()));
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
            key_code: KeyCode::Down,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        // Expect caret to move to same horizontal position on Line Two
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, "First Line\nSecond Line".len());
        assert_eq!(st.anchor, "First Line\nSecond Line".len());
    }
}
