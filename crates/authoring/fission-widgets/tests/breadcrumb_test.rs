use fission_core::authoring::BuildCtx;
use fission_core::ui::{Container, Text, Widget};
use fission_core::{
    build, ActionEnvelope, ActionId, Env, GlobalState, RuntimeState, View, WidgetId,
};
use fission_ir::{ActionTrigger, CoreIR, LayoutOp, Op, PaintOp, Role, Semantics};
use fission_widgets::{Breadcrumb, BreadcrumbEntry, BreadcrumbItem, BreadcrumbLayout};

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: b"null".to_vec(),
    }
}

fn lower(build_widget: impl FnOnce() -> Widget) -> CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = TestState;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    fission_core::internal::lower_widget_to_ir(&widget)
}

fn semantics_at(ir: &CoreIR, id: WidgetId) -> &Semantics {
    match &ir.nodes[&id].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected semantics at {id:?}, got {op:?}"),
    }
}

fn contains_text(ir: &CoreIR, expected: &str) -> bool {
    ir.nodes.values().any(|node| match &node.op {
        Op::Paint(PaintOp::DrawText { text, .. }) => text == expected,
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
            runs.iter().any(|run| run.text == expected)
        }
        _ => false,
    })
}

#[test]
fn retained_breadcrumb_preserves_content_gap_separator_and_semantics() {
    let root_id = WidgetId::explicit("breadcrumb.rich");
    let ancestor_id = WidgetId::explicit("breadcrumb.rich.ancestor");
    let section_id = WidgetId::explicit("breadcrumb.rich.section");
    let current_id = WidgetId::explicit("breadcrumb.rich.current");
    let first_separator_content_id = WidgetId::explicit("breadcrumb.rich.separator.first");
    let second_separator_content_id = WidgetId::explicit("breadcrumb.rich.separator.second");
    let navigate = action("breadcrumb.navigate");

    let ir = lower(|| {
        let mut first_separator = Text::new("/");
        first_separator.id = Some(first_separator_content_id);
        let mut second_separator = Text::new("·");
        second_separator.id = Some(second_separator_content_id);
        BreadcrumbLayout::new(vec![
            BreadcrumbEntry::new(
                ancestor_id,
                "Home",
                Container::new(Text::new("Custom home")),
            )
            .on_click(navigate.clone()),
            BreadcrumbEntry::new(section_id, "Projects", Text::new("Custom projects")),
            BreadcrumbEntry::new(current_id, "Current project", Text::new("Custom current"))
                .on_click(action("breadcrumb.must-not-run")),
        ])
        .id(root_id)
        .separators(vec![first_separator.into(), second_separator.into()])
        .gap(13.0)
        .into()
    });

    match &ir.nodes[&root_id].op {
        Op::Layout(LayoutOp::Flex { gap, .. }) => assert_eq!(*gap, Some(13.0)),
        op => panic!("expected breadcrumb row at the stable root, got {op:?}"),
    }
    assert!(contains_text(&ir, "Custom home"));
    assert!(contains_text(&ir, "Custom projects"));
    assert!(contains_text(&ir, "Custom current"));
    assert!(contains_text(&ir, "/"));
    assert!(contains_text(&ir, "·"));
    assert!(ir.nodes.contains_key(&first_separator_content_id));
    assert!(ir.nodes.contains_key(&second_separator_content_id));
    for index in 1..=2 {
        let separator_id = WidgetId::derived(root_id.as_u128(), &[0x5345_5041, index as u32]);
        assert!(ir.nodes.contains_key(&separator_id));
    }

    let ancestor = semantics_at(&ir, ancestor_id);
    assert_eq!(ancestor.role, Role::Button);
    assert_eq!(ancestor.label.as_deref(), Some("Home"));
    assert_eq!(ancestor.actions.entries.len(), 1);
    assert_eq!(ancestor.actions.entries[0].trigger, ActionTrigger::Default);
    assert_eq!(ancestor.actions.entries[0].action_id, navigate.id.as_u128());

    let section = semantics_at(&ir, section_id);
    assert_eq!(section.role, Role::Text);
    assert_eq!(section.label.as_deref(), Some("Projects"));
    assert!(section.actions.entries.is_empty());

    let current = semantics_at(&ir, current_id);
    assert_eq!(current.role, Role::Text);
    assert_eq!(current.label.as_deref(), Some("Current project"));
    assert!(current.actions.entries.is_empty());
    assert!(!current.focusable);
}

#[test]
fn legacy_breadcrumb_keeps_only_non_final_navigation_actionable() {
    let ir = lower(|| {
        Breadcrumb {
            items: vec![
                BreadcrumbItem {
                    label: "Home".into(),
                    on_click: Some(action("breadcrumb.home")),
                },
                BreadcrumbItem {
                    label: "Current".into(),
                    on_click: Some(action("breadcrumb.current.must-not-run")),
                },
            ],
        }
        .into()
    });

    let actionable = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if !semantics.actions.entries.is_empty() => Some(semantics),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actionable.len(), 1);
    assert_eq!(actionable[0].label.as_deref(), Some("Home"));
    assert_eq!(actionable[0].role, Role::Button);

    let current = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.label.as_deref() == Some("Current") => {
                Some(semantics)
            }
            _ => None,
        })
        .expect("current-location semantics");
    assert_eq!(current.role, Role::Text);
    assert!(current.actions.entries.is_empty());
}
