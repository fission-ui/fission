use fission_core::input::{ControllerContext, InputController};
use fission_core::input::text::TextInputController;
use fission_core::env::{Clipboard, InteractionStateMap, ScrollStateMap, TextEditStateMap};
use fission_core::event::{InputEvent, KeyCode, KeyEvent};
use fission_ir::{CoreIR, NodeId, Op, Semantics, Role, ActionSet, ActionEntry};
use fission_layout::{LayoutSnapshot, LayoutSize};
use std::sync::{Arc, Mutex};

struct MockClipboard {
    text: Mutex<String>,
}

impl MockClipboard {
    fn new() -> Self {
        Self { text: Mutex::new(String::new()) }
    }
}

impl Clipboard for MockClipboard {
    fn get_text(&self) -> Option<String> {
        Some(self.text.lock().unwrap().clone())
    }
    fn set_text(&self, text: &str) {
        *self.text.lock().unwrap() = text.to_string();
    }
}

fn setup_ctx<'a>(
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    text_edit: &'a mut TextEditStateMap,
    interaction: &'a mut InteractionStateMap,
    scroll: &'a mut ScrollStateMap,
    ime_preedit: &'a mut Option<(NodeId, String)>,
    clipboard: &'a Arc<dyn Clipboard>,
) -> ControllerContext<'a> {
    ControllerContext {
        ir,
        layout,
        text_edit,
        interaction,
        scroll,
        ime_preedit,
        clipboard: Some(clipboard),
        measurer: None,
        dispatched_actions: Vec::new(),
    }
}

fn create_text_node(id: NodeId, val: &str) -> CoreIR {
    let mut ir = CoreIR::default();
    ir.nodes.insert(id, fission_ir::CoreNode {
        id,
        parent: None,
        children: vec![],
        op: Op::Semantics(Semantics {
            role: Role::TextInput,
            value: Some(val.to_string()),
            label: None,
            actions: ActionSet { entries: vec![ActionEntry { action_id: 1, payload_data: None }] },
            focusable: true,
        }),
        hash: 0,
    });
    ir
}

#[test]
fn test_text_input_typing() {
    let node_id = NodeId::derived(1, &[0]);
    let ir = create_text_node(node_id, "Hello");
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
    let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('!'), modifiers: 0 });
    assert!(controller.handle_event(&mut ctx, &event));
    
    let (target, env) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    let new_text: String = serde_json::from_slice(&env.payload).unwrap();
    assert_eq!(new_text, "Hello!");
    
    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.caret, 6);
}

#[test]
fn test_text_input_copy_paste() {
    let node_id = NodeId::derived(1, &[0]);
    let ir = create_text_node(node_id, "SelectMe");
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 6, Some(0)); // Select "Select"

    let mut controller = TextInputController;

    // Cmd+C
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('c'), modifiers: 8 });
        assert!(controller.handle_event(&mut ctx, &event));
        assert_eq!(clipboard.get_text().as_deref(), Some("Select"));
    }

    text_edit.set_caret(node_id, 8, Some(8)); // "SelectMe|"

    // Cmd+V
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('v'), modifiers: 8 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, "SelectMeSelect");
    }
}

#[test]
fn test_emoji_navigation_and_deletion() {
    let node_id = NodeId::derived(1, &[0]);
    // "Hi 🧘🏻‍♂️"
    // 'H', 'i', ' ', [Emoji Cluster]
    let initial_text = "Hi 🧘🏻‍♂️";
    let ir = create_text_node(node_id, initial_text);
    
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());

    interaction.set_focused(Some(node_id));
    // Caret at end.
    let len = initial_text.len(); 
    text_edit.set_caret(node_id, len, Some(len));

    let mut controller = TextInputController;

    // Backspace should delete the entire emoji
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Backspace, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, "Hi "); // Emoji gone
        
        // Simulating result for state
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 3); // "Hi " len is 3
    }
    
    // Reset
    text_edit.set_caret(node_id, len, Some(len));
    
    // Left arrow should jump over emoji
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Left, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let st = ctx.text_edit.get(node_id).unwrap();
        // Position should be start of emoji (index 3)
        assert_eq!(st.caret, 3);
        assert_eq!(st.anchor, 3); // Collapsed
    }
}

#[test]
fn test_word_navigation() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "hello world code";
    let ir = create_text_node(node_id, initial_text);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());

    interaction.set_focused(Some(node_id));
    // Caret at end
    let len = initial_text.len();
    text_edit.set_caret(node_id, len, Some(len));

    let mut controller = TextInputController;

    // Alt+Left -> "hello world |code" (Start of "code")? 
    // Or previous word boundary?
    // "code" -> boundary is index 12 ('c').
    // Logic: skips non-whitespace, then whitespace? Or vice versa.
    // `prev_word_boundary`: 
    // 1. skip whitespace backward (none).
    // 2. skip alphanumeric backward ("code").
    // 3. stop at space.
    // Result: 12.
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Left, modifiers: 2 }); // Alt
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 12); // "hello world " is 12 chars
    }
    
    // Alt+Left again -> "hello |world code"
    // " " -> skip whitespace (11). "world" -> skip alpha (6).
    // Result: 6.
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Left, modifiers: 2 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 6); // "hello " is 6
    }
}

#[test]
fn test_selection_mechanics() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "ABCD";
    let ir = create_text_node(node_id, initial_text);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 0, Some(0)); // "|ABCD"

    let mut controller = TextInputController;

    // Shift+Right -> "A|BCD" with selection [0,1)
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Right, modifiers: 1 }); // Shift
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 1);
        assert_eq!(st.anchor, 0); // Anchor stayed at 0
    }

    // Shift+Right again -> "AB|CD" with selection [0,2)
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Right, modifiers: 1 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 2);
        assert_eq!(st.anchor, 0);
    }

    // Type 'X' -> Replace selection -> "XCD"
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('X'), modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, "XCD");
        
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 1); // After 'X'
        assert_eq!(st.anchor, 1); // Collapsed
    }
}

#[test]
fn test_home_end_navigation() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "Start to End";
    let ir = create_text_node(node_id, initial_text);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5)); // Middle

    let mut controller = TextInputController;

    // Home
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Home, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 0);
    }

    // End
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &clipboard);
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::End, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, initial_text.len());
    }
}
