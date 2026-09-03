use anyhow::Result;
use fission_core::motion::MotionPropertyId;
use fission_core::ui::{Text, Widget};
use fission_core::{Action, ActionEnvelope, GlobalState, ReducerContext, WidgetId};
use fission_ir::semantics::{Role, TextInputType};
use fission_render::DisplayOp;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{
    CircularProgress, CircularProgressMotion, DatePicker, Drawer, DrawerSide, NumberInput,
};
use std::f32::consts::PI;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct State {
    number: f32,
    date_year: i32,
    date_month: u32,
    selected_date: Option<String>,
    drawer_open: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            number: 0.0,
            date_year: 2026,
            date_month: 5,
            selected_date: None,
            drawer_open: true,
        }
    }
}

impl GlobalState for State {}

#[fission_macros::fission_action(no_eq)]
struct NumberChanged(f32);

#[fission_macros::fission_action]
struct DateNavigated(i32, u32);

#[fission_macros::fission_action]
struct DateSelected(String);

#[fission_macros::fission_action]
struct DrawerDismissed;

fn number_changed(state: &mut State, _action: NumberChanged, ctx: &mut ReducerContext<State>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    if let Ok(number) = change.new_text.parse::<f32>() {
        state.number = number;
    }
}

fn date_navigated(state: &mut State, action: DateNavigated) {
    state.date_year = action.0;
    state.date_month = action.1;
}

fn date_selected(state: &mut State, action: DateSelected) {
    state.selected_date = Some(action.0);
}

fn dismiss_drawer(state: &mut State, _action: DrawerDismissed) {
    state.drawer_open = false;
}

fn navigate_action(year: i32, month: u32) -> ActionEnvelope {
    ActionEnvelope {
        id: DateNavigated::static_id(),
        payload: serde_json::to_vec(&DateNavigated(year, month)).unwrap(),
    }
}

#[test]
fn number_input_text_entry_dispatches_parsed_float() -> Result<()> {
    #[derive(Clone)]
    struct Root;

    impl From<Root> for Widget {
        fn from(_component: Root) -> Self {
            let (ctx, view) = fission_core::build::current::<State>();
            NumberInput {
                id: Some(WidgetId::explicit("quantity")),
                value: view.state().number,
                display_text: Some(String::new()),
                on_input: Some(ctx.bind(
                    NumberChanged(0.0),
                    number_changed as fn(&mut State, NumberChanged, &mut ReducerContext<State>),
                )),
                ..Default::default()
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    let inputs = driver.find_role(Role::TextInput);
    assert_eq!(inputs.len(), 1, "NumberInput should expose one text field");
    let input_node = inputs[0].node_id;
    let input_semantics = driver
        .harness
        .last_ir
        .as_ref()
        .and_then(|ir| ir.nodes.get(&input_node))
        .and_then(|node| match &node.op {
            fission_ir::Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
        .expect("NumberInput semantics");
    assert_eq!(input_semantics.text_input_type, TextInputType::Number);

    let bounds = inputs[0].bounds;
    driver.tap_point(
        bounds.x() + bounds.width() / 2.0,
        bounds.y() + bounds.height() / 2.0,
    )?;
    driver.type_text("12.5")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.number, 12.5);

    Ok(())
}

#[test]
fn number_input_ignores_invalid_intermediate_float() -> Result<()> {
    #[derive(Clone)]
    struct Root;

    impl From<Root> for Widget {
        fn from(_component: Root) -> Self {
            let (ctx, view) = fission_core::build::current::<State>();
            NumberInput {
                id: Some(WidgetId::explicit("quantity")),
                value: view.state().number,
                display_text: Some(String::new()),
                on_input: Some(ctx.bind(
                    NumberChanged(0.0),
                    number_changed as fn(&mut State, NumberChanged, &mut ReducerContext<State>),
                )),
                ..Default::default()
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    let input = driver
        .find_role(Role::TextInput)
        .into_iter()
        .next()
        .expect("NumberInput text field");
    driver.tap_point(
        input.bounds.x() + input.bounds.width() / 2.0,
        input.bounds.y() + input.bounds.height() / 2.0,
    )?;
    driver.type_text("-")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(
        state.number, 0.0,
        "invalid intermediate numeric text must not dispatch a parsed value"
    );

    Ok(())
}

#[test]
fn date_picker_navigation_is_controlled_by_parent_state() -> Result<()> {
    #[derive(Clone)]
    struct Root;

    impl From<Root> for Widget {
        fn from(_component: Root) -> Self {
            let (ctx, view) = fission_core::build::current::<State>();
            ctx.register(date_navigated as fn(&mut State, DateNavigated));

            DatePicker {
                id: WidgetId::explicit("due_date"),
                value: None,
                is_open: true,
                width: Some(180.0),
                view_year: Some(view.state().date_year),
                view_month: Some(view.state().date_month),
                on_navigate: Some(Arc::new(navigate_action)),
                on_change: None,
                on_toggle: None,
                on_close: None,
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;
    driver.assert_text_visible("May 2026");

    driver.tap_text(">")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!((state.date_year, state.date_month), (2026, 6));
    driver.assert_text_visible("June 2026");

    driver.tap_text("<")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!((state.date_year, state.date_month), (2026, 5));
    driver.assert_text_visible("May 2026");

    Ok(())
}

#[test]
fn date_picker_navigation_wraps_year_and_selection_dispatches_date() -> Result<()> {
    #[derive(Clone)]
    struct Root;

    impl From<Root> for Widget {
        fn from(_component: Root) -> Self {
            let (ctx, view) = fission_core::build::current::<State>();
            ctx.register(date_navigated as fn(&mut State, DateNavigated));
            ctx.register(date_selected as fn(&mut State, DateSelected));

            DatePicker {
                id: WidgetId::explicit("due_date"),
                value: None,
                is_open: true,
                width: Some(180.0),
                view_year: Some(view.state().date_year),
                view_month: Some(view.state().date_month),
                on_navigate: Some(Arc::new(navigate_action)),
                on_change: Some(Arc::new(|date| ActionEnvelope {
                    id: DateSelected::static_id(),
                    payload: serde_json::to_vec(&DateSelected(date.to_string())).unwrap(),
                })),
                on_toggle: None,
                on_close: None,
            }
            .into()
        }
    }

    let harness = TestHarness::new(State {
        date_year: 2026,
        date_month: 12,
        ..State::default()
    })
    .with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;
    driver.assert_text_visible("December 2026");

    driver.tap_text(">")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!((state.date_year, state.date_month), (2027, 1));
    driver.assert_text_visible("January 2027");

    driver.tap_text("15")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.selected_date.as_deref(), Some("2027-01-15"));

    Ok(())
}

#[test]
fn drawer_backdrop_dismisses_and_registers_focus_barrier() -> Result<()> {
    let drawer_id = WidgetId::explicit("settings_drawer");

    #[derive(Clone)]
    struct Root {
        drawer_id: WidgetId,
    }

    impl From<Root> for Widget {
        fn from(component: Root) -> Self {
            let (ctx, view) = fission_core::build::current::<State>();
            Drawer {
                id: component.drawer_id,
                side: DrawerSide::Left,
                is_open: view.state().drawer_open,
                on_dismiss: Some(ctx.bind(
                    DrawerDismissed,
                    dismiss_drawer as fn(&mut State, DrawerDismissed),
                )),
                dismiss_semantics_identifier: None,
                content: Text::new("Drawer content").into(),
                width: Some(300.0),
                motion: None,
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { drawer_id });
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(800.0, 600.0);
    driver.pump()?;
    driver.assert_text_visible("Drawer content");

    driver.tap_text("Drawer content")?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert!(
        state.drawer_open,
        "tapping inside the drawer panel must not trigger backdrop dismissal"
    );

    let has_focus_barrier = driver
        .harness
        .last_ir
        .as_ref()
        .unwrap()
        .nodes
        .values()
        .any(|node| {
            matches!(
                &node.op,
                fission_ir::Op::Semantics(semantics) if semantics.is_focus_barrier
            )
        });
    assert!(has_focus_barrier, "drawer overlay should trap focus");

    driver.tap_point(790.0, 10.0)?;

    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert!(!state.drawer_open, "backdrop tap should close the drawer");
    driver.assert_text_not_visible("Drawer content");

    Ok(())
}

#[test]
fn right_drawer_clamps_to_viewport_width() -> Result<()> {
    let drawer_id = WidgetId::explicit("right_drawer");

    #[derive(Clone)]
    struct Root {
        drawer_id: WidgetId,
    }

    impl From<Root> for Widget {
        fn from(component: Root) -> Self {
            let (_ctx, view) = fission_core::build::current::<State>();
            Drawer {
                id: component.drawer_id,
                side: DrawerSide::Right,
                is_open: view.state().drawer_open,
                on_dismiss: None,
                dismiss_semantics_identifier: None,
                content: Text::new("Right drawer content").into(),
                width: Some(500.0),
                motion: None,
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { drawer_id });
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(360.0, 640.0);
    driver.pump()?;
    driver.assert_text_visible("Right drawer content");

    let content = driver
        .find_text("Right drawer content")
        .expect("drawer text");
    assert!(content.bounds.x() >= 0.0);
    assert!(content.bounds.x() + content.bounds.width() <= 360.0);

    Ok(())
}

#[test]
fn circular_progress_indeterminate_registers_repeating_rotation() -> Result<()> {
    let progress_id = WidgetId::explicit("loading_spinner");

    #[derive(Clone)]
    struct Root {
        progress_id: WidgetId,
    }

    impl From<Root> for Widget {
        fn from(component: Root) -> Self {
            let (_ctx, _view) = fission_core::build::current::<State>();
            CircularProgress {
                id: component.progress_id,
                value: None,
                motion: Some(CircularProgressMotion::Spin),
                ..Default::default()
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { progress_id });
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    let motion_id = WidgetId::derived(progress_id.as_u128(), &[0x1D1_CA70]);
    let key = (motion_id, MotionPropertyId::Rotation);
    let active = driver
        .harness
        .runtime
        .runtime_state
        .motion
        .active
        .get(&key)
        .expect("indeterminate progress rotation motion");
    assert_eq!(active.start_value.as_scalar_like(), Some(0.0));
    assert!((active.end_value.as_scalar_like().unwrap() - 2.0 * PI).abs() < 0.001);
    assert!(active.repeat);

    driver.tick(250)?;

    let current = driver
        .harness
        .runtime
        .runtime_state
        .motion
        .values
        .get(&key)
        .and_then(|value| value.as_scalar_like())
        .expect("animated rotation value");
    assert!(
        current > 0.0 && current < 2.0 * PI,
        "rotation should advance after ticking, got {current}"
    );
    let has_transform = driver
        .harness
        .get_last_display_list()
        .map(|display_list| {
            display_list
                .ops
                .iter()
                .any(|op| matches!(op, DisplayOp::Transform(_)))
        })
        .unwrap_or(false);
    assert!(
        has_transform,
        "animated circular progress should render through a composite transform"
    );

    Ok(())
}

#[test]
fn circular_progress_indeterminate_without_motion_renders_static_indicator() -> Result<()> {
    #[derive(Clone)]
    struct Root;

    impl From<Root> for Widget {
        fn from(_component: Root) -> Self {
            let (_ctx, _view) = fission_core::build::current::<State>();
            CircularProgress {
                id: WidgetId::explicit("static_spinner"),
                value: None,
                motion: None,
                ..Default::default()
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .motion
            .active
            .is_empty(),
        "indeterminate progress without explicit motion should not register motion"
    );

    Ok(())
}

#[test]
fn circular_progress_determinate_does_not_register_rotation() -> Result<()> {
    let progress_id = WidgetId::explicit("static_progress");

    #[derive(Clone)]
    struct Root {
        progress_id: WidgetId,
    }

    impl From<Root> for Widget {
        fn from(component: Root) -> Self {
            let (_ctx, _view) = fission_core::build::current::<State>();
            CircularProgress {
                id: component.progress_id,
                value: Some(0.5),
                motion: Some(CircularProgressMotion::Spin),
                ..Default::default()
            }
            .into()
        }
    }

    let harness = TestHarness::new(State::default()).with_root_widget(Root { progress_id });
    let mut driver = TestDriver::new(harness);
    driver.pump()?;

    let motion_id = WidgetId::derived(progress_id.as_u128(), &[0x1D1_CA70]);
    assert!(
        !driver
            .harness
            .runtime
            .runtime_state
            .motion
            .active
            .contains_key(&(motion_id, MotionPropertyId::Rotation)),
        "determinate progress should not spin"
    );

    Ok(())
}
