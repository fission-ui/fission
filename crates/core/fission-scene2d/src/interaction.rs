use fission_scene::Vec2;
use serde::{Deserialize, Serialize};

/// Stable serialisable reference to an application action.
///
/// The Fission adapter maps this value to the same typed action for pointer,
/// keyboard, accessibility, and semantic-test activation.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ActionToken(pub u128);

/// One Fission action ID and its serialised typed payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionBinding2D {
    pub action: ActionToken,
    pub payload: Vec<u8>,
}

impl ActionBinding2D {
    pub fn new(action: ActionToken, payload: Vec<u8>) -> Self {
        Self { action, payload }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticRole2D {
    Button,
    Image,
    Generic,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DragActions2D {
    pub start: Option<ActionBinding2D>,
    pub update: Option<ActionBinding2D>,
    pub end: Option<ActionBinding2D>,
    pub cancel: Option<ActionBinding2D>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interaction2D {
    pub tap: Option<ActionBinding2D>,
    pub drag: Option<DragActions2D>,
    pub long_press: Option<ActionBinding2D>,
    pub semantic_label: Option<String>,
    pub semantic_role: Option<SemanticRole2D>,
}

impl Interaction2D {
    pub fn is_interactive(&self) -> bool {
        self.tap.is_some()
            || self.long_press.is_some()
            || self.drag.as_ref().is_some_and(|drag| {
                drag.start.is_some()
                    || drag.update.is_some()
                    || drag.end.is_some()
                    || drag.cancel.is_some()
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionEvent2D {
    Tap {
        scene_position: Vec2,
    },
    DragStart {
        scene_position: Vec2,
    },
    DragUpdate {
        scene_position: Vec2,
        scene_delta: Vec2,
    },
    DragEnd {
        scene_position: Vec2,
    },
    DragCancel,
    LongPress {
        scene_position: Vec2,
    },
    Activate,
}

impl Interaction2D {
    /// Resolve every input path to its declared application action.
    pub fn action_for(&self, event: InteractionEvent2D) -> Option<&ActionBinding2D> {
        match event {
            InteractionEvent2D::Tap { .. } | InteractionEvent2D::Activate => self.tap.as_ref(),
            InteractionEvent2D::LongPress { .. } => self.long_press.as_ref(),
            InteractionEvent2D::DragStart { .. } => self.drag.as_ref()?.start.as_ref(),
            InteractionEvent2D::DragUpdate { .. } => self.drag.as_ref()?.update.as_ref(),
            InteractionEvent2D::DragEnd { .. } => self.drag.as_ref()?.end.as_ref(),
            InteractionEvent2D::DragCancel => self.drag.as_ref()?.cancel.as_ref(),
        }
    }
}
