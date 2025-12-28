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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEntry {
    pub action_id: u128,               // Raw ActionId (u128)
    pub payload_data: Option<Vec<u8>>, // Serialized Action instance, if it has payload
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ActionSet {
    pub entries: Vec<ActionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
