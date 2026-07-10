use crate::{
    action::map::{MapSetCenter, MapSetZoom},
    env::MapStateMap,
    Action, ActionEnvelope,
};
use anyhow::{anyhow, Result};
use serde_json;

pub fn handle_map_action(map_state: &mut MapStateMap, action: &ActionEnvelope) -> Result<bool> {
    if action.id == MapSetCenter::static_id() {
        let cmd: MapSetCenter = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize MapSetCenter: {}", e))?;
        if let Some(state) = map_state.states.get_mut(&cmd.target) {
            state.center = (cmd.latitude, cmd.longitude);
        }
        return Ok(true);
    }

    if action.id == MapSetZoom::static_id() {
        let cmd: MapSetZoom = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize MapSetZoom: {}", e))?;
        if let Some(state) = map_state.states.get_mut(&cmd.target) {
            state.zoom = cmd.zoom.clamp(0.0, 22.0);
        }
        return Ok(true);
    }

    Ok(false)
}
