use fission_theme::Theme;
use fission_i18n::{I18nRegistry, Locale};
use fission_ir::NodeId;
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