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
    FileOpenDialog { extensions: Vec<String> }, // Returns path as InlineBytes (utf8)
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
    Pointer { x: f32, y: f32, delta_x: f32, delta_y: f32 },
    Drop { paths: Vec<String>, x: f32, y: f32 },
}

impl ActionInput {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            ActionInput::EffectOk { payload: EffectPayload::InlineBytes(b), .. } => Some(b),
            _ => None,
        }
    }
    
    pub fn as_pointer(&self) -> Option<(f32, f32, f32, f32)> {
        match self {
            ActionInput::Pointer { x, y, delta_x, delta_y } => Some((*x, *y, *delta_x, *delta_y)),
            ActionInput::Drop { x, y, .. } => Some((*x, *y, 0.0, 0.0)),
            _ => None,
        }
    }
    
    pub fn as_drop_paths(&self) -> Option<&[String]> {
        match self {
            ActionInput::Drop { paths, .. } => Some(paths),
            _ => None,
        }
    }
}
