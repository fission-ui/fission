use fission_core::internal::BuildCtx;
use fission_core::ui::{Text, Widget};
use fission_core::{build, ActionEnvelope, ActionId, GlobalState, View, WidgetId};
use fission_ir::{CoreIR, Op, Role, Semantics};
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
    let matches = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.as_deref() == Some(identifier) => {
                Some(semantics)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        matches.len(),
        1,
        "expected exactly one semantics node with identifier {identifier:?}"
    );
    matches[0]
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
    assert_actionable(&tabs_ir, "settings.first", Role::Button);
    assert_actionable(&tabs_ir, "settings.second", Role::Button);

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
    assert_actionable(&menu_ir, "menu.first", Role::Button);
    assert_actionable(&menu_ir, "menu.second", Role::Button);
}

#[test]
fn select_and_menu_button_identify_triggers_and_popup_items() {
    let (select_ir, select_portals) = build_widget(|| {
        Select {
            id: WidgetId::explicit("country"),
            selected_label: None,
            items: vec![SelectItem {
                label: "Canada".into(),
                icon: None,
                on_select: action("country.ca"),
                semantics_identifier: Some("country.ca".into()),
            }],
            is_open: true,
            on_toggle: Some(action("country.toggle")),
            trigger_semantics_identifier: Some("country.trigger".into()),
            placeholder: "Country".into(),
            width: None,
        }
        .into()
    });
    assert_actionable(&select_ir, "country.trigger", Role::Button);
    assert_eq!(select_portals.len(), 1);
    assert_actionable(&select_portals[0], "country.ca", Role::Button);

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
    assert_actionable(&menu_portals[0], "actions.delete", Role::Button);
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
fn modal_identifies_surface_actions_and_each_dismissal_control() {
    let (_, portals) = build_widget(|| {
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
    assert_eq!(semantics(ir, "modal.surface").role, Role::Generic);
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
        Role::Generic
    );
}
