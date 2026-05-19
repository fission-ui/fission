use crate::env::TextSelectionHandleKind;
use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::{
    traits::Lower, Button, ButtonContentAlign, ButtonVariant, Container, Node, Positioned, Row,
    Spacer, Text, TextContent, TextFontStyle,
};
use crate::ActionEnvelope;
use fission_ir::{
    op::{
        Color as IrColor, Fill, LayoutOp, Op, PaintOp, Stroke, TextAlign as IrTextAlign,
        TextParagraphStyle,
    },
    semantics::{
        InputFormatter, MaxLengthEnforcement, MouseCursor as SemanticsMouseCursor,
        TextCapitalization, TextInputAction, TextInputType,
    },
    AnyRenderObject, FlexDirection, FlexWrap, NodeId, Role, Semantics,
};
use fission_theme::{ComponentSize, ComponentState};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextAlignVertical {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DragStartBehavior {
    #[default]
    Start,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextUndoController {
    pub capacity: usize,
}

impl Default for TextUndoController {
    fn default() -> Self {
        Self { capacity: 100 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellCheckConfiguration {
    pub enabled: bool,
    pub underline_color: Option<IrColor>,
    pub show_suggestions: bool,
}

impl Default for SpellCheckConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            underline_color: Some(IrColor {
                r: 255,
                g: 59,
                b: 48,
                a: 255,
            }),
            show_suggestions: true,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputRuntimeConfig {
    pub drag_start_behavior: DragStartBehavior,
    pub undo_controller: Option<TextUndoController>,
    pub restoration_id: Option<String>,
    pub spell_check_configuration: Option<SpellCheckConfiguration>,
}

#[doc(hidden)]
pub fn downcast_text_input_runtime_config(
    any: &AnyRenderObject,
) -> Option<&TextInputRuntimeConfig> {
    any.downcast_ref::<TextInputRuntimeConfig>()
}

impl TextAlignVertical {
    fn justify_content(self) -> fission_ir::op::JustifyContent {
        match self {
            Self::Top => fission_ir::op::JustifyContent::Start,
            Self::Center => fission_ir::op::JustifyContent::Center,
            Self::Bottom => fission_ir::op::JustifyContent::End,
        }
    }

    fn align_items(self) -> fission_ir::op::AlignItems {
        match self {
            Self::Top => fission_ir::op::AlignItems::Start,
            Self::Center => fission_ir::op::AlignItems::Center,
            Self::Bottom => fission_ir::op::AlignItems::End,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextContextMenuAction {
    Copy,
    Cut,
    Paste,
    SelectAll,
}

impl TextContextMenuAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextContextMenuConfig {
    pub enabled: bool,
    pub actions: Vec<TextContextMenuAction>,
    pub padding: [f32; 4],
    pub gap: f32,
    pub border_radius: f32,
}

impl Default for TextContextMenuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            actions: vec![
                TextContextMenuAction::Copy,
                TextContextMenuAction::Cut,
                TextContextMenuAction::Paste,
                TextContextMenuAction::SelectAll,
            ],
            padding: [10.0, 10.0, 8.0, 8.0],
            gap: 6.0,
            border_radius: 12.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSelectionControls {
    pub show_collapsed_handle: bool,
    pub handle_radius: f32,
    pub handle_fill: IrColor,
    pub handle_stroke: Option<IrColor>,
    pub handle_stroke_width: f32,
}

impl Default for TextSelectionControls {
    fn default() -> Self {
        Self {
            show_collapsed_handle: true,
            handle_radius: 7.0,
            handle_fill: IrColor {
                r: 0,
                g: 122,
                b: 255,
                a: 255,
            },
            handle_stroke: Some(IrColor {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }),
            handle_stroke_width: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMagnifierConfiguration {
    pub enabled: bool,
    pub diameter: f32,
    pub scale: f32,
    pub border_radius: f32,
    pub border_color: Option<IrColor>,
    pub border_width: f32,
}

impl Default for TextMagnifierConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            diameter: 84.0,
            scale: 1.4,
            border_radius: 18.0,
            border_color: Some(IrColor {
                r: 210,
                g: 214,
                b: 224,
                a: 255,
            }),
            border_width: 1.0,
        }
    }
}

pub(crate) fn text_input_selection_handle_id(
    input_id: NodeId,
    kind: TextSelectionHandleKind,
) -> NodeId {
    let suffix = match kind {
        TextSelectionHandleKind::Caret => 0,
        TextSelectionHandleKind::Start => 1,
        TextSelectionHandleKind::End => 2,
    };
    NodeId::derived(input_id.as_u128(), &[900, suffix])
}

pub(crate) fn text_input_toolbar_button_id(
    input_id: NodeId,
    action: TextContextMenuAction,
) -> NodeId {
    let suffix = match action {
        TextContextMenuAction::Copy => 0,
        TextContextMenuAction::Cut => 1,
        TextContextMenuAction::Paste => 2,
        TextContextMenuAction::SelectAll => 3,
    };
    NodeId::derived(input_id.as_u128(), &[901, suffix])
}

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
/// let on_change = ctx.bind(TextChanged { .. }, reduce_with!(handle_text));
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
    /// Optional label shown above the field.
    pub label: Option<TextContent>,
    /// Placeholder text shown when `value` is empty.
    pub placeholder: Option<TextContent>,
    /// Optional supporting text shown below the field when there is no error.
    pub helper_text: Option<TextContent>,
    /// Optional validation error shown below the field.
    pub error_text: Option<TextContent>,
    /// Optional explicit counter text shown below the field.
    pub counter_text: Option<TextContent>,
    /// Action dispatched when the text changes.
    pub on_change: Option<ActionEnvelope>,
    /// Action dispatched when the user submits the field (for example by pressing Enter
    /// on a single-line input).
    pub on_submit: Option<ActionEnvelope>,
    /// Action dispatched when editing is explicitly completed.
    pub on_editing_complete: Option<ActionEnvelope>,
    /// Action dispatched when the user taps/clicks outside the active field.
    pub on_tap_outside: Option<ActionEnvelope>,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Design-system size slot.
    #[serde(default)]
    pub size: ComponentSize,
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
    /// Optional label color override.
    pub label_color: Option<IrColor>,
    /// Optional helper/supporting text color override.
    pub helper_color: Option<IrColor>,
    /// Optional error text color override.
    pub error_color: Option<IrColor>,
    /// Optional counter text color override.
    pub counter_color: Option<IrColor>,
    /// Optional selection highlight color override.
    pub selection_color: Option<IrColor>,
    /// Optional selected text color override.
    pub selection_text_color: Option<IrColor>,
    /// Horizontal text alignment inside the editable region.
    pub text_align: fission_ir::op::TextAlign,
    /// Vertical alignment for the editable region when the field is taller than its content.
    pub text_align_vertical: TextAlignVertical,
    /// When `true`, expand to fill the available height from the parent.
    pub expands: bool,
    /// Optional caret color override.
    pub cursor_color: Option<IrColor>,
    /// Optional caret width override.
    pub cursor_width: Option<f32>,
    /// Optional caret height override.
    pub cursor_height: Option<f32>,
    /// Optional caret corner radius override.
    pub cursor_radius: Option<f32>,
    /// Optional font family override.
    pub font_family: Option<String>,
    /// Optional locale override used for shaping and accessibility.
    pub locale: Option<String>,
    /// Optional font weight override.
    pub font_weight: Option<u16>,
    /// Optional font style override.
    pub font_style: TextFontStyle,
    /// Optional text scale multiplier.
    pub text_scale: Option<f32>,
    /// Optional absolute line-height override in layout points.
    pub line_height: Option<f32>,
    /// Optional letter-spacing override in layout points.
    pub letter_spacing: Option<f32>,
    /// Paragraph text direction override for the editable content.
    pub text_direction: fission_ir::op::TextDirection,
    /// Optional paragraph strut line height.
    pub strut_line_height: Option<f32>,
    /// Paragraph height trimming behavior.
    pub text_height_behavior: fission_ir::op::TextHeightBehavior,
    /// Optional leading decoration node.
    pub prefix: Option<Box<Node>>,
    /// Optional trailing decoration node.
    pub suffix: Option<Box<Node>>,
    /// Optional hover cursor override while pointing at the field.
    pub mouse_cursor: Option<SemanticsMouseCursor>,
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
    /// Extra padding to keep around the caret when auto-scrolling `[left, right, top, bottom]`.
    pub scroll_padding: Option<[f32; 4]>,
    /// Whether selection drags become active on pointer-down or only after slop is crossed.
    pub drag_start_behavior: DragStartBehavior,
    /// Built-in context menu configuration for pointer and touch editing affordances.
    pub context_menu: TextContextMenuConfig,
    /// Selection-handle visual configuration.
    pub selection_controls: TextSelectionControls,
    /// Magnifier visual configuration shown while dragging selection handles.
    pub magnifier_configuration: TextMagnifierConfiguration,
    /// Optional undo-controller configuration for edit history.
    pub undo_controller: Option<TextUndoController>,
    /// Structured spell-check preferences.
    pub spell_check_configuration: Option<SpellCheckConfiguration>,
    /// Stable restoration identifier for rehydrating local edit state.
    pub restoration_id: Option<String>,
}

impl TextInput {
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn label(mut self, label: impl Into<TextContent>) -> Self {
        self.label = Some(label.into());
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

    pub fn helper_text(mut self, helper_text: impl Into<TextContent>) -> Self {
        self.helper_text = Some(helper_text.into());
        self
    }

    pub fn error_text(mut self, error_text: impl Into<TextContent>) -> Self {
        self.error_text = Some(error_text.into());
        self
    }

    pub fn counter_text(mut self, counter_text: impl Into<TextContent>) -> Self {
        self.counter_text = Some(counter_text.into());
        self
    }

    pub fn label_color(mut self, color: IrColor) -> Self {
        self.label_color = Some(color);
        self
    }

    pub fn helper_color(mut self, color: IrColor) -> Self {
        self.helper_color = Some(color);
        self
    }

    pub fn error_color(mut self, color: IrColor) -> Self {
        self.error_color = Some(color);
        self
    }

    pub fn counter_color(mut self, color: IrColor) -> Self {
        self.counter_color = Some(color);
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

    pub fn text_align(mut self, text_align: fission_ir::op::TextAlign) -> Self {
        self.text_align = text_align;
        self
    }

    pub fn text_align_vertical(mut self, text_align_vertical: TextAlignVertical) -> Self {
        self.text_align_vertical = text_align_vertical;
        self
    }

    pub fn expands(mut self, expands: bool) -> Self {
        self.expands = expands;
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

    pub fn cursor_height(mut self, height: f32) -> Self {
        self.cursor_height = Some(height);
        self
    }

    pub fn cursor_radius(mut self, radius: f32) -> Self {
        self.cursor_radius = Some(radius);
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

    pub fn context_menu(mut self, context_menu: TextContextMenuConfig) -> Self {
        self.context_menu = context_menu;
        self
    }

    pub fn drag_start_behavior(mut self, drag_start_behavior: DragStartBehavior) -> Self {
        self.drag_start_behavior = drag_start_behavior;
        self
    }

    pub fn selection_controls(mut self, selection_controls: TextSelectionControls) -> Self {
        self.selection_controls = selection_controls;
        self
    }

    pub fn magnifier_configuration(
        mut self,
        magnifier_configuration: TextMagnifierConfiguration,
    ) -> Self {
        self.magnifier_configuration = magnifier_configuration;
        self
    }

    pub fn on_tap_outside(mut self, action: ActionEnvelope) -> Self {
        self.on_tap_outside = Some(action);
        self
    }

    pub fn undo_controller(mut self, undo_controller: TextUndoController) -> Self {
        self.undo_controller = Some(undo_controller);
        self
    }

    pub fn spell_check_configuration(
        mut self,
        spell_check_configuration: SpellCheckConfiguration,
    ) -> Self {
        self.spell_check_configuration = Some(spell_check_configuration);
        self
    }

    pub fn restoration_id(mut self, restoration_id: impl Into<String>) -> Self {
        self.restoration_id = Some(restoration_id.into());
        self
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
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

    pub fn text_scale(mut self, text_scale: f32) -> Self {
        self.text_scale = Some(text_scale);
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

    pub fn text_direction(mut self, text_direction: fission_ir::op::TextDirection) -> Self {
        self.text_direction = text_direction;
        self
    }

    pub fn strut_line_height(mut self, strut_line_height: f32) -> Self {
        self.strut_line_height = Some(strut_line_height);
        self
    }

    pub fn text_height_behavior(
        mut self,
        text_height_behavior: fission_ir::op::TextHeightBehavior,
    ) -> Self {
        self.text_height_behavior = text_height_behavior;
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

    pub fn mouse_cursor(mut self, mouse_cursor: SemanticsMouseCursor) -> Self {
        self.mouse_cursor = Some(mouse_cursor);
        self
    }

    pub fn scroll_padding(mut self, scroll_padding: [f32; 4]) -> Self {
        self.scroll_padding = Some(scroll_padding);
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
            label: None,
            placeholder: None,
            helper_text: None,
            error_text: None,
            counter_text: None,
            on_change: None,
            on_submit: None,
            on_editing_complete: None,
            on_tap_outside: None,
            width: None,
            height: None,
            size: ComponentSize::Md,
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
            label_color: None,
            helper_color: None,
            error_color: None,
            counter_color: None,
            selection_color: None,
            selection_text_color: None,
            text_align: fission_ir::op::TextAlign::Start,
            text_align_vertical: TextAlignVertical::Center,
            expands: false,
            cursor_color: None,
            cursor_width: None,
            cursor_height: None,
            cursor_radius: None,
            font_family: None,
            locale: None,
            font_weight: None,
            font_style: TextFontStyle::Normal,
            text_scale: None,
            line_height: None,
            letter_spacing: None,
            text_direction: fission_ir::op::TextDirection::Auto,
            strut_line_height: None,
            text_height_behavior: fission_ir::op::TextHeightBehavior::default(),
            prefix: None,
            suffix: None,
            mouse_cursor: None,
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
            scroll_padding: None,
            drag_start_behavior: DragStartBehavior::Start,
            context_menu: TextContextMenuConfig::default(),
            selection_controls: TextSelectionControls::default(),
            magnifier_configuration: TextMagnifierConfiguration::default(),
            undo_controller: None,
            spell_check_configuration: None,
            restoration_id: None,
        }
    }
}

impl TextInput {
    fn resolve_text_content(content: &TextContent, cx: &LoweringContext<'_>) -> String {
        match content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
        }
    }

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

    fn supporting_counter_text(
        &self,
        cx: &LoweringContext<'_>,
        current_text: &str,
    ) -> Option<String> {
        self.counter_text
            .as_ref()
            .map(|content| Self::resolve_text_content(content, cx))
            .or_else(|| {
                self.max_length
                    .map(|max_length| format!("{}/{}", current_text.chars().count(), max_length))
            })
    }

    fn build_selection_handle_overlay(
        &self,
        cx: &mut LoweringContext,
        input_id: NodeId,
        kind: TextSelectionHandleKind,
        point: fission_layout::LayoutPoint,
    ) -> NodeId {
        let controls = &self.selection_controls;
        let diameter = controls.handle_radius * 2.0;
        let handle_node = Button {
            id: Some(text_input_selection_handle_id(input_id, kind)),
            child: Some(Box::new(
                Container::new(
                    Spacer {
                        width: Some(diameter),
                        height: Some(diameter),
                        ..Default::default()
                    }
                    .into_node(),
                )
                .bg_fill(Fill::Solid(controls.handle_fill))
                .border(
                    controls.handle_stroke.unwrap_or(IrColor {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    }),
                    controls.handle_stroke_width,
                )
                .border_radius(controls.handle_radius)
                .into_node(),
            )),
            width: Some(diameter),
            height: Some(diameter),
            padding: Some([0.0; 4]),
            content_align: ButtonContentAlign::Center,
            variant: ButtonVariant::Ghost,
            ..Default::default()
        }
        .into_node();

        Positioned {
            left: Some((point.x - controls.handle_radius).max(0.0)),
            top: Some((point.y - controls.handle_radius).max(0.0)),
            width: Some(diameter),
            height: Some(diameter),
            child: Some(Box::new(handle_node)),
            ..Default::default()
        }
        .lower(cx)
    }

    fn build_toolbar_overlay(
        &self,
        cx: &mut LoweringContext,
        input_id: NodeId,
        anchor: fission_layout::LayoutPoint,
    ) -> NodeId {
        let tokens = &cx.env.theme.tokens;
        let mut row = Row::default().gap(self.context_menu.gap);
        for action in &self.context_menu.actions {
            row.children.push(
                Button {
                    id: Some(text_input_toolbar_button_id(input_id, *action)),
                    child: Some(Box::new(
                        Text::new(action.label())
                            .size(tokens.typography.label_large_size)
                            .color(tokens.colors.text_primary)
                            .into_node(),
                    )),
                    padding: Some([10.0, 10.0, 6.0, 6.0]),
                    content_align: ButtonContentAlign::Center,
                    variant: ButtonVariant::Ghost,
                    ..Default::default()
                }
                .into_node(),
            );
        }

        let toolbar = Container::new(row.into_node())
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(tokens.colors.border, 1.0)
            .border_radius(self.context_menu.border_radius)
            .padding(self.context_menu.padding)
            .into_node();

        Positioned {
            left: Some(anchor.x.max(0.0)),
            top: Some((anchor.y - 44.0).max(0.0)),
            child: Some(Box::new(toolbar)),
            ..Default::default()
        }
        .lower(cx)
    }

    fn magnifier_snippet(display_text: &str, caret: usize) -> String {
        let mut graphemes = Vec::new();
        for (idx, grapheme) in display_text.grapheme_indices(true) {
            graphemes.push((idx, grapheme));
        }
        if graphemes.is_empty() {
            return String::new();
        }

        let caret_grapheme = graphemes
            .iter()
            .position(|(idx, _)| *idx >= caret.min(display_text.len()))
            .unwrap_or(graphemes.len().saturating_sub(1));
        let start = caret_grapheme.saturating_sub(4);
        let end = (caret_grapheme + 5).min(graphemes.len());
        graphemes[start..end]
            .iter()
            .map(|(_, grapheme)| *grapheme)
            .collect::<String>()
    }

    fn build_magnifier_overlay(
        &self,
        cx: &mut LoweringContext,
        anchor: fission_layout::LayoutPoint,
        display_text: &str,
        caret: usize,
        base_text_style: &fission_ir::op::TextStyle,
    ) -> NodeId {
        let cfg = &self.magnifier_configuration;
        let tokens = &cx.env.theme.tokens;
        let preview = Self::magnifier_snippet(display_text, caret);
        let preview_text = Text::new(preview)
            .size(base_text_style.font_size * cfg.scale)
            .color(base_text_style.color)
            .family(
                base_text_style
                    .font_family
                    .clone()
                    .unwrap_or_else(|| "system-ui".to_string()),
            )
            .weight(base_text_style.font_weight)
            .italic(base_text_style.font_style == fission_ir::op::FontStyle::Italic)
            .line_height(
                base_text_style
                    .line_height
                    .unwrap_or(base_text_style.font_size * 1.25)
                    * cfg.scale,
            )
            .letter_spacing(base_text_style.letter_spacing * cfg.scale)
            .into_node();

        let magnifier = Container::new(preview_text)
            .width(cfg.diameter)
            .height(cfg.diameter)
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(
                cfg.border_color.unwrap_or(tokens.colors.border),
                cfg.border_width,
            )
            .border_radius(cfg.border_radius)
            .padding_all(8.0)
            .into_node();

        Positioned {
            left: Some((anchor.x - cfg.diameter * 0.5).max(0.0)),
            top: Some((anchor.y - cfg.diameter - 18.0).max(0.0)),
            width: Some(cfg.diameter),
            height: Some(cfg.diameter),
            child: Some(Box::new(magnifier)),
            ..Default::default()
        }
        .lower(cx)
    }
}

impl Lower for TextInput {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let input_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let is_focused = cx.runtime_state.interaction.is_focused(input_id);

        let theme = &cx.env.theme.components.text_input;
        let tokens = &cx.env.theme.tokens;
        let component_state = if !self.enabled {
            ComponentState::Disabled
        } else if self.error_text.is_some() {
            ComponentState::Error
        } else if is_focused {
            ComponentState::Focus
        } else {
            ComponentState::Default
        };
        let component_style = theme.resolve(self.size, component_state);

        let text_scale = self.text_scale.unwrap_or(1.0).max(0.0);
        let font_size = self
            .font_size
            .unwrap_or(component_style.font_size.unwrap_or(theme.font_size))
            * text_scale;
        let text_color = self
            .text_color
            .unwrap_or(component_style.text_color.unwrap_or(theme.text_color));
        let selection_color = self
            .selection_color
            .unwrap_or(tokens.colors.primary.with_alpha(52));
        let selection_text_color = self.selection_text_color.unwrap_or(text_color);
        let placeholder_color = self.placeholder_color.unwrap_or(
            theme
                .placeholder_style
                .text_color
                .unwrap_or(theme.placeholder_color),
        );
        let cursor_color = self.cursor_color.unwrap_or(theme.focus_color);
        let cursor_width = self.cursor_width.unwrap_or(2.0);
        let font_weight = self
            .font_weight
            .unwrap_or(component_style.font_weight.unwrap_or(theme.font_weight));
        let line_height = self
            .line_height
            .or(component_style.line_height)
            .map(|value| value * text_scale);
        let letter_spacing = self.letter_spacing.unwrap_or(0.0) * text_scale;
        let style_border = component_style.border.clone();
        let border_color = if is_focused {
            self.focus_border_color.unwrap_or_else(|| {
                style_border
                    .as_ref()
                    .and_then(|border| match &border.fill {
                        Fill::Solid(color) => Some(*color),
                        _ => None,
                    })
                    .unwrap_or(theme.focus_color)
            })
        } else {
            self.border_color.unwrap_or_else(|| {
                style_border
                    .as_ref()
                    .and_then(|border| match &border.fill {
                        Fill::Solid(color) => Some(*color),
                        _ => None,
                    })
                    .unwrap_or(theme.border_color)
            })
        };
        let border_width = if is_focused {
            self.focus_border_width.unwrap_or(
                style_border
                    .as_ref()
                    .map(|border| border.width)
                    .unwrap_or(2.0),
            )
        } else {
            self.border_width.unwrap_or(
                style_border
                    .as_ref()
                    .map(|border| border.width)
                    .unwrap_or(theme.border_width),
            )
        };
        let border_radius = self
            .border_radius
            .unwrap_or(component_style.radius.unwrap_or(theme.radius));
        let content_padding = self.padding.unwrap_or(component_style.padding_box(
            component_style.padding_x.unwrap_or(theme.padding_h),
            component_style.padding_y.unwrap_or(4.0),
        ));
        let base_text_style = fission_ir::op::TextStyle {
            font_size,
            color: text_color,
            underline: false,
            font_family: self.font_family.clone(),
            locale: self.locale.clone(),
            font_weight,
            font_style: self.font_style.into(),
            line_height,
            letter_spacing,
            background_color: None,
        };

        let resolved_label = self
            .label
            .as_ref()
            .map(|label| Self::resolve_text_content(label, cx));
        let resolved_placeholder = self
            .placeholder
            .as_ref()
            .map(|placeholder| Self::resolve_text_content(placeholder, cx));

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
                                .or_else(|| component_style.background.clone())
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
                        shadow: component_style.outer_shadows().first().copied(),
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

        let paragraph_overflow = if self.multiline {
            fission_ir::op::TextOverflow::Clip
        } else {
            fission_ir::op::TextOverflow::Visible
        };
        let paragraph_style = Some(TextParagraphStyle {
            text_align: self.text_align,
            max_lines: None,
            overflow: paragraph_overflow,
            text_direction: self.text_direction,
            text_width_basis: fission_ir::op::TextWidthBasis::Parent,
            strut_line_height: self.strut_line_height,
            text_height_behavior: self.text_height_behavior,
        })
        .filter(|style| {
            *style
                != TextParagraphStyle {
                    text_align: IrTextAlign::Start,
                    max_lines: None,
                    overflow: paragraph_overflow,
                    text_direction: self.text_direction,
                    text_width_basis: fission_ir::op::TextWidthBasis::Parent,
                    strut_line_height: self.strut_line_height,
                    text_height_behavior: self.text_height_behavior,
                }
        });

        let text_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                wrap: self.multiline,
                caret_index: caret_idx,
                caret_color: Some(cursor_color),
                caret_width: Some(cursor_width),
                caret_height: self.cursor_height,
                caret_radius: self.cursor_radius,
                paragraph_style,
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
        let mut scroll = NodeBuilder::new(
            cx.next_node_id(),
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

        // 4. Editable content row and vertical alignment container.
        let mut content_row = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                wrap: FlexWrap::NoWrap,
                flex_grow: if self.expands { 1.0 } else { 0.0 },
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: if self.prefix.is_some() || self.suffix.is_some() {
                    Some(theme.padding_h * 0.75)
                } else {
                    None
                },
                align_items: self.text_align_vertical.align_items(),
                justify_content: fission_ir::op::JustifyContent::Start,
            }),
        );
        if let Some(prefix) = &self.prefix {
            content_row.add_child(prefix.lower(cx));
        }
        content_row.add_child(scroll_id);
        if let Some(suffix) = &self.suffix {
            content_row.add_child(suffix.lower(cx));
        }
        let content_row_id = content_row.build(cx);

        let mut content_alignment = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: FlexWrap::NoWrap,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: self.text_align_vertical.justify_content(),
            }),
        );
        content_alignment.add_child(content_row_id);
        let content_id = content_alignment.build(cx);

        let effective_line_height = line_height.unwrap_or((font_size * 1.35).max(font_size + 4.0));
        let min_height = if self.height.is_some() || self.expands {
            None
        } else if self.multiline {
            Some(
                content_padding[2]
                    + content_padding[3]
                    + effective_line_height * self.min_lines.unwrap_or(1) as f32,
            )
        } else {
            Some(
                theme
                    .height
                    .max(content_padding[2] + content_padding[3] + effective_line_height),
            )
        };
        let max_height = if self.height.is_some() || !self.multiline || self.expands {
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
                height: self.height.or(if self.multiline || self.expands {
                    None
                } else {
                    Some(theme.height)
                }),
                min_width: None,
                max_width: None,
                min_height,
                max_height,
                padding: content_padding,
                flex_grow: if self.width.is_none() || self.expands {
                    1.0
                } else {
                    0.0
                },
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );
        if let Some(bg_id) = background_id {
            wrapper.add_child(bg_id); // Fill
        }
        wrapper.add_child(content_id); // Content

        let wrapper_visual_id = wrapper.build(cx);
        let mut final_visual_id = wrapper_visual_id;

        if is_focused && self.enabled {
            if let Some(session_state) = session {
                let affordances = &session_state.affordances;
                let mut overlay_children = Vec::new();

                if caret == anchor {
                    if self.selection_controls.show_collapsed_handle {
                        if let Some(point) = affordances.caret_handle {
                            overlay_children.push(self.build_selection_handle_overlay(
                                cx,
                                input_id,
                                TextSelectionHandleKind::Caret,
                                point,
                            ));
                        }
                    }
                } else {
                    if let Some(point) = affordances.selection_start_handle {
                        overlay_children.push(self.build_selection_handle_overlay(
                            cx,
                            input_id,
                            TextSelectionHandleKind::Start,
                            point,
                        ));
                    }
                    if let Some(point) = affordances.selection_end_handle {
                        overlay_children.push(self.build_selection_handle_overlay(
                            cx,
                            input_id,
                            TextSelectionHandleKind::End,
                            point,
                        ));
                    }
                }

                if self.context_menu.enabled && affordances.toolbar_visible {
                    if let Some(anchor_point) = affordances.toolbar_anchor {
                        overlay_children.push(self.build_toolbar_overlay(
                            cx,
                            input_id,
                            anchor_point,
                        ));
                    }
                }

                if self.magnifier_configuration.enabled && affordances.magnifier_visible {
                    if let Some(anchor_point) = affordances.magnifier_anchor {
                        overlay_children.push(self.build_magnifier_overlay(
                            cx,
                            anchor_point,
                            &display_text,
                            caret.max(anchor),
                            &base_text_style,
                        ));
                    }
                }

                if !overlay_children.is_empty() {
                    let mut stack =
                        NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::ZStack));
                    stack.add_child(wrapper_visual_id);
                    for child in overlay_children {
                        stack.add_child(child);
                    }
                    final_visual_id = stack.build(cx);
                }
            }
        }

        let supporting_text = self
            .error_text
            .as_ref()
            .map(|text| Self::resolve_text_content(text, cx))
            .or_else(|| {
                self.helper_text
                    .as_ref()
                    .map(|text| Self::resolve_text_content(text, cx))
            });
        let counter_text = self.supporting_counter_text(cx, &self.value);

        let field_body_id =
            if resolved_label.is_some() || supporting_text.is_some() || counter_text.is_some() {
                let label_color = self.label_color.unwrap_or(if is_focused {
                    theme.focus_color
                } else {
                    theme
                        .label_style
                        .text_color
                        .unwrap_or(tokens.colors.text_secondary)
                });
                let supporting_color = if self.error_text.is_some() {
                    self.error_color.unwrap_or(tokens.colors.error)
                } else {
                    self.helper_color.unwrap_or(
                        theme
                            .helper_style
                            .text_color
                            .unwrap_or(tokens.colors.text_secondary),
                    )
                };
                let counter_color = self.counter_color.unwrap_or(
                    theme
                        .helper_style
                        .text_color
                        .unwrap_or(tokens.colors.text_secondary),
                );
                let mut column = NodeBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Flex {
                        direction: FlexDirection::Column,
                        wrap: FlexWrap::NoWrap,
                        flex_grow: 0.0,
                        flex_shrink: 1.0,
                        padding: [0.0; 4],
                        gap: Some(6.0),
                        align_items: fission_ir::op::AlignItems::Stretch,
                        justify_content: fission_ir::op::JustifyContent::Start,
                    }),
                );

                if let Some(label) = &resolved_label {
                    column.add_child(
                        Text::new(label.clone())
                            .size(
                                theme
                                    .label_style
                                    .font_size
                                    .unwrap_or(tokens.typography.label_large_size),
                            )
                            .weight(
                                theme
                                    .label_style
                                    .font_weight
                                    .unwrap_or(tokens.typography.font_weight_medium),
                            )
                            .color(label_color)
                            .lower(cx),
                    );
                }

                column.add_child(final_visual_id);

                if supporting_text.is_some() || counter_text.is_some() {
                    let mut row = Row::default().gap(8.0);
                    if let Some(supporting_text) = supporting_text {
                        row.children.push(
                            Text::new(supporting_text)
                                .size(
                                    theme
                                        .helper_style
                                        .font_size
                                        .unwrap_or(tokens.typography.label_large_size),
                                )
                                .color(supporting_color)
                                .into_node(),
                        );
                    }
                    row.children.push(
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                    );
                    if let Some(counter_text) = counter_text {
                        row.children.push(
                            Text::new(counter_text)
                                .size(
                                    theme
                                        .helper_style
                                        .font_size
                                        .unwrap_or(tokens.typography.label_large_size),
                                )
                                .color(counter_color)
                                .into_node(),
                        );
                    }
                    column.add_child(row.lower(cx));
                }

                column.build(cx)
            } else {
                final_visual_id
            };

        // 5. Semantics
        let spell_check_enabled = self
            .spell_check_configuration
            .as_ref()
            .map_or(self.spell_check, |cfg| cfg.enabled);
        let suggestions_enabled = self
            .spell_check_configuration
            .as_ref()
            .map_or(self.enable_suggestions, |cfg| {
                self.enable_suggestions && cfg.show_suggestions
            });

        let mut semantics = Semantics {
            role: Role::TextInput,
            label: resolved_label.clone().or(resolved_placeholder.clone()),
            identifier: None,
            value: Some(self.value.clone()),
            actions: Default::default(),
            action_scope_id: None,
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
            enable_suggestions: suggestions_enabled,
            spell_check: spell_check_enabled,
            smart_dashes: self.smart_dashes,
            smart_quotes: self.smart_quotes,
            autofill_hints: self.autofill_hints.clone(),
            scroll_padding: self.scroll_padding,
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
        if let Some(env) = &self.on_tap_outside {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::TapOutside,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(mouse_cursor) = self.mouse_cursor {
            semantics
                .actions
                .entries
                .push(fission_ir::ActionEntry::hover_cursor(mouse_cursor));
        }
        let mut semantics_builder = NodeBuilder::new(input_id, Op::Semantics(semantics));
        semantics_builder.add_child(field_body_id);
        let semantics_id = semantics_builder.build(cx);
        cx.ir.custom_render_objects.insert(
            semantics_id,
            Arc::new(TextInputRuntimeConfig {
                drag_start_behavior: self.drag_start_behavior,
                undo_controller: self.undo_controller.clone(),
                restoration_id: self.restoration_id.clone(),
                spell_check_configuration: self.spell_check_configuration.clone(),
            }),
        );
        semantics_id
    }
}
