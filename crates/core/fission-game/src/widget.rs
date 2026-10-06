//! Fission widget integration for the deterministic game input map.

use std::collections::BTreeSet;
use std::sync::Arc;

use fission_core::authoring::{lower_widget, IrBuilder, LowerWidget, LoweringContext};
use fission_core::internal::{CustomEventResult, CustomRender, CustomRenderObject};
use fission_core::ui::Widget;
use fission_core::{Action, ActionEnvelope, ActionId, InputEvent, KeyEvent, LayoutRect};
use fission_ir::{ActionEntry, ActionTrigger, KeyAction, KeyCode, Op, Role, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

use crate::{Game, GameKey, HostInputEvent, InputMap, InputTrigger};

/// A device-independent host input delivered through Fission's reducer path.
///
/// Key transitions preserve their pressed state across fixed simulation ticks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GameHostInput {
    Key { key: GameKey, pressed: bool },
    FocusLost,
}

impl GameHostInput {
    /// Applies this semantic host input to a game runtime.
    pub fn apply<G: Game>(&self, runtime: &mut crate::GameRuntime<G>) {
        match self {
            Self::Key { key, pressed } => {
                runtime.handle_input(HostInputEvent::Key {
                    key: key.clone(),
                    pressed: *pressed,
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
                InputTrigger::KeyPressed { key } | InputTrigger::KeyReleased { key } => {
                    Some(key.clone())
                }
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
        let input = Arc::new(GameInputRenderObject {
            action: region.action.clone(),
            keys: region.keys.clone(),
        });
        CustomRender::new(
            "fission_game::GameInputRegion",
            Arc::new(GameInputRegionLowerer(region)),
        )
        .with_render_object(input)
        .into()
    }
}

#[derive(Debug)]
struct GameInputRenderObject {
    action: ActionEnvelope,
    keys: Vec<GameKey>,
}

impl CustomRenderObject for GameInputRenderObject {
    fn handle_event(
        &self,
        node_id: WidgetId,
        event: &InputEvent,
        _node_rect: LayoutRect,
    ) -> CustomEventResult {
        let (key_code, pressed) = match event {
            InputEvent::Keyboard(KeyEvent::Down { key_code, .. })
            | InputEvent::Keyboard(KeyEvent::DownWithText { key_code, .. }) => (key_code, true),
            InputEvent::Keyboard(KeyEvent::Up { key_code, .. }) => (key_code, false),
            _ => return CustomEventResult::ignored(),
        };
        let Some(key) = self
            .keys
            .iter()
            .find(|key| fission_key(key).as_ref() == Some(key_code))
        else {
            return CustomEventResult::ignored();
        };
        let envelope = self.action.with_action(&GameHostInput::Key {
            key: key.clone(),
            pressed,
        });
        CustomEventResult::consumed_with(vec![(node_id, envelope)])
    }

    fn blur_actions(&self, node_id: WidgetId) -> Vec<(WidgetId, ActionEnvelope)> {
        vec![(node_id, self.action.with_action(&GameHostInput::FocusLost))]
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
        let child = cx.with_scope(id, |cx| lower_widget(&self.0.child, cx));
        let key_actions = self
            .0
            .keys
            .iter()
            .filter_map(|key| {
                let key_code = fission_key(key)?;
                let envelope = self.0.action.with_action(&GameHostInput::Key {
                    key: key.clone(),
                    pressed: true,
                });
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
        self.0.identifier.as_deref().map(|identifier| {
            WidgetId::derived(WidgetId::explicit(identifier).as_u128(), &[0x47_414D45])
        })
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
    fn key_transitions_use_the_declared_mapping_and_preserve_held_state() {
        let mut runtime = crate::GameRuntime::new(TestGame::default());
        GameHostInput::Key {
            key: GameKey::ArrowLeft,
            pressed: true,
        }
        .apply(&mut runtime);
        runtime.advance(Duration::from_millis(20));

        assert_eq!(runtime.state().presses, 1);
        assert!(runtime.input_state().key_pressed(&GameKey::ArrowLeft));
        GameHostInput::Key {
            key: GameKey::ArrowLeft,
            pressed: false,
        }
        .apply(&mut runtime);
        assert!(!runtime.input_state().key_pressed(&GameKey::ArrowLeft));
    }

    #[test]
    fn explicit_region_identity_keeps_wrapper_and_semantics_distinct() {
        let widget: Widget = GameInputRegion::for_game::<TestGame>(
            fission_core::ui::Spacer::default(),
            ActionEnvelope {
                id: GameHostInput::static_id(),
                payload: Vec::new(),
            },
        )
        .semantics_identifier("test.game-input")
        .into();
        let ir = fission_core::internal::lower_widget_to_ir(&widget);

        assert!(ir.nodes.iter().all(|(id, node)| node.parent != Some(*id)));
        assert!(ir
            .root
            .is_some_and(|root| ir.custom_render_objects.contains_key(&root)));
        assert!(ir.nodes.values().any(|node| matches!(
            &node.op,
            Op::Semantics(semantics)
                if semantics.identifier.as_deref() == Some("test.game-input")
        )));
    }
}
