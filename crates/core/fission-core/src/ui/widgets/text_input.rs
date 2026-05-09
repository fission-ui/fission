use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::{traits::Lower, Node, TextContent, TextFontStyle};
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color as IrColor, Fill, LayoutOp, Op, PaintOp, Stroke},
    semantics::{
        InputFormatter, MaxLengthEnforcement, TextCapitalization, TextInputAction, TextInputType,
    },
    FlexDirection, FlexWrap, NodeId, Role, Semantics,
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
    /// Action dispatched when the user submits the field (for example by pressing Enter
    /// on a single-line input).
    pub on_submit: Option<ActionEnvelope>,
    /// Action dispatched when editing is explicitly completed.
    pub on_editing_complete: Option<ActionEnvelope>,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Custom content padding `[left, right, top, bottom]`.
    pub padding: Option<[f32; 4]>,
    /// When `true`, the input accepts newlines and scrolls vertically.
    pub multiline: bool,
    /// When `true`, the input requests focus automatically when mounted.
    pub autofocus: bool,
    /// When `false`, the field is non-interactive and does not receive focus.
    pub enabled: bool,
    /// When `true`, the field can be focused and selected but not edited.
    pub read_only: bool,
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
    /// Optional fill override for the field background.
    pub background_fill: Option<Fill>,
    /// Optional border color override when not focused.
    pub border_color: Option<IrColor>,
    /// Optional border color override when focused.
    pub focus_border_color: Option<IrColor>,
    /// Optional border width override when not focused.
    pub border_width: Option<f32>,
    /// Optional border width override when focused.
    pub focus_border_width: Option<f32>,
    /// Optional corner radius override.
    pub border_radius: Option<f32>,
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional text color override.
    pub text_color: Option<IrColor>,
    /// Optional placeholder color override.
    pub placeholder_color: Option<IrColor>,
    /// Optional selection highlight color override.
    pub selection_color: Option<IrColor>,
    /// Optional selected text color override.
    pub selection_text_color: Option<IrColor>,
    /// Optional caret color override.
    pub cursor_color: Option<IrColor>,
    /// Optional caret width override.
    pub cursor_width: Option<f32>,
    /// Optional font family override.
    pub font_family: Option<String>,
    /// Optional font weight override.
    pub font_weight: Option<u16>,
    /// Optional font style override.
    pub font_style: TextFontStyle,
    /// Optional absolute line-height override in layout points.
    pub line_height: Option<f32>,
    /// Optional letter-spacing override in layout points.
    pub letter_spacing: Option<f32>,
    /// Optional leading decoration node.
    pub prefix: Option<Box<Node>>,
    /// Optional trailing decoration node.
    pub suffix: Option<Box<Node>>,
    /// Preferred software keyboard / input modality.
    pub keyboard_type: TextInputType,
    /// Preferred return/submit action.
    pub text_input_action: TextInputAction,
    /// Automatic capitalization strategy for inserted text.
    pub text_capitalization: TextCapitalization,
    /// Maximum number of Unicode scalar values allowed in the field.
    pub max_length: Option<usize>,
    /// Whether `max_length` is enforced during editing.
    pub max_length_enforcement: MaxLengthEnforcement,
    /// Structured input formatters applied to inserted text.
    pub input_formatters: Vec<InputFormatter>,
    /// Hint whether platform autocorrect should be enabled.
    pub autocorrect: bool,
    /// Hint whether platform suggestions should be enabled.
    pub enable_suggestions: bool,
    /// Hint whether platform spell checking should be enabled.
    pub spell_check: bool,
    /// Hint whether smart dashes should be enabled.
    pub smart_dashes: bool,
    /// Hint whether smart quotes should be enabled.
    pub smart_quotes: bool,
    /// Platform autofill categories associated with this field.
    pub autofill_hints: Vec<String>,
}

impl TextInput {
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn padding(mut self, padding: [f32; 4]) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn background_fill(mut self, fill: Fill) -> Self {
        self.background_fill = Some(fill);
        self
    }

    pub fn text_color(mut self, color: IrColor) -> Self {
        self.text_color = Some(color);
        self
    }

    pub fn placeholder_color(mut self, color: IrColor) -> Self {
        self.placeholder_color = Some(color);
        self
    }

    pub fn selection_color(mut self, color: IrColor) -> Self {
        self.selection_color = Some(color);
        self
    }

    pub fn selection_text_color(mut self, color: IrColor) -> Self {
        self.selection_text_color = Some(color);
        self
    }

    pub fn cursor_color(mut self, color: IrColor) -> Self {
        self.cursor_color = Some(color);
        self
    }

    pub fn cursor_width(mut self, width: f32) -> Self {
        self.cursor_width = Some(width);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn keyboard_type(mut self, keyboard_type: TextInputType) -> Self {
        self.keyboard_type = keyboard_type;
        self
    }

    pub fn text_input_action(mut self, action: TextInputAction) -> Self {
        self.text_input_action = action;
        self
    }

    pub fn text_capitalization(mut self, capitalization: TextCapitalization) -> Self {
        self.text_capitalization = capitalization;
        self
    }

    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub fn max_length_enforcement(mut self, enforcement: MaxLengthEnforcement) -> Self {
        self.max_length_enforcement = enforcement;
        self
    }

    pub fn input_formatters(mut self, input_formatters: Vec<InputFormatter>) -> Self {
        self.input_formatters = input_formatters;
        self
    }

    pub fn autocorrect(mut self, autocorrect: bool) -> Self {
        self.autocorrect = autocorrect;
        self
    }

    pub fn enable_suggestions(mut self, enable_suggestions: bool) -> Self {
        self.enable_suggestions = enable_suggestions;
        self
    }

    pub fn spell_check(mut self, spell_check: bool) -> Self {
        self.spell_check = spell_check;
        self
    }

    pub fn smart_dashes(mut self, smart_dashes: bool) -> Self {
        self.smart_dashes = smart_dashes;
        self
    }

    pub fn smart_quotes(mut self, smart_quotes: bool) -> Self {
        self.smart_quotes = smart_quotes;
        self
    }

    pub fn autofill_hints(mut self, autofill_hints: Vec<String>) -> Self {
        self.autofill_hints = autofill_hints;
        self
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.font_weight = Some(weight);
        self
    }

    pub fn italic(mut self, italic: bool) -> Self {
        self.font_style = if italic {
            TextFontStyle::Italic
        } else {
            TextFontStyle::Normal
        };
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = Some(line_height);
        self
    }

    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = Some(letter_spacing);
        self
    }

    pub fn prefix(mut self, node: Node) -> Self {
        self.prefix = Some(Box::new(node));
        self
    }

    pub fn suffix(mut self, node: Node) -> Self {
        self.suffix = Some(Box::new(node));
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
            on_submit: None,
            on_editing_complete: None,
            width: None,
            height: None,
            padding: None,
            multiline: false,
            autofocus: false,
            enabled: true,
            read_only: false,
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
            background_fill: None,
            border_color: None,
            focus_border_color: None,
            border_width: None,
            focus_border_width: None,
            border_radius: None,
            font_size: None,
            text_color: None,
            placeholder_color: None,
            selection_color: None,
            selection_text_color: None,
            cursor_color: None,
            cursor_width: None,
            font_family: None,
            font_weight: None,
            font_style: TextFontStyle::Normal,
            line_height: None,
            letter_spacing: None,
            prefix: None,
            suffix: None,
            keyboard_type: TextInputType::Text,
            text_input_action: TextInputAction::Done,
            text_capitalization: TextCapitalization::None,
            max_length: None,
            max_length_enforcement: MaxLengthEnforcement::Enforced,
            input_formatters: Vec::new(),
            autocorrect: true,
            enable_suggestions: true,
            spell_check: true,
            smart_dashes: true,
            smart_quotes: true,
            autofill_hints: Vec::new(),
        }
    }
}

impl TextInput {
    fn mask_text(text: &str, obscuring_character: char) -> String {
        let mut masked = String::new();
        for _ in text.graphemes(true) {
            masked.push(obscuring_character);
        }
        masked
    }

    fn masked_byte_offset(source: &str, masked: &str, source_byte_offset: usize) -> usize {
        let clamped = source_byte_offset.min(source.len());
        let grapheme_count = source[..clamped].graphemes(true).count();
        masked
            .grapheme_indices(true)
            .nth(grapheme_count)
            .map(|(idx, _)| idx)
            .unwrap_or(masked.len())
    }
}

impl Lower for TextInput {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let input_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let is_focused = cx.runtime_state.interaction.is_focused(input_id);

        let theme = &cx.env.theme.components.text_input;
        let tokens = &cx.env.theme.tokens;

        let font_size = self.font_size.unwrap_or(theme.font_size);
        let text_color = self.text_color.unwrap_or(theme.text_color);
        let selection_color = self
            .selection_color
            .unwrap_or(tokens.colors.primary.with_alpha(52));
        let selection_text_color = self.selection_text_color.unwrap_or(text_color);
        let placeholder_color = self.placeholder_color.unwrap_or(theme.placeholder_color);
        let cursor_color = self.cursor_color.unwrap_or(theme.focus_color);
        let cursor_width = self.cursor_width.unwrap_or(2.0);
        let font_weight = self.font_weight.unwrap_or(400);
        let line_height = self.line_height;
        let letter_spacing = self.letter_spacing.unwrap_or(0.0);
        let border_color = if is_focused {
            self.focus_border_color.unwrap_or(theme.focus_color)
        } else {
            self.border_color.unwrap_or(theme.border_color)
        };
        let border_width = if is_focused {
            self.focus_border_width
                .unwrap_or(self.border_width.unwrap_or(2.0))
        } else {
            self.border_width.unwrap_or(theme.border_width)
        };
        let border_radius = self.border_radius.unwrap_or(theme.radius);
        let content_padding = self
            .padding
            .unwrap_or([theme.padding_h, theme.padding_h, 4.0, 4.0]);
        let base_text_style = fission_ir::op::TextStyle {
            font_size,
            color: text_color,
            underline: false,
            font_family: self.font_family.clone(),
            font_weight,
            font_style: self.font_style.into(),
            line_height,
            letter_spacing,
            background_color: None,
        };

        // Resolve placeholder
        let resolved_placeholder = if let Some(ph) = &self.placeholder {
            match ph {
                TextContent::Literal(s) => Some(s.clone()),
                TextContent::Key(key) => Some(
                    cx.env
                        .i18n
                        .get(&cx.env.locale, key)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("MISSING:{}", key)),
                ),
            }
        } else {
            None
        };

        // 1. Background (skipped in borderless mode)
        let background_id = if self.borderless {
            None
        } else {
            Some(
                NodeBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: Some(
                            self.background_fill
                                .clone()
                                .unwrap_or(Fill::Solid(tokens.colors.background)),
                        ),
                        stroke: Some(Stroke {
                            fill: Fill::Solid(border_color),
                            width: border_width,
                            dash_array: None,
                            line_cap: fission_ir::op::LineCap::Butt,
                            line_join: fission_ir::op::LineJoin::Miter,
                        }),
                        corner_radius: border_radius,
                        shadow: None,
                    }),
                )
                .build(cx),
            )
        };

        // 2. Text Preparation
        let session = cx.runtime_state.text_edit.get(input_id);
        let session_display = if is_focused {
            session.map(|st| st.display_text())
        } else {
            None
        };

        let (display_text, preedit_range, caret, anchor) = if self.obscure_text {
            let mut combined = self.value.clone();
            if let Some((display, _)) = &session_display {
                combined = display.clone();
            }
            let (caret, anchor) = session.map(|st| (st.caret, st.anchor)).unwrap_or((0, 0));
            let masked = Self::mask_text(&combined, self.obscuring_character);
            let mapped_caret = Self::masked_byte_offset(&combined, &masked, caret);
            let mapped_anchor = Self::masked_byte_offset(&combined, &masked, anchor);
            (masked, None, mapped_caret, mapped_anchor)
        } else {
            match session_display {
                Some((combined, preedit_range)) => {
                    let (caret, anchor) = session.map(|st| (st.caret, st.anchor)).unwrap_or((0, 0));
                    (combined, preedit_range, caret, anchor)
                }
                None => {
                    let (caret, anchor) = session.map(|st| (st.caret, st.anchor)).unwrap_or((0, 0));
                    (self.value.clone(), None, caret, anchor)
                }
            }
        };

        // Construct Runs
        let mut runs = Vec::new();
        if is_focused && caret != anchor {
            let (s, e) = if caret < anchor {
                (caret, anchor)
            } else {
                (anchor, caret)
            };
            let s = s.min(display_text.len());
            let e = e.min(display_text.len());

            if s > 0 {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[..s].to_string(),
                    style: base_text_style.clone(),
                });
            }
            if s < e {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[s..e].to_string(),
                    style: fission_ir::op::TextStyle {
                        color: selection_text_color,
                        background_color: Some(selection_color),
                        ..base_text_style.clone()
                    },
                });
            }
            if e < display_text.len() {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[e..].to_string(),
                    style: base_text_style.clone(),
                });
            }
        } else if let Some(styled) = &self.styled_runs {
            // Preserve syntax colouring while letting the widget-level typography
            // define the default family/weight/spacing.
            runs = styled
                .iter()
                .cloned()
                .map(|mut run| {
                    if run.style.font_family.is_none() {
                        run.style.font_family = base_text_style.font_family.clone();
                    }
                    if run.style.font_weight == 400 {
                        run.style.font_weight = base_text_style.font_weight;
                    }
                    if run.style.font_style == fission_ir::op::FontStyle::Normal {
                        run.style.font_style = base_text_style.font_style;
                    }
                    if run.style.line_height.is_none() {
                        run.style.line_height = base_text_style.line_height;
                    }
                    if run.style.letter_spacing == 0.0 {
                        run.style.letter_spacing = base_text_style.letter_spacing;
                    }
                    run
                })
                .collect();
        } else {
            runs.push(fission_ir::op::TextRun {
                text: display_text.clone(),
                style: base_text_style.clone(),
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
                        cuts.push((
                            overlap_start - run_start_byte,
                            overlap_end - run_start_byte,
                            color,
                        ));
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
                text: resolved_placeholder.clone().unwrap(),
                style: fission_ir::op::TextStyle {
                    color: placeholder_color,
                    ..base_text_style.clone()
                },
            }];
        }

        let caret_idx = if is_focused {
            let show = cx
                .runtime_state
                .caret_visible
                .get(&input_id)
                .copied()
                .unwrap_or(true);
            if show {
                Some(
                    preedit_range
                        .map(|(_, end)| end)
                        .unwrap_or(caret)
                        .min(display_text.len()),
                )
            } else {
                None
            }
        } else {
            None
        };

        let text_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                wrap: self.multiline,
                caret_index: caret_idx,
                caret_color: Some(cursor_color),
                caret_width: Some(cursor_width),
            }),
        )
        .build(cx);

        let mut text_box = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        );
        text_box.add_child(text_id);
        let text_layout_id = text_box.build(cx);

        // 3. Scroll Container
        let scroll_id = cx.next_node_id();
        let mut scroll = NodeBuilder::new(
            scroll_id,
            Op::Layout(LayoutOp::Scroll {
                direction: if self.multiline {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                show_scrollbar: false,
                width: None, // Let it fill parent padding box
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 1.0,
                flex_shrink: 1.0,
            }),
        );
        scroll.add_child(text_layout_id);
        let scroll_id = scroll.build(cx);

        // 4. Optional decoration row
        let content_id = if self.prefix.is_some() || self.suffix.is_some() {
            let mut content = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Flex {
                    direction: FlexDirection::Row,
                    wrap: FlexWrap::NoWrap,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    padding: [0.0; 4],
                    gap: Some(theme.padding_h * 0.75),
                    align_items: if self.multiline {
                        fission_ir::op::AlignItems::Start
                    } else {
                        fission_ir::op::AlignItems::Center
                    },
                    justify_content: fission_ir::op::JustifyContent::Start,
                }),
            );
            if let Some(prefix) = &self.prefix {
                content.add_child(prefix.lower(cx));
            }
            content.add_child(scroll_id);
            if let Some(suffix) = &self.suffix {
                content.add_child(suffix.lower(cx));
            }
            content.build(cx)
        } else {
            scroll_id
        };

        let effective_line_height = line_height.unwrap_or((font_size * 1.35).max(font_size + 4.0));
        let min_height = if self.height.is_some() {
            None
        } else if self.multiline {
            Some(
                content_padding[2]
                    + content_padding[3]
                    + effective_line_height * self.min_lines.unwrap_or(1) as f32,
            )
        } else {
            Some(theme.height.max(
                content_padding[2] + content_padding[3] + effective_line_height,
            ))
        };
        let max_height = if self.height.is_some() || !self.multiline {
            None
        } else {
            self.max_lines.map(|lines| {
                content_padding[2] + content_padding[3] + effective_line_height * lines as f32
            })
        };

        // 5. Wrapper (Border + Padding)
        let wrapper_id = cx.next_node_id();
        let mut wrapper = NodeBuilder::new(
            wrapper_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height.or(if self.multiline {
                    None
                } else {
                    Some(theme.height)
                }),
                min_width: None,
                max_width: None,
                min_height,
                max_height,
                padding: content_padding,
                flex_grow: if self.width.is_none() { 1.0 } else { 0.0 },
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );
        if let Some(bg_id) = background_id {
            wrapper.add_child(bg_id); // Fill
        }
        wrapper.add_child(content_id); // Content

        let final_id = wrapper.build(cx);

        // 5. Semantics
        let mut semantics = Semantics {
            role: Role::TextInput,
            label: resolved_placeholder.clone(),
            value: Some(self.value.clone()),
            actions: Default::default(),
            focusable: self.enabled,
            multiline: self.multiline,
            masked: self.obscure_text,
            input_mask: self.mask.clone(),
            ime_preedit_range: preedit_range,
            checked: None,
            disabled: !self.enabled,
            read_only: self.read_only,
            autofocus: self.autofocus,
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
            text_input_type: if self.multiline {
                TextInputType::Multiline
            } else {
                self.keyboard_type
            },
            text_input_action: self.text_input_action,
            text_capitalization: self.text_capitalization,
            max_length: self.max_length,
            max_length_enforcement: self.max_length_enforcement,
            input_formatters: self.input_formatters.clone(),
            autocorrect: self.autocorrect,
            enable_suggestions: self.enable_suggestions,
            spell_check: self.spell_check,
            smart_dashes: self.smart_dashes,
            smart_quotes: self.smart_quotes,
            autofill_hints: self.autofill_hints.clone(),
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
        if let Some(env) = &self.on_submit {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Submit,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_editing_complete {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::EditingComplete,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        let mut semantics_builder = NodeBuilder::new(input_id, Op::Semantics(semantics));
        semantics_builder.add_child(final_id);
        semantics_builder.build(cx)
    }
}
