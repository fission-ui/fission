use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Button,
    Text,
    TextInput,
    Image,
    Checkbox,
    Switch,
    Dialog,
    Slider,
    Input,
    List,
    ListItem,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionTrigger {
    Default, // Tap/Click
    DragStart,
    DragUpdate,
    DragEnd,
    HoverEnter,
    HoverExit,
    Focus,
    Blur,
    Change, // Value change (Slider, Input)
    CursorChange, // Caret/anchor position changed
    Drop,
    DragEnter,
    DragLeave,
    SecondaryClick, // Right-click / secondary mouse button
}

impl Default for ActionTrigger {
    fn default() -> Self {
        ActionTrigger::Default
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionEntry {
    pub trigger: ActionTrigger,
    pub action_id: u128,               // Raw ActionId (u128)
    pub payload_data: Option<Vec<u8>>, // Serialized Action instance, if it has payload
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Semantics {
    pub role: Role,
    pub label: Option<String>,
    pub value: Option<String>,
    pub actions: ActionSet,
    pub focusable: bool,
    pub multiline: bool,
    pub masked: bool,
    pub input_mask: Option<InputMask>,
    pub ime_preedit_range: Option<(usize, usize)>,
    pub checked: Option<bool>,
    pub disabled: bool,
    pub draggable: bool,
    pub scrollable_x: bool,
    pub scrollable_y: bool,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
    pub current_value: Option<f32>,
    pub is_focus_scope: bool,
    pub is_focus_barrier: bool,
    pub drag_payload: Option<Vec<u8>>,
    pub hero_tag: Option<String>,
    pub focus_index: Option<i32>,
    /// When true, Tab key inserts spaces instead of moving focus.
    pub capture_tab: bool,
    /// When true, Enter copies leading whitespace from the current line.
    pub auto_indent: bool,
}

impl std::hash::Hash for Semantics {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.role.hash(state);
        self.label.hash(state);
        self.value.hash(state);
        self.actions.hash(state);
        self.focusable.hash(state);
        self.multiline.hash(state);
        self.masked.hash(state);
        self.input_mask.hash(state);
        self.ime_preedit_range.hash(state);
        self.checked.hash(state);
        self.disabled.hash(state);
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
        self.capture_tab.hash(state);
        self.auto_indent.hash(state);
    }
}

impl Default for Semantics {
    fn default() -> Self {
        Self {
            role: Role::Generic,
            label: None,
            value: None,
            actions: ActionSet::default(),
            focusable: false,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
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
            capture_tab: false,
            auto_indent: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ActionSet {
    pub entries: Vec<ActionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputMask {
    Numeric,
    Alphanumeric,
}

impl InputMask {
    pub fn is_valid_char(&self, ch: char) -> bool {
        match self {
            InputMask::Numeric => ch.is_ascii_digit(),
            InputMask::Alphanumeric => ch.is_ascii_alphanumeric(),
        }
    }
}
