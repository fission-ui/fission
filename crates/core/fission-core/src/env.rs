use crate::{action::AppState, registry::AnimationPropertyId};
use fission_i18n::{I18nRegistry, Locale};
use fission_ir::{NodeId, WidgetNodeId};
use fission_theme::Theme;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Static environment data (Theme, I18n)
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub theme: Theme,
    pub i18n: I18nRegistry,
    pub locale: Locale,
}

// Runtime state managed by framework (Interaction)
#[derive(Clone, Debug, Default)]
pub struct RuntimeState {
    pub interaction: InteractionStateMap,
    pub scroll: ScrollStateMap,
    pub animation: AnimationStateMap,
    pub video: VideoStateMap,
    pub ime_preedit: Option<(NodeId, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationStateMap {
    pub values: HashMap<(WidgetNodeId, AnimationPropertyId), f32>,
    pub active: HashMap<(WidgetNodeId, AnimationPropertyId), ActiveAnimation>,
}

#[derive(Clone, Debug)]
pub struct ActiveAnimation {
    pub target: WidgetNodeId,
    pub property: AnimationPropertyId,
    pub start_value: f32,
    pub end_value: f32,
    pub start_time: u64,
    pub duration: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ScrollStateMap {
    pub offsets: HashMap<NodeId, f32>,
}

impl ScrollStateMap {
    pub fn get_offset(&self, id: NodeId) -> f32 {
        *self.offsets.get(&id).unwrap_or(&0.0)
    }

    pub fn set_offset(&mut self, id: NodeId, offset: f32) {
        self.offsets.insert(id, offset);
    }
}

#[derive(Clone, Debug, Default)]
pub struct InteractionStateMap {
    pub hovered: HashMap<NodeId, bool>,
    pub pressed: HashMap<NodeId, bool>,
    pub focused: Option<NodeId>,
}

impl InteractionStateMap {
    pub fn is_hovered(&self, id: NodeId) -> bool {
        self.hovered.get(&id).copied().unwrap_or(false)
    }
    pub fn is_pressed(&self, id: NodeId) -> bool {
        self.pressed.get(&id).copied().unwrap_or(false)
    }
    pub fn is_focused(&self, id: NodeId) -> bool {
        self.focused == Some(id)
    }

    pub fn set_hovered(&mut self, id: NodeId, value: bool) {
        if value {
            self.hovered.insert(id, true);
        } else {
            self.hovered.remove(&id);
        }
    }

    pub fn set_pressed(&mut self, id: NodeId, value: bool) {
        if value {
            self.pressed.insert(id, true);
        } else {
            self.pressed.remove(&id);
        }
    }

    pub fn set_focused(&mut self, id: Option<NodeId>) {
        self.focused = id;
    }
}

#[derive(Clone, Debug, Default)]
pub struct VideoStateMap {
    pub states: HashMap<WidgetNodeId, VideoState>,
}

impl AppState for VideoStateMap {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoState {
    pub status: VideoStatus,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub rate: f32,
    pub volume: f32,
    pub muted: bool,
    pub looped: bool,
    pub asset_source: String,
    pub surface_id: Option<u64>,
    pub pending_seek: Option<u64>,
}

impl Default for VideoState {
    fn default() -> Self {
        Self {
            status: VideoStatus::Stopped,
            position_ms: 0,
            duration_ms: None,
            rate: 1.0,
            volume: 1.0,
            muted: false,
            looped: false,
            asset_source: String::new(),
            surface_id: None,
            pending_seek: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum VideoStatus {
    Stopped,
    Playing,
    Paused,
    Buffering,
    Ended,
    Error,
}
