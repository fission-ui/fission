use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::TextContent;
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
    pub placeholder: Option<TextContent>,
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
        let is_focused = cx.runtime_state.interaction.is_focused(input_id);

        let font_size = 16.0; // Still hardcoded for now, should be from theme
        let text_color = IrColor::BLACK;
        let selection_color = IrColor::BLUE; // Text color for selection

        // Resolve placeholder
        let resolved_placeholder = if let Some(ph) = &self.placeholder {
            match ph {
                TextContent::Literal(s) => Some(s.clone()),
                TextContent::Key(key) => Some(cx
                    .env
                    .i18n
                    .get(&cx.env.locale, key)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("MISSING:{}", key))),
            }
        } else {
            None
        };

        // 1. Background
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
            let mut combined = self.value.clone();
            if let Some(pre) = &preedit_text { combined.push_str(pre); }
            let g_count = combined.graphemes(true).count();
            let masked = obs.repeat(g_count);
            
            // Caret mapping not implemented for masked yet, defaulting to end
            (masked, 0, 0) 
        } else {
            let mut combined = self.value.clone();
            if let Some(pre) = &preedit_text { combined.push_str(pre); }
            let (caret, anchor) = if let Some(st) = cx.runtime_state.text_edit.get(input_id) {
                (st.caret, st.anchor)
            } else {
                (0, 0)
            };
            (combined, caret, anchor)
        };

        // Construct Runs
        let mut runs = Vec::new();
        if is_focused && caret != anchor {
            let (s, e) = if caret < anchor { (caret, anchor) } else { (anchor, caret) };
            let s = s.min(display_text.len());
            let e = e.min(display_text.len());
            
            if s > 0 {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[..s].to_string(),
                    style: fission_ir::op::TextStyle { font_size, color: text_color, underline: false },
                });
            }
            if s < e {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[s..e].to_string(),
                    style: fission_ir::op::TextStyle { font_size, color: selection_color, underline: true }, // Visual cue for selection
                });
            }
            if e < display_text.len() {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[e..].to_string(),
                    style: fission_ir::op::TextStyle { font_size, color: text_color, underline: false },
                });
            }
        } else {
            runs.push(fission_ir::op::TextRun {
                text: display_text.clone(),
                style: fission_ir::op::TextStyle { font_size, color: text_color, underline: false },
            });
        }
        
        if display_text.is_empty() && resolved_placeholder.is_some() {
             runs = vec![fission_ir::op::TextRun {
                text: resolved_placeholder.unwrap(),
                style: fission_ir::op::TextStyle { font_size, color: IrColor { r: 150, g: 150, b: 150, a: 255 }, underline: false },
            }];
        }

        let caret_idx = if is_focused && !self.obscure_text { 
            let show = cx.runtime_state.caret_visible.get(&input_id).copied().unwrap_or(true);
            if show { Some(caret.min(display_text.len())) } else { None }
        } else { None };

        let text_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                caret_index: caret_idx,
            })
        ).build(cx);
        
        let mut text_box = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            })
        );
        text_box.add_child(text_id);
        let text_layout_id = text_box.build(cx);

        // 3. Scroll Container
        let scroll_id = cx.next_node_id();
        let mut scroll = NodeBuilder::new(
            scroll_id,
            Op::Layout(LayoutOp::Scroll {
                direction: if self.multiline { FlexDirection::Column } else { FlexDirection::Row },
                show_scrollbar: false,
                width: None, // Let it fill parent padding box
                height: None, 
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            })
        );
        scroll.add_child(text_layout_id);
        let scroll_id = scroll.build(cx);

        // 4. Wrapper (Border + Padding)
        let wrapper_id = cx.next_node_id();
        let mut wrapper = NodeBuilder::new(
            wrapper_id,
            Op::Layout(LayoutOp::Box {
                width: self.width.or(Some(200.0)),
                height: self.height,
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [8.0, 8.0, 4.0, 4.0], // Padding applied here
            })
        );
        wrapper.add_child(background_id); // Fill
        wrapper.add_child(scroll_id);     // Content
        
        let final_id = wrapper.build(cx);

        // 5. Semantics
        let mut semantics = Semantics {
            role: Role::TextInput,
            label: None,
            value: Some(self.value.clone()),
            actions: Default::default(), 
            focusable: true,
            multiline: self.multiline,
            masked: self.obscure_text,
            input_mask: self.mask.clone(),
            ime_preedit_range: None, // TODO: Fix preedit highlighting
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