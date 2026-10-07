//! Device-independent host input and declarative message bindings.

use std::collections::{BTreeMap, BTreeSet};

use fission_scene::{NodeId, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameKey {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Confirm,
    Cancel,
    Space,
    Character(char),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameButton {
    South,
    East,
    West,
    North,
    Start,
    Select,
    LeftShoulder,
    RightShoulder,
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameAxis {
    Horizontal,
    Vertical,
    LeftX,
    LeftY,
    RightX,
    RightY,
    Named(String),
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct PointerId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneGesture {
    Tap,
    DragStart,
    DragUpdate,
    DragEnd,
    DragCancel,
    LongPress,
    Activate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "event")]
pub enum HostInputEvent {
    Key {
        key: GameKey,
        pressed: bool,
    },
    Button {
        button: GameButton,
        pressed: bool,
    },
    Axis {
        axis: GameAxis,
        value: f32,
    },
    Pointer {
        id: PointerId,
        kind: PointerKind,
        phase: PointerPhase,
        button: Option<PointerButton>,
        position: Vec2,
        #[serde(default)]
        target: Option<NodeId>,
    },
    SceneGesture {
        node: NodeId,
        gesture: SceneGesture,
    },
    FocusLost,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "trigger")]
pub enum InputTrigger {
    KeyPressed { key: GameKey },
    KeyReleased { key: GameKey },
    ButtonPressed { button: GameButton },
    ButtonReleased { button: GameButton },
    AxisPositive { axis: GameAxis },
    AxisNegative { axis: GameAxis },
    SceneGesture { node: NodeId, gesture: SceneGesture },
    Cancel,
}

impl HostInputEvent {
    pub fn trigger(&self) -> Option<InputTrigger> {
        match self {
            Self::Key { key, pressed: true } => Some(InputTrigger::KeyPressed { key: key.clone() }),
            Self::Key {
                key,
                pressed: false,
            } => Some(InputTrigger::KeyReleased { key: key.clone() }),
            Self::Button {
                button,
                pressed: true,
            } => Some(InputTrigger::ButtonPressed {
                button: button.clone(),
            }),
            Self::Button {
                button,
                pressed: false,
            } => Some(InputTrigger::ButtonReleased {
                button: button.clone(),
            }),
            Self::Axis { axis, value } if *value > 0.0 => {
                Some(InputTrigger::AxisPositive { axis: axis.clone() })
            }
            Self::Axis { axis, value } if *value < 0.0 => {
                Some(InputTrigger::AxisNegative { axis: axis.clone() })
            }
            Self::SceneGesture { node, gesture } => Some(InputTrigger::SceneGesture {
                node: *node,
                gesture: *gesture,
            }),
            Self::Pointer {
                target: Some(node),
                phase: PointerPhase::Cancel,
                ..
            } => Some(InputTrigger::SceneGesture {
                node: *node,
                gesture: SceneGesture::DragCancel,
            }),
            Self::FocusLost => Some(InputTrigger::Cancel),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputState {
    pressed_keys: BTreeSet<GameKey>,
    pressed_buttons: BTreeSet<GameButton>,
    axes: BTreeMap<GameAxis, f32>,
    pointers: BTreeMap<PointerId, Vec2>,
}

impl InputState {
    pub fn key_pressed(&self, key: &GameKey) -> bool {
        self.pressed_keys.contains(key)
    }

    pub fn button_pressed(&self, button: &GameButton) -> bool {
        self.pressed_buttons.contains(button)
    }

    pub fn axis(&self, axis: &GameAxis) -> f32 {
        self.axes.get(axis).copied().unwrap_or_default()
    }

    pub fn pointer(&self, id: PointerId) -> Option<Vec2> {
        self.pointers.get(&id).copied()
    }

    pub(crate) fn apply(&mut self, event: &HostInputEvent) {
        match event {
            HostInputEvent::Key { key, pressed } => {
                if *pressed {
                    self.pressed_keys.insert(key.clone());
                } else {
                    self.pressed_keys.remove(key);
                }
            }
            HostInputEvent::Button { button, pressed } => {
                if *pressed {
                    self.pressed_buttons.insert(button.clone());
                } else {
                    self.pressed_buttons.remove(button);
                }
            }
            HostInputEvent::Axis { axis, value } => {
                self.axes.insert(axis.clone(), value.clamp(-1.0, 1.0));
            }
            HostInputEvent::Pointer {
                id,
                phase,
                position,
                ..
            } => match phase {
                PointerPhase::Down | PointerPhase::Move => {
                    self.pointers.insert(*id, *position);
                }
                PointerPhase::Up | PointerPhase::Cancel => {
                    self.pointers.remove(id);
                }
            },
            HostInputEvent::FocusLost => self.clear_transient(),
            HostInputEvent::SceneGesture { .. } => {}
        }
    }

    fn clear_transient(&mut self) {
        self.pressed_keys.clear();
        self.pressed_buttons.clear();
        self.axes.clear();
        self.pointers.clear();
    }
}

#[derive(Clone, Debug)]
pub struct InputMap<M> {
    bindings: Vec<(InputTrigger, M)>,
}

impl<M> Default for InputMap<M> {
    fn default() -> Self {
        Self { bindings: vec![] }
    }
}

impl<M> InputMap<M> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on(&mut self, trigger: InputTrigger) -> InputBinding<'_, M> {
        InputBinding { map: self, trigger }
    }

    /// Returns the device-independent triggers declared by the game.
    ///
    /// Host adapters use this to expose only the keys and controls the game
    /// actually owns instead of intercepting unrelated application input.
    pub fn triggers(&self) -> impl Iterator<Item = &InputTrigger> {
        self.bindings.iter().map(|(trigger, _)| trigger)
    }
}

impl<M: Clone> InputMap<M> {
    pub(crate) fn messages_for<'a>(
        &'a self,
        trigger: &'a InputTrigger,
    ) -> impl Iterator<Item = M> + 'a {
        self.bindings
            .iter()
            .filter(move |(candidate, _)| candidate == trigger)
            .map(|(_, message)| message.clone())
    }
}

#[must_use = "complete the binding with .send(message)"]
pub struct InputBinding<'a, M> {
    map: &'a mut InputMap<M>,
    trigger: InputTrigger,
}

impl<M> InputBinding<'_, M> {
    pub fn send(self, message: M) {
        self.map.bindings.push((self.trigger, message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_loss_cancels_and_clears_transient_state() {
        let mut state = InputState::default();
        state.apply(&HostInputEvent::Key {
            key: GameKey::ArrowRight,
            pressed: true,
        });
        assert!(state.key_pressed(&GameKey::ArrowRight));
        assert_eq!(
            HostInputEvent::FocusLost.trigger(),
            Some(InputTrigger::Cancel)
        );

        state.apply(&HostInputEvent::FocusLost);
        assert!(!state.key_pressed(&GameKey::ArrowRight));
    }
}
