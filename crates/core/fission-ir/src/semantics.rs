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
    /// A read-only text label.
    Text,
    /// An editable text field (single or multi-line).
    TextInput,
    /// A raster or vector image.
    Image,
    /// A toggle that is either checked or unchecked.
    Checkbox,
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
    /// The node's value changed (sliders, text inputs, etc.).
    Change,
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
    #[default]
    Enforced,
}

/// Structured formatter primitives applied to inserted text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputFormatter {
    DigitsOnly,
    AsciiOnly,
    Lowercase,
    Uppercase,
    TrimWhitespace,
    SingleLine,
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
    /// The set of actions this node responds to.
    pub actions: ActionSet,
    /// Optional raw action dispatch scope inherited by descendant actions.
    #[serde(default)]
    pub action_scope_id: Option<u128>,
    /// Whether this node can receive keyboard focus.
    pub focusable: bool,
    /// Whether this text input supports multiple lines.
    pub multiline: bool,
    /// Whether the value should be obscured (password fields).
    pub masked: bool,
    /// An optional input mask that restricts which characters are accepted.
    pub input_mask: Option<InputMask>,
    /// The byte range of IME pre-edit (composition) text, if any.
    pub ime_preedit_range: Option<(usize, usize)>,
    /// For checkboxes and switches: `Some(true)` = checked, `Some(false)` = unchecked,
    /// `None` = not a toggle.
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
    /// Explicit tab order index. Lower values receive focus first. `None` means
    /// the node follows document order.
    pub focus_index: Option<i32>,
    /// Preferred keyboard/input modality for text entry.
    pub text_input_type: TextInputType,
    /// Preferred submit/return key action.
    pub text_input_action: TextInputAction,
    /// Automatic capitalization strategy for inserted text.
    pub text_capitalization: TextCapitalization,
    /// Maximum number of Unicode scalar values allowed in the field.
    pub max_length: Option<usize>,
    /// Whether `max_length` should be enforced during editing.
    pub max_length_enforcement: MaxLengthEnforcement,
    /// Structured input formatters applied to inserted text.
    pub input_formatters: Vec<InputFormatter>,
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
        self.actions.hash(state);
        self.action_scope_id.hash(state);
        self.focusable.hash(state);
        self.multiline.hash(state);
        self.masked.hash(state);
        self.input_mask.hash(state);
        self.ime_preedit_range.hash(state);
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
            actions: ActionSet::default(),
            action_scope_id: None,
            focusable: false,
            multiline: false,
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
            text_input_type: TextInputType::Text,
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
