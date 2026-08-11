use fission_core::env::{
    Clipboard, ContextMenuState, ImeHandler, InteractionStateMap, ScrollStateMap,
    SelectableTextStateMap, TextEditStateMap,
};
use fission_core::event::{
    ImeEvent, InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent, MOD_ALT, MOD_CTRL,
    MOD_SHIFT, MOD_SUPER,
};
use fission_core::input::text::TextInputController;
use fission_core::input::{ControllerContext, InputController};
use fission_core::ui::widgets::text_input::{
    DragStartBehavior, TextInputRuntimeConfig, TextUndoController,
};
use fission_core::ui::TextContextMenuAction;
use fission_core::Runtime;
use fission_ir::op::{Color, TextRun, TextStyle};
use fission_ir::{
    semantics::{
        ActionTrigger, InputFormatter, MaxLengthEnforcement, TextCapitalization, TextInputType,
    },
    ActionEntry, ActionSet, CoreIR, Op, Role, Semantics, WidgetId,
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

fn primary_shortcut_modifier() -> u8 {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        MOD_SUPER
    } else {
        MOD_CTRL
    }
}

fn word_navigation_modifier() -> u8 {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        MOD_ALT
    } else {
        MOD_CTRL
    }
}

#[derive(Default)]
struct RecordingImeHandler {
    allowed: Mutex<Vec<bool>>,
    cursor_areas: Mutex<Vec<LayoutRect>>,
}

impl ImeHandler for RecordingImeHandler {
    fn set_ime_allowed(&self, allowed: bool) {
        self.allowed.lock().unwrap().push(allowed);
    }

    fn set_ime_cursor_area(&self, rect: LayoutRect) {
        self.cursor_areas.lock().unwrap().push(rect);
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
                    let mut byte_offset_on_line = current_line_start_byte_idx;

                    for (current_char_count, (g_offset, g)) in text[current_line_start_byte_idx..]
                        .grapheme_indices(true)
                        .enumerate()
                    {
                        if current_char_count >= char_idx_on_line || g == "\n" {
                            break;
                        }
                        byte_offset_on_line = current_line_start_byte_idx + g_offset + g.len();
                    }
                    return byte_offset_on_line.min(text.len());
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

    fn hit_test_rich(
        &self,
        runs: &[TextRun],
        available_width: Option<f32>,
        x: f32,
        y: f32,
    ) -> usize {
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        let font_size = runs.first().map(|run| run.style.font_size).unwrap_or(16.0);
        self.hit_test(&text, font_size, available_width, x, y)
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
    let selectable_text = Box::leak(Box::new(SelectableTextStateMap::default()));
    let context_menu = Box::leak(Box::new(ContextMenuState::default()));
    ControllerContext {
        ir,
        layout,
        text_edit,
        selectable_text,
        context_menu,
        interaction,
        scroll,
        gesture,
        clipboard: Some(clipboard),
        measurer,
        dispatched_actions: Vec::new(),
    }
}

fn create_text_node(id: WidgetId, val: &str, multiline: bool) -> CoreIR {
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
                focus_policy: fission_ir::FocusPolicy::FocusOnPointer,
                multiline,
                masked: false,
                input_mask: None,
                ime_preedit_range: None,
                ime_preedit_cursor_range: None,
                text_selection: None,
                selectable_text: false,
                context_menu: false,
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

fn set_read_only(ir: &mut CoreIR, id: WidgetId, read_only: bool) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.read_only = read_only;
        }
    }
}

fn add_submit_action(ir: &mut CoreIR, id: WidgetId) {
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

fn add_editing_complete_action(ir: &mut CoreIR, id: WidgetId) {
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

fn set_masked(ir: &mut CoreIR, id: WidgetId, masked: bool) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.masked = masked;
        }
    }
}

fn set_capitalization(ir: &mut CoreIR, id: WidgetId, capitalization: TextCapitalization) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.text_capitalization = capitalization;
        }
    }
}

fn set_max_length(ir: &mut CoreIR, id: WidgetId, max_length: usize) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.max_length = Some(max_length);
            semantics.max_length_enforcement = MaxLengthEnforcement::Enforced;
        }
    }
}

fn add_formatter(ir: &mut CoreIR, id: WidgetId, formatter: InputFormatter) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.input_formatters.push(formatter);
        }
    }
}

fn set_input_type(ir: &mut CoreIR, id: WidgetId, input_type: TextInputType) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.text_input_type = input_type;
        }
    }
}

fn set_change_trigger(ir: &mut CoreIR, id: WidgetId, trigger: ActionTrigger) {
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
    input_id: WidgetId,
    scroll_id: WidgetId,
    text_id: WidgetId,
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
                focus_policy: fission_ir::FocusPolicy::FocusOnPointer,
                multiline,
                masked: false,
                input_mask: None,
                ime_preedit_range: None,
                ime_preedit_cursor_range: None,
                text_selection: None,
                selectable_text: false,
                context_menu: false,
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
    input_id: WidgetId,
    config: TextInputRuntimeConfig,
) {
    ir.custom_render_objects.insert(input_id, Arc::new(config));
}

fn attach_focusable_overlay_node(
    ir: &mut CoreIR,
    layout: &mut LayoutSnapshot,
    parent_id: WidgetId,
    node_id: WidgetId,
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
    input_id: WidgetId,
    kind: fission_core::env::TextSelectionHandleKind,
) -> WidgetId {
    let suffix = match kind {
        fission_core::env::TextSelectionHandleKind::Caret => 0,
        fission_core::env::TextSelectionHandleKind::Start => 1,
        fission_core::env::TextSelectionHandleKind::End => 2,
    };
    WidgetId::derived(input_id.as_u128(), &[900, suffix])
}

fn test_text_input_toolbar_button_id(
    input_id: WidgetId,
    action: TextContextMenuAction,
) -> WidgetId {
    let suffix = match action {
        TextContextMenuAction::Copy => 0,
        TextContextMenuAction::Cut => 1,
        TextContextMenuAction::Paste => 2,
        TextContextMenuAction::SelectAll => 3,
    };
    WidgetId::derived(input_id.as_u128(), &[901, suffix])
}

#[path = "input_controller_tests/editing.rs"]
mod editing;
#[path = "input_controller_tests/ime.rs"]
mod ime;
#[path = "input_controller_tests/multiline.rs"]
mod multiline;
#[path = "input_controller_tests/pointer.rs"]
mod pointer;
