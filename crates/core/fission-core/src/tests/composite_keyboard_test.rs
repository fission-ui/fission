use crate::action::{ActionEnvelope, ActionId, GlobalState};
use crate::hit_test::get_all_focusable_nodes;
use crate::{InputEvent, KeyCode, KeyEvent, Runtime};
use fission_ir::{
    ActionEntry, ActionSet, ActionTrigger, CoreIR, LayoutDirection, Op, PopupKind, Role,
    SemanticOrientation, Semantics, StructuralOp, WidgetId,
};
use fission_layout::{LayoutSize, LayoutSnapshot};

fn id(value: &str) -> WidgetId {
    WidgetId::explicit(value)
}

fn add_semantics(ir: &mut CoreIR, name: &str, semantics: Semantics, children: Vec<WidgetId>) {
    ir.add_node(id(name), Op::Semantics(semantics), children);
}

fn add_item(
    ir: &mut CoreIR,
    name: &str,
    role: Role,
    label: &str,
    selected: Option<bool>,
    disabled: bool,
    sequential_focusable: bool,
) {
    add_semantics(
        ir,
        name,
        Semantics {
            role,
            label: Some(label.into()),
            selected,
            focusable: true,
            sequential_focusable,
            disabled,
            ..Default::default()
        },
        Vec::new(),
    );
}

fn finish_root(ir: &mut CoreIR, children: Vec<WidgetId>) {
    ir.add_node(
        id("root"),
        Op::Structural(StructuralOp::Group { stable_hash: 1 }),
        children,
    );
    ir.set_root(id("root"));
}

fn layout() -> LayoutSnapshot {
    LayoutSnapshot::new(LayoutSize::new(640.0, 480.0))
}

fn key(runtime: &mut Runtime, ir: &CoreIR, key_code: KeyCode) {
    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers: 0,
            }),
            ir,
            &layout(),
        )
        .unwrap();
}

#[test]
fn vertical_menu_navigation_wraps_skips_disabled_and_owns_its_axis() {
    let mut ir = CoreIR::default();
    add_item(&mut ir, "first", Role::MenuItem, "First", None, false, true);
    add_item(
        &mut ir,
        "disabled",
        Role::MenuItem,
        "Disabled",
        None,
        true,
        true,
    );
    add_item(&mut ir, "last", Role::MenuItem, "Last", None, false, true);
    add_semantics(
        &mut ir,
        "menu",
        Semantics {
            role: Role::Menu,
            orientation: Some(SemanticOrientation::Vertical),
            ..Default::default()
        },
        vec![id("first"), id("disabled"), id("last")],
    );
    add_item(
        &mut ir,
        "outside",
        Role::Button,
        "Outside",
        None,
        false,
        true,
    );
    finish_root(&mut ir, vec![id("menu"), id("outside")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("first")));

    key(&mut runtime, &ir, KeyCode::Down);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("last")));
    key(&mut runtime, &ir, KeyCode::Down);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("first")));
    key(&mut runtime, &ir, KeyCode::Up);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("last")));

    key(&mut runtime, &ir, KeyCode::Home);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("first")));
    key(&mut runtime, &ir, KeyCode::End);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("last")));

    key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("last")),
        "a cross-axis arrow must not escape the composite"
    );
}

#[test]
fn tablist_navigation_includes_programmatic_only_tabs() {
    let mut ir = CoreIR::default();
    add_item(
        &mut ir,
        "selected-tab",
        Role::Tab,
        "Overview",
        Some(true),
        false,
        true,
    );
    add_item(
        &mut ir,
        "second-tab",
        Role::Tab,
        "Activity",
        Some(false),
        false,
        false,
    );
    add_item(
        &mut ir,
        "disabled-tab",
        Role::Tab,
        "Disabled",
        Some(false),
        true,
        false,
    );
    add_item(
        &mut ir,
        "third-tab",
        Role::Tab,
        "Settings",
        Some(false),
        false,
        false,
    );
    add_semantics(
        &mut ir,
        "tabs",
        Semantics {
            role: Role::TabList,
            orientation: Some(SemanticOrientation::Horizontal),
            ..Default::default()
        },
        vec![
            id("selected-tab"),
            id("second-tab"),
            id("disabled-tab"),
            id("third-tab"),
        ],
    );
    finish_root(&mut ir, vec![id("tabs")]);

    assert_eq!(get_all_focusable_nodes(&ir), vec![id("selected-tab")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("selected-tab")));
    key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("second-tab"))
    );
    key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("third-tab")),
        "disabled tabs are skipped"
    );
    key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("selected-tab")),
        "horizontal traversal wraps"
    );
    key(&mut runtime, &ir, KeyCode::Up);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("selected-tab")),
        "vertical arrows stay inside a horizontal tablist without moving focus"
    );
}

#[test]
fn right_to_left_tablist_arrows_follow_physical_direction() {
    let mut ir = CoreIR::default();
    ir.layout_direction = LayoutDirection::RightToLeft;
    for (name, label) in [
        ("first-tab", "Overview"),
        ("second-tab", "Activity"),
        ("third-tab", "Settings"),
    ] {
        add_item(&mut ir, name, Role::Tab, label, None, false, true);
    }
    add_semantics(
        &mut ir,
        "tabs",
        Semantics {
            role: Role::TabList,
            orientation: Some(SemanticOrientation::Horizontal),
            ..Default::default()
        },
        vec![id("first-tab"), id("second-tab"), id("third-tab")],
    );
    finish_root(&mut ir, vec![id("tabs")]);

    assert_eq!(
        get_all_focusable_nodes(&ir),
        vec![id("first-tab"), id("second-tab"), id("third-tab")],
        "Tab order remains logical source order"
    );

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("first-tab")));
    key(&mut runtime, &ir, KeyCode::Left);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("second-tab")),
        "physical Left advances through a right-to-left tablist"
    );
    key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("first-tab")),
        "physical Right moves back through a right-to-left tablist"
    );
}

#[test]
fn menu_typeahead_cycles_matching_labels_for_both_keyboard_event_forms() {
    let mut ir = CoreIR::default();
    for (name, label) in [
        ("alpha", "Alpha"),
        ("bravo", "Bravo"),
        ("blue", "Blue"),
        ("charlie", "Charlie"),
    ] {
        add_item(&mut ir, name, Role::MenuItem, label, None, false, false);
    }
    add_semantics(
        &mut ir,
        "menu",
        Semantics {
            role: Role::Menu,
            ..Default::default()
        },
        vec![id("alpha"), id("bravo"), id("blue"), id("charlie")],
    );
    finish_root(&mut ir, vec![id("menu")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("alpha")));
    key(&mut runtime, &ir, KeyCode::Char('b'));
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("bravo")));
    key(&mut runtime, &ir, KeyCode::Char('B'));
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("blue")));

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('b'),
                modifiers: 0,
                text: "b".into(),
            }),
            &ir,
            &layout(),
        )
        .unwrap();
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("bravo")),
        "matching wraps and the produced-text event is handled once"
    );

    let mut listbox_ir = CoreIR::default();
    add_item(
        &mut listbox_ir,
        "disabled-option",
        Role::Option,
        "Gamma disabled",
        Some(false),
        true,
        false,
    );
    add_item(
        &mut listbox_ir,
        "enabled-option",
        Role::Option,
        "Gamma",
        Some(false),
        false,
        false,
    );
    add_item(
        &mut listbox_ir,
        "other-option",
        Role::Option,
        "Other",
        Some(true),
        false,
        false,
    );
    add_semantics(
        &mut listbox_ir,
        "listbox",
        Semantics {
            role: Role::ListBox,
            ..Default::default()
        },
        vec![
            id("disabled-option"),
            id("enabled-option"),
            id("other-option"),
        ],
    );
    finish_root(&mut listbox_ir, vec![id("listbox")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("other-option")));
    key(&mut runtime, &listbox_ir, KeyCode::Char('g'));
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("enabled-option")),
        "listbox typeahead skips disabled matching options"
    );
}

#[test]
fn expanded_popup_controller_enters_selected_or_directional_boundary_item() {
    let mut selected_ir = CoreIR::default();
    add_item(
        &mut selected_ir,
        "option-a",
        Role::Option,
        "Alpha",
        Some(false),
        false,
        false,
    );
    add_item(
        &mut selected_ir,
        "option-b",
        Role::Option,
        "Beta",
        Some(true),
        false,
        false,
    );
    add_semantics(
        &mut selected_ir,
        "listbox",
        Semantics {
            role: Role::ListBox,
            ..Default::default()
        },
        vec![id("option-a"), id("option-b")],
    );
    add_semantics(
        &mut selected_ir,
        "controller",
        Semantics {
            role: Role::ComboBox,
            focusable: true,
            expanded: Some(true),
            has_popup: Some(PopupKind::ListBox),
            controls: vec![id("listbox")],
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut selected_ir, vec![id("controller"), id("listbox")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("controller")));
    key(&mut runtime, &selected_ir, KeyCode::Down);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("option-b")),
        "the selected option is the preferred entry target"
    );

    let mut menu_ir = CoreIR::default();
    add_item(
        &mut menu_ir,
        "menu-a",
        Role::MenuItem,
        "Alpha",
        None,
        false,
        false,
    );
    add_item(
        &mut menu_ir,
        "menu-b",
        Role::MenuItem,
        "Beta",
        None,
        false,
        false,
    );
    add_semantics(
        &mut menu_ir,
        "menu",
        Semantics {
            role: Role::Menu,
            ..Default::default()
        },
        vec![id("menu-a"), id("menu-b")],
    );
    add_semantics(
        &mut menu_ir,
        "controller",
        Semantics {
            role: Role::Button,
            focusable: true,
            expanded: Some(true),
            has_popup: Some(PopupKind::Menu),
            controls: vec![id("menu")],
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut menu_ir, vec![id("controller"), id("menu")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("controller")));
    key(&mut runtime, &menu_ir, KeyCode::Up);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("menu-b")),
        "Up enters the last item when no selected item exists"
    );
}

#[test]
fn vertical_arrow_requests_open_on_a_collapsed_popup_controller() {
    let mut ir = CoreIR::default();
    add_semantics(
        &mut ir,
        "controller",
        Semantics {
            role: Role::ComboBox,
            focusable: true,
            expanded: Some(false),
            has_popup: Some(PopupKind::ListBox),
            controls: vec![id("listbox")],
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: 99,
                    payload_data: Some(Vec::new()),
                }],
            },
            ..Default::default()
        },
        Vec::new(),
    );
    add_item(
        &mut ir,
        "outside",
        Role::Button,
        "Outside",
        None,
        false,
        true,
    );
    finish_root(&mut ir, vec![id("controller"), id("outside")]);

    for key_code in [KeyCode::Down, KeyCode::Up] {
        let mut runtime = dismiss_runtime();
        runtime
            .runtime_state
            .interaction
            .set_focused(Some(id("controller")));
        key(&mut runtime, &ir, key_code);
        assert_eq!(runtime.get_global_state::<DismissState>().unwrap().count, 1);
        assert_eq!(
            runtime.runtime_state.interaction.focused,
            Some(id("controller")),
            "opening is controlled; focus stays on the trigger until the rebuilt popup exists"
        );
    }
}

#[derive(Debug, Default)]
struct DismissState {
    count: usize,
}

impl GlobalState for DismissState {}

fn dismiss_actions() -> ActionSet {
    ActionSet {
        entries: vec![ActionEntry {
            trigger: ActionTrigger::Dismiss,
            action_id: 99,
            payload_data: Some(Vec::new()),
        }],
    }
}

fn dismiss_runtime() -> Runtime {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(DismissState::default()))
        .unwrap();
    runtime
        .register_reducer::<DismissState>(
            ActionId::from_u128(99),
            |state, _action: &ActionEnvelope, _target| {
                state.count += 1;
                Ok(())
            },
        )
        .unwrap();
    runtime
}

fn editable_combobox_ir() -> CoreIR {
    let mut ir = CoreIR::default();
    for (name, label, disabled) in [
        ("option-alpha", "Alpha", false),
        ("option-disabled", "Disabled", true),
        ("option-gamma", "Gamma", false),
    ] {
        add_semantics(
            &mut ir,
            name,
            Semantics {
                role: Role::Option,
                label: Some(label.into()),
                selected: Some(false),
                focusable: !disabled,
                sequential_focusable: false,
                disabled,
                actions: (!disabled)
                    .then(|| ActionSet {
                        entries: vec![ActionEntry {
                            trigger: ActionTrigger::Default,
                            action_id: 99,
                            payload_data: Some(Vec::new()),
                        }],
                    })
                    .unwrap_or_default(),
                ..Default::default()
            },
            Vec::new(),
        );
    }
    add_semantics(
        &mut ir,
        "listbox",
        Semantics {
            role: Role::ListBox,
            actions: dismiss_actions(),
            ..Default::default()
        },
        vec![
            id("option-alpha"),
            id("option-disabled"),
            id("option-gamma"),
        ],
    );
    add_semantics(
        &mut ir,
        "combobox",
        Semantics {
            role: Role::ComboBox,
            value: Some(String::new()),
            focusable: true,
            text_editable: true,
            expanded: Some(true),
            has_popup: Some(PopupKind::ListBox),
            controls: vec![id("listbox")],
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut ir, vec![id("combobox"), id("listbox")]);
    ir
}

#[test]
fn editable_combobox_navigation_retains_text_focus_and_edits_clear_the_active_option() {
    let ir = editable_combobox_ir();
    let mut runtime = dismiss_runtime();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("combobox")));

    key(&mut runtime, &ir, KeyCode::Down);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("combobox")),
        "the editable text controller retains platform focus"
    );
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        Some(id("option-alpha"))
    );
    assert!(runtime
        .runtime_state
        .interaction
        .is_focused(id("option-alpha")));
    assert_eq!(
        runtime.effective_active_descendant(&ir, id("combobox")),
        Some(id("option-alpha")),
        "runtime virtual focus must override the lowered fallback for accessibility and reveal"
    );

    key(&mut runtime, &ir, KeyCode::Down);
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        Some(id("option-gamma")),
        "disabled options are skipped"
    );
    key(&mut runtime, &ir, KeyCode::Down);
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        Some(id("option-alpha")),
        "virtual option navigation wraps"
    );

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('x'),
                modifiers: 0,
                text: "x".into(),
            }),
            &ir,
            &layout(),
        )
        .unwrap();
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("combobox"))
    );
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        None,
        "editing invalidates the option active for the previous query"
    );
    assert_eq!(
        runtime
            .runtime_state
            .text_edit
            .get(id("combobox"))
            .unwrap()
            .committed_text(),
        "x",
        "typed text edits the combobox instead of running list-box typeahead"
    );
}

#[test]
fn editable_combobox_enter_activates_the_virtual_option_and_escape_clears_it() {
    let ir = editable_combobox_ir();
    let mut runtime = dismiss_runtime();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("combobox")));

    key(&mut runtime, &ir, KeyCode::Down);
    key(&mut runtime, &ir, KeyCode::Enter);
    assert_eq!(runtime.get_global_state::<DismissState>().unwrap().count, 1);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("combobox"))
    );
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        Some(id("option-alpha")),
        "controlled activation does not invent a close or selection mutation"
    );

    key(&mut runtime, &ir, KeyCode::Escape);
    assert_eq!(runtime.get_global_state::<DismissState>().unwrap().count, 2);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("combobox"))
    );
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        None
    );
}

#[test]
fn editable_combobox_reconciliation_drops_removed_and_closed_active_options() {
    let ir = editable_combobox_ir();
    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("combobox")));
    key(&mut runtime, &ir, KeyCode::Up);
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        Some(id("option-gamma"))
    );

    let mut removed_ir = ir.clone();
    removed_ir.nodes.remove(&id("option-gamma"));
    removed_ir
        .nodes
        .get_mut(&id("listbox"))
        .unwrap()
        .children
        .retain(|candidate| *candidate != id("option-gamma"));
    assert!(!runtime.reconcile_focus(&removed_ir).unwrap());
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        None
    );

    key(&mut runtime, &ir, KeyCode::Up);
    let mut closed_ir = ir.clone();
    let Op::Semantics(controller) = &mut closed_ir.nodes.get_mut(&id("combobox")).unwrap().op
    else {
        unreachable!();
    };
    controller.expanded = Some(false);
    closed_ir.nodes.remove(&id("listbox"));
    closed_ir
        .nodes
        .get_mut(&id("root"))
        .unwrap()
        .children
        .retain(|candidate| *candidate != id("listbox"));
    assert!(!runtime.reconcile_focus(&closed_ir).unwrap());
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(id("combobox")),
        None
    );
}

#[test]
fn escape_restores_a_controlled_popup_to_its_controller_before_dismissal() {
    let mut open_ir = CoreIR::default();
    add_item(
        &mut open_ir,
        "item",
        Role::MenuItem,
        "Command",
        None,
        false,
        false,
    );
    add_semantics(
        &mut open_ir,
        "menu",
        Semantics {
            role: Role::Menu,
            actions: dismiss_actions(),
            ..Default::default()
        },
        vec![id("item")],
    );
    add_semantics(
        &mut open_ir,
        "controller",
        Semantics {
            role: Role::Button,
            focusable: true,
            expanded: Some(true),
            has_popup: Some(PopupKind::Menu),
            controls: vec![id("menu")],
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut open_ir, vec![id("controller"), id("menu")]);

    let mut runtime = dismiss_runtime();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("item")));
    key(&mut runtime, &open_ir, KeyCode::Escape);

    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("controller"))
    );
    assert_eq!(runtime.get_global_state::<DismissState>().unwrap().count, 1);

    let mut closed_ir = CoreIR::default();
    add_semantics(
        &mut closed_ir,
        "controller",
        Semantics {
            role: Role::Button,
            focusable: true,
            expanded: Some(false),
            has_popup: Some(PopupKind::Menu),
            controls: vec![id("menu")],
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut closed_ir, vec![id("controller")]);
    assert!(!runtime.reconcile_focus(&closed_ir).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("controller"))
    );
}

#[test]
fn tab_dismisses_a_controlled_popup_and_moves_relative_to_its_controller() {
    let mut ir = CoreIR::default();
    add_item(
        &mut ir,
        "previous",
        Role::Button,
        "Previous",
        None,
        false,
        true,
    );
    add_item(
        &mut ir,
        "popup-item",
        Role::MenuItem,
        "Command",
        None,
        false,
        false,
    );
    add_semantics(
        &mut ir,
        "menu",
        Semantics {
            role: Role::Menu,
            actions: dismiss_actions(),
            is_focus_scope: true,
            is_focus_barrier: true,
            ..Default::default()
        },
        vec![id("popup-item")],
    );
    add_semantics(
        &mut ir,
        "controller",
        Semantics {
            role: Role::Button,
            focusable: true,
            expanded: Some(true),
            has_popup: Some(PopupKind::Menu),
            controls: vec![id("menu")],
            ..Default::default()
        },
        Vec::new(),
    );
    add_item(&mut ir, "next", Role::Button, "Next", None, false, true);
    finish_root(
        &mut ir,
        vec![id("previous"), id("controller"), id("menu"), id("next")],
    );

    let mut forward = dismiss_runtime();
    forward
        .runtime_state
        .interaction
        .set_focused(Some(id("controller")));
    assert!(forward.reconcile_focus(&ir).unwrap());
    assert_eq!(
        forward.runtime_state.interaction.focused,
        Some(id("popup-item"))
    );
    key(&mut forward, &ir, KeyCode::Tab);
    assert_eq!(forward.runtime_state.interaction.focused, Some(id("next")));
    assert_eq!(forward.get_global_state::<DismissState>().unwrap().count, 1);

    let mut reverse = dismiss_runtime();
    reverse
        .runtime_state
        .interaction
        .set_focused(Some(id("controller")));
    assert!(reverse.reconcile_focus(&ir).unwrap());
    reverse
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Tab,
                modifiers: crate::event::MOD_SHIFT,
            }),
            &ir,
            &layout(),
        )
        .unwrap();
    assert_eq!(
        reverse.runtime_state.interaction.focused,
        Some(id("previous"))
    );
    assert_eq!(reverse.get_global_state::<DismissState>().unwrap().count, 1);

    let mut closed_ir = CoreIR::default();
    add_item(
        &mut closed_ir,
        "previous",
        Role::Button,
        "Previous",
        None,
        false,
        true,
    );
    add_semantics(
        &mut closed_ir,
        "controller",
        Semantics {
            role: Role::Button,
            focusable: true,
            expanded: Some(false),
            has_popup: Some(PopupKind::Menu),
            controls: vec![id("menu")],
            ..Default::default()
        },
        Vec::new(),
    );
    add_item(
        &mut closed_ir,
        "next",
        Role::Button,
        "Next",
        None,
        false,
        true,
    );
    finish_root(
        &mut closed_ir,
        vec![id("previous"), id("controller"), id("next")],
    );

    assert!(!forward.reconcile_focus(&closed_ir).unwrap());
    assert_eq!(forward.runtime_state.interaction.focused, Some(id("next")));
    assert!(!reverse.reconcile_focus(&closed_ir).unwrap());
    assert_eq!(
        reverse.runtime_state.interaction.focused,
        Some(id("previous"))
    );
}

#[test]
fn tab_dismisses_an_editable_combobox_popup_while_focus_is_on_its_controller() {
    let mut ir = CoreIR::default();
    add_semantics(
        &mut ir,
        "combobox",
        Semantics {
            role: Role::ComboBox,
            focusable: true,
            text_editable: true,
            expanded: Some(true),
            has_popup: Some(PopupKind::ListBox),
            controls: vec![id("listbox")],
            ..Default::default()
        },
        Vec::new(),
    );
    add_item(
        &mut ir,
        "option",
        Role::Option,
        "Result",
        Some(false),
        false,
        false,
    );
    add_semantics(
        &mut ir,
        "listbox",
        Semantics {
            role: Role::ListBox,
            actions: dismiss_actions(),
            ..Default::default()
        },
        vec![id("option")],
    );
    add_item(&mut ir, "next", Role::Button, "Next", None, false, true);
    finish_root(&mut ir, vec![id("combobox"), id("listbox"), id("next")]);

    let mut runtime = dismiss_runtime();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("combobox")));
    key(&mut runtime, &ir, KeyCode::Tab);

    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("next")));
    assert_eq!(runtime.get_global_state::<DismissState>().unwrap().count, 1);
}

#[test]
fn escape_leaves_modal_focus_restoration_to_the_focus_barrier() {
    let mut open_ir = CoreIR::default();
    add_item(
        &mut open_ir,
        "background",
        Role::Button,
        "Background",
        None,
        false,
        true,
    );
    add_item(
        &mut open_ir,
        "dialog-action",
        Role::Button,
        "Confirm",
        None,
        false,
        true,
    );
    add_semantics(
        &mut open_ir,
        "dialog-scope",
        Semantics {
            role: Role::Dialog,
            actions: dismiss_actions(),
            is_focus_scope: true,
            is_focus_barrier: true,
            ..Default::default()
        },
        vec![id("dialog-action")],
    );
    finish_root(&mut open_ir, vec![id("background"), id("dialog-scope")]);

    let mut runtime = dismiss_runtime();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("background")));
    assert!(runtime.reconcile_focus(&open_ir).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("dialog-action"))
    );

    key(&mut runtime, &open_ir, KeyCode::Escape);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("dialog-action")),
        "modal focus remains trapped until the controlled tree closes"
    );
    assert_eq!(runtime.get_global_state::<DismissState>().unwrap().count, 1);

    let mut closed_ir = CoreIR::default();
    add_item(
        &mut closed_ir,
        "background",
        Role::Button,
        "Background",
        None,
        false,
        true,
    );
    finish_root(&mut closed_ir, vec![id("background")]);
    assert!(runtime.reconcile_focus(&closed_ir).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("background"))
    );
}

#[test]
fn focus_barrier_enters_a_roving_popup_without_adding_items_to_tab_order() {
    let mut ir = CoreIR::default();
    add_item(
        &mut ir,
        "controller",
        Role::Button,
        "Commands",
        None,
        false,
        true,
    );
    add_item(
        &mut ir,
        "first-item",
        Role::Option,
        "First",
        Some(false),
        false,
        false,
    );
    add_item(
        &mut ir,
        "second-item",
        Role::Option,
        "Second",
        Some(true),
        false,
        false,
    );
    add_semantics(
        &mut ir,
        "menu",
        Semantics {
            role: Role::ListBox,
            is_focus_scope: true,
            is_focus_barrier: true,
            ..Default::default()
        },
        vec![id("first-item"), id("second-item")],
    );
    finish_root(&mut ir, vec![id("controller"), id("menu")]);

    assert_eq!(get_all_focusable_nodes(&ir), vec![id("controller")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("controller")));
    assert!(runtime.reconcile_focus(&ir).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("second-item")),
        "a selected option is the preferred barrier entry target"
    );
}

#[test]
fn tree_navigation_moves_between_rows_and_supports_typeahead() {
    let mut ir = CoreIR::default();
    for (name, label) in [("src", "src"), ("assets", "assets"), ("readme", "readme")] {
        add_item(
            &mut ir,
            name,
            Role::TreeItem,
            label,
            Some(false),
            false,
            false,
        );
    }
    add_semantics(
        &mut ir,
        "tree",
        Semantics {
            role: Role::Tree,
            focusable: false,
            ..Default::default()
        },
        vec![id("src"), id("assets"), id("readme")],
    );
    finish_root(&mut ir, vec![id("tree")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("src")));

    key(&mut runtime, &ir, KeyCode::Down);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("assets"))
    );
    key(&mut runtime, &ir, KeyCode::End);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("readme"))
    );
    key(&mut runtime, &ir, KeyCode::Home);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("src")));

    // A tree is a list of names, so typing jumps like a menu does.
    key(&mut runtime, &ir, KeyCode::Char('a'));
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("assets"))
    );
}

#[test]
fn toolbar_navigation_is_horizontal_without_typeahead() {
    let mut ir = CoreIR::default();
    for (name, label) in [("bold", "Bold"), ("italic", "Italic")] {
        add_item(&mut ir, name, Role::Button, label, None, false, false);
    }
    add_semantics(
        &mut ir,
        "toolbar",
        Semantics {
            role: Role::Toolbar,
            focusable: false,
            ..Default::default()
        },
        vec![id("bold"), id("italic")],
    );
    finish_root(&mut ir, vec![id("toolbar")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("bold")));

    key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("italic"))
    );
    key(&mut runtime, &ir, KeyCode::Down);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("italic")),
        "a cross-axis arrow must not move focus in a horizontal toolbar"
    );

    // Typing belongs to whatever the toolbar acts on, not the toolbar.
    key(&mut runtime, &ir, KeyCode::Char('b'));
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("italic")),
        "a toolbar must not steal characters for typeahead"
    );
}
