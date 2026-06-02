use crate::{Action, ActionId};
use fission_ir::WidgetId;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPlay {
    pub target: WidgetId,
}

impl Action for VideoPlay {
    fn static_id() -> ActionId {
        *VIDEO_PLAY_ID
    }
}

lazy_static! {
    pub static ref VIDEO_PLAY_ID: ActionId = ActionId::from_name("fission_core::VideoPlay");
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPause {
    pub target: WidgetId,
}

impl Action for VideoPause {
    fn static_id() -> ActionId {
        *VIDEO_PAUSE_ID
    }
}

lazy_static! {
    pub static ref VIDEO_PAUSE_ID: ActionId = ActionId::from_name("fission_core::VideoPause");
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoStop {
    pub target: WidgetId,
}

impl Action for VideoStop {
    fn static_id() -> ActionId {
        *VIDEO_STOP_ID
    }
}

lazy_static! {
    pub static ref VIDEO_STOP_ID: ActionId = ActionId::from_name("fission_core::VideoStop");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSeek {
    pub target: WidgetId,
    pub position_ms: u64,
}

impl Action for VideoSeek {
    fn static_id() -> ActionId {
        *VIDEO_SEEK_ID
    }
}

lazy_static! {
    pub static ref VIDEO_SEEK_ID: ActionId = ActionId::from_name("fission_core::VideoSeek");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSetRate {
    pub target: WidgetId,
    pub rate: f32,
}

impl Action for VideoSetRate {
    fn static_id() -> ActionId {
        *VIDEO_SET_RATE_ID
    }
}

lazy_static! {
    pub static ref VIDEO_SET_RATE_ID: ActionId = ActionId::from_name("fission_core::VideoSetRate");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSetVolume {
    pub target: WidgetId,
    pub volume: f32,
}

impl Action for VideoSetVolume {
    fn static_id() -> ActionId {
        *VIDEO_SET_VOLUME_ID
    }
}

lazy_static! {
    pub static ref VIDEO_SET_VOLUME_ID: ActionId =
        ActionId::from_name("fission_core::VideoSetVolume");
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoSetMuted {
    pub target: WidgetId,
    pub muted: bool,
}

impl Action for VideoSetMuted {
    fn static_id() -> ActionId {
        *VIDEO_SET_MUTED_ID
    }
}

lazy_static! {
    pub static ref VIDEO_SET_MUTED_ID: ActionId =
        ActionId::from_name("fission_core::VideoSetMuted");
}
