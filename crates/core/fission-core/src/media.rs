use crate::{
    action::video::{
        VideoPause, VideoPlay, VideoSeek, VideoSetMuted, VideoSetRate, VideoSetVolume, VideoStop,
    },
    env::{VideoStateMap, VideoStatus},
    Action, ActionEnvelope,
};
use anyhow::{anyhow, Result};
use serde_json;

pub fn handle_video_action(video_map: &mut VideoStateMap, action: &ActionEnvelope) -> Result<bool> {
    if action.id == VideoPlay::static_id() {
        let cmd: VideoPlay = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize VideoPlay: {}", e))?;
        if let Some(video_state) = video_map.states.get_mut(&cmd.target) {
            video_state.status = VideoStatus::Playing;
        }
        return Ok(true);
    }

    if action.id == VideoPause::static_id() {
        let cmd: VideoPause = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize VideoPause: {}", e))?;
        if let Some(video_state) = video_map.states.get_mut(&cmd.target) {
            video_state.status = VideoStatus::Paused;
        }
        return Ok(true);
    }

    if action.id == VideoStop::static_id() {
        let cmd: VideoStop = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize VideoStop: {}", e))?;
        if let Some(video_state) = video_map.states.get_mut(&cmd.target) {
            video_state.status = VideoStatus::Stopped;
            video_state.position_ms = 0;
            video_state.pending_seek = Some(0);
        }
        return Ok(true);
    }

    if action.id == VideoSeek::static_id() {
        let cmd: VideoSeek = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize VideoSeek: {}", e))?;
        if let Some(video_state) = video_map.states.get_mut(&cmd.target) {
            video_state.position_ms = cmd.position_ms;
            video_state.pending_seek = Some(cmd.position_ms);
        }
        return Ok(true);
    }

    if action.id == VideoSetRate::static_id() {
        let cmd: VideoSetRate = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize VideoSetRate: {}", e))?;
        if let Some(video_state) = video_map.states.get_mut(&cmd.target) {
            video_state.rate = cmd.rate;
        }
        return Ok(true);
    }

    if action.id == VideoSetVolume::static_id() {
        let cmd: VideoSetVolume = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize VideoSetVolume: {}", e))?;
        if let Some(video_state) = video_map.states.get_mut(&cmd.target) {
            video_state.volume = cmd.volume.clamp(0.0, 1.0);
        }
        return Ok(true);
    }

    if action.id == VideoSetMuted::static_id() {
        let cmd: VideoSetMuted = serde_json::from_slice(&action.payload)
            .map_err(|e| anyhow!("Failed to deserialize VideoSetMuted: {}", e))?;
        if let Some(video_state) = video_map.states.get_mut(&cmd.target) {
            video_state.muted = cmd.muted;
        }
        return Ok(true);
    }

    Ok(false)
}
