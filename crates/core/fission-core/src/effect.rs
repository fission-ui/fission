use serde::{Deserialize, Serialize};
use crate::action::ActionEnvelope;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReqId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u64);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SystemEffect {
    HttpGet { url: String, headers: Vec<(String, String)> },
    FileRead { path: String },
    Cancel { req_id: u64 },
    ReleaseResource { resource_id: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    System(SystemEffect),
    App(Vec<u8>), // Opaque app-specific payload
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectEnvelope {
    pub req_id: u64,
    pub effect: Effect,
    pub on_ok: Option<ActionEnvelope>,
    pub on_err: Option<ActionEnvelope>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EffectPayload {
    InlineBytes(Vec<u8>),
    Resource(u64),
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActionInput {
    None,
    EffectOk { req_id: u64, payload: EffectPayload },
    EffectErr { req_id: u64, message: String },
}

impl ActionInput {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            ActionInput::EffectOk { payload: EffectPayload::InlineBytes(b), .. } => Some(b),
            _ => None,
        }
    }
}
