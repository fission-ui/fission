//! A range slider draws a thin full-width track, a thin selected segment and two visible thumbs,
//! and dragging a thumb proposes a new range.

use fission_core::event::{InputEvent, PointerButton, PointerEvent};
use fission_core::ui::{Container, Widget};
use fission_core::{GlobalState, ReducerContext, WidgetId};
use fission_ir::op::{Op, PaintOp};
use fission_layout::{LayoutPoint, LayoutRect};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::RangeSlider;

#[derive(Debug, Clone)]
struct State {
    start: f32,
    end: f32,
    changes: u32,
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct ChangeRange;

fn change_range(state: &mut State, _action: ChangeRange, ctx: &mut ReducerContext<State>) {
    if let Some(change) = ctx.input.range_slider_change() {
        state.start = change.start;
        state.end = change.end;
        state.changes += 1;
    }
}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        let on_change = ctx.bind(
            ChangeRange,
            change_range as fn(&mut State, ChangeRange, &mut ReducerContext<State>),
        );
        Container::new(RangeSlider {
            id: Some(WidgetId::explicit("range")),
            semantics_identifier: Some("range".into()),
            start: view.state().start,
            end: view.state().end,
            min: 0.0,
            max: 100.0,
            step: None,
            on_change: Some(on_change),
        })
        .width(300.0)
        .into()
    }
}

fn driver() -> TestDriver<State> {
    let mut driver = TestDriver::new(
        TestHarness::new(State {
            start: 20.0,
            end: 60.0,
            changes: 0,
        })
        .with_root_widget(Page),
    );
    driver.pump().expect("first frame");
    driver
}

fn filled_rects(driver: &TestDriver<State>) -> Vec<LayoutRect> {
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let snapshot = driver.harness.last_snapshot.as_ref().expect("snapshot");
    ir.nodes
        .iter()
        .filter(|(_, node)| {
            matches!(
                &node.op,
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(_),
                    stroke: None,
                    ..
                })
            )
        })
        .filter_map(|(id, _)| snapshot.get_node_rect(*id))
        .collect()
}

#[test]
fn the_tracks_are_thin_and_the_thumbs_are_visible_at_their_values() {
    let driver = driver();
    let rects = filled_rects(&driver);

    let track = rects
        .iter()
        .filter(|rect| rect.width() >= 250.0)
        .min_by(|a, b| a.height().total_cmp(&b.height()))
        .unwrap_or_else(|| panic!("a full-width track is drawn, got {rects:?}"));
    assert!(
        track.height() <= 6.0,
        "the track is a thin line, got {track:?}"
    );

    let selected = rects
        .iter()
        .filter(|rect| rect.width() > 60.0 && rect.width() < 200.0)
        .min_by(|a, b| a.height().total_cmp(&b.height()))
        .unwrap_or_else(|| panic!("a selected segment is drawn, got {rects:?}"));
    assert!(
        selected.height() <= 6.0,
        "the selected segment is as thin as the track, got {selected:?}"
    );

    for (identifier, pct) in [("range.start", 0.2), ("range.end", 0.6)] {
        let thumb = driver
            .find_semantics_identifier(identifier)
            .unwrap_or_else(|| panic!("{identifier} thumb"))
            .bounds;
        assert!(
            (thumb.width() - 16.0).abs() < 0.5 && (thumb.height() - 16.0).abs() < 0.5,
            "{identifier} is a 16x16 thumb, got {thumb:?}"
        );
        // The thumb's layout box starts at its value and its paint is shifted back by half its
        // width, so the drawn thumb is centred on the box's leading edge.
        let centre = thumb.x();
        let expected = track.x() + track.width() * pct;
        assert!(
            (centre - expected).abs() < 1.0,
            "{identifier} is centred on its value along the track: centre {centre}, expected {expected}"
        );
        let paints_at_thumb = rects
            .iter()
            .filter(|rect| {
                (rect.width() - 16.0).abs() < 0.5
                    && (rect.x() - thumb.x()).abs() < 0.5
                    && (rect.y() - thumb.y()).abs() < 0.5
            })
            .count();
        assert!(paints_at_thumb >= 1, "{identifier} thumb is painted");
    }
}

#[test]
fn dragging_the_end_thumb_proposes_a_larger_end() {
    let mut driver = driver();
    let thumb = driver
        .find_semantics_identifier("range.end")
        .expect("end thumb")
        .bounds;
    let start = LayoutPoint::new(
        thumb.x() + thumb.width() / 2.0,
        thumb.y() + thumb.height() / 2.0,
    );
    let events = [
        PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: start,
            button: PointerButton::Primary,
            modifiers: 0,
        },
        PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(start.x + 30.0, start.y),
            modifiers: 0,
        },
        PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(start.x + 60.0, start.y),
            modifiers: 0,
        },
        PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: LayoutPoint::new(start.x + 60.0, start.y),
            button: PointerButton::Primary,
            modifiers: 0,
        },
    ];
    for event in events {
        driver
            .harness
            .send_event(InputEvent::Pointer(event))
            .expect("pointer event");
        driver.pump().expect("pump");
    }
    let state = driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("state");
    assert!(state.changes >= 1, "dragging proposed a change");
    assert!(
        state.end > 60.0,
        "the end moved to a larger value, got {}",
        state.end
    );
}
