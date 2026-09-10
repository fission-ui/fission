use fission_core::internal::BuildCtx;
use fission_core::ui::{Text, Widget};
use fission_core::{build, ActionEnvelope, ActionId, GlobalState, View, WidgetId};
use fission_ir::{
    ActionTrigger, CoreIR, LayoutOp, Op, PaintOp, PopupKind, Role, SemanticOrientation, Semantics,
};
use fission_widgets::{
    Drawer, DrawerSide, FileUpload, Menu, MenuButton, MenuItem, Modal, ModalAction, Select,
    SelectItem, TabItem, Tabs,
};

#[derive(Default, Clone, Debug)]
struct State;

impl GlobalState for State {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: b"null".to_vec(),
    }
}

fn semantics<'a>(ir: &'a CoreIR, identifier: &str) -> &'a Semantics {
    semantics_entry(ir, identifier).1
}

fn semantics_entry<'a>(ir: &'a CoreIR, identifier: &str) -> (WidgetId, &'a Semantics) {
    let mut matches = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.as_deref() == Some(identifier) => {
                Some((node.id, semantics))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        matches.len(),
        1,
        "expected exactly one semantics node with identifier {identifier:?}"
    );
    matches.remove(0)
}

fn semantics_for_role(ir: &CoreIR, role: Role) -> (WidgetId, &Semantics) {
    let mut matches = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.role == role => Some((node.id, semantics)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        matches.len(),
        1,
        "expected exactly one semantics node with role {role:?}"
    );
    matches.remove(0)
}

fn flyout_alignment_target(ir: &CoreIR) -> Option<WidgetId> {
    let mut matches = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Layout(LayoutOp::Flyout { options, .. }) => Some(options.alignment_target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected exactly one flyout");
    matches.remove(0)
}

fn assert_actionable(ir: &CoreIR, identifier: &str, expected_role: Role) {
    let semantics = semantics(ir, identifier);
    assert_eq!(semantics.role, expected_role);
    assert!(semantics.focusable, "{identifier} should remain focusable");
    assert_eq!(
        semantics.actions.entries.len(),
        1,
        "{identifier} should own its configured action"
    );
}

fn build_widget(build_widget: impl FnOnce() -> Widget) -> (CoreIR, Vec<CoreIR>) {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();
    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );

    let widget = build::enter(&mut ctx, &view, build_widget);
    let root_ir = fission_core::internal::lower_widget_to_ir(&widget);
    let portal_irs = ctx
        .take_portals()
        .into_iter()
        .map(|(_, portal)| fission_core::internal::lower_widget_to_ir(&portal))
        .collect();
    (root_ir, portal_irs)
}

#[test]
fn duplicate_tab_and_menu_labels_have_distinct_actionable_identifiers() {
    let (tabs_ir, _) = build_widget(|| {
        Tabs {
            items: vec![
                TabItem {
                    title: "Settings".into(),
                    content: Text::new("First").into(),
                    on_press: Some(action("tab.first")),
                    semantics_identifier: Some("settings.first".into()),
                },
                TabItem {
                    title: "Settings".into(),
                    content: Text::new("Second").into(),
                    on_press: Some(action("tab.second")),
                    semantics_identifier: Some("settings.second".into()),
                },
            ],
            ..Default::default()
        }
        .into()
    });
    assert_actionable(&tabs_ir, "settings.first", Role::Tab);
    assert_actionable(&tabs_ir, "settings.second", Role::Tab);

    let (menu_ir, _) = build_widget(|| {
        Menu {
            items: vec![
                MenuItem {
                    label: "Open".into(),
                    icon: None,
                    on_select: Some(action("menu.first")),
                    semantics_identifier: Some("menu.first".into()),
                },
                MenuItem {
                    label: "Open".into(),
                    icon: None,
                    on_select: Some(action("menu.second")),
                    semantics_identifier: Some("menu.second".into()),
                },
            ],
            width: None,
            max_height: None,
        }
        .into()
    });
    assert_actionable(&menu_ir, "menu.first", Role::MenuItem);
    assert_actionable(&menu_ir, "menu.second", Role::MenuItem);
    let (_, menu) = semantics_for_role(&menu_ir, Role::Menu);
    assert_eq!(menu.orientation, Some(SemanticOrientation::Vertical));
}

#[test]
fn select_and_menu_button_identify_triggers_and_popup_items() {
    let (select_ir, select_portals) = build_widget(|| {
        Select {
            id: WidgetId::explicit("country"),
            selected_label: Some("Canada".into()),
            items: vec![
                SelectItem {
                    label: "Canada".into(),
                    icon: None,
                    on_select: action("country.ca"),
                    semantics_identifier: Some("country.ca".into()),
                },
                SelectItem {
                    label: "France".into(),
                    icon: None,
                    on_select: action("country.fr"),
                    semantics_identifier: Some("country.fr".into()),
                },
            ],
            is_open: true,
            on_toggle: Some(action("country.toggle")),
            trigger_semantics_identifier: Some("country.trigger".into()),
            placeholder: "Country".into(),
            width: Some(240.0),
        }
        .into()
    });
    assert_actionable(&select_ir, "country.trigger", Role::ComboBox);
    assert_eq!(select_portals.len(), 1);
    let select_trigger = semantics(&select_ir, "country.trigger");
    assert_eq!(select_trigger.value.as_deref(), Some("Canada"));
    assert_eq!(select_trigger.expanded, Some(true));
    assert_eq!(select_trigger.has_popup, Some(PopupKind::ListBox));
    assert_eq!(select_trigger.controls.len(), 1);

    let select_popup_id = select_trigger.controls[0];
    let select_popup = match &select_portals[0].nodes[&select_popup_id].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected list-box semantics at controlled popup id, got {op:?}"),
    };
    assert_eq!(select_popup.role, Role::ListBox);
    assert_eq!(
        select_popup.orientation,
        Some(SemanticOrientation::Vertical)
    );
    assert_eq!(select_popup.actions.entries.len(), 1);
    assert_eq!(
        select_popup.actions.entries[0].trigger,
        ActionTrigger::Dismiss
    );
    assert_eq!(
        select_popup.actions.entries[0].action_id,
        action("country.toggle").id.as_u128()
    );

    let (canada_id, canada) = semantics_entry(&select_portals[0], "country.ca");
    assert_actionable(&select_portals[0], "country.ca", Role::Option);
    assert_eq!(canada.selected, Some(true));
    assert_eq!(
        canada_id,
        WidgetId::scoped(select_popup_id.as_u128(), "country.ca")
    );
    assert_eq!(
        select_trigger.active_descendant,
        Some(canada_id),
        "an open Select must expose the stable selected option id for focus and popup alignment"
    );
    assert_eq!(
        flyout_alignment_target(&select_portals[0]),
        Some(canada_id),
        "Select must align the selected option's centre with its trigger"
    );
    let (france_id, france) = semantics_entry(&select_portals[0], "country.fr");
    assert_actionable(&select_portals[0], "country.fr", Role::Option);
    assert_eq!(france.selected, Some(false));
    assert_eq!(
        france_id,
        WidgetId::scoped(select_popup_id.as_u128(), "country.fr")
    );

    let (menu_button_ir, menu_portals) = build_widget(|| {
        MenuButton {
            id: WidgetId::explicit("actions"),
            label: "Actions".into(),
            items: vec![MenuItem {
                label: "Delete".into(),
                icon: None,
                on_select: Some(action("actions.delete")),
                semantics_identifier: Some("actions.delete".into()),
            }],
            is_open: true,
            on_toggle: Some(action("actions.toggle")),
            trigger_semantics_identifier: Some("actions.trigger".into()),
        }
        .into()
    });
    assert_actionable(&menu_button_ir, "actions.trigger", Role::Button);
    assert_eq!(menu_portals.len(), 1);
    assert_actionable(&menu_portals[0], "actions.delete", Role::MenuItem);

    let menu_trigger = semantics(&menu_button_ir, "actions.trigger");
    assert_eq!(menu_trigger.expanded, Some(true));
    assert_eq!(menu_trigger.has_popup, Some(PopupKind::Menu));
    assert_eq!(menu_trigger.controls.len(), 1);
    let menu_popup = match &menu_portals[0].nodes[&menu_trigger.controls[0]].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected menu semantics at controlled popup id, got {op:?}"),
    };
    assert_eq!(menu_popup.role, Role::Menu);
    assert_eq!(menu_popup.orientation, Some(SemanticOrientation::Vertical));
    assert_eq!(menu_popup.actions.entries.len(), 1);
    assert_eq!(
        menu_popup.actions.entries[0].trigger,
        ActionTrigger::Dismiss
    );
}

#[test]
fn menu_and_select_use_compact_bounded_popup_anatomy() {
    let (trigger_ir, portals) = build_widget(|| {
        Select {
            id: WidgetId::explicit("density"),
            selected_label: Some("Second".into()),
            items: vec![
                SelectItem {
                    label: "First".into(),
                    icon: None,
                    on_select: action("density.first"),
                    semantics_identifier: Some("density.first".into()),
                },
                SelectItem {
                    label: "Second".into(),
                    icon: None,
                    on_select: action("density.second"),
                    semantics_identifier: Some("density.second".into()),
                },
            ],
            is_open: true,
            on_toggle: Some(action("density.toggle")),
            trigger_semantics_identifier: Some("density.trigger".into()),
            placeholder: "Choose".into(),
            width: Some(240.0),
        }
        .into()
    });
    let popup = &portals[0];
    let (trigger_id, _) = semantics_entry(&trigger_ir, "density.trigger");
    match &trigger_ir.nodes[&trigger_ir.nodes[&trigger_id].children[0]].op {
        Op::Layout(LayoutOp::Box { height, .. }) => assert_eq!(*height, Some(32.0)),
        op => panic!("expected a compact select trigger box, got {op:?}"),
    }
    let (popup_id, _) = semantics_for_role(popup, Role::ListBox);
    let popup_node = &popup.nodes[&popup_id];
    let surface = &popup.nodes[&popup_node.children[0]];
    match &surface.op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            use fission_ir::op::Length;

            assert_eq!(style.width, Some(Length::Points(240.0)));
            assert_eq!(style.height, Some(Length::Points(76.0)));
            assert_eq!(
                style.padding,
                Some([
                    Length::Points(4.0),
                    Length::Points(4.0),
                    Length::Points(4.0),
                    Length::Points(4.0),
                ])
            );
        }
        op => panic!("expected a styled popup surface, got {op:?}"),
    }
    assert!(surface.children.iter().any(|id| matches!(
        &popup.nodes[id].op,
        Op::Paint(PaintOp::DrawRect {
            shadow: Some(_),
            ..
        })
    )));

    for identifier in ["density.first", "density.second"] {
        let (id, _) = semantics_entry(popup, identifier);
        let node = &popup.nodes[&id];
        match &popup.nodes[&node.children[0]].op {
            Op::Layout(LayoutOp::Box { height, .. }) => assert_eq!(*height, Some(32.0)),
            op => panic!("expected a compact option button box, got {op:?}"),
        }
    }

    let icon_box_count = trigger_ir
        .nodes
        .values()
        .chain(popup.nodes.values())
        .filter(|node| {
            matches!(
                &node.op,
                Op::Layout(LayoutOp::Box {
                    width: Some(width),
                    height: Some(height),
                    ..
                }) if *width == 16.0 && *height == 16.0
            )
        })
        .count();
    assert!(
        icon_box_count >= 2,
        "the trigger chevron and selected-option check should use compact icons"
    );

    let one_point_dividers = popup
        .nodes
        .values()
        .filter(|node| match &node.op {
            Op::Layout(LayoutOp::StyledBox { style, .. }) => {
                style.height == Some(fission_ir::op::Length::Points(1.0))
            }
            _ => false,
        })
        .count();
    assert_eq!(one_point_dividers, 0);

    let (bounded_menu, _) = build_widget(|| {
        Menu {
            items: vec![
                MenuItem {
                    label: "First".into(),
                    icon: None,
                    on_select: Some(action("bounded.first")),
                    semantics_identifier: Some("bounded.first".into()),
                },
                MenuItem {
                    label: "Second".into(),
                    icon: None,
                    on_select: Some(action("bounded.second")),
                    semantics_identifier: Some("bounded.second".into()),
                },
            ],
            width: Some(200.0),
            max_height: Some(50.0),
        }
        .into()
    });
    let (menu_id, _) = semantics_for_role(&bounded_menu, Role::Menu);
    let menu_surface = &bounded_menu.nodes[&bounded_menu.nodes[&menu_id].children[0]];
    match &menu_surface.op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            assert_eq!(style.height, Some(fission_ir::op::Length::Points(50.0)));
        }
        op => panic!("expected a bounded menu surface, got {op:?}"),
    }
    let scroll = menu_surface
        .children
        .iter()
        .find_map(|id| match &bounded_menu.nodes[id].op {
            Op::Layout(LayoutOp::Scroll {
                height,
                show_scrollbar,
                ..
            }) => Some((*height, *show_scrollbar)),
            _ => None,
        })
        .expect("bounded menu surface should contain a scroll viewport");
    assert_eq!(scroll, (Some(42.0), true));
}

#[test]
fn closed_menu_and_select_triggers_are_collapsed_without_mounting_popups() {
    let select_id = WidgetId::explicit("closed-select");
    let expected_select_popup = WidgetId::derived(select_id.as_u128(), &[1]);
    let (select_ir, select_portals) = build_widget(|| {
        Select {
            id: select_id,
            selected_label: None,
            items: Vec::new(),
            is_open: false,
            on_toggle: Some(action("closed-select.toggle")),
            trigger_semantics_identifier: Some("closed-select.trigger".into()),
            placeholder: "Choose".into(),
            width: Some(200.0),
        }
        .into()
    });
    assert!(select_portals.is_empty());
    let select_trigger = semantics(&select_ir, "closed-select.trigger");
    assert_eq!(select_trigger.role, Role::ComboBox);
    assert_eq!(select_trigger.value.as_deref(), Some("Choose"));
    assert_eq!(select_trigger.expanded, Some(false));
    assert_eq!(select_trigger.has_popup, Some(PopupKind::ListBox));
    assert_eq!(select_trigger.controls, vec![expected_select_popup]);

    let menu_id = WidgetId::explicit("closed-menu");
    let expected_menu_popup = WidgetId::derived(menu_id.as_u128(), &[1]);
    let (menu_ir, menu_portals) = build_widget(|| {
        MenuButton {
            id: menu_id,
            label: "Actions".into(),
            items: Vec::new(),
            is_open: false,
            on_toggle: Some(action("closed-menu.toggle")),
            trigger_semantics_identifier: Some("closed-menu.trigger".into()),
        }
        .into()
    });
    assert!(menu_portals.is_empty());
    let menu_trigger = semantics(&menu_ir, "closed-menu.trigger");
    assert_eq!(menu_trigger.role, Role::Button);
    assert_eq!(menu_trigger.expanded, Some(false));
    assert_eq!(menu_trigger.has_popup, Some(PopupKind::Menu));
    assert_eq!(menu_trigger.controls, vec![expected_menu_popup]);
}

#[test]
fn select_option_without_an_external_identifier_has_a_stable_structural_id() {
    let select_id = WidgetId::explicit("structural-option-id");
    let expected_popup = WidgetId::derived(select_id.as_u128(), &[1]);
    let expected_option = WidgetId::derived(expected_popup.as_u128(), &[0x4D45_4E55, 0]);
    let (_, portals) = build_widget(|| {
        Select {
            id: select_id,
            selected_label: Some("Only option".into()),
            items: vec![SelectItem {
                label: "Only option".into(),
                icon: None,
                on_select: action("structural-option-id.select"),
                semantics_identifier: None,
            }],
            is_open: true,
            on_toggle: Some(action("structural-option-id.toggle")),
            trigger_semantics_identifier: Some("structural-option-id.trigger".into()),
            placeholder: "Choose".into(),
            width: Some(200.0),
        }
        .into()
    });
    let (option_id, option) = semantics_for_role(&portals[0], Role::Option);
    assert_eq!(option_id, expected_option);
    assert_eq!(option.selected, Some(true));
    assert_eq!(option.actions.entries.len(), 1);
}

#[test]
fn file_upload_identifier_is_attached_to_the_browse_button() {
    let (ir, _) = build_widget(|| {
        FileUpload {
            label: "Browse".into(),
            selected_file: None,
            on_browse: Some(action("file.browse")),
            browse_semantics_identifier: Some("file.browse".into()),
        }
        .into()
    });

    assert_actionable(&ir, "file.browse", Role::Button);
}

#[test]
fn modal_and_modal_action_exhaustive_literal_shapes_remain_source_compatible() {
    let (_, portals) = build_widget(|| {
        // Deliberately exhaustive: these are the exact public field shapes from
        // before the retained modal anatomy was introduced.
        Modal {
            id: WidgetId::explicit("confirm"),
            title: "Confirm".into(),
            content: Text::new("Continue?").into(),
            is_open: true,
            on_dismiss: Some(action("modal.dismiss")),
            backdrop_semantics_identifier: Some("modal.backdrop".into()),
            close_semantics_identifier: Some("modal.close".into()),
            surface_semantics_identifier: Some("modal.surface".into()),
            actions: vec![ModalAction {
                label: "Confirm".into(),
                on_press: Some(action("modal.confirm")),
                is_primary: true,
                semantics_identifier: Some("modal.confirm".into()),
            }],
            width: None,
            motion: None,
        }
        .into()
    });

    assert_eq!(portals.len(), 1);
    let ir = &portals[0];
    assert_actionable(ir, "modal.backdrop", Role::Generic);
    assert_actionable(ir, "modal.close", Role::Button);
    assert_actionable(ir, "modal.confirm", Role::Button);
    assert_eq!(semantics(ir, "modal.surface").role, Role::Dialog);
}

#[test]
fn drawer_identifier_is_attached_to_the_dismissal_backdrop() {
    let (_, portals) = build_widget(|| {
        Drawer {
            id: WidgetId::explicit("navigation"),
            side: DrawerSide::Left,
            is_open: true,
            on_dismiss: Some(action("drawer.dismiss")),
            dismiss_semantics_identifier: Some("drawer.backdrop".into()),
            content: Text::new("Navigation").into(),
            width: None,
            motion: None,
        }
        .into()
    });

    assert_eq!(portals.len(), 1);
    assert_actionable(&portals[0], "drawer.backdrop", Role::Generic);
}

#[test]
fn modal_keeps_legacy_surface_identifier_when_none_is_supplied() {
    let (_, portals) = build_widget(|| {
        Modal {
            id: WidgetId::explicit("legacy"),
            title: "Legacy".into(),
            content: Text::new("Content").into(),
            is_open: true,
            on_dismiss: None,
            backdrop_semantics_identifier: None,
            close_semantics_identifier: None,
            surface_semantics_identifier: None,
            actions: Vec::new(),
            width: None,
            motion: None,
        }
        .into()
    });

    assert_eq!(portals.len(), 1);
    assert_eq!(
        semantics(&portals[0], "fission-modal-surface").role,
        Role::Dialog
    );
}
