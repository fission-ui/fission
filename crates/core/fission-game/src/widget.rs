//! Fission widget integration for the deterministic game input map.

use std::collections::BTreeSet;

use fission_core::authoring::{custom_widget, IrBuilder, Lower, LowerWidget, LoweringContext};
use fission_core::ui::Widget;
use fission_core::{Action, ActionEnvelope, ActionId};
use fission_ir::{ActionEntry, ActionTrigger, KeyAction, KeyCode, Op, Role, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

use crate::{Game, GameKey, HostInputEvent, InputMap, InputTrigger};

/// A device-independent host input delivered through Fission's reducer path.
///
/// Keyboard bindings use a pulse because Fission semantic key actions represent
/// one press. The reducer feeds the press and matching release to
/// [`crate::GameRuntime`], preserving the game's declared message mapping
/// without leaving a key stuck in the runtime input state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GameHostInput {
    KeyPulse(GameKey),
    FocusLost,
}

impl GameHostInput {
    /// Applies this semantic host input to a game runtime.
    pub fn apply<G: Game>(&self, runtime: &mut crate::GameRuntime<G>) {
        match self {
            Self::KeyPulse(key) => {
                runtime.handle_input(HostInputEvent::Key {
                    key: key.clone(),
                    pressed: true,
                });
                runtime.handle_input(HostInputEvent::Key {
                    key: key.clone(),
                    pressed: false,
                });
            }
            Self::FocusLost => runtime.handle_input(HostInputEvent::FocusLost),
        }
    }
}

impl Action for GameHostInput {
    fn static_id() -> ActionId {
        ActionId::from_name("fission_game::GameHostInput")
    }
}

/// Wraps a game subtree with the physical-key bindings declared by `G::input`.
///
/// Bind [`GameHostInput`] to a reducer once, pass that envelope here, and call
/// [`GameHostInput::apply`] in the reducer. Key handling remains focus-scoped:
/// it is active only while this region or one of its descendants owns focus.
#[derive(Clone, Debug)]
pub struct GameInputRegion {
    child: Widget,
    action: ActionEnvelope,
    keys: Vec<GameKey>,
    identifier: Option<String>,
}

impl GameInputRegion {
    /// Creates an input region from the key-press triggers declared by a game.
    pub fn for_game<G: Game>(child: impl Into<Widget>, action: ActionEnvelope) -> Self {
        let mut map = InputMap::<G::Message>::new();
        G::input(&mut map);
        let keys = map
            .triggers()
            .filter_map(|trigger| match trigger {
                InputTrigger::KeyPressed { key } => Some(key.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            child: child.into(),
            action,
            keys,
            identifier: None,
        }
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = Some(identifier.into());
        self
    }
}

impl From<GameInputRegion> for Widget {
    fn from(region: GameInputRegion) -> Self {
        custom_widget(
            "fission_game::GameInputRegion",
            GameInputRegionLowerer(region),
        )
    }
}

#[derive(Debug)]
struct GameInputRegionLowerer(GameInputRegion);

impl LowerWidget for GameInputRegionLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let id = self
            .0
            .identifier
            .as_deref()
            .map(WidgetId::explicit)
            .unwrap_or_else(|| cx.next_node_id());
        let child = cx.with_scope(id, |cx| self.0.child.lower(cx));
        let key_actions = self
            .0
            .keys
            .iter()
            .filter_map(|key| {
                let key_code = fission_key(key)?;
                let envelope = self
                    .0
                    .action
                    .with_action(&GameHostInput::KeyPulse(key.clone()));
                Some(KeyAction::new(key_code, envelope.id.as_u128()).payload(envelope.payload))
            })
            .collect();
        let blur = self.0.action.with_action(&GameHostInput::FocusLost);
        let semantics = Semantics {
            role: Role::Group,
            identifier: self.0.identifier.clone(),
            actions: fission_ir::ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Blur,
                    action_id: blur.id.as_u128(),
                    payload_data: Some(blur.payload),
                }],
            },
            key_actions,
            ..Default::default()
        };
        let mut root = IrBuilder::new(id, Op::Semantics(semantics));
        root.add_child(child);
        root.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        self.0.identifier.as_deref().map(WidgetId::explicit)
    }
}

fn fission_key(key: &GameKey) -> Option<KeyCode> {
    Some(match key {
        GameKey::ArrowUp => KeyCode::Up,
        GameKey::ArrowDown => KeyCode::Down,
        GameKey::ArrowLeft => KeyCode::Left,
        GameKey::ArrowRight => KeyCode::Right,
        GameKey::Confirm => KeyCode::Enter,
        GameKey::Cancel => KeyCode::Escape,
        GameKey::Space => KeyCode::Space,
        GameKey::Character(character) => KeyCode::Char(*character),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use fission_scene::SceneId;

    use super::*;
    use crate::{GameCtx, GameState, GameTime, StepCtx};

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    struct TestGame {
        presses: u32,
    }

    impl GameState for TestGame {}

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Message {
        Move,
    }

    impl Game for TestGame {
        type Message = Message;
        type Presentation = SceneId;

        fn input(map: &mut InputMap<Self::Message>) {
            map.on(InputTrigger::KeyPressed {
                key: GameKey::ArrowLeft,
            })
            .send(Message::Move);
        }

        fn react(&mut self, message: Self::Message, _ctx: &mut GameCtx<'_, Self>) {
            if matches!(message, Message::Move) {
                self.presses += 1;
            }
        }

        fn step(&mut self, _ctx: &mut StepCtx<'_, Self>) {}

        fn present(&self, _time: GameTime) -> Self::Presentation {
            SceneId::new(1)
        }
    }

    #[test]
    fn key_pulse_uses_the_declared_game_mapping_without_sticking() {
        let mut runtime = crate::GameRuntime::new(TestGame::default());
        GameHostInput::KeyPulse(GameKey::ArrowLeft).apply(&mut runtime);
        runtime.advance(Duration::from_millis(20));

        assert_eq!(runtime.state().presses, 1);
        assert!(!runtime.input_state().key_pressed(&GameKey::ArrowLeft));
    }
}
