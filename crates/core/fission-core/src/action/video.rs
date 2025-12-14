use crate::{Action, ActionId};
use serde::{Deserialize, Serialize};
use lazy_static::lazy_static;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPlay;

impl Action for VideoPlay {
    fn static_id() -> ActionId { *VIDEO_PLAY_ID }
}

lazy_static! {
    pub static ref VIDEO_PLAY_ID: ActionId = ActionId::from_name("fission_core::VideoPlay");
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPause;

impl Action for VideoPause {
    fn static_id() -> ActionId { *VIDEO_PAUSE_ID }
}

lazy_static! {
    pub static ref VIDEO_PAUSE_ID: ActionId = ActionId::from_name("fission_core::VideoPause");
}
