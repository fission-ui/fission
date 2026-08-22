use super::*;

#[test]
fn test_text_input_typing() {
    let node_id = WidgetId::derived(1, &[0]);
    let ir = create_text_node(node_id, "Hello", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5));

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

    let (target, env, input) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    assert_eq!(env.payload, b"null");
    assert_eq!(input.text_change().unwrap().new_text, "Hello!");

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.caret, 6);
}

#[test]
fn test_text_input_typing_without_relayout_does_not_drop_chars() {
    let node_id = WidgetId::derived(1, &[0]);
    let ir = create_text_node(node_id, "", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 0, Some(0));

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

    let event_a = InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('a'),
        modifiers: 0,
    });
    let event_b = InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('b'),
        modifiers: 0,
    });

    assert!(controller.handle_event(&mut ctx, &event_a));
    assert!(controller.handle_event(&mut ctx, &event_b));
    assert_eq!(ctx.dispatched_actions.len(), 2);

    assert_eq!(
        ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
        "a"
    );
    assert_eq!(
        ctx.dispatched_actions[1].2.text_change().unwrap().new_text,
        "ab"
    );

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.buffer.to_string(), "ab");
    assert_eq!(st.caret, 2);
}

#[test]
fn test_text_input_copy_paste() {
    let node_id = WidgetId::derived(1, &[0]);
    let ir = create_text_node(node_id, "SelectMe", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 6, Some(0)); // Select "Select"

    let mut controller = TextInputController;

    // Cmd+C
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
            key_code: KeyCode::Char('c'),
            modifiers: primary_shortcut_modifier(),
        });
        assert!(controller.handle_event(&mut ctx, &event));
        assert_eq!(clipboard.get_text().as_deref(), Some("Select"));
    }

    text_edit.set_caret(node_id, 8, Some(8)); // "SelectMe|"

    // Cmd+V
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
            key_code: KeyCode::Char('v'),
            modifiers: primary_shortcut_modifier(),
        });
        assert!(controller.handle_event(&mut ctx, &event));

        assert_eq!(
            ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
            "SelectMeSelect"
        );
    }
}

#[test]
fn test_emoji_navigation_and_deletion() {
    let node_id = WidgetId::derived(1, &[0]);
    let initial_text = "Hi 🧘🏻‍♂️";
    let ir = create_text_node(node_id, initial_text, false);

    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    let len = initial_text.len();
    text_edit.set_caret(node_id, len, Some(len));

    let mut controller = TextInputController;

    // Backspace should delete the entire emoji
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
            key_code: KeyCode::Backspace,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));

        assert_eq!(
            ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
            "Hi "
        );

        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 3);
    }

    // Reset
    text_edit.set_caret(node_id, len, Some(len));

    // Left arrow should jump over emoji
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
            key_code: KeyCode::Left,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));

        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 3);
        assert_eq!(st.anchor, 3);
    }
}

#[test]
fn test_word_navigation() {
    let node_id = WidgetId::derived(1, &[0]);
    let initial_text = "hello world code";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    let len = initial_text.len();
    text_edit.set_caret(node_id, len, Some(len));

    let mut controller = TextInputController;

    // Alt+Left -> "hello world |code"
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
            key_code: KeyCode::Left,
            modifiers: word_navigation_modifier(),
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 12);
    }

    // Alt+Left again -> "hello |world code"
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
            key_code: KeyCode::Left,
            modifiers: word_navigation_modifier(),
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 6);
    }
}

#[test]
fn test_word_navigation_skips_non_word_segments() {
    let node_id = WidgetId::derived(35, &[0]);
    let initial_text = "hi 👩‍💻 world";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    let len = initial_text.len();
    text_edit.set_caret(node_id, len, Some(len));

    let mut controller = TextInputController;

    let world_start = initial_text.find("world").unwrap();
    for expected in [world_start, 0] {
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
            key_code: KeyCode::Left,
            modifiers: word_navigation_modifier(),
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, expected);
    }
}

#[test]
fn test_selection_mechanics() {
    let node_id = WidgetId::derived(1, &[0]);
    let initial_text = "ABCD";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 0, Some(0)); // "|ABCD"

    let mut controller = TextInputController;

    // Shift+Right -> "A|BCD" with selection [0,1)
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
            key_code: KeyCode::Right,
            modifiers: 1,
        }); // Shift
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 1);
        assert_eq!(st.anchor, 0);
    }

    // Shift+Right again -> "AB|CD" with selection [0,2)
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
            key_code: KeyCode::Right,
            modifiers: 1,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 2);
        assert_eq!(st.anchor, 0);
    }

    // Type 'X' -> Replace selection -> "XCD"
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
            key_code: KeyCode::Char('X'),
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));

        let (_target, env, input) = &ctx.dispatched_actions[0];
        assert_eq!(env.payload, b"null");
        assert_eq!(input.text_change().unwrap().new_text, "XCD");

        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 1);
        assert_eq!(st.anchor, 1);
    }
}

#[test]
fn test_home_end_navigation() {
    let node_id = WidgetId::derived(1, &[0]);
    let initial_text = "Start to End";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5)); // Middle

    let mut controller = TextInputController;

    // Home
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
            key_code: KeyCode::Home,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 0);
    }

    // End
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
            key_code: KeyCode::End,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, initial_text.len());
    }
}

#[test]
fn test_primary_shortcut_select_all() {
    let node_id = WidgetId::derived(20, &[0]);
    let initial_text = "Select everything";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 3, Some(3));

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
        key_code: KeyCode::Char('a'),
        modifiers: MOD_CTRL,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.anchor, 0);
    assert_eq!(st.caret, initial_text.len());
}

#[test]
fn test_apple_primary_shortcut_is_runtime_selected() {
    let node_id = WidgetId::derived(201, &[0]);
    let initial_text = "Select everything";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 3, Some(3));
    let mut ctx = setup_ctx_with_convention(
        &ir,
        &layout,
        &mut text_edit,
        &mut interaction,
        &mut scroll,
        &mut gesture,
        &clipboard,
        Some(&measurer),
        TextEditingConvention::Apple,
    );

    assert!(TextInputController.handle_event(
        &mut ctx,
        &InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Char('a'),
            modifiers: MOD_SUPER,
        }),
    ));
    let state = ctx.text_edit.get(node_id).expect("text edit state");
    assert_eq!((state.anchor, state.caret), (0, initial_text.len()));
}

#[test]
fn test_unhandled_primary_chord_is_not_inserted() {
    for (convention, modifier) in [
        (TextEditingConvention::Standard, MOD_CTRL),
        (TextEditingConvention::Apple, MOD_SUPER),
    ] {
        let node_id = WidgetId::derived(202, &[modifier as u32]);
        let ir = create_text_node(node_id, "", false);
        let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 40.0));
        let mut text_edit = TextEditStateMap::default();
        let mut interaction = InteractionStateMap::default();
        let mut scroll = ScrollStateMap::default();
        let mut gesture = fission_core::env::GestureState::default();
        let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
        let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);
        interaction.set_focused(Some(node_id));

        let mut ctx = setup_ctx_with_convention(
            &ir,
            &layout,
            &mut text_edit,
            &mut interaction,
            &mut scroll,
            &mut gesture,
            &clipboard,
            Some(&measurer),
            convention,
        );
        assert!(TextInputController.handle_event(
            &mut ctx,
            &InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Char('q'),
                modifiers: modifier,
            }),
        ));
        assert!(ctx.dispatched_actions.is_empty());
        assert_eq!(
            ctx.text_edit
                .get(node_id)
                .expect("text edit state")
                .committed_text(),
            ""
        );
    }
}

#[test]
fn test_standard_alt_gr_character_remains_text_input() {
    let node_id = WidgetId::derived(203, &[0]);
    let ir = create_text_node(node_id, "", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 40.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);
    interaction.set_focused(Some(node_id));

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
    assert!(TextInputController.handle_event(
        &mut ctx,
        &InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Char('€'),
            modifiers: MOD_CTRL | MOD_ALT,
        }),
    ));
    assert_eq!(
        ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
        "€"
    );
}

#[test]
fn test_semantic_editing_commands_share_text_input_state() {
    let node_id = WidgetId::derived(204, &[0]);
    let ir = create_text_node(node_id, "abc", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 40.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);
    interaction.set_focused(Some(node_id));
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
    let mut controller = TextInputController;

    for command in [EditingCommand::SelectAll, EditingCommand::Copy] {
        assert!(controller.handle_event(&mut ctx, &InputEvent::Editing(command)));
    }
    assert_eq!(clipboard.get_text().as_deref(), Some("abc"));

    assert!(controller.handle_event(&mut ctx, &InputEvent::Editing(EditingCommand::Cut),));
    assert_eq!(
        ctx.text_edit
            .get(node_id)
            .expect("text edit state")
            .committed_text(),
        ""
    );

    assert!(controller.handle_event(
        &mut ctx,
        &InputEvent::Editing(EditingCommand::Paste("xyz".into())),
    ));
    assert_eq!(
        ctx.text_edit
            .get(node_id)
            .expect("text edit state")
            .committed_text(),
        "xyz"
    );

    assert!(controller.handle_event(&mut ctx, &InputEvent::Editing(EditingCommand::Undo),));
    assert_eq!(
        ctx.text_edit
            .get(node_id)
            .expect("text edit state")
            .committed_text(),
        ""
    );

    assert!(controller.handle_event(&mut ctx, &InputEvent::Editing(EditingCommand::Redo),));
    assert_eq!(
        ctx.text_edit
            .get(node_id)
            .expect("text edit state")
            .committed_text(),
        "xyz"
    );
}

#[test]
fn test_forward_delete_removes_next_grapheme() {
    let node_id = WidgetId::derived(21, &[0]);
    let ir = create_text_node(node_id, "abcd", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 1, Some(1));

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
        key_code: KeyCode::Delete,
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    assert_eq!(
        ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
        "acd"
    );

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.caret, 1);
    assert_eq!(st.anchor, 1);
}

#[test]
fn test_apple_ctrl_bindings_cover_line_and_char_navigation() {
    let node_id = WidgetId::derived(211, &[0]);
    let initial_text = "hello";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 2, Some(2));

    let mut controller = TextInputController;

    for (key_code, expected) in [
        (KeyCode::Char('b'), 1usize),
        (KeyCode::Char('f'), 2usize),
        (KeyCode::Char('a'), 0usize),
        (KeyCode::Char('e'), initial_text.len()),
    ] {
        let mut ctx = setup_ctx_with_convention(
            &ir,
            &layout,
            &mut text_edit,
            &mut interaction,
            &mut scroll,
            &mut gesture,
            &clipboard,
            Some(&measurer),
            TextEditingConvention::Apple,
        );
        let event = InputEvent::Keyboard(KeyEvent::Down {
            key_code,
            modifiers: MOD_CTRL,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, expected);
        assert_eq!(st.anchor, expected);
    }
}

#[test]
fn test_apple_meta_delete_shortcuts_trim_current_line() {
    let node_id = WidgetId::derived(213, &[0]);
    let initial_text = "hello world";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5));

    let mut controller = TextInputController;

    {
        let mut ctx = setup_ctx_with_convention(
            &ir,
            &layout,
            &mut text_edit,
            &mut interaction,
            &mut scroll,
            &mut gesture,
            &clipboard,
            Some(&measurer),
            TextEditingConvention::Apple,
        );
        let event = InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Backspace,
            modifiers: MOD_SUPER,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        assert_eq!(
            ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
            " world"
        );
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 0);
    }

    text_edit.set_caret(node_id, 6, Some(6));

    {
        let mut ctx = setup_ctx_with_convention(
            &ir,
            &layout,
            &mut text_edit,
            &mut interaction,
            &mut scroll,
            &mut gesture,
            &clipboard,
            Some(&measurer),
            TextEditingConvention::Apple,
        );
        let event = InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Delete,
            modifiers: MOD_SUPER,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        assert_eq!(
            ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
            "hello "
        );
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 6);
    }
}

#[test]
fn test_page_up_down_navigate_by_viewport_height() {
    let input_id = WidgetId::derived(212, &[0]);
    let scroll_id = WidgetId::derived(212, &[1]);
    let text_id = WidgetId::derived(212, &[2]);
    let value = "One\nTwo\nThree\nFour\nFive";
    let ir = create_rich_text_input_tree(input_id, scroll_id, text_id, value, true);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    layout.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(120.0, 40.0, 120.0, 40.0),
            content_size: LayoutSize::new(120.0, 120.0),
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
            key_code: KeyCode::PageDown,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(input_id).unwrap();
        assert_eq!(st.caret, "One\nTwo\n".len());
    }

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
            key_code: KeyCode::PageUp,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(input_id).unwrap();
        assert_eq!(st.caret, 0);
    }
}

#[test]
fn test_read_only_blocks_edits_but_allows_navigation() {
    let node_id = WidgetId::derived(22, &[0]);
    let mut ir = create_text_node(node_id, "locked", false);
    set_read_only(&mut ir, node_id, true);

    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 6, Some(6));

    let mut controller = TextInputController;

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
            key_code: KeyCode::Char('x'),
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        assert!(ctx.dispatched_actions.is_empty());
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.buffer.to_string(), "locked");
        assert_eq!(st.caret, 6);
    }

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
            key_code: KeyCode::Left,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 5);
        assert_eq!(st.anchor, 5);
    }
}

#[test]
fn test_multiline_home_end_stay_on_current_line() {
    let node_id = WidgetId::derived(23, &[0]);
    let initial_text = "aa\nbbbb\ncc";
    let ir = create_text_node(node_id, initial_text, true);
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5));

    let mut controller = TextInputController;

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
            key_code: KeyCode::Home,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        assert_eq!(ctx.text_edit.get(node_id).unwrap().caret, 3);
    }

    text_edit.set_caret(node_id, 5, Some(5));

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
            key_code: KeyCode::End,
            modifiers: 0,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        assert_eq!(ctx.text_edit.get(node_id).unwrap().caret, 7);
    }
}

#[test]
fn test_single_line_enter_dispatches_submit() {
    let node_id = WidgetId::derived(24, &[0]);
    let mut ir = create_text_node(node_id, "submit me", false);
    add_submit_action(&mut ir, node_id);

    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 9, Some(9));

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

    assert_eq!(ctx.dispatched_actions.len(), 1);
    let submitted: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    assert_eq!(submitted, "submit me");
}

#[test]
fn test_single_line_enter_dispatches_editing_complete_before_submit() {
    let node_id = WidgetId::derived(29, &[0]);
    let mut ir = create_text_node(node_id, "complete me", false);
    add_editing_complete_action(&mut ir, node_id);
    add_submit_action(&mut ir, node_id);
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, "complete me".len(), Some("complete me".len()));

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

    assert_eq!(ctx.dispatched_actions.len(), 2);
    assert_eq!(
        ctx.dispatched_actions[0].1.id,
        fission_core::ActionId::from_u128(3)
    );
    assert_eq!(
        ctx.dispatched_actions[1].1.id,
        fission_core::ActionId::from_u128(2)
    );
}

#[test]
fn test_shift_home_preserves_anchor() {
    let node_id = WidgetId::derived(25, &[0]);
    let initial_text = "aa\nbbbb\ncc";
    let ir = create_text_node(node_id, initial_text, true);
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 6, Some(6));

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
        key_code: KeyCode::Home,
        modifiers: MOD_SHIFT,
    });
    assert!(controller.handle_event(&mut ctx, &event));
    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.anchor, 6);
    assert_eq!(st.caret, 3);
}

#[test]
fn test_max_length_enforced_on_typing() {
    let node_id = WidgetId::derived(26, &[0]);
    let mut ir = create_text_node(node_id, "abcd", false);
    set_max_length(&mut ir, node_id, 4);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 4, Some(4));

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
        key_code: KeyCode::Char('z'),
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));
    assert!(ctx.dispatched_actions.is_empty());
    assert_eq!(
        ctx.text_edit.get(node_id).unwrap().buffer.to_string(),
        "abcd"
    );
}

#[test]
fn test_text_capitalization_words_applies_to_inserted_text() {
    let node_id = WidgetId::derived(27, &[0]);
    let mut ir = create_text_node(node_id, "hello ", false);
    set_capitalization(&mut ir, node_id, TextCapitalization::Words);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 6, Some(6));

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
        key_code: KeyCode::Char('w'),
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &event));
    assert_eq!(
        ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
        "hello W"
    );
}

#[test]
fn test_digits_only_formatter_filters_paste() {
    let node_id = WidgetId::derived(28, &[0]);
    let mut ir = create_text_node(node_id, "", false);
    add_formatter(&mut ir, node_id, InputFormatter::DigitsOnly);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    clipboard.set_text("ab12c3");
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 0, Some(0));

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
        key_code: KeyCode::Char('v'),
        modifiers: MOD_CTRL,
    });
    assert!(controller.handle_event(&mut ctx, &event));
    assert_eq!(
        ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
        "123"
    );
}

#[test]
fn test_number_keyboard_hint_filters_ime_commit_but_dispatches_text() {
    let node_id = WidgetId::derived(29, &[0]);
    let mut ir = create_text_node(node_id, "", false);
    set_input_type(&mut ir, node_id, TextInputType::Number);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 0, Some(0));

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
    let event = InputEvent::Ime(fission_core::event::ImeEvent::Commit {
        text: "12ab.3".into(),
    });
    assert!(controller.handle_event(&mut ctx, &event));
    assert_eq!(
        ctx.dispatched_actions[0].2.text_change().unwrap().new_text,
        "12.3"
    );
}
