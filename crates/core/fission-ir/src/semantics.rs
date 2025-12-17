use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Button,
    Text,
    TextInput,
    Image,
    Checkbox,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ActionSet {
    pub entries: Vec<ActionEntry>,
}
