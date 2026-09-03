use anyhow::Result;
use fission_core::ui::Widget;
use fission_core::{GlobalState, KeyCode, ReducerContext, WidgetId};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::RangeSlider;

#[derive(Clone, Debug)]
struct State {
    start: f32,
    end: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            start: 20.0,
            end: 80.0,
        }
    }
}

impl GlobalState for State {}

#[fission_macros::fission_action(no_eq)]
struct ChangeRange;

fn change_range(state: &mut State, _action: ChangeRange, ctx: &mut ReducerContext<State>) {
    let change = ctx
        .input
        .range_slider_change()
        .expect("range slider supplies typed input");
    state.start = change.start;
    state.end = change.end;
}

#[derive(Clone)]
struct Root;

impl From<Root> for Widget {
    fn from(_: Root) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        RangeSlider {
            id: Some(WidgetId::explicit("price-range")),
            semantics_identifier: Some("filters.price".into()),
            start: view.state().start,
            end: view.state().end,
            min: 0.0,
            max: 100.0,
            step: Some(5.0),
            on_change: Some(ctx.bind(
                ChangeRange,
                change_range as fn(&mut State, ChangeRange, &mut ReducerContext<State>),
            )),
        }
        .into()
    }
}

#[test]
fn public_test_driver_operates_each_stable_range_thumb() -> Result<()> {
    let harness = TestHarness::new_with_mock_measurer(State::default()).with_root_widget(Root);
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(400.0, 80.0);
    driver.pump()?;

    let start_id = driver
        .find_semantics_identifier("filters.price.start")
        .expect("stable lower-thumb semantics")
        .node_id;
    let end_id = driver
        .find_semantics_identifier("filters.price.end")
        .expect("stable upper-thumb semantics")
        .node_id;
    assert_ne!(start_id, end_id);

    let root_rect = driver
        .harness
        .last_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.get_node_rect(WidgetId::explicit("price-range")))
        .expect("range bounds");
    // Select the lower thumb, move it, then use its focused keyboard path.
    driver.tap_point(
        root_rect.x() + root_rect.width() * 0.3,
        root_rect.y() + root_rect.height() / 2.0,
    )?;
    driver.press_key(KeyCode::Right, 0)?;
    let state = driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("application state");
    assert_eq!((state.start, state.end), (35.0, 80.0));

    // The controlled rebuild retains that lower value while the independently
    // identified upper thumb is selected and adjusted.
    driver.tap_point(
        root_rect.x() + root_rect.width() * 0.8,
        root_rect.y() + root_rect.height() / 2.0,
    )?;
    driver.press_key(KeyCode::Right, 0)?;

    let state = driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("application state");
    assert_eq!((state.start, state.end), (35.0, 85.0));
    assert_eq!(
        driver
            .find_semantics_identifier("filters.price.start")
            .expect("lower thumb remains addressable")
            .node_id,
        start_id
    );
    assert_eq!(
        driver
            .find_semantics_identifier("filters.price.end")
            .expect("upper thumb remains addressable")
            .node_id,
        end_id
    );
    Ok(())
}
