use anyhow::Result;
use fission_core::action::ActionEnvelope;
use fission_core::registry::ActionRegistry;
use fission_core::{
    reduce_with, GlobalState, InputEvent, OpenUrlRequest, PointerButton, PointerEvent,
    ReducerContext, OPEN_URL,
};
use fission_widgets::{Button, ButtonVariant, Container, Text, Widget};

#[derive(Debug, Default)]
struct TestState;
impl GlobalState for TestState {}

#[fission_macros::fission_action]
struct OpenLink(pub String);

fn on_open_link(_state: &mut TestState, action: OpenLink, ctx: &mut ReducerContext<TestState>) {
    ctx.effects.capability(
        OPEN_URL,
        OpenUrlRequest {
            url: action.0,
            in_app: false,
        },
    );
}

#[derive(Clone)]
struct Root;

impl From<Root> for Widget {
    fn from(_component: Root) -> Self {
        let (_ctx, _view) = fission_core::build::current::<TestState>();
        Container::new(Button {
            child: Some(Text::new("Open").into()),
            on_press: Some(ActionEnvelope::from(OpenLink("https://example.com".into()))),
            variant: ButtonVariant::Filled,
            width: Some(200.0),
            height: Some(40.0),
            ..Default::default()
        })
        .width(300.0)
        .height(100.0)
        .into()
    }
}
#[test]
fn persistent_reducers_survive_clear_reducers_frames() -> Result<()> {
    let mut h = fission_test::TestHarness::new(TestState::default()).with_root_widget(Root);

    let mut registry = ActionRegistry::new();
    registry.register(reduce_with!(on_open_link));
    h.runtime.absorb_persistent_registry(registry);

    // Frame 1: build
    h.pump()?;

    // Click button -> 1 effect
    let point = fission_core::LayoutPoint { x: 10.0, y: 10.0 };
    h.send_event(InputEvent::Pointer(PointerEvent::Down {
        point,
        button: PointerButton::Primary,
        modifiers: 0,
    }))?;
    h.send_event(InputEvent::Pointer(PointerEvent::Up {
        point,
        button: PointerButton::Primary,
        modifiers: 0,
    }))?;
    assert_eq!(h.runtime.pending_effects.len(), 1);

    // Frame 2: pump triggers `runtime.clear_reducers()` in the harness.
    h.pump()?;

    // Click again; persistent reducer should still be present -> 2 effects
    h.send_event(InputEvent::Pointer(PointerEvent::Down {
        point,
        button: PointerButton::Primary,
        modifiers: 0,
    }))?;
    h.send_event(InputEvent::Pointer(PointerEvent::Up {
        point,
        button: PointerButton::Primary,
        modifiers: 0,
    }))?;
    assert_eq!(h.runtime.pending_effects.len(), 2);

    Ok(())
}
