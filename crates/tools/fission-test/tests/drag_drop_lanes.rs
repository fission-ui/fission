//! Dragging a card onto another lane drops it there, with a preview under the pointer.

use anyhow::Result;
use fission_core::event::{InputEvent, PointerButton, PointerEvent};
use fission_core::ui::{Column, Container, Text, Widget};
use fission_core::{GlobalState, WidgetId};
use fission_layout::{LayoutPoint, LayoutSize};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{DragPreviewOptions, DragTarget, Draggable};

#[derive(Clone, Debug, Default)]
struct State {
    drops: u32,
}

impl GlobalState for State {}

#[fission_macros::fission_reducer(Dropped)]
fn dropped(state: &mut State) {
    state.drops += 1;
}

#[derive(Clone)]
struct Lanes;

impl From<Lanes> for Widget {
    fn from(_: Lanes) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        Column {
            children: vec![
                Draggable {
                    id: Some(WidgetId::explicit("lanes.card")),
                    semantics_identifier: Some("lanes.card".into()),
                    payload: b"card".to_vec(),
                    child: Container::new(Text::new("Card")).padding_all(8.0).into(),
                    preview: Some(Container::new(Text::new("Card preview")).padding_all(8.0).into()),
                    preview_options: DragPreviewOptions::default(),
                    on_drag_start: None,
                    on_drag_end: None,
                }
                .into(),
                DragTarget {
                    id: Some(WidgetId::explicit("lanes.done")),
                    semantics_identifier: Some("lanes.done".into()),
                    on_drop: Some(fission_core::with_reducer!(ctx, Dropped, dropped)),
                    child: Container::new(Text::new("Done lane")).width(300.0).height(200.0).into(),
                    hover_child: None,
                }
                .into(),
            ],
            gap: Some(40.0),
            ..Default::default()
        }
        .into()
    }
}

fn pointer(kind: &str, point: LayoutPoint) -> InputEvent {
    match kind {
        "down" => InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        "up" => InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        _ => InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            modifiers: 0,
        }),
    }
}

#[test]
fn dragging_a_card_onto_a_lane_drops_it_and_shows_a_preview() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State::default()).with_root_widget(Lanes));
    driver.harness.env.viewport_size = LayoutSize::new(800.0, 600.0);
    driver.pump()?;
    let card = driver.find_semantics_identifier("lanes.card").expect("card").bounds;
    let lane = driver.find_semantics_identifier("lanes.done").expect("lane").bounds;
    let start = LayoutPoint::new(card.x() + 10.0, card.y() + card.height() / 2.0);
    let end = LayoutPoint::new(lane.x() + lane.width() / 2.0, lane.y() + lane.height() / 2.0);

    driver.harness.send_event(pointer("down", start))?;
    driver.pump()?;
    for step in 1..=10 {
        let t = step as f32 / 10.0;
        let point = LayoutPoint::new(start.x + (end.x - start.x) * t, start.y + (end.y - start.y) * t);
        driver.harness.send_event(pointer("move", point))?;
        driver.pump()?;
    }
    let preview_visible = driver.find_text("Card preview").is_some();
    driver.harness.send_event(pointer("up", end))?;
    driver.pump()?;

    let drops = driver.harness.runtime.get_app_state::<State>().unwrap().drops;
    assert!(preview_visible, "the drag preview should render under the pointer mid-drag");
    assert_eq!(drops, 1, "releasing over the lane should drop the card there");
    Ok(())
}
