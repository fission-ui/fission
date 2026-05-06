use anyhow::Result;
use fission_core::action::{Action, ActionEnvelope, ActionId};
use fission_core::effect::SystemEffect;
use fission_core::registry::{ActionRegistry, Handler};
use fission_core::{
    AppState, BuildCtx, InputEvent, PointerButton, PointerEvent, ReducerContext, View, Widget,
};
use fission_widgets::{Button, ButtonVariant, Container, Node, Text};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct TestState;
impl AppState for TestState {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OpenLink(pub String);

impl Action for OpenLink {
    fn static_id() -> ActionId {
        lazy_static! {
            static ref ID: ActionId = ActionId::from_name("fission_test::OpenLink");
        }
        *ID
    }
}

fn on_open_link(_state: &mut TestState, action: OpenLink, ctx: &mut ReducerContext<TestState>) {
    ctx.effects.add(SystemEffect::OpenUrl {
        url: action.0,
        in_app: false,
    });
}

struct Root;

impl Widget<TestState> for Root {
    fn build(&self, _ctx: &mut BuildCtx<TestState>, _view: &View<TestState>) -> Node {
        Container::new(
            Button {
                child: Some(Box::new(Text::new("Open").into_node())),
                on_press: Some(ActionEnvelope::from(OpenLink("https://example.com".into()))),
                variant: ButtonVariant::Filled,
                width: Some(200.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into_node(),
        )
        .width(300.0)
        .height(100.0)
        .into_node()
    }
}

#[test]
fn persistent_reducers_survive_clear_reducers_frames() -> Result<()> {
    let mut h = fission_test::TestHarness::new(TestState::default()).with_root_widget(Root);

    let mut registry = ActionRegistry::new();
    registry.register(on_open_link as Handler<TestState, OpenLink>);
    h.runtime.absorb_persistent_registry(registry);

    // Frame 1: build
    h.pump()?;

    // Click button -> 1 effect
    let point = fission_core::LayoutPoint { x: 10.0, y: 10.0 };
    h.send_event(InputEvent::Pointer(PointerEvent::Down {
        point,
        button: PointerButton::Primary,
    }))?;
    h.send_event(InputEvent::Pointer(PointerEvent::Up {
        point,
        button: PointerButton::Primary,
    }))?;
    assert_eq!(h.runtime.pending_effects.len(), 1);

    // Frame 2: pump triggers `runtime.clear_reducers()` in the harness.
    h.pump()?;

    // Click again; persistent reducer should still be present -> 2 effects
    h.send_event(InputEvent::Pointer(PointerEvent::Down {
        point,
        button: PointerButton::Primary,
    }))?;
    h.send_event(InputEvent::Pointer(PointerEvent::Up {
        point,
        button: PointerButton::Primary,
    }))?;
    assert_eq!(h.runtime.pending_effects.len(), 2);

    Ok(())
}
