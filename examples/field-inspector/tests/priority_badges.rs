//! Urgency regression: work-order priorities read by urgency, not by selection.
//!
//! The priority pill used to take its colour from whether the card was
//! selected, so a Critical job looked exactly like a Medium one. Each priority
//! now carries its own design-system tone: destructive for Critical, warning
//! for High, neutral for Medium.

use fission::op::{Fill, Op, PaintOp};
use fission::prelude::{BadgeTone, ComponentSize, WidgetId};
use fission_test::{op_text, TestHarness};
use field_inspector::{FieldInspectorApp, FieldInspectorState};

const WIDTH: f32 = 1_280.0;
const HEIGHT: f32 = 860.0;

/// The seeded work orders, each with the priority its card should announce.
const ORDERS: [(&str, &str); 3] = [
    ("WO-1048", "High"),
    ("WO-1052", "Critical"),
    ("WO-1060", "Medium"),
];

fn pump() -> TestHarness<FieldInspectorState> {
    let mut harness =
        TestHarness::new(FieldInspectorState::default()).with_root_widget(FieldInspectorApp);
    harness.env.viewport_size = fission::layout::LayoutSize::new(WIDTH, HEIGHT);
    harness.pump().expect("pump field inspector");
    harness
}

/// The fill painted behind a work order's priority badge.
///
/// Words like "High" also appear in capability panels, so the search starts at
/// the order's own id, climbs to the card that holds both the id and the
/// priority, and then takes the nearest rectangle painted behind that label —
/// the badge's pill.
fn priority_background(
    harness: &TestHarness<FieldInspectorState>,
    order_id: &str,
    priority: &str,
) -> Option<Fill> {
    let ir = harness.last_ir.as_ref().expect("ir");
    let id_node = ir
        .nodes
        .values()
        .find(|node| op_text(&node.op).as_deref() == Some(order_id))
        .unwrap_or_else(|| panic!("no card reads {order_id:?}"));

    let mut ancestor = id_node.parent;
    while let Some(root) = ancestor {
        let mut stack: Vec<WidgetId> = vec![root];
        let mut label = None;
        while let Some(id) = stack.pop() {
            let node = &ir.nodes[&id];
            if op_text(&node.op).as_deref() == Some(priority) {
                label = Some(id);
                break;
            }
            stack.extend(node.children.iter().copied());
        }

        if let Some(label) = label {
            let mut painted = ir.nodes[&label].parent;
            while let Some(id) = painted {
                let node = &ir.nodes[&id];
                let fill = node
                    .children
                    .iter()
                    .find_map(|child| match &ir.nodes[child].op {
                        Op::Paint(PaintOp::DrawRect { fill, .. }) => fill.clone(),
                        _ => None,
                    });
                if fill.is_some() {
                    return fill;
                }
                painted = node.parent;
            }
            return None;
        }

        ancestor = ir.nodes[&root].parent;
    }
    None
}

fn tone_background(harness: &TestHarness<FieldInspectorState>, tone: BadgeTone) -> Option<Fill> {
    harness
        .env
        .theme
        .components
        .badge
        .resolve(tone, ComponentSize::Sm)
        .background
}

fn background(harness: &TestHarness<FieldInspectorState>, priority: &str) -> Option<Fill> {
    let (order_id, _) = ORDERS
        .iter()
        .find(|(_, seeded)| *seeded == priority)
        .unwrap_or_else(|| panic!("no seeded order has priority {priority:?}"));
    priority_background(harness, order_id, priority)
}

#[test]
fn critical_outranks_medium_and_carries_the_error_tone() {
    let harness = pump();
    let critical = background(&harness, "Critical");
    let medium = background(&harness, "Medium");

    assert_ne!(
        critical, medium,
        "the most urgent work order must not look like a routine one"
    );
    assert_eq!(
        critical,
        tone_background(&harness, BadgeTone::Error),
        "Critical should paint the design system's error tone"
    );
    assert_eq!(
        medium,
        tone_background(&harness, BadgeTone::Gray),
        "Medium should stay neutral"
    );
}

#[test]
fn high_sits_between_critical_and_medium() {
    let harness = pump();
    let high = background(&harness, "High");

    assert_eq!(
        high,
        tone_background(&harness, BadgeTone::Warning),
        "High should paint the design system's warning tone"
    );
    assert_ne!(
        high,
        background(&harness, "Critical"),
        "High and Critical must stay distinguishable"
    );
    assert_ne!(
        high,
        background(&harness, "Medium"),
        "High and Medium must stay distinguishable"
    );
}

/// Priority is a property of the job, so selecting a card must not restyle it.
#[test]
fn selecting_a_card_leaves_its_priority_tone_alone() {
    let harness = pump();
    let selected = FieldInspectorState::default().selected_order_id;
    assert!(
        ORDERS.iter().any(|(id, _)| *id == selected),
        "one seeded work order should start selected, so both cases are covered"
    );

    for (order_id, priority) in ORDERS {
        let fill = priority_background(&harness, order_id, priority);
        let expected = tone_background(
            &harness,
            match priority {
                "Critical" => BadgeTone::Error,
                "High" => BadgeTone::Warning,
                _ => BadgeTone::Gray,
            },
        );
        assert_eq!(
            fill, expected,
            "{order_id} ({priority}) should take its tone from its priority, selected or not"
        );
    }
}
