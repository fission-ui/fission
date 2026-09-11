use fission_core::authoring::BuildCtx;
use fission_core::op::{Color, LayoutOp, Length, Op, PaintOp};
use fission_core::ui::Column;
use fission_core::{build, ActionEnvelope, ActionId, Env, GlobalState, View, Widget, WidgetId};
use fission_ir::{Role, Semantics};
use fission_widgets::{
    MenuActionItem, MenuContent, MenuGroup, MenuItemTone, MenuSeparator, SelectContent,
    SelectOption,
};

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: b"null".to_vec(),
    }
}

fn build_widget(env: &Env, build_widget: impl FnOnce() -> Widget) -> fission_ir::CoreIR {
    let state = State;
    let runtime = fission_core::RuntimeState::default();
    let view = View::new(&state, &runtime, env, None);
    let mut ctx = BuildCtx::<State>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    fission_core::internal::lower_widget_to_ir(&widget)
}

fn semantics_at(ir: &fission_ir::CoreIR, id: WidgetId) -> &Semantics {
    match &ir.nodes[&id].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected semantics at {id:?}, got {op:?}"),
    }
}

fn text_color(ir: &fission_ir::CoreIR, text: &str) -> Option<Color> {
    ir.nodes.values().find_map(|node| match &node.op {
        Op::Paint(PaintOp::DrawText {
            text: candidate,
            color,
            ..
        }) if candidate == text => Some(*color),
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => runs
            .iter()
            .find(|run| run.text == text)
            .map(|run| run.style.color),
        _ => None,
    })
}

#[test]
fn composed_menu_anatomy_uses_one_recipe_and_keeps_disabled_items_inert() {
    let content_id = WidgetId::explicit("project-actions");
    let open_id = WidgetId::scoped(content_id.as_u128(), "project.open");
    let delete_id = WidgetId::scoped(content_id.as_u128(), "project.delete");
    let unavailable_id = WidgetId::scoped(content_id.as_u128(), "project.unavailable");
    let selected_id = WidgetId::scoped(content_id.as_u128(), "project.selected");
    let group_id = WidgetId::derived(content_id.as_u128(), &[0x4752_4f55, 0]);
    let group_label_id = WidgetId::derived(group_id.as_u128(), &[0x4c41_424c]);
    let description_id = WidgetId::derived(open_id.as_u128(), &[0x4445_5343]);
    let separator_id = WidgetId::derived(content_id.as_u128(), &[0x5345_5052, 1]);

    let mut env = Env::default();
    let menu = &mut env.theme.components.menu;
    menu.surface_style.width = Some(244.0);
    menu.surface_style.padding = Some([5.0; 4]);
    menu.surface_style.radius = Some(13.0);
    menu.item_states.default.height = Some(36.0);
    menu.item_states.default.padding_x = Some(11.0);
    menu.item_states.default.padding_y = Some(7.0);
    menu.item_states.default.radius = Some(7.0);
    let destructive_color = Color {
        r: 197,
        g: 27,
        b: 74,
        a: 255,
    };
    menu.destructive_item_states.default.text_color = Some(destructive_color);

    let ir = build_widget(&env, || {
        MenuContent::new(vec![
            MenuGroup::new(vec![
                MenuActionItem::new("Open project")
                    .leading_icon("<svg viewBox='0 0 24 24'></svg>")
                    .description("Open in the current workspace")
                    .shortcut("Ctrl+O")
                    .semantics_identifier("project.open")
                    .on_select(action("open"))
                    .into(),
                MenuActionItem::new("Delete project")
                    .metadata("Permanent")
                    .tone(MenuItemTone::Destructive)
                    .semantics_identifier("project.delete")
                    .on_select(action("delete"))
                    .into(),
                MenuActionItem::new("Unavailable action")
                    .disabled(true)
                    .semantics_identifier("project.unavailable")
                    .on_select(action("unavailable"))
                    .into(),
            ])
            .label("Project")
            .into(),
            MenuSeparator::default().into(),
            MenuActionItem::new("Current project")
                .selected(true)
                .semantics_identifier("project.selected")
                .on_select(action("select"))
                .into(),
        ])
        .id(content_id)
        .into()
    });

    let content = semantics_at(&ir, content_id);
    assert_eq!(content.role, Role::Menu);

    let open = semantics_at(&ir, open_id);
    assert_eq!(open.role, Role::MenuItem);
    assert!(open.focusable);
    assert!(open.sequential_focusable);
    assert_eq!(open.actions.entries.len(), 1);
    assert_eq!(open.described_by, vec![description_id]);
    assert_eq!(semantics_at(&ir, description_id).role, Role::Text);

    let destructive = semantics_at(&ir, delete_id);
    assert!(destructive.focusable);
    assert!(!destructive.sequential_focusable);
    assert_eq!(destructive.actions.entries.len(), 1);
    assert_eq!(text_color(&ir, "Delete project"), Some(destructive_color));

    let disabled = semantics_at(&ir, unavailable_id);
    assert!(disabled.disabled);
    assert!(!disabled.focusable);
    assert!(!disabled.sequential_focusable);
    assert!(disabled.actions.entries.is_empty());

    let selected = semantics_at(&ir, selected_id);
    assert_eq!(selected.selected, Some(true));
    assert!(
        ir.nodes
            .values()
            .filter(|node| matches!(&node.op, Op::Paint(PaintOp::DrawSvg { .. })))
            .count()
            >= 2,
        "the leading icon and selected indicator should both render"
    );

    let group = semantics_at(&ir, group_id);
    assert_eq!(group.role, Role::Group);
    assert_eq!(group.labelled_by, vec![group_label_id]);
    assert_eq!(semantics_at(&ir, group_label_id).role, Role::Text);

    match &ir.nodes[&ir.nodes[&open_id].children[0]].op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            assert_eq!(style.width, Some(Length::Points(234.0)));
            assert_eq!(style.height, None);
            assert_eq!(style.min_height, Some(Length::Points(36.0)));
            // Menu rows carry logical [start, end, top, bottom] padding so the
            // selection indicator's reserved space follows reading order.
            assert_eq!(style.padding, None);
            assert_eq!(
                style.padding_directional,
                Some([
                    Length::Points(11.0),
                    Length::Points(11.0),
                    Length::Points(7.0),
                    Length::Points(7.0)
                ])
            );
        }
        op => panic!("expected recipe-backed item box, got {op:?}"),
    }

    let separator = semantics_at(&ir, separator_id);
    assert_eq!(separator.role, Role::Separator);
    assert!(!separator.focusable);
    assert!(!separator.sequential_focusable);
    match &ir.nodes[&ir.nodes[&separator_id].children[0]].op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            assert_eq!(style.height, Some(Length::Points(9.0)));
        }
        op => panic!("expected retained separator box, got {op:?}"),
    }

    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(PaintOp::DrawRect {
            corner_radius: 13.0,
            ..
        })
    )));
}

#[test]
fn standalone_list_box_has_one_roving_tab_entry_at_the_selected_option() {
    let content_id = WidgetId::explicit("roving-options");
    let first_id = WidgetId::explicit("roving.first");
    let selected_id = WidgetId::explicit("roving.selected");
    let last_id = WidgetId::explicit("roving.last");
    let ir = build_widget(&Env::default(), || {
        MenuContent::list_box(vec![
            MenuActionItem::option("First", false).id(first_id).into(),
            MenuActionItem::option("Selected", true)
                .id(selected_id)
                .into(),
            MenuActionItem::option("Last", false).id(last_id).into(),
        ])
        .id(content_id)
        .into()
    });

    assert!(!semantics_at(&ir, first_id).sequential_focusable);
    assert!(semantics_at(&ir, selected_id).sequential_focusable);
    assert!(!semantics_at(&ir, last_id).sequential_focusable);
}

#[test]
fn legacy_menu_items_lower_into_the_same_action_item_recipe() {
    use fission_widgets::{Menu, MenuItem};

    let mut env = Env::default();
    env.theme.components.menu.item_states.default.height = Some(41.0);
    let ir = build_widget(&env, || {
        Menu {
            items: vec![MenuItem {
                label: "Inspect".into(),
                icon: None,
                on_select: Some(action("inspect")),
                semantics_identifier: Some("menu.inspect".into()),
            }],
            width: Some(220.0),
            max_height: None,
        }
        .into()
    });

    let item_id = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.as_deref() == Some("menu.inspect") => {
                Some(node.id)
            }
            _ => None,
        })
        .expect("legacy item should produce action-item semantics");
    match &ir.nodes[&ir.nodes[&item_id].children[0]].op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            assert_eq!(style.height, Some(Length::Points(41.0)));
        }
        op => panic!("expected legacy item to use the menu item recipe, got {op:?}"),
    }
}

#[test]
fn implicit_menu_and_select_anatomy_ids_are_unique_across_siblings() {
    let ir = build_widget(&Env::default(), || {
        Column {
            children: vec![
                MenuContent::new(vec![
                    MenuGroup::new(vec![MenuActionItem::new("Nested action").into()]).into(),
                    MenuSeparator::new().into(),
                ])
                .into(),
                MenuContent::new(vec![
                    MenuGroup::new(vec![MenuActionItem::new("Nested action").into()]).into(),
                    MenuSeparator::new().into(),
                ])
                .into(),
                MenuGroup::new(vec![MenuActionItem::new("Grouped action").into()]).into(),
                MenuGroup::new(vec![MenuActionItem::new("Grouped action").into()]).into(),
                MenuSeparator::new().into(),
                MenuSeparator::new().into(),
                MenuActionItem::new("Standalone action").into(),
                MenuActionItem::new("Standalone action").into(),
                SelectContent::new(vec![SelectOption::option("Nested option", false).into()])
                    .into(),
                SelectContent::new(vec![SelectOption::option("Nested option", false).into()])
                    .into(),
            ],
            ..Default::default()
        }
        .into()
    });

    let role_count = |role| {
        ir.nodes
            .values()
            .filter(|node| matches!(&node.op, Op::Semantics(value) if value.role == role))
            .count()
    };
    assert_eq!(role_count(Role::Menu), 2);
    assert_eq!(role_count(Role::ListBox), 2);
    assert_eq!(role_count(Role::Group), 4);
    assert_eq!(role_count(Role::Separator), 4);
    assert_eq!(role_count(Role::MenuItem), 6);
    assert_eq!(role_count(Role::Option), 2);
}
