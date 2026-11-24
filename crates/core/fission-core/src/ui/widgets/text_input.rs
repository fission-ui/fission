use crate::env::TextSelectionHandleKind;
use crate::lowering::{IrBuilder, LoweringContext};
use crate::ui::{
    traits::Lower,
    widgets::context_menu::{TextContextMenuAction, TextContextMenuConfig},
    Button, ButtonContentAlign, ButtonVariant, Container, Positioned, Row, Spacer, Text,
    TextContent, TextFontStyle, Widget,
};
use crate::ActionEnvelope;
use fission_ir::{
    op::{
        Color as IrColor, Fill, LayoutOp, Op, PaintOp, Stroke, TextAlign as IrTextAlign,
        TextParagraphLayoutStyle, TextParagraphStyle, TextParagraphStyleSpan,
    },
    semantics::{
        InputFormatter, MaxLengthEnforcement, MouseCursor as SemanticsMouseCursor,
        TextCapitalization, TextFieldValidationState, TextInputAction, TextInputType,
    },
    FlexDirection, FlexWrap, Role, Semantics, WidgetId,
};
use fission_theme::{ComponentSize, ComponentState};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

mod config;
mod decoration;
mod display_breaks;
mod lowering;
mod text_runs;
pub use config::*;
use display_breaks::{map_display_breaks_through_preedit, map_paragraph_styles_through_preedit};
use text_runs::{clamp_text_offset, split_runs_for_range};

#[cfg(debug_assertions)]
fn report_invalid_validation_pattern() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        eprintln!(
            "Fission TextInput ignored an invalid validation_pattern; use a valid regular expression or a custom validator"
        );
    });
}

pub(crate) fn text_input_selection_handle_id(
    input_id: WidgetId,
    kind: TextSelectionHandleKind,
) -> WidgetId {
    let suffix = match kind {
        TextSelectionHandleKind::Caret => 0,
        TextSelectionHandleKind::Start => 1,
        TextSelectionHandleKind::End => 2,
    };
    WidgetId::derived(input_id.as_u128(), &[900, suffix])
}

pub(crate) fn text_input_toolbar_button_id(
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

fn text_input_text_node_id(input_id: WidgetId) -> WidgetId {
    // Keep paragraph identity independent of focus-dependent decoration nodes;
    // the layout engine caches resolved text by this ID and its width.
    WidgetId::derived(input_id.as_u128(), &[902])
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
/// let on_input = ctx.bind(EditSearch, reduce_with!(handle_search_input));
///
/// TextInput {
///     value: view.state().query.clone(),
///     placeholder: Some("Search...".into()),
///     on_input: Some(on_input),
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
    pub id: Option<WidgetId>,
    /// Stable identifier exposed on the input's interactive semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    /// Supplemental semantics for composite controls and labelled form fields.
    ///
    /// Relationship metadata, an explicit non-generic role, and additional
    /// actions are preserved. Text-editing value, focusability, validation,
    /// input configuration, and enabled/read-only state remain authoritative on
    /// `TextInput` and are refreshed during lowering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<Semantics>,
    /// The current text value (controlled by the application).
    pub value: String,
    /// Optional complete controlled editing value.
    ///
    /// When supplied, text, directional selection, and composing range are
    /// reconciled atomically. `value` remains as the compatibility shorthand.
    pub editing_value: Option<crate::TextEditingValue>,
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
    ///
    /// The bound action payload is preserved. The reducer reads the live text,
    /// widget identity, caret, and anchor from [`crate::ActionInput::text_change`].
    pub on_input: Option<ActionEnvelope>,
    /// Action dispatched when the user submits the field (for example by pressing Enter
    /// on a single-line input).
    pub on_submit: Option<ActionEnvelope>,
    /// Action dispatched for Tab and Shift+Tab while this input is focused.
    ///
    /// Setting this explicitly opts the field into handling both keys. The
    /// reducer receives an [`crate::ActionInput::KeyPress`] whose modifiers
    /// distinguish forward Tab (`0`) from reverse Tab (`MOD_SHIFT`). When this
    /// is `None`, focus traversal remains in control unless `capture_tab` is
    /// enabled, in which case Tab inserts spaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_tab: Option<ActionEnvelope>,
    /// Action dispatched when editing is explicitly completed.
    pub on_editing_complete: Option<ActionEnvelope>,
    /// Action dispatched when the user taps/clicks outside the active field.
    pub on_tap_outside: Option<ActionEnvelope>,
    /// Action dispatched when the field receives focus.
    pub on_focus: Option<ActionEnvelope>,
    /// Action dispatched when the field loses focus.
    pub on_blur: Option<ActionEnvelope>,
    /// Action dispatched when validation is requested.
    pub on_validation: Option<ActionEnvelope>,
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
    /// Whether focus may be requested by input, traversal, accessibility, or a controller.
    pub can_request_focus: bool,
    /// Select the complete value when focus enters the field.
    pub select_all_on_focus: bool,
    /// Whether the framework-painted caret is visible while focused.
    pub show_cursor: bool,
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
    /// Visual-only breaks inside the logical text value.
    ///
    /// Each offset is a UTF-8 byte offset at a grapheme boundary in the
    /// concatenated logical value. Active IME replacements remap surrounding
    /// breaks to the displayed text; breaks inside the replaced range are
    /// hidden until composition commits. A break never changes editing or
    /// accessibility text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub display_breaks: Vec<fission_ir::op::TextDisplayBreak>,
    /// Paragraph-specific alignment and leading indentation for the displayed value.
    ///
    /// Offsets use UTF-8 bytes at grapheme boundaries. A span applies until the
    /// next span; provide one at each paragraph start whose layout differs.
    /// Active IME replacements remap the spans around the composing range.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paragraph_styles: Vec<TextParagraphStyleSpan>,
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
    pub prefix: Option<Widget>,
    /// Optional trailing decoration node.
    pub suffix: Option<Widget>,
    /// Optional hover cursor override while pointing at the field.
    pub mouse_cursor: Option<SemanticsMouseCursor>,
    /// Preferred software keyboard / input modality.
    pub keyboard_type: TextInputType,
    /// Preferred return/submit action.
    pub text_input_action: TextInputAction,
    /// Automatic capitalization strategy for inserted text.
    pub text_capitalization: TextCapitalization,
    /// Maximum number of user-perceived grapheme clusters allowed in the field.
    pub max_length: Option<usize>,
    /// Whether `max_length` is enforced during editing.
    pub max_length_enforcement: MaxLengthEnforcement,
    /// Structured input formatters applied to inserted text.
    pub input_formatters: Vec<InputFormatter>,
    /// Application-defined complete-value formatters.
    ///
    /// These remain in core's runtime object rather than serializable IR. Use
    /// built-in [`InputFormatter`] values when a formatter must cross a process
    /// or static serialization boundary.
    #[serde(skip, default)]
    pub custom_input_formatters: Vec<crate::SharedTextInputFormatter>,
    /// Semantic form field name.
    pub name: Option<String>,
    /// Logical form membership used by [`TextFormController`](crate::TextFormController).
    pub form_id: Option<String>,
    /// Autofill session/group shared by related fields.
    pub autofill_group: Option<String>,
    /// Whether the field requires a non-empty value.
    pub required: bool,
    /// Minimum number of user-perceived graphemes.
    pub min_length: Option<usize>,
    /// Declarative validation pattern for supported targets.
    pub validation_pattern: Option<String>,
    /// Application-authoritative validity state.
    pub validation_state: TextFieldValidationState,
    /// Accessible validation message. This does not replace `error_text`.
    pub validation_message: Option<String>,
    /// Application validation evaluated against the complete editing value.
    #[serde(skip, default)]
    pub validator: Option<SharedTextInputValidator>,
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
    /// Editable paragraph wrapping behavior.
    pub wrap_mode: TextWrapMode,
    /// Automatic scrolling and scrollbar visibility policy.
    pub scroll_policy: TextScrollPolicy,
    /// Whether overflowing multiline content displays a scrollbar.
    pub show_scrollbar: bool,
    /// User-driven scrolling policy.
    pub scroll_physics: TextScrollPhysics,
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
    /// Sets the stable identifier exposed to accessibility and test tooling.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Adds semantic role/relationship metadata to this editable control.
    ///
    /// This is intended for composites such as editable popup controls and for
    /// form wrappers that associate external labels or descriptions. The
    /// widget continues to own all text-editing semantics.
    pub fn semantics(mut self, semantics: Semantics) -> Self {
        self.semantics = Some(semantics);
        self
    }

    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self.editing_value = None;
        self
    }

    pub fn editing_value(mut self, value: crate::TextEditingValue) -> Self {
        self.value = value.text.clone();
        self.editing_value = Some(value);
        self
    }

    /// Adds visual-only line/page breaks without changing the logical text.
    pub fn display_breaks(
        mut self,
        breaks: impl Into<Vec<fission_ir::op::TextDisplayBreak>>,
    ) -> Self {
        self.display_breaks = breaks.into();
        self
    }

    /// Applies alignment and indentation from each logical paragraph start.
    pub fn paragraph_styles(mut self, styles: impl Into<Vec<TextParagraphStyleSpan>>) -> Self {
        self.paragraph_styles = styles.into();
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

    pub fn custom_input_formatter(mut self, formatter: crate::SharedTextInputFormatter) -> Self {
        self.custom_input_formatters.push(formatter);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn autofill_group(mut self, group: impl Into<String>) -> Self {
        self.autofill_group = Some(group.into());
        self
    }
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
    pub fn min_length(mut self, min_length: usize) -> Self {
        self.min_length = Some(min_length);
        self
    }
    pub fn validation_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.validation_pattern = Some(pattern.into());
        self
    }
    pub fn validation_state(mut self, state: TextFieldValidationState) -> Self {
        self.validation_state = state;
        self
    }
    pub fn validation_message(mut self, message: impl Into<String>) -> Self {
        self.validation_message = Some(message.into());
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

    /// Controls whether overflowing multiline content displays a scrollbar.
    pub fn show_scrollbar(mut self, show_scrollbar: bool) -> Self {
        self.show_scrollbar = show_scrollbar;
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

    /// Handles Tab and Shift+Tab instead of allowing focus traversal to move
    /// away from this field. Inspect `ctx.input.key_press().modifiers` to
    /// distinguish forward from reverse navigation.
    pub fn on_tab(mut self, action: ActionEnvelope) -> Self {
        self.on_tab = Some(action);
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

    pub fn prefix(mut self, node: impl Into<Widget>) -> Self {
        self.prefix = Some(node.into());
        self
    }

    pub fn suffix(mut self, node: impl Into<Widget>) -> Self {
        self.suffix = Some(node.into());
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
}

impl Default for TextInput {
    fn default() -> Self {
        Self {
            id: None,
            semantics_identifier: None,
            semantics: None,
            value: String::new(),
            editing_value: None,
            label: None,
            placeholder: None,
            helper_text: None,
            error_text: None,
            counter_text: None,
            on_input: None,
            on_submit: None,
            on_tab: None,
            on_editing_complete: None,
            on_tap_outside: None,
            on_focus: None,
            on_blur: None,
            on_validation: None,
            width: None,
            height: None,
            size: ComponentSize::Md,
            padding: None,
            multiline: false,
            autofocus: false,
            can_request_focus: true,
            select_all_on_focus: false,
            show_cursor: true,
            enabled: true,
            read_only: false,
            min_lines: None,
            max_lines: None,
            obscure_text: false,
            obscuring_character: '•',
            mask: None,
            styled_runs: None,
            display_breaks: Vec::new(),
            paragraph_styles: Vec::new(),
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
            text_align_vertical: TextAlignVertical::Auto,
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
            max_length_enforcement: MaxLengthEnforcement::AfterComposition,
            input_formatters: Vec::new(),
            custom_input_formatters: Vec::new(),
            name: None,
            form_id: None,
            autofill_group: None,
            required: false,
            min_length: None,
            validation_pattern: None,
            validation_state: TextFieldValidationState::Unvalidated,
            validation_message: None,
            validator: None,
            autocorrect: true,
            enable_suggestions: true,
            spell_check: true,
            smart_dashes: true,
            smart_quotes: true,
            autofill_hints: Vec::new(),
            scroll_padding: None,
            wrap_mode: TextWrapMode::Soft,
            scroll_policy: TextScrollPolicy::Auto,
            show_scrollbar: true,
            scroll_physics: TextScrollPhysics::Platform,
            drag_start_behavior: DragStartBehavior::Start,
            context_menu: TextContextMenuConfig::editing(),
            selection_controls: TextSelectionControls::default(),
            magnifier_configuration: TextMagnifierConfiguration::default(),
            undo_controller: None,
            spell_check_configuration: None,
            restoration_id: None,
        }
    }
}
