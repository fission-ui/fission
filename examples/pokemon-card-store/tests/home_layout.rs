//! The store home page lays its sections out one after another, with no section
//! drawn over the next, whether the page fits the window or scrolls inside a
//! shorter preview.

use fission::layout::{LayoutRect, LayoutSize};
use fission::prelude::*;
use fission_test::TestHarness;
use pokemon_card_store::{StoreHomePage, StoreState};

const PROBES: [&str; 4] = [
    "Session cart",
    "Server-rendered collector commerce",
    "0 items in the server cart",
    "Browser bridge",
];

/// The home page inside a scroll view, the way a browser or a preview hosts it.
#[derive(Clone)]
struct ScrolledHome;

impl From<ScrolledHome> for Widget {
    fn from(_: ScrolledHome) -> Self {
        Scroll {
            id: Some(WidgetId::explicit("store.home.scroll")),
            child: Some(StoreHomePage.into()),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn home_sections_do_not_overlap_when_the_page_fits() {
    let rects = probe(StoreHomePage, LayoutSize::new(890.0, 1600.0));
    assert_stacked(&rects);
}

#[test]
fn a_short_scrolling_preview_keeps_every_section_at_its_natural_height() {
    let tall = probe(StoreHomePage, LayoutSize::new(826.0, 1600.0));
    let short = probe(ScrolledHome, LayoutSize::new(826.0, 660.0));
    assert_stacked(&short);
    for ((name, tall), (_, short)) in tall.iter().zip(&short) {
        assert!(
            (tall.height() - short.height()).abs() < 0.5,
            "{name:?} is {} high in a short preview but {} when the page fits",
            short.height(),
            tall.height()
        );
    }
}

fn probe<W: Into<Widget> + Clone + 'static>(
    root: W,
    viewport: LayoutSize,
) -> Vec<(&'static str, LayoutRect)> {
    let mut harness = TestHarness::new(StoreState::default()).with_root_widget(root);
    harness.env.viewport_size = viewport;
    harness.pump().expect("first frame");
    let ir = harness.last_ir.as_ref().expect("ir");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    PROBES
        .iter()
        .map(|needle| {
            let rect = ir
                .nodes
                .iter()
                .find(|(_, node)| node.op.text().is_some_and(|text| text == *needle))
                .and_then(|(id, _)| snapshot.get_node_rect(*id))
                .unwrap_or_else(|| panic!("{needle:?} is on the home page"));
            (*needle, rect)
        })
        .collect()
}

fn assert_stacked(rects: &[(&str, LayoutRect)]) {
    let heading = rects[1].1;
    let cart = rects[2].1;
    assert!(
        cart.y() >= heading.y() + heading.height(),
        "the cart summary starts at {} inside the hero heading, which ends at {}",
        cart.y(),
        heading.y() + heading.height()
    );
}
