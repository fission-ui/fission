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

/// An editable text field with support for single-line and multiline input,
/// syntax highlighting, password masking, and IME composition.
///
/// `TextInput` is the primary text-editing widget. It manages its own scroll
/// container, caret, selection, and (when `styled_runs` is provided)
/// multi-colour syntax-highlighted rendering.
///
/// # Example
///
/// ```rust,ignore
/// let on_change = ctx.bind(TextChanged { .. }, handle_text as fn(&mut S, TextChanged));
///
/// TextInput {
///     value: view.state.query.clone(),
///     placeholder: Some("Search...".into()),
///     on_change: Some(on_change),
///     ..Default::default()
/// }
/// ```
///
/// # Code editor mode
///
/// For embedding in a code editor, enable `borderless`, `capture_tab`,
/// `auto_indent`, and provide `styled_runs` for syntax highlighting:
///
/// ```rust,ignore
/// TextInput {
///     value: source_code.clone(),
///     multiline: true,
///     borderless: true,
///     capture_tab: true,
///     auto_indent: true,
///     styled_runs: Some(highlighted_runs),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInput {
    /// Explicit node identity (used for focus tracking and scroll state).
    pub id: Option<NodeId>,
    /// The current text value (controlled by the application).
    pub value: String,
    /// Placeholder text shown when `value` is empty.
    pub placeholder: Option<TextContent>,
    /// Action dispatched when the text changes.
    pub on_change: Option<ActionEnvelope>,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// When `true`, the input accepts newlines and scrolls vertically.
    pub multiline: bool,
    /// Minimum number of visible lines (multiline only).
    pub min_lines: Option<usize>,
    /// Maximum number of visible lines (multiline only).
    pub max_lines: Option<usize>,
    /// When `true`, display each grapheme as `obscuring_character` (password mode).
    pub obscure_text: bool,
    /// The character used when `obscure_text` is `true` (default: `'•'`).
    pub obscuring_character: char,
    /// Structural input mask (e.g. phone number, date).
    pub mask: Option<fission_ir::semantics::InputMask>,
    /// Pre-styled text runs for syntax highlighting.
    ///
    /// When provided and no selection is active, these runs are rendered instead
    /// of the default single-colour text. The concatenated text of all runs
    /// **must** match `value` exactly.
    pub styled_runs: Option<Vec<fission_ir::op::TextRun>>,
    /// When `true`, the background rect and border are omitted (for embedding
    /// in editor chrome).
    pub borderless: bool,
    /// When `true`, the Tab key inserts whitespace instead of moving focus.
    pub capture_tab: bool,
    /// When `true`, pressing Enter copies the leading whitespace of the current
    /// line (auto-indentation).
    pub auto_indent: bool,
    /// Action dispatched when the caret or selection anchor changes.
    pub on_cursor_change: Option<ActionEnvelope>,
    /// Ranges to highlight in the text (e.g. find-match results).
    ///
    /// Each entry is `(start_byte, end_byte, background_color)`.
    pub highlight_ranges: Vec<(usize, usize, IrColor)>,
}

impl TextInput {
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::TextInput(self)
    }
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
            styled_runs: None,
            borderless: false,
            capture_tab: false,
            auto_indent: false,
            on_cursor_change: None,
            highlight_ranges: Vec::new(),
        }
    }
}

impl Lower for TextInput {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let input_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let is_focused = cx.runtime_state.interaction.is_focused(input_id);
        
        let theme = &cx.env.theme.components.text_input;
        let tokens = &cx.env.theme.tokens;

        let font_size = theme.font_size;
        let text_color = theme.text_color;
        let selection_color = theme.focus_color;
        let border_color = if is_focused { theme.focus_color } else { theme.border_color };
        let border_width = if is_focused { 2.0 } else { theme.border_width };

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

        // 1. Background (skipped in borderless mode)
        let background_id = if self.borderless {
            None
        } else {
            Some(NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(fission_ir::op::Fill::Solid(tokens.colors.background)),
                    stroke: Some(fission_ir::op::Stroke {
                        fill: fission_ir::op::Fill::Solid(border_color),
                        width: border_width,
                        dash_array: None,
                        line_cap: fission_ir::op::LineCap::Butt,
                        line_join: fission_ir::op::LineJoin::Miter,
                    }),
                    corner_radius: theme.radius,
                    shadow: None,
                })
            ).build(cx))
        };

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
                    style: fission_ir::op::TextStyle { font_size, color: text_color, underline: false, background_color: None },
                });
            }
            if s < e {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[s..e].to_string(),
                    style: fission_ir::op::TextStyle { font_size, color: selection_color, underline: true, background_color: None }, // Visual cue for selection
                });
            }
            if e < display_text.len() {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[e..].to_string(),
                    style: fission_ir::op::TextStyle { font_size, color: text_color, underline: false, background_color: None },
                });
            }
        } else if let Some(styled) = &self.styled_runs {
            // Use pre-styled syntax-highlighted runs
            runs = styled.clone();
        } else {
            runs.push(fission_ir::op::TextRun {
                text: display_text.clone(),
                style: fission_ir::op::TextStyle { font_size, color: text_color, underline: false, background_color: None },
            });
        }

        // Apply highlight_ranges by splitting existing runs at highlight boundaries
        if !self.highlight_ranges.is_empty() && !runs.is_empty() {
            let mut final_runs = Vec::new();
            let mut run_start_byte: usize = 0;

            for run in runs {
                let run_end_byte = run_start_byte + run.text.len();
                let mut cuts = Vec::new();

                for &(hs, he, color) in &self.highlight_ranges {
                    let overlap_start = hs.max(run_start_byte);
                    let overlap_end = he.min(run_end_byte);
                    if overlap_start < overlap_end {
                        cuts.push((overlap_start - run_start_byte, overlap_end - run_start_byte, color));
                    }
                }

                if cuts.is_empty() {
                    final_runs.push(run);
                } else {
                    cuts.sort_by_key(|c| c.0);
                    let mut pos = 0usize;
                    for (cs, ce, bg_color) in cuts {
                        if cs > pos {
                            final_runs.push(fission_ir::op::TextRun {
                                text: run.text[pos..cs].to_string(),
                                style: run.style.clone(),
                            });
                        }
                        let mut hl_style = run.style.clone();
                        hl_style.background_color = Some(bg_color);
                        final_runs.push(fission_ir::op::TextRun {
                            text: run.text[cs..ce].to_string(),
                            style: hl_style,
                        });
                        pos = ce;
                    }
                    if pos < run.text.len() {
                        final_runs.push(fission_ir::op::TextRun {
                            text: run.text[pos..].to_string(),
                            style: run.style.clone(),
                        });
                    }
                }
                run_start_byte = run_end_byte;
            }
            runs = final_runs;
        }

        if display_text.is_empty() && resolved_placeholder.is_some() {
             runs = vec![fission_ir::op::TextRun {
                text: resolved_placeholder.unwrap(),
                style: fission_ir::op::TextStyle { font_size, color: theme.placeholder_color, underline: false, background_color: None },
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
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
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
                flex_grow: 1.0,
                flex_shrink: 1.0,
            })
        );
        scroll.add_child(text_layout_id);
        let scroll_id = scroll.build(cx);

        // 4. Wrapper (Border + Padding)
        let wrapper_id = cx.next_node_id();
        let mut wrapper = NodeBuilder::new(
            wrapper_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height.or(if self.multiline { None } else { Some(theme.height) }),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [theme.padding_h, theme.padding_h, 4.0, 4.0], // Padding applied here
                flex_grow: if self.width.is_none() { 1.0 } else { 0.0 },
                flex_shrink: 1.0,
                aspect_ratio: None,
            })
        );
        if let Some(bg_id) = background_id {
            wrapper.add_child(bg_id); // Fill
        }
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
            capture_tab: self.capture_tab,
            auto_indent: self.auto_indent,
        };
        if let Some(env) = &self.on_change {
             semantics.actions.entries.push(fission_ir::ActionEntry {
                 trigger: fission_ir::semantics::ActionTrigger::Change,
                 action_id: env.id.as_u128(),
                 payload_data: None,
             });
        }
        if let Some(env) = &self.on_cursor_change {
             semantics.actions.entries.push(fission_ir::ActionEntry {
                 trigger: fission_ir::semantics::ActionTrigger::CursorChange,
                 action_id: env.id.as_u128(),
                 payload_data: None,
             });
        }
        let mut semantics_builder = NodeBuilder::new(input_id, Op::Semantics(semantics));
        semantics_builder.add_child(final_id);
        semantics_builder.build(cx)
    }
}
