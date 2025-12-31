use fission_core::input::{ControllerContext, InputController};
use fission_core::input::text::TextInputController;
use fission_core::env::{Clipboard, InteractionStateMap, ScrollStateMap, TextEditStateMap};
use fission_core::event::{InputEvent, KeyCode, KeyEvent};
use fission_core::ActionInput;
use fission_ir::{CoreIR, NodeId, Op, Semantics, Role, ActionSet, ActionEntry, semantics::ActionTrigger};
use fission_layout::{LayoutSnapshot, LayoutSize, TextMeasurer, LineMetric};
use std::sync::{Arc, Mutex};
use unicode_segmentation::UnicodeSegmentation;

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

struct MockTextMeasurer;
impl TextMeasurer for MockTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let line_height = 20.0;
        let char_width = 10.0;

        if let Some(aw) = available_width {
            let mut current_line_width = 0.0;
            let mut num_lines = 1;
            for g in text.graphemes(true) {
                if g == "\n" { num_lines += 1; current_line_width = 0.0; continue; }
                let g_width = g.len() as f32 * char_width;
                if current_line_width + g_width > aw {
                    num_lines += 1;
                    current_line_width = g_width;
                } else {
                    current_line_width += g_width;
                }
            }
            (aw, num_lines as f32 * line_height)
        } else {
            (text.len() as f32 * char_width, line_height)
        }
    }

    fn hit_test(&self, text: &str, _font_size: f32, available_width: Option<f32>, x: f32, y: f32) -> usize {
        let char_width = 10.0;
        let line_height = 20.0;

        let mut current_y = 0.0;
        let mut current_line_start_byte_idx = 0;

        if let Some(aw) = available_width {
            let mut current_line_width_chars = 0.0;
            let target_line_y = y;

            for (grapheme_byte_offset, grapheme) in text.grapheme_indices(true) {
                if grapheme == "\n" {
                    current_y += line_height;
                    current_line_width_chars = 0.0;
                    current_line_start_byte_idx = grapheme_byte_offset + grapheme.len();
                    continue;
                }

                if current_y + line_height > target_line_y && current_y <= target_line_y {
                    // This is the target line
                    let char_idx_on_line = (x / char_width).floor() as usize;
                    let mut current_char_count = 0;
                    let mut byte_offset_on_line = current_line_start_byte_idx;

                    for (g_offset, g) in text[current_line_start_byte_idx..].grapheme_indices(true) {
                        if current_char_count >= char_idx_on_line || current_line_start_byte_idx + g_offset >= text.len() || g == "\n"{
                            break;
                        }
                        byte_offset_on_line = current_line_start_byte_idx + g_offset;
                        current_char_count += 1;
                    }
                    return byte_offset_on_line;
                }
                let g_width = grapheme.len() as f32 * char_width;
                if current_line_width_chars + g_width > aw {
                    current_y += line_height;
                    current_line_width_chars = g_width;
                    current_line_start_byte_idx = grapheme_byte_offset;
                } else {
                    current_line_width_chars += g_width;
                }
            }
            // Fallback for last line
            return text.len();

        } else {
            // Single line behavior
            let char_idx = (x / char_width).floor() as usize;
            let mut byte_offset = 0;
            for (idx, g) in text.grapheme_indices(true).take(char_idx) {
                byte_offset = idx + g.len();
            }
            return byte_offset;
        }
    }

    fn get_line_metrics(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> Vec<LineMetric> {
        let char_width = 10.0;
        let line_height = 20.0;
        
        let mut metrics = Vec::new();
        let mut current_start_index = 0;
        let mut current_y = 0.0;

        if let Some(aw) = available_width {
            let mut current_line_width = 0.0;
            for (grapheme_byte_offset, grapheme) in text.grapheme_indices(true) {
                if grapheme == "\n" {
                    metrics.push(fission_layout::LineMetric {
                        start_index: current_start_index,
                        end_index: grapheme_byte_offset + grapheme.len(),
                        baseline: current_y + line_height * 0.8,
                        height: line_height,
                        width: current_line_width,
                    });
                    current_y += line_height;
                    current_line_width = 0.0;
                    current_start_index = grapheme_byte_offset + grapheme.len();
                    continue;
                }

                let g_width = grapheme.len() as f32 * char_width;
                if current_line_width + g_width > aw {
                    // New line due to wrapping
                    metrics.push(fission_layout::LineMetric {
                        start_index: current_start_index,
                        end_index: grapheme_byte_offset,
                        baseline: current_y + line_height * 0.8,
                        height: line_height,
                        width: current_line_width,
                    });
                    current_y += line_height;
                    current_line_width = g_width;
                    current_start_index = grapheme_byte_offset;
                } else {
                    current_line_width += g_width;
                }
            }
            // Add the last line
            metrics.push(fission_layout::LineMetric {
                start_index: current_start_index,
                end_index: text.len(),
                baseline: current_y + line_height * 0.8,
                height: line_height,
                width: current_line_width,
            });
        } else {
            // Single line
            metrics.push(fission_layout::LineMetric {
                start_index: 0,
                end_index: text.len(),
                baseline: line_height * 0.8,
                height: line_height,
                width: text.len() as f32 * char_width,
            });
        }
        metrics
    }

    fn get_caret_position(&self, text: &str, _font_size: f32, available_width: Option<f32>, caret_index: usize) -> (f32, f32) {
        let char_width = 10.0;
        let line_height = 20.0;
        
        let mut current_x = 0.0;
        let mut current_y = 0.0;

        if let Some(aw) = available_width {
            let mut current_line_width = 0.0; // in grapheme width, not actual pixels for now
            for (grapheme_byte_offset, grapheme) in text.grapheme_indices(true) {
                if grapheme_byte_offset >= caret_index { break; }

                if grapheme == "\n" {
                    current_y += line_height;
                    current_x = 0.0;
                    current_line_width = 0.0;
                    continue;
                }

                let g_width = grapheme.len() as f32 * char_width;
                if current_line_width + g_width > aw {
                    current_y += line_height;
                    current_x = g_width;
                    current_line_width = g_width;
                } else {
                    current_x += g_width;
                    current_line_width += g_width;
                }
            }
        } else {
            // Single line behavior
            for (grapheme_byte_offset, grapheme) in text.grapheme_indices(true) {
                if grapheme_byte_offset >= caret_index { break; }
                current_x += grapheme.len() as f32 * char_width;
            }
        }
        (current_x, current_y + line_height * 0.8) // Return baseline y
    }
}

fn setup_ctx<'a>(
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    text_edit: &'a mut TextEditStateMap,
    interaction: &'a mut InteractionStateMap,
    scroll: &'a mut ScrollStateMap,
    ime_preedit: &'a mut Option<(NodeId, String)>,
    gesture: &'a mut fission_core::env::GestureState,
    clipboard: &'a Arc<dyn Clipboard>,
    measurer: Option<&'a Arc<dyn TextMeasurer>>,
) -> ControllerContext<'a> {
    ControllerContext {
        ir,
        layout,
        text_edit,
        interaction,
        scroll,
        ime_preedit,
        gesture,
        clipboard: Some(clipboard),
        measurer,
        dispatched_actions: Vec::new(),
    }
}

fn create_text_node(id: NodeId, val: &str, multiline: bool) -> CoreIR {
    let mut ir = CoreIR::default();
    ir.nodes.insert(id, fission_ir::CoreNode {
        id,
        parent: None,
        children: vec![],
        op: Op::Semantics(Semantics {
            role: Role::TextInput,
            value: Some(val.to_string()),
            label: None,
            actions: ActionSet { entries: vec![ActionEntry { trigger: ActionTrigger::Change, action_id: 1, payload_data: None }] },
            focusable: true,
            multiline,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
            disabled: false,
            draggable: false,
            scrollable_x: false,
            scrollable_y: false,
            min_value: None,
            max_value: None,
            current_value: None,
            is_focus_scope: false,
            is_focus_barrier: false,
            drag_payload: None,
            hero_tag: None,
            focus_index: None,
        }),
        hash: 0,
    });
    ir
}

#[test]
fn test_text_input_typing() {
    let node_id = NodeId::derived(1, &[0]);
    let ir = create_text_node(node_id, "Hello", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5));

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
    let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('!'), modifiers: 0 });
    assert!(controller.handle_event(&mut ctx, &event));
    
    let (target, env, _input) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    let new_text: String = serde_json::from_slice(&env.payload).unwrap();
    assert_eq!(new_text, "Hello!");
    
    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.caret, 6);
}

#[test]
fn test_text_input_copy_paste() {
    let node_id = NodeId::derived(1, &[0]);
    let ir = create_text_node(node_id, "SelectMe", false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 6, Some(0)); // Select "Select"

    let mut controller = TextInputController;

    // Cmd+C
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('c'), modifiers: 8 });
        assert!(controller.handle_event(&mut ctx, &event));
        assert_eq!(clipboard.get_text().as_deref(), Some("Select"));
    }

    text_edit.set_caret(node_id, 8, Some(8)); // "SelectMe|"

    // Cmd+V
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('v'), modifiers: 8 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, "SelectMeSelect");
    }
}

#[test]
fn test_emoji_navigation_and_deletion() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "Hi 🧘🏻‍♂️";
    let ir = create_text_node(node_id, initial_text, false);
    
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    let len = initial_text.len(); 
    text_edit.set_caret(node_id, len, Some(len));

    let mut controller = TextInputController;

    // Backspace should delete the entire emoji
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Backspace, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, "Hi ");
        
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 3);
    }
    
    // Reset
    text_edit.set_caret(node_id, len, Some(len));
    
    // Left arrow should jump over emoji
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Left, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 3);
        assert_eq!(st.anchor, 3);
    }
}

#[test]
fn test_word_navigation() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "hello world code";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    let len = initial_text.len();
    text_edit.set_caret(node_id, len, Some(len));

    let mut controller = TextInputController;

    // Alt+Left -> "hello world |code"
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Left, modifiers: 2 }); // Alt
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 12);
    }
    
    // Alt+Left again -> "hello |world code"
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Left, modifiers: 2 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 6);
    }
}

#[test]
fn test_selection_mechanics() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "ABCD";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 0, Some(0)); // "|ABCD"

    let mut controller = TextInputController;

    // Shift+Right -> "A|BCD" with selection [0,1)
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Right, modifiers: 1 }); // Shift
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 1);
        assert_eq!(st.anchor, 0);
    }

    // Shift+Right again -> "AB|CD" with selection [0,2)
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Right, modifiers: 1 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 2);
        assert_eq!(st.anchor, 0);
    }

    // Type 'X' -> Replace selection -> "XCD"
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Char('X'), modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        
        let (_target, env, _input) = &ctx.dispatched_actions[0];
        let new_text: String = serde_json::from_slice(&env.payload).unwrap();
        assert_eq!(new_text, "XCD");
        
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 1);
        assert_eq!(st.anchor, 1);
    }
}

#[test]
fn test_home_end_navigation() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "Start to End";
    let ir = create_text_node(node_id, initial_text, false);
    let layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, 5, Some(5)); // Middle

    let mut controller = TextInputController;

    // Home
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Home, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 0);
    }

    // End
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::End, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, initial_text.len());
    }
}

#[test]
fn test_multiline_enter_key() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "Line One";
    let ir = create_text_node(node_id, initial_text, true); // Multiline
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0)); // Fixed width for wrapping and calc
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    text_edit.set_caret(node_id, initial_text.len(), Some(initial_text.len())); // Caret at end

    let mut controller = TextInputController;
    let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
    let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Enter, modifiers: 0 });
    assert!(controller.handle_event(&mut ctx, &event));

    let (target, env, _input) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    let new_text: String = serde_json::from_slice(&env.payload).unwrap();
    assert_eq!(new_text, "Line One\n");
    assert_eq!(ctx.text_edit.get(node_id).unwrap().caret, "Line One\n".len());
}

#[test]
#[ignore]
fn test_multiline_vertical_navigation_up_down() {
    let node_id = NodeId::derived(1, &[0]);
    let initial_text = "First Line\nSecond Line\nThird Line";
    let ir = create_text_node(node_id, initial_text, true); // Multiline
    let layout = LayoutSnapshot::new(LayoutSize::new(200.0, 100.0)); // Fixed width for wrapping and calc
    let mut text_edit = TextEditStateMap::default();
    let mut interaction = InteractionStateMap::default();
    let mut scroll = ScrollStateMap::default();
    let mut ime_preedit = None;
    let mut gesture = fission_core::env::GestureState::default();
    let clipboard: Arc<dyn Clipboard> = Arc::new(MockClipboard::new());
    let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);

    interaction.set_focused(Some(node_id));
    // Caret at end of Line Two
    text_edit.set_caret(node_id, "First Line\nSecond Line".len(), Some("First Line\nSecond Line".len())); 

    let mut controller = TextInputController;

    // Move Up
    {
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Up, modifiers: 0 });
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
        let mut ctx = setup_ctx(&ir, &layout, &mut text_edit, &mut interaction, &mut scroll, &mut ime_preedit, &mut gesture, &clipboard, Some(&measurer));
        let event = InputEvent::Keyboard(KeyEvent::Down { key_code: KeyCode::Down, modifiers: 0 });
        assert!(controller.handle_event(&mut ctx, &event));
        // Expect caret to move to same horizontal position on Line Two
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, "First Line\nSecond Line".len());
        assert_eq!(st.anchor, "First Line\nSecond Line".len());
    }
}