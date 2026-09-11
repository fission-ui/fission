use fission_core::authoring::BuildCtx;
use fission_core::ui::{Container, Spacer, Text};
use fission_core::{build, GlobalState, View, WidgetId};
use fission_ir::op::Color;
use fission_ir::{LayoutOp, Op};
use fission_widgets::timeline::{Timeline, TimelineEntry, TimelineItem, TimelineLayout};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl GlobalState for TestState {}

#[test]
fn test_timeline_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let timeline = Timeline {
        items: vec![
            TimelineItem {
                title: "Step 1".into(),
                description: None,
                timestamp: None,
            },
            TimelineItem {
                title: "Step 2".into(),
                description: None,
                timestamp: None,
            },
        ],
    };

    let node = build::enter(&mut ctx, &view, || timeline.into());
    // A timeline is an ordered list of events, so it lowers through a semantics
    // region carrying Role::List rather than a bare column.
    assert_eq!(
        fission_core::internal::widget_kind_name(&node),
        "SemanticsRegion"
    );
}

#[test]
fn rich_timeline_keeps_stable_entries_and_continuous_connector_segments() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let ids = [
        WidgetId::explicit("timeline.first"),
        WidgetId::explicit("timeline.second"),
        WidgetId::explicit("timeline.third"),
    ];

    let widget = build::enter(&mut ctx, &view, || {
        TimelineLayout::new(vec![
            TimelineEntry::new(ids[0], Text::new("A tall retained card")),
            TimelineEntry::new(ids[1], Text::new("Interactive middle card")).marker(
                Container::new(Spacer::default())
                    .size(42.0, 42.0)
                    .border_radius(21.0),
            ),
            TimelineEntry::new(ids[2], Text::new("Final retained card")),
        ])
        .id(WidgetId::explicit("timeline.rich"))
        .marker_geometry(42.0, 42.0)
        .spacing(12.0, 18.0)
        .connector(2.0, Color::BLACK)
        .into()
    });
    let ir = fission_core::internal::lower_widget_to_ir(&widget);

    for id in ids {
        assert!(ir.nodes.contains_key(&id), "missing stable entry {id:?}");
    }
    let connector_segments = ir
        .nodes
        .values()
        .filter(|node| {
            matches!(
                node.op,
                Op::Layout(LayoutOp::Positioned {
                    width: Some(width),
                    ..
                }) if width == 2.0
            )
        })
        .count();
    assert_eq!(connector_segments, 3);
}

#[test]
fn one_rich_timeline_entry_has_no_connector() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let widget = build::enter(&mut ctx, &view, || {
        TimelineLayout::new(vec![TimelineEntry::new(
            WidgetId::explicit("timeline.only"),
            Text::new("Only entry"),
        )])
        .connector(2.0, Color::BLACK)
        .into()
    });
    let ir = fission_core::internal::lower_widget_to_ir(&widget);

    assert!(!ir.nodes.values().any(|node| {
        matches!(
            node.op,
            Op::Layout(LayoutOp::Positioned {
                width: Some(width),
                ..
            }) if width == 2.0
        )
    }));
}
