use crate::{Action, ActionId};
use fission_ir::WidgetId;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapSetCenter {
    pub target: WidgetId,
    pub latitude: f64,
    pub longitude: f64,
}

impl Action for MapSetCenter {
    fn static_id() -> ActionId {
        *MAP_SET_CENTER_ID
    }
}

lazy_static! {
    pub static ref MAP_SET_CENTER_ID: ActionId =
        ActionId::from_name("fission_core::MapSetCenter");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapSetZoom {
    pub target: WidgetId,
    pub zoom: f32,
}

impl Action for MapSetZoom {
    fn static_id() -> ActionId {
        *MAP_SET_ZOOM_ID
    }
}

lazy_static! {
    pub static ref MAP_SET_ZOOM_ID: ActionId = ActionId::from_name("fission_core::MapSetZoom");
}
