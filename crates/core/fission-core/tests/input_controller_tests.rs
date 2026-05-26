use fission_core::env::{Clipboard, InteractionStateMap, ScrollStateMap, TextEditStateMap};
use fission_core::event::{
    InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent, MOD_CTRL, MOD_SHIFT, MOD_SUPER,
};
use fission_core::input::text::TextInputController;
use fission_core::input::{ControllerContext, InputController};
use fission_core::ui::widgets::text_input::{
    DragStartBehavior, TextContextMenuAction, TextInputRuntimeConfig, TextUndoController,
};
use fission_ir::op::{Color, TextRun, TextStyle};
use fission_ir::{
    semantics::{
        ActionTrigger, InputFormatter, MaxLengthEnforcement, TextCapitalization, TextInputType,
    },
    ActionEntry, ActionSet, CoreIR, NodeId, Op, Role, Semantics,
};
use fission_layout::{
    LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LineMetric,
    TextMeasurer,
};
use std::sync::{Arc, Mutex};
use unicode_segmentation::UnicodeSegmentation;

struct MockClipboard {
    text: Mutex<String>,
}

impl MockClipboard {
    fn new() -> Self {
        Self {
            text: Mutex::new(String::new()),
        }
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
                if g == "\n" {
                    num_lines += 1;
                    current_line_width = 0.0;
                    continue;
                }
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

    fn hit_test(
        &self,
        text: &str,
        _font_size: f32,
        available_width: Option<f32>,
        x: f32,
        y: f32,
    ) -> usize {
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

                    for (g_offset, g) in text[current_line_start_byte_idx..].grapheme_indices(true)
                    {
                        if current_char_count >= char_idx_on_line
                            || current_line_start_byte_idx + g_offset >= text.len()
                            || g == "\n"
                        {
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

    fn get_line_metrics(
        &self,
        text: &str,
        _font_size: f32,
        available_width: Option<f32>,
    ) -> Vec<LineMetric> {
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

    fn get_caret_position(
        &self,
        text: &str,
        _font_size: f32,
        available_width: Option<f32>,
        caret_index: usize,
    ) -> (f32, f32) {
        let char_width = 10.0;
        let line_height = 20.0;

        let mut current_x = 0.0;
        let mut current_y = 0.0;

        if let Some(aw) = available_width {
            let mut current_line_width = 0.0; // in grapheme width, not actual pixels for now
            for (grapheme_byte_offset, grapheme) in text.grapheme_indices(true) {
                if grapheme_byte_offset >= caret_index {
                    break;
                }

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
                if grapheme_byte_offset >= caret_index {
                    break;
                }
                current_x += grapheme.len() as f32 * char_width;
            }
        }
        (current_x, current_y + line_height * 0.8) // Return baseline y
    }
}

struct FineTextMeasurer;

impl TextMeasurer for FineTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (text.graphemes(true).count() as f32, 20.0)
    }

    fn hit_test(
        &self,
        text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        x: f32,
        _y: f32,
    ) -> usize {
        let target = x.max(0.0).floor() as usize;
        let mut byte_offset = 0;
        for (index, (idx, grapheme)) in text.grapheme_indices(true).enumerate() {
            if index >= target {
                break;
            }
            byte_offset = idx + grapheme.len();
        }
        byte_offset
    }

    fn get_line_metrics(
        &self,
        text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![LineMetric {
            start_index: 0,
            end_index: text.len(),
            baseline: 16.0,
            height: 20.0,
            width: text.graphemes(true).count() as f32,
        }]
    }

    fn get_caret_position(
        &self,
        text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        caret_index: usize,
    ) -> (f32, f32) {
        let mut x = 0.0;
        for (idx, _) in text.grapheme_indices(true) {
            if idx >= caret_index {
                break;
            }
            x += 1.0;
        }
        (x, 0.0)
    }
}

fn setup_ctx<'a>(
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    text_edit: &'a mut TextEditStateMap,
    interaction: &'a mut InteractionStateMap,
    scroll: &'a mut ScrollStateMap,
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
        gesture,
        clipboard: Some(clipboard),
        measurer,
        dispatched_actions: Vec::new(),
    }
}

fn create_text_node(id: NodeId, val: &str, multiline: bool) -> CoreIR {
    let mut ir = CoreIR::default();
    ir.nodes.insert(
        id,
        fission_ir::CoreNode {
            id,
            parent: None,
            children: vec![],
            op: Op::Semantics(Semantics {
                role: Role::TextInput,
                value: Some(val.to_string()),
                label: None,
                identifier: None,
                actions: ActionSet {
                    entries: vec![ActionEntry {
                        trigger: ActionTrigger::Change,
                        action_id: 1,
                        payload_data: None,
                    }],
                },
                action_scope_id: None,
                focusable: true,
                multiline,
                masked: false,
                input_mask: None,
                ime_preedit_range: None,
                checked: None,
                disabled: false,
                read_only: false,
                autofocus: false,
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
                text_input_type: fission_ir::semantics::TextInputType::Text,
                text_input_action: fission_ir::semantics::TextInputAction::Done,
                text_capitalization: fission_ir::semantics::TextCapitalization::None,
                max_length: None,
                max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
                input_formatters: Vec::new(),
                autocorrect: true,
                enable_suggestions: true,
                spell_check: true,
                smart_dashes: true,
                smart_quotes: true,
                autofill_hints: Vec::new(),
                scroll_padding: None,
                capture_tab: false,
                auto_indent: false,
            }),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );
    ir
}

fn set_read_only(ir: &mut CoreIR, id: NodeId, read_only: bool) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.read_only = read_only;
        }
    }
}

fn add_submit_action(ir: &mut CoreIR, id: NodeId) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::Submit,
                action_id: 2,
                payload_data: None,
            });
        }
    }
}

fn add_editing_complete_action(ir: &mut CoreIR, id: NodeId) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::EditingComplete,
                action_id: 3,
                payload_data: None,
            });
        }
    }
}

fn set_masked(ir: &mut CoreIR, id: NodeId, masked: bool) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.masked = masked;
        }
    }
}

fn set_capitalization(ir: &mut CoreIR, id: NodeId, capitalization: TextCapitalization) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.text_capitalization = capitalization;
        }
    }
}

fn set_max_length(ir: &mut CoreIR, id: NodeId, max_length: usize) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.max_length = Some(max_length);
            semantics.max_length_enforcement = MaxLengthEnforcement::Enforced;
        }
    }
}

fn add_formatter(ir: &mut CoreIR, id: NodeId, formatter: InputFormatter) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.input_formatters.push(formatter);
        }
    }
}

fn set_input_type(ir: &mut CoreIR, id: NodeId, input_type: TextInputType) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.text_input_type = input_type;
        }
    }
}

fn set_change_trigger(ir: &mut CoreIR, id: NodeId, trigger: ActionTrigger) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            if let Some(entry) = semantics
                .actions
                .entries
                .iter_mut()
                .find(|entry| entry.trigger == ActionTrigger::Change)
            {
                entry.trigger = trigger;
            }
        }
    }
}

fn create_rich_text_input_tree(
    input_id: NodeId,
    scroll_id: NodeId,
    text_id: NodeId,
    val: &str,
    multiline: bool,
) -> CoreIR {
    let mut ir = CoreIR::default();

    ir.nodes.insert(
        input_id,
        fission_ir::CoreNode {
            id: input_id,
            parent: None,
            children: vec![scroll_id],
            op: Op::Semantics(Semantics {
                role: Role::TextInput,
                value: Some(val.to_string()),
                label: None,
                identifier: None,
                actions: ActionSet {
                    entries: vec![ActionEntry {
                        trigger: ActionTrigger::Change,
                        action_id: 1,
                        payload_data: None,
                    }],
                },
                action_scope_id: None,
                focusable: true,
                multiline,
                masked: false,
                input_mask: None,
                ime_preedit_range: None,
                checked: None,
                disabled: false,
                read_only: false,
                autofocus: false,
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
                text_input_type: fission_ir::semantics::TextInputType::Text,
                text_input_action: fission_ir::semantics::TextInputAction::Done,
                text_capitalization: fission_ir::semantics::TextCapitalization::None,
                max_length: None,
                max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
                input_formatters: Vec::new(),
                autocorrect: true,
                enable_suggestions: true,
                spell_check: true,
                smart_dashes: true,
                smart_quotes: true,
                autofill_hints: Vec::new(),
                scroll_padding: None,
                capture_tab: false,
                auto_indent: false,
            }),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );

    ir.nodes.insert(
        scroll_id,
        fission_ir::CoreNode {
            id: scroll_id,
            parent: Some(input_id),
            children: vec![text_id],
            op: Op::Layout(fission_ir::LayoutOp::Scroll {
                direction: if multiline {
                    fission_ir::op::FlexDirection::Column
                } else {
                    fission_ir::op::FlexDirection::Row
                },
                show_scrollbar: false,
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );

    ir.nodes.insert(
        text_id,
        fission_ir::CoreNode {
            id: text_id,
            parent: Some(scroll_id),
            children: vec![],
            op: Op::Paint(fission_ir::PaintOp::DrawRichText {
                runs: vec![TextRun {
                    text: val.to_string(),
                    style: TextStyle {
                        font_size: 16.0,
                        color: Color::BLACK,
                        underline: false,
                        font_family: None,
                        locale: None,
                        font_weight: 400,
                        font_style: fission_ir::op::FontStyle::Normal,
                        line_height: None,
                        letter_spacing: 0.0,
                        background_color: None,
                    },
                }],
                wrap: true,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: None,
            }),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );

    ir.root = Some(input_id);
    ir
}

fn attach_text_input_runtime_config(
    ir: &mut CoreIR,
    input_id: NodeId,
    config: TextInputRuntimeConfig,
) {
    ir.custom_render_objects.insert(input_id, Arc::new(config));
}

fn attach_focusable_overlay_node(
    ir: &mut CoreIR,
    layout: &mut LayoutSnapshot,
    parent_id: NodeId,
    node_id: NodeId,
    rect: LayoutRect,
) {
    ir.nodes.insert(
        node_id,
        fission_ir::CoreNode {
            id: node_id,
            parent: Some(parent_id),
            children: vec![],
            op: Op::Semantics(Semantics {
                focusable: true,
                ..Default::default()
            }),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );
    ir.nodes
        .get_mut(&parent_id)
        .expect("parent node")
        .children
        .push(node_id);
    layout.nodes.insert(
        node_id,
        LayoutNodeGeometry {
            rect,
            content_size: rect.size,
        },
    );
}

fn test_text_input_selection_handle_id(
    input_id: NodeId,
    kind: fission_core::env::TextSelectionHandleKind,
) -> NodeId {
    let suffix = match kind {
        fission_core::env::TextSelectionHandleKind::Caret => 0,
        fission_core::env::TextSelectionHandleKind::Start => 1,
        fission_core::env::TextSelectionHandleKind::End => 2,
    };
    NodeId::derived(input_id.as_u128(), &[900, suffix])
}

fn test_text_input_toolbar_button_id(input_id: NodeId, action: TextContextMenuAction) -> NodeId {
    let suffix = match action {
        TextContextMenuAction::Copy => 0,
        TextContextMenuAction::Cut => 1,
        TextContextMenuAction::Paste => 2,
        TextContextMenuAction::SelectAll => 3,
    };
    NodeId::derived(input_id.as_u128(), &[901, suffix])
}

#[test]
fn test_text_input_typing() {
    let node_id = NodeId::derived(1, &[0]);
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

    let (target, env, _input) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    let new_text: String = serde_json::from_slice(&env.payload).unwrap();
    assert_eq!(new_text, "Hello!");

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.caret, 6);
}

#[test]
fn test_text_input_typing_without_relayout_does_not_drop_chars() {
    let node_id = NodeId::derived(1, &[0]);
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

    let first_payload: String =
        serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    let second_payload: String =
        serde_json::from_slice(&ctx.dispatched_actions[1].1.payload).unwrap();
    assert_eq!(first_payload, "a");
    assert_eq!(second_payload, "ab");

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.buffer.to_string(), "ab");
    assert_eq!(st.caret, 2);
}

#[test]
fn test_text_input_copy_paste() {
    let node_id = NodeId::derived(1, &[0]);
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
            modifiers: 8,
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
            modifiers: 8,
        });
        assert!(controller.handle_event(&mut ctx, &event));

        let new_text: String =
            serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
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

        let new_text: String =
            serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, "Hi ");

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
    let node_id = NodeId::derived(1, &[0]);
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
            modifiers: 2,
        }); // Alt
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
            modifiers: 2,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 6);
    }
}

#[test]
fn test_word_navigation_skips_non_word_segments() {
    let node_id = NodeId::derived(35, &[0]);
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
            modifiers: 2,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, expected);
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
    let node_id = NodeId::derived(20, &[0]);
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

    let primary_mod = if cfg!(any(target_os = "macos", target_os = "ios")) {
        MOD_SUPER
    } else {
        MOD_CTRL
    };

    let event = InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('a'),
        modifiers: primary_mod,
    });
    assert!(controller.handle_event(&mut ctx, &event));

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.anchor, 0);
    assert_eq!(st.caret, initial_text.len());
}

#[test]
fn test_forward_delete_removes_next_grapheme() {
    let node_id = NodeId::derived(21, &[0]);
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

    let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    assert_eq!(new_text, "acd");

    let st = ctx.text_edit.get(node_id).unwrap();
    assert_eq!(st.caret, 1);
    assert_eq!(st.anchor, 1);
}

#[test]
fn test_apple_ctrl_bindings_cover_line_and_char_navigation() {
    if !cfg!(any(target_os = "macos", target_os = "ios")) {
        return;
    }

    let node_id = NodeId::derived(211, &[0]);
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
    if !cfg!(any(target_os = "macos", target_os = "ios")) {
        return;
    }

    let node_id = NodeId::derived(213, &[0]);
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
            modifiers: MOD_SUPER,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let new_text: String =
            serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, " world");
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 0);
    }

    text_edit.set_caret(node_id, 6, Some(6));

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
            key_code: KeyCode::Delete,
            modifiers: MOD_SUPER,
        });
        assert!(controller.handle_event(&mut ctx, &event));
        let new_text: String =
            serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
        assert_eq!(new_text, "hello ");
        let st = ctx.text_edit.get(node_id).unwrap();
        assert_eq!(st.caret, 6);
    }
}

#[test]
fn test_page_up_down_navigate_by_viewport_height() {
    let input_id = NodeId::derived(212, &[0]);
    let scroll_id = NodeId::derived(212, &[1]);
    let text_id = NodeId::derived(212, &[2]);
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
    let node_id = NodeId::derived(22, &[0]);
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
    let node_id = NodeId::derived(23, &[0]);
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
    let node_id = NodeId::derived(24, &[0]);
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
    let node_id = NodeId::derived(29, &[0]);
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
    let node_id = NodeId::derived(25, &[0]);
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
    let node_id = NodeId::derived(26, &[0]);
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
    let node_id = NodeId::derived(27, &[0]);
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
    let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    assert_eq!(new_text, "hello W");
}

#[test]
fn test_digits_only_formatter_filters_paste() {
    let node_id = NodeId::derived(28, &[0]);
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
    let paste_mod = if cfg!(any(target_os = "macos", target_os = "ios")) {
        MOD_SUPER
    } else {
        MOD_CTRL
    };
    let event = InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('v'),
        modifiers: paste_mod,
    });
    assert!(controller.handle_event(&mut ctx, &event));
    let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    assert_eq!(new_text, "123");
}

#[test]
fn test_number_keyboard_hint_filters_ime_commit_but_dispatches_text() {
    let node_id = NodeId::derived(29, &[0]);
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
    let new_text: String = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    assert_eq!(new_text, "12.3");
}

#[test]
fn test_explicit_number_change_trigger_dispatches_float_payload() {
    let node_id = NodeId::derived(29, &[2]);
    let mut ir = create_text_node(node_id, "", false);
    set_input_type(&mut ir, node_id, TextInputType::Number);
    set_change_trigger(&mut ir, node_id, ActionTrigger::NumberChange);
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
    let new_val: f32 = serde_json::from_slice(&ctx.dispatched_actions[0].1.payload).unwrap();
    assert_eq!(new_val, 12.3);
}

#[test]
fn test_explicit_number_change_trigger_skips_invalid_float_commit() {
    let node_id = NodeId::derived(29, &[1]);
    let mut ir = create_text_node(node_id, "", false);
    set_input_type(&mut ir, node_id, TextInputType::Number);
    set_change_trigger(&mut ir, node_id, ActionTrigger::NumberChange);
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
    let event = InputEvent::Ime(fission_core::event::ImeEvent::Commit { text: "-".into() });

    assert!(controller.handle_event(&mut ctx, &event));
    assert!(
        ctx.dispatched_actions.is_empty(),
        "invalid numeric text should update editing state without dispatching a f32 payload"
    );
}

#[test]
fn test_single_line_auto_scroll_with_rich_text_uses_local_coordinates() {
    let input_id = NodeId::derived(10, &[0]);
    let scroll_id = NodeId::derived(10, &[1]);
    let text_id = NodeId::derived(10, &[2]);
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
    let input_id = NodeId::derived(210, &[0]);
    let scroll_id = NodeId::derived(210, &[1]);
    let text_id = NodeId::derived(210, &[2]);
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
    let input_id = NodeId::derived(211, &[0]);
    let scroll_id = NodeId::derived(211, &[1]);
    let text_id = NodeId::derived(211, &[2]);
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
    ctx.interaction.pressed.insert(input_id, true);
    ctx.interaction.last_down_point = Some(LayoutPoint::new(200.0, 44.0));

    let drag = InputEvent::Pointer(PointerEvent::Move {
        point: LayoutPoint::new(201.0, 44.0),
        modifiers: 0,
    });
    assert!(controller.handle_event(&mut ctx, &drag));

    let state = ctx.text_edit.get(input_id).expect("text state");
    assert_eq!(state.caret, 1);
}

#[test]
fn test_restoration_id_restores_local_edit_state() {
    let first_id = NodeId::derived(212, &[0]);
    let second_id = NodeId::derived(212, &[1]);
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
    let input_id = NodeId::derived(213, &[0]);
    let scroll_id = NodeId::derived(213, &[1]);
    let text_id = NodeId::derived(213, &[2]);
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
        modifiers: MOD_SUPER,
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
    let input_id = NodeId::derived(11, &[0]);
    let scroll_id = NodeId::derived(11, &[1]);
    let text_id = NodeId::derived(11, &[2]);
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
    let input_id = NodeId::derived(28, &[0]);
    let scroll_id = NodeId::derived(28, &[1]);
    let text_id = NodeId::derived(28, &[2]);
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
    let input_id = NodeId::derived(31, &[0]);
    let scroll_id = NodeId::derived(31, &[1]);
    let text_id = NodeId::derived(31, &[2]);
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
    let input_id = NodeId::derived(34, &[0]);
    let scroll_id = NodeId::derived(34, &[1]);
    let text_id = NodeId::derived(34, &[2]);
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
    let input_id = NodeId::derived(32, &[0]);
    let scroll_id = NodeId::derived(32, &[1]);
    let text_id = NodeId::derived(32, &[2]);
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
    let input_id = NodeId::derived(33, &[0]);
    let scroll_id = NodeId::derived(33, &[1]);
    let text_id = NodeId::derived(33, &[2]);
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
    let input_id = NodeId::derived(30, &[0]);
    let scroll_id = NodeId::derived(30, &[1]);
    let text_id = NodeId::derived(30, &[2]);
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
        point: LayoutPoint::new(235.0, 44.0),
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

#[test]
fn test_multiline_enter_key() {
    let node_id = NodeId::derived(1, &[0]);
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

    let (target, env, _input) = &ctx.dispatched_actions[0];
    assert_eq!(*target, node_id);
    let new_text: String = serde_json::from_slice(&env.payload).unwrap();
    assert_eq!(new_text, "Line One\n");
    assert_eq!(
        ctx.text_edit.get(node_id).unwrap().caret,
        "Line One\n".len()
    );
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
