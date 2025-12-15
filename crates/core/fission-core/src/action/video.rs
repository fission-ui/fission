use crate::{Action, ActionId};
use fission_ir::WidgetNodeId;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPlay {
    pub target: WidgetNodeId,
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
    pub target: WidgetNodeId,
}

impl Action for VideoPause {
    fn static_id() -> ActionId {
        *VIDEO_PAUSE_ID
    }
}

lazy_static! {
    pub static ref VIDEO_PAUSE_ID: ActionId = ActionId::from_name("fission_core::VideoPause");
}
