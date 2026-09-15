//! An auto-fit grid is as tall as its wrapped content, so what follows it starts
//! below it instead of being drawn over it.

use fission_core::ui::{Column, Container, Grid, Text, Widget};
use fission_core::GlobalState;
use fission_ir::op::GridTrack;
use fission_ir::op::Length;
use fission_layout::LayoutSize;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

const HEADING: &str = "Server-rendered collector commerce";
const AFTER: &str = "Content after the grid";

#[derive(Clone, Copy)]
struct Page {
    /// Line height as a multiple of the font size, like display type's 0.98.
    line_height: Option<f32>,
    /// Wraps the copy in a growing container, as the store hero does.
    grow_copy: bool,
    /// Sizes the spotlight with `min(100%, 360px)` instead of a fixed width.
    min_width_spotlight: bool,
}

impl From<Page> for Widget {
    fn from(page: Page) -> Self {
        let mut heading = Text::new(HEADING).size(48.0);
        if let Some(multiple) = page.line_height {
            heading = heading.line_height(48.0 * multiple);
        }
        let copy: Widget = if page.grow_copy {
            Container::new(Column {
                gap: Some(16.0),
                children: vec![heading.into(), Text::new("Supporting text").into()],
                ..Default::default()
            })
            .flex_grow(1.0)
            .into()
        } else {
            heading.into()
        };
        let spotlight = Container::new(Text::new("Featured card"));
        let spotlight: Widget = if page.min_width_spotlight {
            spotlight
                .width_length(Length::min(vec![
                    Length::percent(100.0),
                    Length::points(360.0),
                ]))
                .into()
        } else {
            spotlight.width(360.0).into()
        };
        Column {
            gap: Some(24.0),
            children: vec![
                Container::new(Grid {
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(360.0),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(32.0),
                    row_gap: Some(32.0),
                    children: vec![copy, spotlight],
                    ..Default::default()
                })
                .padding_all(32.0)
                .into(),
                Text::new(AFTER).into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

fn assert_after_starts_below_heading(page: Page, label: &str) {
    let mut harness = TestHarness::new(State).with_root_widget(page);
    harness.env.viewport_size = LayoutSize::new(890.0, 1600.0);
    harness.pump().expect("first frame");
    let ir = harness.last_ir.as_ref().expect("ir");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    let rect_of = |needle: &str| {
        ir.nodes
            .iter()
            .find(|(_, node)| node.op.text().is_some_and(|text| text == needle))
            .and_then(|(id, _)| snapshot.get_node_rect(*id))
            .unwrap_or_else(|| panic!("{needle} rect"))
    };
    let heading = rect_of(HEADING);
    let after = rect_of(AFTER);
    eprintln!("{label}: heading {heading:?} after {after:?}");
    assert!(
        after.y() >= heading.y() + heading.height(),
        "{label}: content after the grid starts at {} but the heading ends at {}",
        after.y(),
        heading.y() + heading.height()
    );
}

#[test]
fn content_after_an_auto_fit_grid_starts_below_its_wrapped_text() {
    assert_after_starts_below_heading(
        Page {
            line_height: None,
            grow_copy: false,
            min_width_spotlight: false,
        },
        "plain",
    );
}

#[test]
fn tight_display_line_height_still_pushes_following_content_down() {
    assert_after_starts_below_heading(
        Page {
            line_height: Some(0.98),
            grow_copy: false,
            min_width_spotlight: false,
        },
        "tight line height",
    );
}

#[test]
fn a_growing_copy_container_still_pushes_following_content_down() {
    assert_after_starts_below_heading(
        Page {
            line_height: Some(0.98),
            grow_copy: true,
            min_width_spotlight: false,
        },
        "growing copy",
    );
}

#[test]
fn a_min_width_spotlight_still_pushes_following_content_down() {
    assert_after_starts_below_heading(
        Page {
            line_height: Some(0.98),
            grow_copy: true,
            min_width_spotlight: true,
        },
        "store hero",
    );
}
