use crate::action::{Action, ActionEnvelope, ActionId, GlobalState};
use crate::event::ImeEvent;
use crate::hit_test::get_all_focusable_nodes;
use crate::{
    reduce_with, ActionRegistry, InputEvent, KeyCode, KeyEvent, ReducerContext, Runtime,
    TextEntryInput, TextEntryPhase,
};
use fission_ir::{
    ActionEntry, ActionSet, ActionTrigger, CoreIR, KeyAction, KeyBinding, LayoutDirection, Op,
    PopupKind, Role, SemanticOrientation, Semantics, StructuralOp, WidgetId,
};
use fission_layout::{LayoutSize, LayoutSnapshot};
use serde::{Deserialize, Serialize};

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

#[test]
fn declared_key_action_claims_down_with_text_before_text_insertion() {
    let mut ir = CoreIR::default();
    let key_action = StartInlineEdit;
    add_semantics(
        &mut ir,
        "command",
        Semantics {
            role: Role::Button,
            focusable: true,
            key_actions: vec![KeyAction {
                binding: KeyBinding::new(KeyCode::Char('x')),
                action: ActionEntry {
                    trigger: ActionTrigger::Key,
                    action_id: StartInlineEdit::static_id().as_u128(),
                    payload_data: Some(key_action.encode()),
                },
            }],
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut ir, vec![id("command")]);

    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(KeyActionState::default()))
        .unwrap();
    let mut registry = ActionRegistry::new();
    registry.register(reduce_with!(on_start_inline_edit));
    runtime.absorb_registry(registry);
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("command")));
    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('x'),
                modifiers: 0,
                text: "x\u{301}".into(),
            }),
            &ir,
            &layout(),
        )
        .unwrap();

    assert_eq!(
        runtime
            .get_global_state::<KeyActionState>()
            .unwrap()
            .initial_text
            .as_deref(),
        Some("x\u{301}")
    );
}

#[test]
fn focused_nonediting_control_receives_complete_unicode_and_ime_events() {
    let action = ReceiveTextEntry;
    let mut ir = CoreIR::default();
    add_semantics(
        &mut ir,
        "cell",
        Semantics {
            role: Role::TableCell,
            focusable: true,
            key_actions: vec![KeyAction {
                binding: KeyBinding::new(KeyCode::Char('x')),
                action: ActionEntry {
                    trigger: ActionTrigger::Key,
                    action_id: ReceiveTextEntry::static_id().as_u128(),
                    payload_data: Some(action.encode()),
                },
            }],
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::TextEntry,
                    action_id: ReceiveTextEntry::static_id().as_u128(),
                    payload_data: Some(action.encode()),
                }],
            },
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut ir, vec![id("cell")]);

    let mut runtime = text_entry_runtime();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("cell")));
    let committed = ["é", "e\u{301}", "wide text", "👩‍💻"];
    for text in committed {
        runtime
            .handle_input(
                InputEvent::Keyboard(KeyEvent::DownWithText {
                    key_code: KeyCode::Char('a'),
                    modifiers: 0,
                    text: text.into(),
                }),
                &ir,
                &layout(),
            )
            .unwrap();
    }
    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Space,
                modifiers: 0,
            }),
            &ir,
            &layout(),
        )
        .unwrap();

    for event in [
        ImeEvent::Preedit {
            text: "に".into(),
            cursor: Some((0, 3)),
        },
        ImeEvent::Preedit {
            text: "日本".into(),
            cursor: Some((0, 6)),
        },
        ImeEvent::Commit {
            text: "日本語".into(),
        },
        ImeEvent::Preedit {
            text: "cancel me".into(),
            cursor: None,
        },
        ImeEvent::Cancel,
    ] {
        runtime
            .handle_input(InputEvent::Ime(event), &ir, &layout())
            .unwrap();
    }

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
    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('x'),
                modifiers: crate::event::MOD_CTRL,
                text: "x".into(),
            }),
            &ir,
            &layout(),
        )
        .unwrap();
    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Enter,
                modifiers: 0,
                text: "\n".into(),
            }),
            &ir,
            &layout(),
        )
        .unwrap();

    let received = &runtime
        .get_global_state::<TextEntryState>()
        .unwrap()
        .received;
    let mut expected: Vec<_> = committed
        .into_iter()
        .map(|text| TextEntryInput {
            text: text.into(),
            source: crate::TextEditSource::Keyboard,
            phase: TextEntryPhase::Committed,
            cursor: None,
        })
        .collect();
    expected.push(TextEntryInput {
        text: " ".into(),
        source: crate::TextEditSource::Keyboard,
        phase: TextEntryPhase::Committed,
        cursor: None,
    });
    expected.extend([
        TextEntryInput {
            text: "に".into(),
            source: crate::TextEditSource::Ime,
            phase: TextEntryPhase::CompositionPreedit,
            cursor: Some((0, 3)),
        },
        TextEntryInput {
            text: "日本".into(),
            source: crate::TextEditSource::Ime,
            phase: TextEntryPhase::CompositionPreedit,
            cursor: Some((0, 6)),
        },
        TextEntryInput {
            text: "日本語".into(),
            source: crate::TextEditSource::Ime,
            phase: TextEntryPhase::CompositionCommitted,
            cursor: None,
        },
        TextEntryInput {
            text: "cancel me".into(),
            source: crate::TextEditSource::Ime,
            phase: TextEntryPhase::CompositionPreedit,
            cursor: None,
        },
        TextEntryInput {
            text: String::new(),
            source: crate::TextEditSource::Ime,
            phase: TextEntryPhase::CompositionCancelled,
            cursor: None,
        },
    ]);
    assert_eq!(received, &expected);
}

#[test]
fn text_entry_skips_disabled_and_inert_semantics_and_preserves_text_input() {
    let action = ReceiveTextEntry;
    let mut disabled_ir = CoreIR::default();
    add_semantics(
        &mut disabled_ir,
        "disabled-cell",
        Semantics {
            role: Role::TableCell,
            focusable: true,
            disabled: true,
            actions: text_entry_actions(&action),
            ..Default::default()
        },
        vec![id("enabled-child")],
    );
    add_semantics(
        &mut disabled_ir,
        "enabled-child",
        Semantics {
            role: Role::Generic,
            focusable: true,
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut disabled_ir, vec![id("disabled-cell")]);

    let mut disabled_runtime = text_entry_runtime();
    disabled_runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("enabled-child")));
    disabled_runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('a'),
                modifiers: 0,
                text: "é".into(),
            }),
            &disabled_ir,
            &layout(),
        )
        .unwrap();
    disabled_runtime
        .handle_input(
            InputEvent::Ime(ImeEvent::Commit { text: "字".into() }),
            &disabled_ir,
            &layout(),
        )
        .unwrap();
    assert!(disabled_runtime
        .get_global_state::<TextEntryState>()
        .unwrap()
        .received
        .is_empty());

    let mut inert_ir = CoreIR::default();
    add_semantics(
        &mut inert_ir,
        "inert-cell",
        Semantics {
            role: Role::TableCell,
            focusable: true,
            actions: text_entry_actions(&action),
            ..Default::default()
        },
        Vec::new(),
    );
    inert_ir.add_node(
        id("inert-boundary"),
        Op::Structural(StructuralOp::InteractionInert { stable_hash: 2 }),
        vec![id("inert-cell")],
    );
    finish_root(&mut inert_ir, vec![id("inert-boundary")]);
    let mut inert_runtime = text_entry_runtime();
    inert_runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("inert-cell")));
    inert_runtime
        .handle_input(
            InputEvent::Ime(ImeEvent::Preedit {
                text: "hidden".into(),
                cursor: None,
            }),
            &inert_ir,
            &layout(),
        )
        .unwrap();
    inert_runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('a'),
                modifiers: 0,
                text: "é".into(),
            }),
            &inert_ir,
            &layout(),
        )
        .unwrap();
    assert!(inert_runtime
        .get_global_state::<TextEntryState>()
        .unwrap()
        .received
        .is_empty());

    let mut text_input_ir = CoreIR::default();
    add_semantics(
        &mut text_input_ir,
        "text-input",
        Semantics {
            role: Role::TextInput,
            focusable: true,
            value: Some(String::new()),
            actions: text_entry_actions(&action),
            ..Default::default()
        },
        Vec::new(),
    );
    finish_root(&mut text_input_ir, vec![id("text-input")]);
    let mut text_input_runtime = text_entry_runtime();
    text_input_runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("text-input")));
    text_input_runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('a'),
                modifiers: 0,
                text: "x".into(),
            }),
            &text_input_ir,
            &layout(),
        )
        .unwrap();
    text_input_runtime
        .handle_input(
            InputEvent::Ime(ImeEvent::Preedit {
                text: "あ".into(),
                cursor: Some((0, 3)),
            }),
            &text_input_ir,
            &layout(),
        )
        .unwrap();
    assert!(text_input_runtime
        .get_global_state::<TextEntryState>()
        .unwrap()
        .received
        .is_empty());
    assert_eq!(
        text_input_runtime
            .runtime_state
            .text_edit
            .get(id("text-input"))
            .unwrap()
            .committed_text(),
        "x"
    );
}

fn text_entry_actions(action: &ReceiveTextEntry) -> ActionSet {
    ActionSet {
        entries: vec![ActionEntry {
            trigger: ActionTrigger::TextEntry,
            action_id: ReceiveTextEntry::static_id().as_u128(),
            payload_data: Some(action.encode()),
        }],
    }
}

#[derive(Debug, Default)]
struct TextEntryState {
    received: Vec<TextEntryInput>,
}

impl GlobalState for TextEntryState {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReceiveTextEntry;

impl crate::Action for ReceiveTextEntry {
    fn static_id() -> crate::ActionId {
        crate::ActionId::from_name("fission_core::composite_keyboard_test::ReceiveTextEntry")
    }
}

fn on_receive_text_entry<'a, 'b, 'c>(
    state: &mut TextEntryState,
    _action: ReceiveTextEntry,
    ctx: &mut ReducerContext<'a, 'b, 'c, TextEntryState>,
) {
    if let Some(input) = ctx.input.text_entry() {
        state.received.push(input.clone());
    }
}

fn text_entry_runtime() -> Runtime {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(TextEntryState::default()))
        .unwrap();
    let mut registry = ActionRegistry::new();
    registry.register(reduce_with!(on_receive_text_entry));
    runtime.absorb_registry(registry);
    runtime
}

#[derive(Debug, Default)]
struct KeyActionState {
    initial_text: Option<String>,
    modifiers: Vec<u8>,
}

impl GlobalState for KeyActionState {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StartInlineEdit;

impl Action for StartInlineEdit {
    fn static_id() -> ActionId {
        ActionId::from_name("fission_core::composite_keyboard_test::StartInlineEdit")
    }
}

fn on_start_inline_edit<'a, 'b, 'c>(
    state: &mut KeyActionState,
    _action: StartInlineEdit,
    ctx: &mut ReducerContext<'a, 'b, 'c, KeyActionState>,
) {
    if let Some(press) = ctx.input.key_press() {
        state.initial_text = press.text.clone();
        state.modifiers.push(press.modifiers);
    }
}

#[test]
fn focused_text_input_can_handle_tab_and_shift_tab_explicitly() {
    let mut ir = CoreIR::default();
    add_item(&mut ir, "before", Role::Button, "Before", None, false, true);
    add_semantics(
        &mut ir,
        "editor",
        Semantics {
            role: Role::TextInput,
            focusable: true,
            text_editable: true,
            key_actions: vec![
                KeyAction::new(KeyCode::Tab, StartInlineEdit::static_id().as_u128()),
                KeyAction::with_modifiers(
                    KeyCode::Tab,
                    crate::event::MOD_SHIFT,
                    StartInlineEdit::static_id().as_u128(),
                ),
            ],
            ..Default::default()
        },
        Vec::new(),
    );
    add_item(&mut ir, "after", Role::Button, "After", None, false, true);
    finish_root(&mut ir, vec![id("before"), id("editor"), id("after")]);

    let mut runtime = Runtime::default().with_global_state(KeyActionState::default());
    let mut registry = ActionRegistry::new();
    registry.register(reduce_with!(on_start_inline_edit));
    runtime.absorb_registry(registry);
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("editor")));

    key(&mut runtime, &ir, KeyCode::Tab);
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(id("editor"))
    );
    runtime
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
        runtime.runtime_state.interaction.focused,
        Some(id("editor"))
    );
    assert_eq!(
        runtime
            .get_global_state::<KeyActionState>()
            .unwrap()
            .modifiers,
        vec![0, crate::event::MOD_SHIFT]
    );
}

#[test]
fn text_input_without_a_tab_handler_uses_normal_focus_traversal() {
    let mut ir = CoreIR::default();
    add_semantics(
        &mut ir,
        "editor",
        Semantics {
            role: Role::TextInput,
            focusable: true,
            text_editable: true,
            ..Default::default()
        },
        Vec::new(),
    );
    add_item(&mut ir, "next", Role::Button, "Next", None, false, true);
    finish_root(&mut ir, vec![id("editor"), id("next")]);

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(id("editor")));

    key(&mut runtime, &ir, KeyCode::Tab);

    assert_eq!(runtime.runtime_state.interaction.focused, Some(id("next")));
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
