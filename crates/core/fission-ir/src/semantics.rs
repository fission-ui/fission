//! Accessibility and interaction semantics.
//!
//! The [`Semantics`] struct describes what a node *means* to assistive technology
//! and to the event system. It carries a [`Role`] (button, text input, slider, ...),
//! an optional human-readable label, a set of [`ActionEntry`]s that map input
//! triggers to framework actions, and flags for focus, drag-and-drop, scrollability,
//! and more.
//!
//! Semantics nodes appear in the IR as `Op::Semantics(semantics)`.

use serde::{Deserialize, Serialize};

/// The accessibility role of a node.
///
/// Roles tell screen readers and other assistive technology what kind of control a
/// node represents. Choose the most specific role that applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// A clickable button that triggers an action.
    Button,
    /// A navigational link.
    Link,
    /// An actionable item inside a menu.
    MenuItem,
    /// A read-only text label.
    Text,
    /// An editable text field (single or multi-line).
    TextInput,
    /// A raster or vector image.
    Image,
    /// A toggle that is either checked or unchecked.
    Checkbox,
    /// A one-of-many selectable option in a radio group.
    Radio,
    /// A toggle switch (on/off).
    Switch,
    /// A modal or non-modal dialog overlay.
    Dialog,
    /// A continuous range input (e.g., volume control).
    Slider,
    /// A generic form input that does not fit the other roles.
    Input,
    /// A scrollable list container.
    List,
    /// An individual item inside a [`List`](Role::List).
    ListItem,
    /// A node with no specific semantic role. The default.
    Generic,
}

/// Where a hyperlink should open its destination.
///
/// Shells map the standard variants to the host's native navigation model. HTML
/// renderers use the corresponding `_self`, `_blank`, `_parent`, and `_top`
/// targets, while [`Named`](Self::Named) preserves an application-provided
/// browsing-context name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LinkTarget {
    /// Reuse the active browsing context or native application window.
    #[default]
    Current,
    /// Open a new browser tab/window or the closest native equivalent.
    NewWindow,
    /// Navigate the parent browsing context.
    Parent,
    /// Navigate the top-level browsing context.
    Top,
    /// Navigate a named browsing context.
    Named(String),
}

impl LinkTarget {
    /// Returns the HTML `target` value represented by this target.
    pub fn as_html_target(&self) -> &str {
        match self {
            Self::Current => "_self",
            Self::NewWindow => "_blank",
            Self::Parent => "_parent",
            Self::Top => "_top",
            Self::Named(name) => name,
        }
    }
}

/// Declarative hyperlink metadata understood by every shell.
///
/// This lives on semantic nodes rather than on one concrete `Link` widget so
/// application and third-party widgets can expose genuine navigation without
/// inheriting Fission's visual link treatment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hyperlink {
    /// Destination URL or logical application route.
    pub href: String,
    /// Browsing context in which the destination opens.
    #[serde(default)]
    pub target: LinkTarget,
    /// Optional HTML relationship tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,
    /// Optional download filename; presence requests download behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<String>,
}

impl Hyperlink {
    pub fn new(href: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            target: LinkTarget::Current,
            rel: None,
            download: None,
        }
    }

    pub fn target(mut self, target: LinkTarget) -> Self {
        self.target = target;
        self
    }

    pub fn rel(mut self, rel: impl Into<String>) -> Self {
        self.rel = Some(rel.into());
        self
    }

    pub fn download(mut self, filename: impl Into<String>) -> Self {
        self.download = Some(filename.into());
        self
    }
}

/// Action requested from an HTML popover invocation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PopoverAction {
    /// Toggle the target popover's visibility.
    #[default]
    Toggle,
    /// Show the target popover.
    Show,
    /// Hide the target popover.
    Hide,
}

impl PopoverAction {
    pub fn as_html_action(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Show => "show",
            Self::Hide => "hide",
        }
    }
}

/// Declarative relationship between an invoker and an HTML popover.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PopoverTarget {
    /// DOM id of the popover controlled by this invoker.
    pub id: String,
    /// Visibility operation requested on activation.
    #[serde(default)]
    pub action: PopoverAction,
}

/// How a focusable node responds to pointer focus.
///
/// `FocusPolicy` only changes pointer-driven focus assignment. Keyboard focus,
/// accessibility focus, and semantic activation still work for focusable nodes.
///
/// # Example
///
/// A toolbar button can run its action without taking focus from an editor:
///
/// ```rust
/// use fission_ir::semantics::{FocusPolicy, Role};
/// use fission_ir::Semantics;
///
/// let semantics = Semantics {
///     role: Role::Button,
///     focusable: true,
///     focus_policy: FocusPolicy::PreserveCurrentOnPointer,
///     ..Semantics::default()
/// };
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FocusPolicy {
    /// Pointer-down focuses this node when it is focusable. This is the normal
    /// behavior for buttons, text inputs, and other controls.
    #[default]
    FocusOnPointer,
    /// Pointer-down keeps the currently focused node focused while still letting
    /// this node receive pointer state and activation actions.
    PreserveCurrentOnPointer,
}

/// What user interaction triggers an action.
///
/// Each [`ActionEntry`] pairs an `ActionTrigger` with an action ID so the event
/// system knows which callback to invoke for a given input gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionTrigger {
    /// Primary activation: tap, click, or Enter key.
    Default,
    /// The user began dragging this node.
    DragStart,
    /// The drag position changed (fires continuously).
    DragUpdate,
    /// The user released the drag.
    DragEnd,
    /// The pointer entered the node's hit area.
    HoverEnter,
    /// The pointer left the node's hit area.
    HoverExit,
    /// A semantic cursor request applied while the pointer hovers this node.
    ///
    /// This is metadata, not a dispatched reducer action.
    HoverCursor,
    /// The node received keyboard focus.
    Focus,
    /// The node lost keyboard focus.
    Blur,
    /// A pointer-down happened outside the active text field.
    TapOutside,
    /// The node's value changed (for example, a slider moved).
    Change,
    /// Reserved legacy numeric text-change trigger.
    ///
    /// Fission 0.11 `TextInput` never emits this trigger and shells must not
    /// interpret it. It remains in place to preserve serialized IR enum
    /// discriminants for the variants that follow it.
    #[deprecated(
        since = "0.11.0",
        note = "TextInput uses TextChanged and carries live edits in ActionInput"
    )]
    NumberChange,
    /// Text editing was explicitly completed by the current input method.
    EditingComplete,
    /// The user submitted a text field.
    Submit,
    /// The caret or selection anchor position changed in a text field.
    CursorChange,
    /// A dragged payload was dropped onto this node.
    Drop,
    /// A drag entered this node's hit area (for drop targets).
    DragEnter,
    /// A drag left this node's hit area (for drop targets).
    DragLeave,
    /// Right-click or secondary mouse button.
    SecondaryClick,
    /// A text field changed.
    ///
    /// The bound action payload remains unchanged. The edited value, widget
    /// identity, caret, and anchor are delivered as runtime action input.
    TextChanged,
    /// An interactive viewport began a pan or zoom gesture.
    ViewportInteractionStart,
    /// An interactive viewport's camera changed during a gesture.
    ViewportInteractionUpdate,
    /// An interactive viewport gesture ended; configured inertia may continue.
    ViewportInteractionEnd,
    /// A text field's validation state was requested or changed.
    Validation,
}

#[cfg(test)]
mod tests {
    use super::ActionTrigger;

    #[test]
    fn text_changed_round_trips_through_ir_serialization() {
        let encoded = serde_json::to_string(&ActionTrigger::TextChanged).unwrap();
        assert_eq!(encoded, "\"TextChanged\"");
        let decoded: ActionTrigger = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ActionTrigger::TextChanged);
    }

    #[test]
    #[allow(deprecated)]
    fn existing_action_trigger_discriminants_remain_stable() {
        assert_eq!(ActionTrigger::Change as u8, 10);
        assert_eq!(ActionTrigger::NumberChange as u8, 11);
        assert_eq!(ActionTrigger::EditingComplete as u8, 12);
        assert_eq!(ActionTrigger::SecondaryClick as u8, 18);
        assert_eq!(ActionTrigger::TextChanged as u8, 19);
        assert_eq!(ActionTrigger::ViewportInteractionStart as u8, 20);
        assert_eq!(ActionTrigger::ViewportInteractionUpdate as u8, 21);
        assert_eq!(ActionTrigger::ViewportInteractionEnd as u8, 22);
    }
}

impl Default for ActionTrigger {
    fn default() -> Self {
        ActionTrigger::Default
    }
}

/// Semantic cursor requests that shells map onto platform cursor icons.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MouseCursor {
    #[default]
    Default = 0,
    Pointer = 1,
    Text = 2,
    Crosshair = 3,
    Move = 4,
    NotAllowed = 5,
    Grab = 6,
    Grabbing = 7,
    Wait = 8,
    Help = 9,
    VerticalText = 10,
}

impl MouseCursor {
    pub fn from_repr(value: u128) -> Option<Self> {
        match value {
            0 => Some(Self::Default),
            1 => Some(Self::Pointer),
            2 => Some(Self::Text),
            3 => Some(Self::Crosshair),
            4 => Some(Self::Move),
            5 => Some(Self::NotAllowed),
            6 => Some(Self::Grab),
            7 => Some(Self::Grabbing),
            8 => Some(Self::Wait),
            9 => Some(Self::Help),
            10 => Some(Self::VerticalText),
            _ => None,
        }
    }
}

/// Preferred software keyboard / input modality for a text field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextInputType {
    #[default]
    Text,
    Multiline,
    Number,
    EmailAddress,
    Url,
    Phone,
    Name,
}

/// Editable multiline wrapping and submission behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextWrapMode {
    /// Wrap visually without inserting line breaks into submitted text.
    #[default]
    Soft,
    /// Wrap visually and allow HTML textarea targets to submit hard line breaks.
    Hard,
    /// Do not wrap; overflow scrolls horizontally.
    NoWrap,
}

/// Preferred action for the return/submit key on software keyboards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextInputAction {
    #[default]
    Done,
    Go,
    Search,
    Send,
    Next,
    Previous,
    Continue,
    Join,
    Route,
    EmergencyCall,
    Newline,
}

/// Automatic capitalization strategy for inserted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextCapitalization {
    #[default]
    None,
    Characters,
    Words,
    Sentences,
}

/// Whether the framework should enforce `max_length` during editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MaxLengthEnforcement {
    None,
    Enforced,
    /// Allow the active composing value to exceed the limit and enforce it
    /// once the input method commits.
    #[default]
    AfterComposition,
}

/// Structured formatter primitives applied to inserted text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputFormatter {
    DigitsOnly,
    AsciiOnly,
    InternalLowercase,
    Uppercase,
    TrimWhitespace,
    SingleLine,
}

/// Marks a semantic subtree as one coordinated read-only text selection region.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SelectionRegionSemantics {
    /// Excluded regions prevent an ancestor region from selecting this subtree.
    pub excluded: bool,
    /// Text inserted between selectable descendants when copying or exposing
    /// the region as one accessibility value.
    pub separator: String,
}

/// Declarative validity of an editable field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextFieldValidationState {
    #[default]
    Unvalidated,
    Valid,
    Invalid,
}

/// A single action binding: a trigger, an action ID, and optional payload.
///
/// When the event system detects the input described by `trigger`, it dispatches
/// the action identified by `action_id`. If the action carries data (e.g., drag
/// coordinates), `payload_data` holds the serialized payload.
///
/// # Example
///
/// ```rust
/// use fission_ir::semantics::{ActionEntry, ActionTrigger};
///
/// let entry = ActionEntry {
///     trigger: ActionTrigger::Default,
///     action_id: 42,
///     payload_data: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionEntry {
    /// Which input gesture triggers this action.
    pub trigger: ActionTrigger,
    /// The raw 128-bit action ID dispatched to the widget's action handler.
    pub action_id: u128,
    /// Optional serialized payload. `None` for actions with no data.
    pub payload_data: Option<Vec<u8>>,
}

/// Canvas-specific semantic target used by the shared gesture controller.
///
/// This keeps stable document identity and geometry in backend-neutral IR while
/// action payloads remain entirely application-defined.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasTarget {
    pub canvas_id: u128,
    pub kind: CanvasTargetKind,
    pub selection_policy: CanvasSelectionPolicy,
    pub snap_spacing: Option<f32>,
    pub snap_threshold: f32,
}

impl std::hash::Hash for CanvasTarget {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canvas_id.hash(state);
        self.kind.hash(state);
        self.selection_policy.hash(state);
        self.snap_spacing.map(f32::to_bits).hash(state);
        self.snap_threshold.to_bits().hash(state);
    }
}

/// Declarative selection behavior requested by an infinite canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanvasSelectionPolicy {
    None,
    Single,
    Toggle,
    Marquee,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CanvasTargetKind {
    Node {
        node_id: u128,
        bounds: [f32; 4],
    },
    ResizeHandle {
        node_id: u128,
        handle: u8,
        bounds: [f32; 4],
    },
    Edge {
        edge_id: u128,
        points: Vec<[f32; 2]>,
        cubic: bool,
        hit_tolerance: f32,
    },
    Marquee,
}

impl std::hash::Hash for CanvasTargetKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Node { node_id, bounds } => {
                node_id.hash(state);
                bounds.iter().for_each(|value| value.to_bits().hash(state));
            }
            Self::ResizeHandle {
                node_id,
                handle,
                bounds,
            } => {
                node_id.hash(state);
                handle.hash(state);
                bounds.iter().for_each(|value| value.to_bits().hash(state));
            }
            Self::Edge {
                edge_id,
                points,
                cubic,
                hit_tolerance,
            } => {
                edge_id.hash(state);
                for point in points {
                    point[0].to_bits().hash(state);
                    point[1].to_bits().hash(state);
                }
                cubic.hash(state);
                hit_tolerance.to_bits().hash(state);
            }
            Self::Marquee => {}
        }
    }
}

impl ActionEntry {
    /// Creates a non-dispatched cursor request consumed by hover handling.
    pub fn hover_cursor(cursor: MouseCursor) -> Self {
        Self {
            trigger: ActionTrigger::HoverCursor,
            action_id: cursor as u128,
            payload_data: None,
        }
    }

    /// Returns the semantic cursor encoded by this entry, if any.
    pub fn as_hover_cursor(&self) -> Option<MouseCursor> {
        (self.trigger == ActionTrigger::HoverCursor)
            .then(|| MouseCursor::from_repr(self.action_id))
            .flatten()
    }
}

/// Accessibility and interaction metadata for a node.
///
/// `Semantics` is the IR's way of describing *what a node means* rather than how it
/// looks or where it is positioned. It is consumed by:
///
/// * Assistive technology (screen readers, switch control) via the accessibility tree.
/// * The event/focus system, which uses `focusable`, `actions`, and `disabled` to
///   route input.
/// * The drag-and-drop subsystem, which reads `draggable` and `drag_payload`.
///
/// Most fields default to "inert" values (see [`Default`] impl), so you only need to
/// set the fields that matter for a given widget.
///
/// # Example
///
/// ```rust
/// use fission_ir::Semantics;
/// use fission_ir::semantics::Role;
///
/// let sem = Semantics {
///     role: Role::Button,
///     label: Some("Submit".into()),
///     focusable: true,
///     ..Semantics::default()
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Semantics {
    /// The accessibility role. Defaults to [`Role::Generic`].
    pub role: Role,
    /// A human-readable label for assistive technology (e.g., "Close" for a button).
    pub label: Option<String>,
    /// Stable semantic identifier for tooling and automation.
    pub identifier: Option<String>,
    /// The current value as a string (e.g., the text in an input field).
    pub value: Option<String>,
    /// Optional hyperlink destination for this semantic region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<Hyperlink>,
    /// Optional HTML popover invocation metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popover_target: Option<PopoverTarget>,
    /// The set of actions this node responds to.
    pub actions: ActionSet,
    /// Structured InfiniteCanvas target metadata for contextual gesture input.
    #[serde(default)]
    pub canvas_target: Option<CanvasTarget>,
    /// Optional raw action dispatch scope inherited by descendant actions.
    #[serde(default)]
    pub action_scope_id: Option<u128>,
    /// Whether this node can receive keyboard focus.
    pub focusable: bool,
    /// How pointer-down should affect focus for this node.
    #[serde(default)]
    pub focus_policy: FocusPolicy,
    /// Whether this text input supports multiple lines.
    pub multiline: bool,
    /// Editable multiline wrapping and submission behavior.
    #[serde(default)]
    pub text_wrap_mode: TextWrapMode,
    /// Whether the value should be obscured (password fields).
    pub masked: bool,
    /// An optional input mask that restricts which characters are accepted.
    pub input_mask: Option<InputMask>,
    /// The byte range of IME pre-edit (composition) text, if any.
    pub ime_preedit_range: Option<(usize, usize)>,
    /// The active byte range within [`Semantics::ime_preedit_range`], if the
    /// platform IME exposes a pre-edit cursor or marked sub-range.
    #[serde(default)]
    pub ime_preedit_cursor_range: Option<(usize, usize)>,
    /// Editable or selectable text selection as byte offsets `(anchor, focus)`.
    #[serde(default)]
    pub text_selection: Option<(usize, usize)>,
    /// Whether this read-only text node supports pointer/keyboard selection.
    #[serde(default)]
    pub selectable_text: bool,
    /// Coordinated selection metadata for this semantic subtree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_region: Option<SelectionRegionSemantics>,
    /// Whether this node can open a framework-managed context menu.
    #[serde(default)]
    pub context_menu: bool,
    /// For checkboxes, radios, and switches: `Some(true)` = checked or selected,
    /// `Some(false)` = unchecked or unselected, and `None` = no checked state.
    pub checked: Option<bool>,
    /// Whether the node is disabled (grayed out, non-interactive).
    pub disabled: bool,
    /// Whether the node can be focused and selected but not edited.
    pub read_only: bool,
    /// Whether this node should receive focus automatically when mounted.
    pub autofocus: bool,
    /// Whether this node can be dragged.
    pub draggable: bool,
    /// Whether the node scrolls horizontally.
    pub scrollable_x: bool,
    /// Whether the node scrolls vertically.
    pub scrollable_y: bool,
    /// Minimum value for range inputs (sliders).
    pub min_value: Option<f32>,
    /// Maximum value for range inputs (sliders).
    pub max_value: Option<f32>,
    /// Current numeric value for range inputs (sliders).
    pub current_value: Option<f32>,
    /// When `true`, this node creates a new focus scope (like a dialog or panel).
    pub is_focus_scope: bool,
    /// When `true`, Tab traversal does not leave this subtree.
    pub is_focus_barrier: bool,
    /// Serialized payload attached to a drag operation.
    pub drag_payload: Option<Vec<u8>>,
    /// An identifier for hero/shared-element transitions.
    pub hero_tag: Option<String>,
    /// Explicit tab order index. InternalLower values receive focus first. `None` means
    /// the node follows document order.
    pub focus_index: Option<i32>,
    /// Preferred keyboard/input modality for text entry.
    pub text_input_type: TextInputType,
    /// Preferred submit/return key action.
    pub text_input_action: TextInputAction,
    /// Automatic capitalization strategy for inserted text.
    pub text_capitalization: TextCapitalization,
    /// Maximum number of user-perceived grapheme clusters allowed in the field.
    pub max_length: Option<usize>,
    /// Whether `max_length` should be enforced during editing.
    pub max_length_enforcement: MaxLengthEnforcement,
    /// Structured input formatters applied to inserted text.
    pub input_formatters: Vec<InputFormatter>,
    /// Name submitted by semantic form targets.
    #[serde(default)]
    pub text_field_name: Option<String>,
    /// Logical form membership for coordinated validation/submission.
    #[serde(default)]
    pub text_form_id: Option<String>,
    /// Autofill session/group shared by related fields.
    #[serde(default)]
    pub autofill_group: Option<String>,
    /// Whether a non-empty value is required.
    #[serde(default)]
    pub required: bool,
    /// Minimum number of user-perceived graphemes.
    #[serde(default)]
    pub min_length: Option<usize>,
    /// Pattern constraint for targets with pattern-validation support.
    #[serde(default)]
    pub validation_pattern: Option<String>,
    /// Application-authoritative field validity.
    #[serde(default)]
    pub validation_state: TextFieldValidationState,
    /// Accessible validation message, independent of visual decoration.
    #[serde(default)]
    pub validation_message: Option<String>,
    /// Hint to the platform IME whether autocorrect should be enabled.
    pub autocorrect: bool,
    /// Hint to the platform IME whether suggestions should be enabled.
    pub enable_suggestions: bool,
    /// Hint to the platform IME whether spell checking should be enabled.
    pub spell_check: bool,
    /// Hint to the platform IME whether smart dashes should be enabled.
    pub smart_dashes: bool,
    /// Hint to the platform IME whether smart quotes should be enabled.
    pub smart_quotes: bool,
    /// Platform autofill categories associated with this field.
    pub autofill_hints: Vec<String>,
    /// Extra padding to keep around the caret/selection when auto-scrolling `[left, right, top, bottom]`.
    pub scroll_padding: Option<[f32; 4]>,
    /// When true, Tab key inserts spaces instead of moving focus.
    pub capture_tab: bool,
    /// When true, Enter copies leading whitespace from the current line.
    pub auto_indent: bool,
}

impl std::hash::Hash for Semantics {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.role.hash(state);
        self.label.hash(state);
        self.identifier.hash(state);
        self.value.hash(state);
        self.hyperlink.hash(state);
        self.popover_target.hash(state);
        self.actions.hash(state);
        self.canvas_target.hash(state);
        self.action_scope_id.hash(state);
        self.focusable.hash(state);
        self.focus_policy.hash(state);
        self.multiline.hash(state);
        self.text_wrap_mode.hash(state);
        self.masked.hash(state);
        self.input_mask.hash(state);
        self.ime_preedit_range.hash(state);
        self.ime_preedit_cursor_range.hash(state);
        self.text_selection.hash(state);
        self.selectable_text.hash(state);
        self.selection_region.hash(state);
        self.context_menu.hash(state);
        self.checked.hash(state);
        self.disabled.hash(state);
        self.read_only.hash(state);
        self.autofocus.hash(state);
        self.draggable.hash(state);
        self.scrollable_x.hash(state);
        self.scrollable_y.hash(state);
        self.min_value.map(|f| f.to_bits()).hash(state);
        self.max_value.map(|f| f.to_bits()).hash(state);
        self.current_value.map(|f| f.to_bits()).hash(state);
        self.is_focus_scope.hash(state);
        self.is_focus_barrier.hash(state);
        self.drag_payload.hash(state);
        self.hero_tag.hash(state);
        self.focus_index.hash(state);
        self.text_input_type.hash(state);
        self.text_input_action.hash(state);
        self.text_capitalization.hash(state);
        self.max_length.hash(state);
        self.max_length_enforcement.hash(state);
        self.input_formatters.hash(state);
        self.text_field_name.hash(state);
        self.text_form_id.hash(state);
        self.autofill_group.hash(state);
        self.required.hash(state);
        self.min_length.hash(state);
        self.validation_pattern.hash(state);
        self.validation_state.hash(state);
        self.validation_message.hash(state);
        self.autocorrect.hash(state);
        self.enable_suggestions.hash(state);
        self.spell_check.hash(state);
        self.smart_dashes.hash(state);
        self.smart_quotes.hash(state);
        self.autofill_hints.hash(state);
        self.scroll_padding
            .map(|padding| padding.map(f32::to_bits))
            .hash(state);
        self.capture_tab.hash(state);
        self.auto_indent.hash(state);
    }
}

impl Default for Semantics {
    fn default() -> Self {
        Self {
            role: Role::Generic,
            label: None,
            identifier: None,
            value: None,
            hyperlink: None,
            popover_target: None,
            actions: ActionSet::default(),
            canvas_target: None,
            action_scope_id: None,
            focusable: false,
            focus_policy: FocusPolicy::FocusOnPointer,
            multiline: false,
            text_wrap_mode: TextWrapMode::Soft,
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
            focus_index: None,
            text_input_type: TextInputType::Text,
            text_input_action: TextInputAction::Done,
            text_capitalization: TextCapitalization::None,
            max_length: None,
            max_length_enforcement: MaxLengthEnforcement::AfterComposition,
            input_formatters: Vec::new(),
            text_field_name: None,
            text_form_id: None,
            autofill_group: None,
            required: false,
            min_length: None,
            validation_pattern: None,
            validation_state: TextFieldValidationState::Unvalidated,
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
        }
    }
}

/// A collection of [`ActionEntry`]s attached to a semantics node.
///
/// `ActionSet` is a simple wrapper around a `Vec<ActionEntry>`. It exists as a
/// named type so that serialization and hashing are straightforward.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ActionSet {
    /// The action entries. Order does not matter for dispatch; the event system
    /// matches on [`ActionTrigger`].
    pub entries: Vec<ActionEntry>,
}

/// Restricts which characters a text input accepts.
///
/// Apply an `InputMask` to a [`Semantics`] node to filter keystrokes before they
/// reach the text editing logic.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputMask {
    /// Accept only ASCII digits (`0`-`9`).
    Numeric,
    /// Accept only ASCII letters and digits (`a`-`z`, `A`-`Z`, `0`-`9`).
    Alphanumeric,
}

impl InputMask {
    /// Returns `true` if `ch` is accepted by this mask.
    ///
    /// # Example
    ///
    /// ```rust
    /// use fission_ir::semantics::InputMask;
    /// assert!(InputMask::Numeric.is_valid_char('5'));
    /// assert!(!InputMask::Numeric.is_valid_char('a'));
    /// ```
    pub fn is_valid_char(&self, ch: char) -> bool {
        match self {
            InputMask::Numeric => ch.is_ascii_digit(),
            InputMask::Alphanumeric => ch.is_ascii_alphanumeric(),
        }
    }
}
