//! Enter and Space in a focused text field do not activate a control around it.

use fission_core::event::{InputEvent, KeyCode, KeyEvent};
use fission_core::ui::widgets::SemanticsRegion;
use fission_core::ui::{TextInput, Widget};
use fission_core::{GlobalState, ReducerContext, WidgetId};
use fission_ir::semantics::ActionTrigger;
use fission_ir::{ActionEntry, ActionSet, Role};
use fission_test::{TestDriver, TestHarness};

#[derive(Debug, Default, Clone)]
struct State {
    activations: u32,
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct Activate;

fn activate(state: &mut State, _action: Activate, _ctx: &mut ReducerContext<State>) {
    state.activations += 1;
}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        let action = ctx.bind(
            Activate,
            activate as fn(&mut State, Activate, &mut ReducerContext<State>),
        );
        let field: Widget = TextInput {
            id: Some(WidgetId::explicit("keys.field").into()),
            width: Some(240.0),
            ..Default::default()
        }
        .into();
        SemanticsRegion {
            role: Role::Group,
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: action.id.as_u128(),
                    payload_data: Some(action.payload),
                }],
            },
            child: Some(field),
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn enter_and_space_stay_in_a_focused_text_field() {
    let mut driver = TestDriver::new(TestHarness::new(State::default()).with_root_widget(Page));
    driver.pump().expect("first frame");
    let field = WidgetId::explicit("keys.field");
    driver
        .harness
        .runtime
        .set_focused_widget(
            driver.harness.last_ir.as_ref().expect("ir"),
            Some(field),
            fission_core::TextEditSource::Programmatic,
        )
        .expect("focus the field");

    for key_code in [KeyCode::Enter, KeyCode::Space] {
        driver
            .harness
            .send_event(InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers: 0,
            }))
            .expect("key");
        driver.pump().expect("pump");
    }

    let state = driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("state");
    assert_eq!(
        state.activations, 0,
        "the surrounding control is not activated"
    );
    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused,
        Some(field),
        "focus stays in the field"
    );
}
