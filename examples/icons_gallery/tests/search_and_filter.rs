//! Findability regression: ten thousand icons need a way in.
//!
//! The gallery used to render one unbroken list, so reaching an icon meant
//! scrolling thousands of rows. A search field and a category filter now narrow
//! it, a count says how much is left, and a query that matches nothing says so
//! instead of showing a blank page.

use fission_test::{ir_has_text, ir_texts, TestHarness};
use icons_gallery::{filter_icons, IconsApp, State};

const WIDE: (f32, f32) = (1_280.0, 860.0);
const NARROW: (f32, f32) = (390.0, 844.0);
const EMPTY_STATE_TITLE: &str = "No icons match";

fn pump(state: State, (width, height): (f32, f32)) -> TestHarness<State> {
    let mut harness = TestHarness::new(state).with_root_widget(IconsApp);
    harness.env.viewport_size = fission::layout::LayoutSize::new(width, height);
    harness.pump().expect("pump icons gallery");
    harness
}

fn searching(query: &str) -> State {
    State {
        query: query.to_string(),
        category: None,
    }
}

/// The icon labels the list currently renders, which read `category/name/variant`.
fn visible_labels(harness: &TestHarness<State>) -> Vec<String> {
    ir_texts(harness.last_ir.as_ref().expect("ir"))
        .into_iter()
        .filter(|text| text.contains('/'))
        .collect()
}

fn count_line(harness: &TestHarness<State>) -> String {
    ir_texts(harness.last_ir.as_ref().expect("ir"))
        .into_iter()
        .find(|text| text.contains("icon variants"))
        .expect("the gallery should say how many icons the filters leave")
}

#[test]
fn a_query_narrows_the_list_and_the_count_says_by_how_much() {
    let total = filter_icons(&State::default()).total;
    let matching = filter_icons(&searching("battery")).visible.len();

    assert!(
        matching > 0 && matching < total,
        "'battery' should match some but not all of {total} icons, matched {matching}"
    );

    let harness = pump(searching("battery"), WIDE);
    assert_eq!(count_line(&harness), format!("{matching} of {total} icon variants"));

    let labels = visible_labels(&harness);
    assert!(!labels.is_empty(), "a matching query should still show rows");
    for label in labels {
        assert!(
            label.to_lowercase().contains("battery"),
            "row {label:?} does not match the query"
        );
    }
}

/// The match is a case-insensitive substring, so shouting finds the same icons.
#[test]
fn the_query_ignores_case() {
    assert_eq!(
        filter_icons(&searching("BaTTeRy")).visible.len(),
        filter_icons(&searching("battery")).visible.len()
    );
}

#[test]
fn a_nonsense_query_shows_the_empty_state() {
    let harness = pump(searching("zzqqnotanicon"), WIDE);
    let ir = harness.last_ir.as_ref().expect("ir");

    assert!(
        ir_has_text(ir, EMPTY_STATE_TITLE),
        "a query matching nothing must say so, not show a blank page"
    );
    assert!(
        visible_labels(&harness).is_empty(),
        "no icon rows should survive a query that matches nothing"
    );
    let total = filter_icons(&State::default()).total;
    assert_eq!(count_line(&harness), format!("0 of {total} icon variants"));
}

#[test]
fn the_unfiltered_gallery_shows_rows_and_no_empty_state() {
    let harness = pump(State::default(), WIDE);
    let total = filter_icons(&State::default()).total;

    assert_eq!(count_line(&harness), format!("{total} of {total} icon variants"));
    assert!(!visible_labels(&harness).is_empty());
    assert!(!ir_has_text(harness.last_ir.as_ref().expect("ir"), EMPTY_STATE_TITLE));
}

#[test]
fn a_category_keeps_only_its_own_icons() {
    let category = filter_icons(&State::default())
        .categories
        .first()
        .copied()
        .expect("the icon set should have categories");
    let filtered = filter_icons(&State {
        query: String::new(),
        category: Some(category.to_string()),
    });

    assert!(!filtered.visible.is_empty());
    assert!(filtered.visible.len() < filtered.total);
    for (entry_category, ..) in &filtered.visible {
        assert_eq!(*entry_category, category);
    }

    let harness = pump(
        State {
            query: String::new(),
            category: Some(category.to_string()),
        },
        WIDE,
    );
    // The chip rail scrolls on a narrow window, so the count names the filter.
    assert_eq!(
        count_line(&harness),
        format!(
            "{} of {} icon variants in {category}",
            filtered.visible.len(),
            filtered.total
        )
    );
    for label in visible_labels(&harness) {
        assert!(
            label.starts_with(&format!("{category}/")),
            "row {label:?} is outside the {category:?} category"
        );
    }
}

/// A search and nineteen category chips must not overflow a phone or a desktop.
#[test]
fn the_controls_fit_both_widths() {
    for (width, height) in [NARROW, WIDE] {
        let harness = pump(searching("battery"), (width, height));
        let ir = harness.last_ir.as_ref().expect("ir");
        let snapshot = harness.last_snapshot.as_ref().expect("snapshot");

        for (id, node) in ir.nodes.iter() {
            let Some(text) = fission_test::op_text(&node.op) else {
                continue;
            };
            if !text.contains("icon variants") && text != "Search icons" {
                continue;
            }
            let rect = snapshot
                .get_node_rect(*id)
                .unwrap_or_else(|| panic!("{text:?} is not laid out at {width}x{height}"));
            assert!(
                rect.x() >= -0.5 && rect.right() <= width + 0.5,
                "{text:?} runs outside a {width}pt viewport: {rect:?}"
            );
        }

        assert!(
            !visible_labels(&harness).is_empty(),
            "the list should still render rows at {width}x{height}"
        );
    }
}
