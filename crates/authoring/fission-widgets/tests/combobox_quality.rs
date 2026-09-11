use fission_core::authoring::{BuildCtx, LoweringCx};
use fission_core::ui::{Overlay, TextInput, ZStack};
use fission_core::{
    build, ActionEnvelope, ActionId, Env, GlobalState, InputEvent, KeyCode, KeyEvent, Runtime,
    View, Widget, WidgetId,
};
use fission_ir::{
    ActionTrigger, CoreIR, LayoutOp, Op, PopupKind, Role, SemanticOrientation, Semantics,
};
use fission_layout::{LayoutSize, LayoutSnapshot};
use fission_widgets::{
    Combobox, ComboboxContent, ComboboxEntry, ComboboxInput, ComboboxLayout, ComboboxOption,
};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: b"null".to_vec(),
    }
}

fn build_widget(build: impl FnOnce() -> Widget) -> (CoreIR, Vec<CoreIR>) {
    let state = State;
    let runtime = fission_core::RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let widget = build::enter(&mut ctx, &view, build);
    let root_ir = fission_core::internal::lower_widget_to_ir(&widget);
    let portal_irs = ctx
        .take_portals()
        .into_iter()
        .map(|(_, portal)| fission_core::internal::lower_widget_to_ir(&portal))
        .collect();
    (root_ir, portal_irs)
}

fn build_runtime_widget(runtime: &Runtime, build: impl FnOnce() -> Widget) -> CoreIR {
    let state = State;
    let env = Env::default();
    let view = View::new(&state, &runtime.runtime_state, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let content = build::enter(&mut ctx, &view, build);
    let portals = ctx
        .take_portals()
        .into_iter()
        .map(|(_, portal)| portal)
        .collect();
    let root: Widget = Overlay {
        id: None,
        content,
        overlay: ZStack {
            id: None,
            children: portals,
        }
        .into(),
    }
    .into();
    let mut lower = LoweringCx::new(&env, &runtime.runtime_state, None, None);
    let root_id = fission_core::internal::lower_widget(&root, &mut lower);
    lower.ir.root = Some(root_id);
    lower.ir
}

fn runtime_combobox(combobox_id: WidgetId) -> ComboboxLayout {
    ComboboxLayout::new(
        combobox_id,
        ComboboxInput::new(""),
        ComboboxContent::new(vec![
            ComboboxEntry::Item(
                ComboboxOption::option("Alpha", false).id(WidgetId::explicit("option.alpha")),
            ),
            ComboboxEntry::Item(
                ComboboxOption::option("Disabled", false)
                    .id(WidgetId::explicit("option.disabled"))
                    .disabled(true),
            ),
            ComboboxEntry::Item(
                ComboboxOption::option("Gamma", false).id(WidgetId::explicit("option.gamma")),
            ),
        ]),
    )
    .open(true)
}

fn semantics_at(ir: &CoreIR, id: WidgetId) -> &Semantics {
    match &ir.nodes[&id].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected semantics at {id:?}, got {op:?}"),
    }
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

#[test]
fn closed_combobox_exposes_editable_popup_semantics_and_opens_on_focus() {
    let combobox_id = WidgetId::explicit("assignee");
    let input_id = WidgetId::derived(combobox_id.as_u128(), &[0, 1]);
    let popup_id = WidgetId::derived(combobox_id.as_u128(), &[1]);
    let on_input = action("assignee.input");
    let on_toggle = action("assignee.toggle");
    let (ir, portals) = build_widget(|| {
        Combobox {
            id: combobox_id,
            semantics_identifier: None,
            value: "Av".into(),
            items: vec!["Avery".into(), "Ava".into()],
            is_open: false,
            width: Some(260.0),
            max_popup_height: Some(160.0),
            on_input: Some(on_input.clone()),
            on_select: None,
            on_toggle: Some(on_toggle.clone()),
        }
        .into()
    });

    assert!(portals.is_empty());
    let input = semantics_at(&ir, input_id);
    assert_eq!(input.role, Role::ComboBox);
    assert_eq!(input.value.as_deref(), Some("Av"));
    assert_eq!(input.expanded, Some(false));
    assert_eq!(input.has_popup, Some(PopupKind::ListBox));
    assert_eq!(input.controls, vec![popup_id]);
    assert_eq!(input.active_descendant, None);
    assert!(input.focusable);
    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Layout(LayoutOp::Box {
            width: Some(16.0),
            height: Some(16.0),
            ..
        })
    )));

    let text_change = input
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::TextChanged)
        .expect("editable combobox should preserve text input action");
    assert_eq!(text_change.action_id, on_input.id.as_u128());
    let focus = input
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Focus)
        .expect("closed combobox should open when its input receives focus");
    assert_eq!(focus.action_id, on_toggle.id.as_u128());
}

#[test]
fn explicit_input_focus_action_remains_authoritative() {
    let combobox_id = WidgetId::explicit("custom-focus");
    let input_id = WidgetId::derived(combobox_id.as_u128(), &[0, 1]);
    let focus_action = action("custom-focus.field-focused");
    let (ir, portals) = build_widget(|| {
        ComboboxLayout::new(
            combobox_id,
            ComboboxInput::custom(TextInput {
                on_focus: Some(focus_action.clone()),
                ..Default::default()
            }),
            ComboboxContent::new(vec![ComboboxEntry::Item(ComboboxOption::option(
                "Avery", false,
            ))]),
        )
        .on_toggle(action("custom-focus.toggle"))
        .into()
    });

    assert!(portals.is_empty());
    let focus = semantics_at(&ir, input_id)
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Focus)
        .expect("explicit focus action");
    assert_eq!(focus.action_id, focus_action.id.as_u128());
}

#[test]
fn combobox_reuses_text_input_and_select_content_visual_recipes() {
    let combobox_id = WidgetId::explicit("shared-recipes");
    let input_id = WidgetId::explicit("shared-recipes.input");
    let popup_id = WidgetId::derived(combobox_id.as_u128(), &[1]);
    let option_id = WidgetId::explicit("shared-recipes.option");
    let input = TextInput {
        id: Some(input_id),
        value: "Avery".into(),
        width: Some(260.0),
        ..Default::default()
    };
    let content = ComboboxContent::new(vec![ComboboxEntry::Item(
        ComboboxOption::option("Avery", true).id(option_id),
    )])
    .width(260.0);

    let (standalone_input, _) = build_widget(|| input.clone().into());
    let (standalone_content, _) =
        build_widget(|| build::with_widget_id(popup_id, || content.clone().into()));
    let (combobox, portals) = build_widget(|| {
        ComboboxLayout::new(
            combobox_id,
            ComboboxInput::custom(input).show_indicator(false),
            content,
        )
        .open(true)
        .into()
    });

    let standalone_input_layout = standalone_input.nodes[&input_id].children[0];
    let combobox_input_layout = combobox.nodes[&input_id].children[0];
    assert_eq!(
        combobox.nodes[&combobox_input_layout].op,
        standalone_input.nodes[&standalone_input_layout].op,
        "combobox input geometry must continue to come from TextInput lowering"
    );

    let popup = &portals[0];
    let standalone_surface = standalone_content.nodes[&popup_id].children[0];
    let combobox_surface = popup.nodes[&popup_id].children[0];
    assert_eq!(
        popup.nodes[&combobox_surface].op, standalone_content.nodes[&standalone_surface].op,
        "combobox popup geometry must continue to come from SelectContent/MenuContent lowering"
    );
    let standalone_option = standalone_content.nodes[&option_id].children[0];
    let combobox_option = popup.nodes[&option_id].children[0];
    assert_eq!(
        popup.nodes[&combobox_option].op, standalone_content.nodes[&standalone_option].op,
        "combobox options must continue to use the shared select/menu item recipe"
    );
}

#[test]
fn open_combobox_uses_a_bounded_listbox_with_stable_options() {
    let combobox_id = WidgetId::explicit("workspace-member");
    let input_id = WidgetId::derived(combobox_id.as_u128(), &[0, 1]);
    let popup_id = WidgetId::derived(combobox_id.as_u128(), &[1]);
    let selected_option_id = WidgetId::derived(popup_id.as_u128(), &[0x4D45_4E55, 1]);
    let on_toggle = action("workspace-member.toggle");
    let (ir, portals) = build_widget(|| {
        Combobox {
            id: combobox_id,
            semantics_identifier: None,
            value: "Avery".into(),
            items: vec!["Ava".into(), "Avery".into(), "Avril".into()],
            is_open: true,
            width: Some(260.0),
            max_popup_height: Some(72.0),
            on_input: Some(action("workspace-member.input")),
            on_select: Some(Arc::new(|value| action(&format!("select.{value}")))),
            on_toggle: Some(on_toggle.clone()),
        }
        .into()
    });

    assert_eq!(portals.len(), 1);
    let input = semantics_at(&ir, input_id);
    assert_eq!(input.role, Role::ComboBox);
    assert_eq!(input.expanded, Some(true));
    assert_eq!(input.has_popup, Some(PopupKind::ListBox));
    assert_eq!(input.controls, vec![popup_id]);
    assert_eq!(input.active_descendant, Some(selected_option_id));
    assert!(input
        .actions
        .entries
        .iter()
        .all(|entry| entry.trigger != ActionTrigger::Focus));

    let popup = &portals[0];
    assert_eq!(
        flyout_alignment_target(popup),
        Some(selected_option_id),
        "an exact, unique combobox selection should align with its trigger"
    );
    let listbox = semantics_at(popup, popup_id);
    assert_eq!(listbox.role, Role::ListBox);
    assert_eq!(listbox.orientation, Some(SemanticOrientation::Vertical));
    let dismiss = listbox
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Dismiss)
        .expect("open listbox should expose its dismissal action");
    assert_eq!(dismiss.action_id, on_toggle.id.as_u128());

    for (index, selected) in [false, true, false].into_iter().enumerate() {
        let option_id = WidgetId::derived(popup_id.as_u128(), &[0x4D45_4E55, index as u32]);
        let option = semantics_at(popup, option_id);
        assert_eq!(option.role, Role::Option);
        assert_eq!(option.selected, Some(selected));
        assert!(option.focusable);
        assert!(
            !option.sequential_focusable,
            "editable combobox options must not add Tab stops"
        );
        assert_eq!(option.actions.entries.len(), 1);
        assert_eq!(option.actions.entries[0].trigger, ActionTrigger::Default);
    }

    let surface = &popup.nodes[&popup.nodes[&popup_id].children[0]];
    match &surface.op {
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            use fission_ir::op::Length;

            assert_eq!(style.width, Some(Length::Points(260.0)));
            assert_eq!(style.height, Some(Length::Points(72.0)));
        }
        op => panic!("expected bounded combobox popup surface, got {op:?}"),
    }
    let scroll = surface
        .children
        .iter()
        .find_map(|id| match &popup.nodes[id].op {
            Op::Layout(LayoutOp::Scroll {
                height,
                show_scrollbar,
                ..
            }) => Some((*height, *show_scrollbar)),
            _ => None,
        })
        .expect("combobox popup should contain a scroll viewport");
    assert_eq!(scroll, (Some(64.0), true));
}

#[test]
fn combobox_keeps_side_placement_when_a_matching_label_is_ambiguous() {
    let combobox_id = WidgetId::explicit("ambiguous-member");
    let (_, portals) = build_widget(|| {
        Combobox {
            id: combobox_id,
            semantics_identifier: None,
            value: "Avery".into(),
            items: vec!["Avery".into(), "Avery".into()],
            is_open: true,
            width: Some(260.0),
            max_popup_height: None,
            on_input: None,
            on_select: None,
            on_toggle: None,
        }
        .into()
    });

    assert_eq!(portals.len(), 1);
    assert_eq!(flyout_alignment_target(&portals[0]), None);
}

#[test]
fn empty_combobox_never_claims_an_open_popup() {
    let combobox_id = WidgetId::explicit("empty-combobox");
    let input_id = WidgetId::derived(combobox_id.as_u128(), &[0, 1]);
    let (ir, portals) = build_widget(|| {
        Combobox {
            id: combobox_id,
            semantics_identifier: None,
            value: String::new(),
            items: Vec::new(),
            is_open: true,
            width: None,
            max_popup_height: None,
            on_input: None,
            on_select: None,
            on_toggle: Some(action("empty-combobox.toggle")),
        }
        .into()
    });

    assert!(portals.is_empty());
    assert_eq!(semantics_at(&ir, input_id).expanded, Some(false));
}

#[test]
fn real_combobox_runtime_keeps_editing_focus_and_projects_keyboard_navigation() {
    let combobox_id = WidgetId::explicit("runtime-combobox");
    let input_id = WidgetId::derived(combobox_id.as_u128(), &[0, 1]);
    let alpha_id = WidgetId::explicit("option.alpha");
    let gamma_id = WidgetId::explicit("option.gamma");
    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(input_id));
    let ir = build_runtime_widget(&runtime, || runtime_combobox(combobox_id).into());
    let layout = LayoutSnapshot::new(LayoutSize::new(640.0, 480.0));

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Down,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .unwrap();
    assert_eq!(runtime.runtime_state.interaction.focused, Some(input_id));
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(input_id),
        Some(alpha_id)
    );

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Down,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .unwrap();
    assert_eq!(runtime.runtime_state.interaction.focused, Some(input_id));
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(input_id),
        Some(gamma_id),
        "the disabled middle option is skipped"
    );

    let navigated = build_runtime_widget(&runtime, || runtime_combobox(combobox_id).into());
    assert_eq!(
        semantics_at(&navigated, input_id).active_descendant,
        Some(gamma_id),
        "the runtime-owned option is projected to accessibility semantics"
    );

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code: KeyCode::Char('x'),
                modifiers: 0,
                text: "x".into(),
            }),
            &navigated,
            &layout,
        )
        .unwrap();
    assert_eq!(runtime.runtime_state.interaction.focused, Some(input_id));
    assert_eq!(
        runtime
            .runtime_state
            .interaction
            .active_descendant(input_id),
        None,
        "typing clears navigation state for the previous filter"
    );
    assert_eq!(
        runtime
            .runtime_state
            .text_edit
            .get(input_id)
            .unwrap()
            .committed_text(),
        "x",
        "text after keyboard navigation edits the field instead of running option typeahead"
    );
}
