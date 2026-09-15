//! Mock platform services and context builders for text input controller tests.

#![allow(dead_code)]

pub use fission_core::env::{
    Clipboard, ContextMenuState, ImeHandler, InteractionStateMap, ScrollStateMap,
    SelectableTextStateMap, TextEditStateMap,
};
pub use fission_core::event::{
    EditingCommand, ImeEvent, InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent, MOD_ALT,
    MOD_CTRL, MOD_SHIFT, MOD_SUPER,
};
pub use fission_core::input::text::TextInputController;
pub use fission_core::input::{ControllerContext, InputController, TextEditingConvention};
pub use fission_core::ui::widgets::text_input::{
    DragStartBehavior, TextInputRuntimeConfig, TextUndoController,
};
pub use fission_core::ui::TextContextMenuAction;
pub use fission_core::{Runtime, TextEditingValue};
pub use fission_ir::op::{Color, TextRun, TextStyle};
pub use fission_ir::{
    semantics::{
        ActionTrigger, InputFormatter, MaxLengthEnforcement, TextCapitalization, TextInputType,
    },
    ActionEntry, ActionSet, CoreIR, Op, Role, Semantics, WidgetId,
};
pub use fission_layout::{
    LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LineMetric,
    TextMeasurer,
};
pub use std::sync::{Arc, Mutex};
pub use unicode_segmentation::UnicodeSegmentation;

pub struct MockClipboard {
    pub text: Mutex<String>,
}

impl MockClipboard {
    pub fn new() -> Self {
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

pub fn primary_shortcut_modifier() -> u8 {
    MOD_CTRL
}

pub fn word_navigation_modifier() -> u8 {
    MOD_CTRL
}

#[derive(Default)]
pub struct RecordingImeHandler {
    pub allowed: Mutex<Vec<bool>>,
    pub cursor_areas: Mutex<Vec<LayoutRect>>,
    pub editing_values: Mutex<Vec<TextEditingValue>>,
}

impl ImeHandler for RecordingImeHandler {
    fn set_ime_allowed(&self, allowed: bool) {
        self.allowed.lock().unwrap().push(allowed);
    }

    fn set_ime_cursor_area(&self, rect: LayoutRect) {
        self.cursor_areas.lock().unwrap().push(rect);
    }

    fn set_editing_value(&self, value: &TextEditingValue) {
        self.editing_values.lock().unwrap().push(value.clone());
    }
}

pub struct MockTextMeasurer;
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
            text.len()
        } else {
            // Single line behavior
            let char_idx = (x / char_width).floor() as usize;
            let mut byte_offset = 0;
            for (idx, g) in text.grapheme_indices(true).take(char_idx) {
                byte_offset = idx + g.len();
            }
            byte_offset
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

pub struct FineTextMeasurer;

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

#[allow(clippy::too_many_arguments)]
pub fn setup_ctx<'a>(
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    text_edit: &'a mut TextEditStateMap,
    interaction: &'a mut InteractionStateMap,
    scroll: &'a mut ScrollStateMap,
    gesture: &'a mut fission_core::env::GestureState,
    clipboard: &'a Arc<dyn Clipboard>,
    measurer: Option<&'a Arc<dyn TextMeasurer>>,
) -> ControllerContext<'a> {
    setup_ctx_with_convention(
        ir,
        layout,
        text_edit,
        interaction,
        scroll,
        gesture,
        clipboard,
        measurer,
        TextEditingConvention::Standard,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn setup_ctx_with_convention<'a>(
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    text_edit: &'a mut TextEditStateMap,
    interaction: &'a mut InteractionStateMap,
    scroll: &'a mut ScrollStateMap,
    gesture: &'a mut fission_core::env::GestureState,
    clipboard: &'a Arc<dyn Clipboard>,
    measurer: Option<&'a Arc<dyn TextMeasurer>>,
    editing_convention: TextEditingConvention,
) -> ControllerContext<'a> {
    let selectable_text = Box::leak(Box::new(SelectableTextStateMap::default()));
    let context_menu = Box::leak(Box::new(ContextMenuState::default()));
    let viewport = Box::leak(Box::new(fission_core::ViewportStateMap::default()));
    ControllerContext {
        ir,
        layout,
        text_edit,
        selectable_text,
        context_menu,
        interaction,
        scroll,
        viewport,
        gesture,
        editing_convention,
        current_time: 0,
        clipboard: Some(clipboard),
        measurer,
        dispatched_actions: Vec::new(),
    }
}

pub fn create_text_node(id: WidgetId, val: &str, multiline: bool) -> CoreIR {
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
                hyperlink: None,
                popover_target: None,
                actions: ActionSet {
                    entries: vec![ActionEntry {
                        trigger: ActionTrigger::TextChanged,
                        action_id: 1,
                        payload_data: Some(b"null".to_vec()),
                    }],
                },
                action_scope_id: None,
                focusable: true,
                focus_policy: fission_ir::FocusPolicy::FocusOnPointer,
                text_editable: true,
                multiline,
                text_wrap_mode: fission_ir::semantics::TextWrapMode::Soft,
                masked: false,
                input_mask: None,
                ime_preedit_range: None,
                ime_preedit_cursor_range: None,
                text_selection: None,
                selectable_text: false,
                selection_region: None,
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
                canvas_target: None,
                focus_index: None,
                text_input_type: fission_ir::semantics::TextInputType::Text,
                text_input_action: fission_ir::semantics::TextInputAction::Done,
                text_capitalization: fission_ir::semantics::TextCapitalization::None,
                max_length: None,
                max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
                input_formatters: Vec::new(),
                text_field_name: None,
                text_form_id: None,
                autofill_group: None,
                required: false,
                min_length: None,
                validation_pattern: None,
                validation_state: Default::default(),
                validation_message: None,
                autocorrect: true,
                enable_suggestions: true,
                spell_check: true,
                smart_dashes: true,
                smart_quotes: true,
                autofill_hints: Vec::new(),
                scroll_padding: None,
                capture_tab: false,
                auto_indent: false,
                ..Semantics::default()
            }),
            composite: fission_ir::CompositeStyle::default(),
            hash: 0,
        },
    );
    ir
}

pub fn set_read_only(ir: &mut CoreIR, id: WidgetId, read_only: bool) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.read_only = read_only;
        }
    }
}

pub fn add_submit_action(ir: &mut CoreIR, id: WidgetId) {
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

pub fn add_editing_complete_action(ir: &mut CoreIR, id: WidgetId) {
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

pub fn set_masked(ir: &mut CoreIR, id: WidgetId, masked: bool) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.masked = masked;
        }
    }
}

pub fn set_capitalization(ir: &mut CoreIR, id: WidgetId, capitalization: TextCapitalization) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.text_capitalization = capitalization;
        }
    }
}

pub fn set_max_length(ir: &mut CoreIR, id: WidgetId, max_length: usize) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.max_length = Some(max_length);
            semantics.max_length_enforcement = MaxLengthEnforcement::Enforced;
        }
    }
}

pub fn add_formatter(ir: &mut CoreIR, id: WidgetId, formatter: InputFormatter) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.input_formatters.push(formatter);
        }
    }
}

pub fn set_input_type(ir: &mut CoreIR, id: WidgetId, input_type: TextInputType) {
    if let Some(node) = ir.nodes.get_mut(&id) {
        if let Op::Semantics(semantics) = &mut node.op {
            semantics.text_input_type = input_type;
        }
    }
}
