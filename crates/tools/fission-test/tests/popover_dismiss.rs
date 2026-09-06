use anyhow::Result;
use fission_core::ui::{Button, Positioned, Text, Widget, ZStack};
use fission_core::{with_reducer, GlobalState, WidgetId};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{Popover, PopoverMotion};

#[derive(Clone, Debug, Default)]
struct State {
    first_open: bool,
    second_open: bool,
    background_presses: u32,
    first_openings: u32,
    first_dismissals: u32,
    second_openings: u32,
    second_dismissals: u32,
}

impl GlobalState for State {}

#[fission_macros::fission_reducer(PressBackground)]
fn press_background(state: &mut State) {
    state.background_presses += 1;
}

#[fission_macros::fission_reducer(ToggleFirst)]
fn toggle_first(state: &mut State) {
    if state.first_open {
        state.first_dismissals += 1;
    } else {
        state.first_openings += 1;
    }
    state.first_open = !state.first_open;
}

#[fission_macros::fission_reducer(ToggleSecond)]
fn toggle_second(state: &mut State) {
    if state.second_open {
        state.second_dismissals += 1;
    } else {
        state.second_openings += 1;
    }
    state.second_open = !state.second_open;
}

#[derive(Clone)]
struct Root {
    include_second: bool,
}

impl From<Root> for Widget {
    fn from(root: Root) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        let background = Button {
            id: Some(WidgetId::explicit("popover-test.background")),
            child: Some(Text::new("Background").into()),
            on_press: Some(with_reducer!(ctx, PressBackground, press_background)),
            width: Some(800.0),
            height: Some(600.0),
            ..Default::default()
        };
        let first = Popover {
            id: WidgetId::explicit("popover-test.first"),
            is_open: view.state().first_open,
            on_close: Some(with_reducer!(ctx, ToggleFirst, toggle_first)),
            trigger: Button {
                id: Some(WidgetId::explicit("popover-test.open-first")),
                child: Some(Text::new("Open first").into()),
                on_press: Some(with_reducer!(ctx, ToggleFirst, toggle_first)),
                width: Some(120.0),
                height: Some(44.0),
                ..Default::default()
            }
            .into(),
            content: Text::new("First flyout").into(),
            motion: Some(PopoverMotion::Default),
        };
        let second = Popover {
            id: WidgetId::explicit("popover-test.second"),
            is_open: view.state().second_open,
            on_close: Some(with_reducer!(ctx, ToggleSecond, toggle_second)),
            trigger: Button {
                id: Some(WidgetId::explicit("popover-test.second-trigger")),
                child: Some(Text::new("Second closed").into()),
                on_press: Some(with_reducer!(ctx, ToggleSecond, toggle_second)),
                width: Some(120.0),
                height: Some(44.0),
                ..Default::default()
            }
            .into(),
            content: Text::new("Second flyout").into(),
            motion: Some(PopoverMotion::Default),
        };

        let mut children = vec![
            background.into(),
            Positioned {
                left: Some(20.0),
                top: Some(20.0),
                child: Some(first.into()),
                ..Default::default()
            }
            .into(),
        ];
        if root.include_second {
            children.push(
                Positioned {
                    left: Some(180.0),
                    top: Some(20.0),
                    child: Some(second.into()),
                    ..Default::default()
                }
                .into(),
            );
        }

        ZStack {
            children,
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn closed_and_exiting_animated_popovers_do_not_intercept_pointer_input() -> Result<()> {
    let harness = TestHarness::new_with_mock_measurer(State::default()).with_root_widget(Root {
        include_second: false,
    });
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(800.0, 600.0);
    driver.pump()?;

    // The animated popover is closed. Its retained Presence surface may exist,
    // but it is not allowed to install a full-window input backdrop.
    driver.tap_point(700.0, 550.0)?;
    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.background_presses, 1);
    assert_eq!(state.first_openings, 0);
    assert_eq!(state.first_dismissals, 0);
    assert!(!state.first_open);

    driver.tap_text("Open first")?;
    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert!(state.first_open);
    assert_eq!(state.first_openings, 1);
    driver.tick(150)?;
    driver.assert_text_visible("First flyout");

    // While open, the same coordinate belongs to the dismissal backdrop.
    driver.tap_point(700.0, 550.0)?;
    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.background_presses, 1);
    assert_eq!(state.first_openings, 1);
    assert_eq!(state.first_dismissals, 1);
    assert!(!state.first_open);

    // Presence keeps the surface alive during its exit motion, but input must
    // already have returned to the underlying application.
    driver.assert_text_visible("First flyout");
    driver.tap_point(700.0, 550.0)?;
    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.background_presses, 2);
    assert_eq!(state.first_openings, 1);
    assert_eq!(state.first_dismissals, 1);
    assert!(!state.first_open);

    driver.tick(200)?;
    driver.assert_text_not_visible("First flyout");
    driver.tap_point(700.0, 550.0)?;
    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.background_presses, 3);
    assert_eq!(state.first_openings, 1);
    assert_eq!(state.first_dismissals, 1);
    assert!(!state.first_open);

    Ok(())
}

#[test]
fn two_closed_animated_popovers_leave_the_background_interactive() -> Result<()> {
    let harness = TestHarness::new_with_mock_measurer(State::default()).with_root_widget(Root {
        include_second: true,
    });
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(800.0, 600.0);
    driver.pump()?;

    // The second popover is registered last. A hidden backdrop on either
    // popover would intercept this real coordinate before the background.
    driver.tap_point(700.0, 550.0)?;
    let state = driver.harness.runtime.get_app_state::<State>().unwrap();
    assert_eq!(state.background_presses, 1);
    assert_eq!(state.first_openings, 0);
    assert_eq!(state.first_dismissals, 0);
    assert_eq!(state.second_openings, 0);
    assert_eq!(state.second_dismissals, 0);
    assert!(!state.first_open);
    assert!(!state.second_open);

    Ok(())
}
