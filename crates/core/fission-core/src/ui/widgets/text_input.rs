use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color as IrColor, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId, Role, Semantics, FlexDirection
};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInput {
    pub id: Option<NodeId>,
    pub value: String,
    pub placeholder: Option<String>,
    pub on_change: Option<ActionEnvelope>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub multiline: bool,
    pub min_lines: Option<usize>,
    pub max_lines: Option<usize>,
    pub obscure_text: bool,
    pub obscuring_character: char,
    pub mask: Option<fission_ir::semantics::InputMask>,
}

impl Default for TextInput {
    fn default() -> Self {
        Self {
            id: None,
            value: String::new(),
            placeholder: None,
            on_change: None,
            width: None,
            height: None,
            multiline: false,
            min_lines: None,
            max_lines: None,
            obscure_text: false,
            obscuring_character: '•',
            mask: None,
        }
    }
}

impl Lower for TextInput {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let input_id = self.id.unwrap_or_else(|| cx.next_node_id());

        // Use the semantics node id (input_id) for focus checks so the caret reflects focus.
        let is_focused = cx.runtime_state.interaction.is_focused(input_id);

        // Compute base line height for calculations
        let font_size = 16.0; // Default font size
        let line_height = if let Some(measurer) = cx.measurer {
            measurer.measure("Tg", font_size, None).1 // Measure height of typical text
        } else {
            font_size * 1.2 // Fallback approx
        };

        // 1. Background (Paint) - AbsoluteFill
        let stroke_w = if is_focused { 2.0 } else { 1.0 };
        let background_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: IrColor::WHITE }), 
                stroke: Some(Stroke { 
                    color: if is_focused { IrColor::BLUE } else { IrColor::BLACK }, 
                    width: stroke_w 
                }),
                corner_radius: 4.0,
                shadow: None,
            })
        ).build(cx);

        // 2. Text Preparation
        let preedit_text = if is_focused {
            cx.runtime_state.ime_preedit.clone().filter(|(id, _)| *id == input_id).map(|(_, t)| t)
        } else { None };

        let (display_text, caret, anchor) = if self.obscure_text {
            let obs = self.obscuring_character.to_string();
            let obs_len = obs.len();

            let mut combined_val_with_preedit = self.value.clone();
            if let Some(pre) = &preedit_text { combined_val_with_preedit.push_str(pre); }

            let grapheme_count = combined_val_with_preedit.graphemes(true).count();
            let masked_text = obs.repeat(grapheme_count);

            let st_caret = if is_focused {
                 cx.runtime_state.text_edit.get(input_id).map(|s| s.caret).unwrap_or(combined_val_with_preedit.len()).min(combined_val_with_preedit.len())
            } else { 0 };

            let st_anchor = if is_focused {
                 cx.runtime_state.text_edit.get(input_id).map(|s| s.anchor).unwrap_or(combined_val_with_preedit.len()).min(combined_val_with_preedit.len())
            } else { 0 };

            let to_masked_idx = |byte_idx: usize, original_text: &str| {
                let prefix = &original_text[..byte_idx];
                let g_count = prefix.graphemes(true).count();
                g_count * obs_len
            };

            (masked_text, to_masked_idx(st_caret, &combined_val_with_preedit), to_masked_idx(st_anchor, &combined_val_with_preedit))
        } else {
            let mut combined_val_with_preedit = self.value.clone();
            if let Some(pre) = &preedit_text { combined_val_with_preedit.push_str(pre); }

            let st_caret = if is_focused {
                 cx.runtime_state.text_edit.get(input_id).map(|s| s.caret).unwrap_or(combined_val_with_preedit.len()).min(combined_val_with_preedit.len())
            } else { 0 };
            let st_anchor = if is_focused {
                 cx.runtime_state.text_edit.get(input_id).map(|s| s.anchor).unwrap_or(combined_val_with_preedit.len()).min(combined_val_with_preedit.len())
            } else { 0 };

            (combined_val_with_preedit, st_caret, st_anchor)
        };

        let using_placeholder = display_text.is_empty() && !self.obscure_text && preedit_text.is_none(); // Don't show placeholder if preedit is active.
        
        let ime_preedit_range_byte_idx = if is_focused && preedit_text.is_some() && !self.obscure_text {
            let base_len_graphemes = self.value.graphemes(true).count();
            let preedit_len_graphemes = preedit_text.as_ref().map(|s| s.graphemes(true).count()).unwrap_or(0);

            let ime_start_grapheme_idx = base_len_graphemes;
            let ime_end_grapheme_idx = base_len_graphemes + preedit_len_graphemes;

            let to_byte_idx = |g_idx: usize, text: &str| {
                text.graphemes(true).take(g_idx).map(|g| g.len()).sum()
            };
            Some((to_byte_idx(ime_start_grapheme_idx, &display_text), to_byte_idx(ime_end_grapheme_idx, &display_text)))
        } else { None };

        let text_color = if preedit_text.is_some() { IrColor::BLUE } else { IrColor::BLACK };

        // Build segments for selection rendering (if focused and selection non-empty)
        let mut left_layout_id = None;
        let mut sel_layout_id = None;
        let mut right_layout_id = None;

        let has_selection = is_focused && caret != anchor;

        let (s,e) = if caret <= anchor { (caret, anchor) } else { (anchor, caret) };
        let left_str = display_text.get(0..s).unwrap_or("");
        let sel_str = display_text.get(s..e).unwrap_or("");
        let right_str = display_text.get(e..).unwrap_or("");

        if has_selection {
            // Left text
            if !left_str.is_empty() {
                let left_text_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawText { text: left_str.to_string(), size: font_size, color: IrColor::BLACK, underline: false })).build(cx);
                let mut left_box = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4] }));
                left_box.add_child(left_text_id);
                left_layout_id = Some(left_box.build(cx));
            }

            // Selected text with background rect (behind)
            if !sel_str.is_empty() {
                let bg_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawRect { fill: Some(Fill { color: IrColor { r: 173, g: 208, b: 255, a: 255 } }), stroke: None, corner_radius: 0.0, shadow: None })).build(cx);
                let sel_text_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawText { text: sel_str.to_string(), size: font_size, color: IrColor::BLACK, underline: false })).build(cx);
                let mut sel_box = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4] }));
                sel_box.add_child(bg_id);
                sel_box.add_child(sel_text_id);
                sel_layout_id = Some(sel_box.build(cx));
            }

            // Right text
            if !right_str.is_empty() {
                let right_text_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawText { text: right_str.to_string(), size: font_size, color: text_color, underline: false })).build(cx);
                let mut right_box = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4] }));
                right_box.add_child(right_text_id);
                right_layout_id = Some(right_box.build(cx));
            }
        } else {
            // No selection: split around caret into left and right
            let left_only = left_str;
            let right_only = right_str; // For no selection, s==e==caret, so right_str is everything after caret

            if !left_only.is_empty() {
                let left_text_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawText { text: left_only.to_string(), size: font_size, color: IrColor::BLACK, underline: false })).build(cx);
                let mut left_box = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4] }));
                left_box.add_child(left_text_id);
                left_layout_id = Some(left_box.build(cx));
            }
            if !right_only.is_empty() {
                let right_text_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawText { text: right_only.to_string(), size: font_size, color: text_color, underline: false })).build(cx);
                let mut right_box = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4] }));
                right_box.add_child(right_text_id);
                right_layout_id = Some(right_box.build(cx));
            }
        }

        // 3. Content container (Flex Row -> Column if multiline)
        let flex_id = cx.next_node_id();
        let flex_direction = if self.multiline { FlexDirection::Column } else { FlexDirection::Row };
        let mut flex_builder = NodeBuilder::new(
            flex_id,
            Op::Layout(LayoutOp::Flex {
                direction: flex_direction,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0, 0.0, 0.0, 0.0],
            }),
        );
        
        // Wrapper (Box) with layout and visuals
        let wrapper_id = cx.next_node_id();

        let wrapper_min_height = self.min_lines.map(|l| l as f32 * line_height);
        let wrapper_max_height = self.max_lines.map(|l| l as f32 * line_height);

        let mut wrapper_builder = NodeBuilder::new(
            wrapper_id,
            Op::Layout(LayoutOp::Box {
                width: self.width.or(Some(200.0)),
                height: self.height, // Explicit height from user takes precedence
                min_width: None, max_width: None,
                min_height: wrapper_min_height,
                max_height: wrapper_max_height,
                padding: [0.0, 0.0, 0.0, 0.0], // Padding moved to inner box
            }),
        );
        
        // Build caret node (if focused and visible)
        let mut caret_id_opt: Option<NodeId> = None;
        if is_focused {
            let caret_visible = cx
                .runtime_state
                .caret_visible
                .get(&input_id)
                .copied()
                .unwrap_or(true);
            if caret_visible {
                let cursor_paint_id = NodeBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: Some(Fill { color: IrColor::BLACK }),
                        stroke: None,
                        corner_radius: 0.0,
                        shadow: None,
                    }),
                )
                .build(cx);

                let mut cursor_layout_builder = NodeBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Box {
                        width: Some(2.0),
                        height: Some(font_size * 1.2), // approximate line height
                        min_width: None, max_width: None, min_height: None, max_height: None,
                        padding: [0.0, 0.0, 0.0, 0.0],
                    }),
                );
                cursor_layout_builder.add_child(cursor_paint_id);
                caret_id_opt = Some(cursor_layout_builder.build(cx));
            }
        }

        // Add children to flex in correct order relative to caret
        if has_selection {
            if caret <= anchor {
                // caret at selection start: left, caret, selection, right
                if let Some(id) = left_layout_id { flex_builder.add_child(id); }
                if let Some(cid) = caret_id_opt { flex_builder.add_child(cid); }
                if let Some(sel) = sel_layout_id { flex_builder.add_child(sel); }
                if let Some(id) = right_layout_id { flex_builder.add_child(id); }
            } else {
                // caret at selection end: left, selection, caret, right
                if let Some(id) = left_layout_id { flex_builder.add_child(id); }
                if let Some(sel) = sel_layout_id { flex_builder.add_child(sel); }
                if let Some(cid) = caret_id_opt { flex_builder.add_child(cid); }
                if let Some(id) = right_layout_id { flex_builder.add_child(id); }
            }
        } else {
            // no selection: left, caret, right (caret added only if focused)
            if let Some(id) = left_layout_id { flex_builder.add_child(id); }
            if let Some(cid) = caret_id_opt { flex_builder.add_child(cid); }
            if let Some(id) = right_layout_id { flex_builder.add_child(id); }
        }
        
        let flex_node_id = flex_builder.build(cx);

        // 3.5 Clip content using a Scroll viewport
        let scroll_id = cx.next_node_id();
        let outer_w = self.width.unwrap_or(200.0);
        let inner_w = (outer_w - (8.0 + 8.0)).max(0.0);

        let scroll_direction = if self.multiline { FlexDirection::Column } else { FlexDirection::Row };
        
        let mut scroll_builder = NodeBuilder::new(
            scroll_id,
            Op::Layout(LayoutOp::Scroll {
                direction: scroll_direction,
                show_scrollbar: false,
                width: Some(inner_w), // Always constrain width for wrapping
                height: if self.multiline { None } else { self.height.map(|h| (h - (8.0 + 4.0)).max(0.0)) },
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
            }),
        );
        scroll_builder.add_child(flex_node_id);
        let scroll_node_id = scroll_builder.build(cx);
        
        // Intermediate Padding Box to separate Border (Wrapper) from Content (Scroll)
        let padding_box_id = cx.next_node_id();
        let mut padding_box_builder = NodeBuilder::new(
            padding_box_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row, // Padding box itself is row to center content
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [8.0, 8.0, 4.0, 4.0], // Padding is here
            })
        );
        
        // Placeholder handling
        if using_placeholder {
            let placeholder_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: self.placeholder.clone().unwrap_or_default(),
                    size: font_size,
                    color: IrColor { r: 150, g: 150, b: 150, a: 255 },
                    underline: false,
                })
            ).build(cx);
            let mut ph_box = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4] })
            );
            ph_box.add_child(placeholder_id);
            let mut ph_abs_builder = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::AbsoluteFill)
            );
            ph_abs_builder.add_child(ph_box.build(cx));
            let ph_abs_id = ph_abs_builder.build(cx);
            
            padding_box_builder.add_child(ph_abs_id);
        }
        
        padding_box_builder.add_child(scroll_node_id);
        let padding_node_id = padding_box_builder.build(cx);
        
        wrapper_builder.add_child(background_id); // Background fills wrapper (Border)
        wrapper_builder.add_child(padding_node_id);  // Content inset by padding
        
        let final_id = wrapper_builder.build(cx);

        // 4. Semantics Wrapper (use input_id for semantics so focus id == input_id)
        let mut semantics = Semantics {
            role: Role::TextInput,
            label: None,
            value: Some(self.value.clone()),
            actions: Default::default(), 
            focusable: true,
            multiline: self.multiline,
            masked: self.obscure_text,
            input_mask: self.mask.clone(),
            ime_preedit_range: ime_preedit_range_byte_idx,
            checked: None,
            disabled: false,
        };
        if let Some(env) = &self.on_change {
             semantics.actions.entries.push(fission_ir::ActionEntry {
                 action_id: env.id.as_u128(),
                 payload_data: None,
             });
        }
        
        let mut semantics_builder = NodeBuilder::new(input_id, Op::Semantics(semantics));
        semantics_builder.add_child(final_id);
        
        semantics_builder.build(cx)
    }
}