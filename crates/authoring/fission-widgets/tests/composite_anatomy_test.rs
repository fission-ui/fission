use fission_core::authoring::BuildCtx;
use fission_core::ui::{ComponentSize, Text, TextInput};
use fission_core::{build, ActionEnvelope, ActionId, Env, GlobalState, View, Widget, WidgetId};
use fission_ir::{
    ActionTrigger, CoreIR, FlyoutAlignment, FlyoutOptions, LayoutOp, Op, PopupKind, Role, Semantics,
};
use fission_widgets::{
    ComboboxContent, ComboboxEntry, ComboboxInput, ComboboxLayout, ComboboxOption,
    FormControlLayout, FormDescription, FormError, FormLabel, HStack, MenuActionItem,
    MenuButtonLayout, MenuContent, MenuTrigger, SelectContent, SelectEntry, SelectGroup,
    SelectLayout, SelectOption, SelectSeparator, SelectTrigger, TabList, TabPanel, TabTrigger,
    TabsLayout,
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

fn build_widget(build_widget: impl FnOnce() -> Widget) -> (CoreIR, Vec<CoreIR>) {
    let state = State;
    let runtime = fission_core::RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    let root = fission_core::internal::lower_widget_to_ir(&widget);
    let portals = ctx
        .take_portals()
        .into_iter()
        .map(|(_, portal)| fission_core::internal::lower_widget_to_ir(&portal))
        .collect();
    (root, portals)
}

fn semantics_at(ir: &CoreIR, id: WidgetId) -> &Semantics {
    match &ir.nodes[&id].op {
        Op::Semantics(semantics) => semantics,
        op => panic!("expected semantics at {id:?}, got {op:?}"),
    }
}

fn semantics_for_identifier<'a>(ir: &'a CoreIR, identifier: &str) -> (WidgetId, &'a Semantics) {
    let mut matches = ir.nodes.values().filter_map(|node| match &node.op {
        Op::Semantics(semantics) if semantics.identifier.as_deref() == Some(identifier) => {
            Some((node.id, semantics))
        }
        _ => None,
    });
    let result = matches.next().expect("semantic identifier");
    assert!(
        matches.next().is_none(),
        "semantic identifier must be unique"
    );
    result
}

fn semantics_for_role(ir: &CoreIR, role: Role) -> Vec<(WidgetId, &Semantics)> {
    ir.nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.role == role => Some((node.id, semantics)),
            _ => None,
        })
        .collect()
}

fn flyout_options(ir: &CoreIR) -> FlyoutOptions {
    let mut matches = ir.nodes.values().filter_map(|node| match &node.op {
        Op::Layout(LayoutOp::Flyout { options, .. }) => Some(*options),
        _ => None,
    });
    let options = matches.next().expect("flyout layout");
    assert!(matches.next().is_none(), "expected exactly one flyout");
    options
}

#[test]
fn menu_button_layout_controls_rich_content_from_a_custom_sized_trigger() {
    let menu_id = WidgetId::explicit("project.actions.layout");
    let popup_id = WidgetId::derived(menu_id.as_u128(), &[1]);
    let toggle = action("project.actions.toggle");
    let layout = MenuButtonLayout::new(
        menu_id,
        MenuTrigger::custom(
            "Project actions",
            HStack {
                children: vec![Text::new("Project").into(), Text::new("3").into()],
                ..Default::default()
            },
        )
        .size(ComponentSize::Sm)
        .semantics_identifier("project.actions.trigger"),
        MenuContent::new(vec![MenuActionItem::new("Open project")
            .description("Open in this workspace")
            .metadata("Current")
            .semantics_identifier("project.actions.open")
            .on_select(action("project.actions.open"))
            .into()]),
    )
    .open(true)
    .on_toggle(toggle.clone());
    let (root, portals) = build_widget(|| layout.into());

    assert_eq!(portals.len(), 1);
    let (trigger_id, trigger) = semantics_for_identifier(&root, "project.actions.trigger");
    assert_eq!(trigger.role, Role::Button);
    assert_eq!(trigger.label.as_deref(), Some("Project actions"));
    assert_eq!(trigger.expanded, Some(true));
    assert_eq!(trigger.has_popup, Some(PopupKind::Menu));
    assert_eq!(trigger.controls, vec![popup_id]);
    assert!(trigger.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::Default && entry.action_id == toggle.id.as_u128()
    }));
    let expected_height = {
        let env = Env::default();
        env.theme
            .components
            .menu
            .resolve_trigger(ComponentSize::Sm, fission_theme::ComponentState::Default)
            .height
            .expect("menu trigger height")
    };
    match &root.nodes[&root.nodes[&trigger_id].children[0]].op {
        Op::Layout(LayoutOp::Box { height, .. }) => {
            assert_eq!(*height, Some(expected_height));
        }
        op => panic!("expected sized menu trigger box, got {op:?}"),
    }

    let popup = &portals[0];
    let menu = semantics_at(popup, popup_id);
    assert_eq!(menu.role, Role::Menu);
    assert!(menu.is_focus_scope);
    assert!(menu.is_focus_barrier);
    assert!(menu.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::Dismiss && entry.action_id == toggle.id.as_u128()
    }));
    let (_, item) = semantics_for_identifier(popup, "project.actions.open");
    assert_eq!(item.role, Role::MenuItem);
    assert!(!item.sequential_focusable);
    assert_eq!(flyout_options(popup).alignment, FlyoutAlignment::End);
}

#[test]
fn disabled_menu_trigger_cannot_open_or_dispatch() {
    let menu_id = WidgetId::explicit("disabled.actions.layout");
    let layout = MenuButtonLayout::new(
        menu_id,
        MenuTrigger::new("Unavailable actions")
            .disabled(true)
            .semantics_identifier("disabled.actions.trigger"),
        MenuContent::new(vec![MenuActionItem::new("Unavailable").into()]),
    )
    .open(true)
    .on_toggle(action("disabled.actions.toggle"));
    let (root, portals) = build_widget(|| layout.into());

    assert!(portals.is_empty());
    let (_, trigger) = semantics_for_identifier(&root, "disabled.actions.trigger");
    assert!(trigger.disabled);
    assert!(!trigger.focusable);
    assert_eq!(trigger.expanded, Some(false));
    assert!(trigger.actions.entries.is_empty());
}

#[test]
fn select_layout_preserves_rich_grouped_option_anatomy() {
    let select_id = WidgetId::explicit("country.select.layout");
    let popup_id = WidgetId::derived(select_id.as_u128(), &[1]);
    let toggle = action("country.select.toggle");
    let select = SelectLayout::new(
        select_id,
        SelectTrigger::custom(
            "Canada",
            HStack {
                children: vec![Text::new("CA").into(), Text::new("Canada").into()],
                ..Default::default()
            },
        )
        .semantics_identifier("country.select.trigger"),
        SelectContent::new(vec![
            SelectEntry::Group(
                SelectGroup::new(vec![
                    SelectOption::option("Canada", true)
                        .description("English and French")
                        .metadata("CA")
                        .semantics_identifier("country.ca")
                        .on_select(action("country.select.ca"))
                        .into(),
                    SelectOption::option("United States", false)
                        .disabled(true)
                        .semantics_identifier("country.us")
                        .into(),
                ])
                .label("North America"),
            ),
            SelectEntry::Separator(SelectSeparator::new()),
            SelectOption::option("France", false)
                .semantics_identifier("country.fr")
                .on_select(action("country.select.fr"))
                .into(),
        ]),
    )
    .open(true)
    .on_toggle(toggle.clone())
    .width(264.0);
    let (root, portals) = build_widget(|| select.into());

    assert_eq!(portals.len(), 1);
    let (trigger_id, trigger) = semantics_for_identifier(&root, "country.select.trigger");
    assert_eq!(trigger.role, Role::ComboBox);
    assert_eq!(trigger.value.as_deref(), Some("Canada"));
    assert_eq!(trigger.expanded, Some(true));
    assert_eq!(trigger.has_popup, Some(PopupKind::ListBox));
    assert_eq!(trigger.controls, vec![popup_id]);

    let popup = &portals[0];
    assert_eq!(semantics_at(popup, popup_id).role, Role::ListBox);
    let (canada_id, canada) = semantics_for_identifier(popup, "country.ca");
    assert_eq!(canada.role, Role::Option);
    assert_eq!(canada.selected, Some(true));
    assert_eq!(trigger.active_descendant, Some(canada_id));
    assert_ne!(trigger_id, canada_id);
    let (_, united_states) = semantics_for_identifier(popup, "country.us");
    assert!(united_states.disabled);
    assert!(!united_states.focusable);
    assert_eq!(semantics_for_role(popup, Role::Group).len(), 1);
    assert_eq!(semantics_for_role(popup, Role::Separator).len(), 1);
    assert!(semantics_for_role(popup, Role::Text)
        .iter()
        .any(|(_, semantics)| semantics.label.as_deref() == Some("North America")));
    assert!(semantics_at(popup, popup_id)
        .actions
        .entries
        .iter()
        .any(|entry| {
            entry.trigger == ActionTrigger::Dismiss && entry.action_id == toggle.id.as_u128()
        }));
}

#[test]
fn combobox_layout_keeps_text_input_configuration_and_option_relationships() {
    let id = WidgetId::explicit("member.combobox.layout");
    let popup_id = WidgetId::derived(id.as_u128(), &[1]);
    let input_id = WidgetId::explicit("member.combobox.input");
    let toggle = action("member.combobox.toggle");
    let changed = action("member.combobox.changed");
    let mut field = TextInput {
        id: Some(input_id),
        value: "Avery".into(),
        placeholder: Some("Find a member".into()),
        on_input: Some(changed.clone()),
        prefix: Some(Text::new("@ ").into()),
        ..Default::default()
    };
    field.semantics = Some(Semantics {
        labelled_by: vec![WidgetId::explicit("member.external-label")],
        ..Default::default()
    });
    let combobox = ComboboxLayout::new(
        id,
        ComboboxInput::custom(field),
        ComboboxContent::new(vec![ComboboxEntry::Group(
            fission_widgets::ComboboxGroup::new(vec![
                ComboboxOption::option("Avery", true)
                    .description("Engineering")
                    .semantics_identifier("member.avery")
                    .on_select(action("member.select.avery"))
                    .into(),
                ComboboxOption::option("Ava", false)
                    .disabled(true)
                    .semantics_identifier("member.ava")
                    .into(),
            ])
            .label("People"),
        )])
        .max_height(180.0),
    )
    .open(true)
    .on_toggle(toggle.clone())
    .width(280.0);
    let (root, portals) = build_widget(|| combobox.into());

    assert_eq!(portals.len(), 1);
    let input = semantics_at(&root, input_id);
    assert_eq!(input.role, Role::ComboBox);
    assert!(input.text_editable);
    assert_eq!(input.value.as_deref(), Some("Avery"));
    assert_eq!(input.controls, vec![popup_id]);
    assert_eq!(
        input.labelled_by,
        vec![WidgetId::explicit("member.external-label")]
    );
    assert!(input.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::TextChanged && entry.action_id == changed.id.as_u128()
    }));

    let popup = &portals[0];
    let (avery_id, avery) = semantics_for_identifier(popup, "member.avery");
    assert_eq!(input.active_descendant, Some(avery_id));
    assert_eq!(avery.role, Role::Option);
    assert_eq!(avery.selected, Some(true));
    let (_, ava) = semantics_for_identifier(popup, "member.ava");
    assert!(ava.disabled);
    assert!(semantics_at(popup, popup_id)
        .actions
        .entries
        .iter()
        .any(|entry| {
            entry.trigger == ActionTrigger::Dismiss && entry.action_id == toggle.id.as_u128()
        }));
}

#[test]
fn named_tabs_keep_list_trigger_and_panel_as_distinct_retained_regions() {
    let list_id = WidgetId::explicit("settings.tab-list");
    let general_id = WidgetId::explicit("settings.tab.general");
    let general_panel_id = WidgetId::explicit("settings.panel.general");
    let team_id = WidgetId::explicit("settings.tab.team");
    let team_panel_id = WidgetId::explicit("settings.panel.team");
    let layout = TabsLayout::new(
        TabList::new(
            list_id,
            vec![
                TabTrigger::new(general_id, general_panel_id, "General")
                    .semantics_identifier("settings.tab.general")
                    .on_press(action("settings.select.general")),
                TabTrigger::custom(
                    team_id,
                    team_panel_id,
                    "Team",
                    HStack {
                        children: vec![Text::new("Team").into(), Text::new("4").into()],
                        ..Default::default()
                    },
                )
                .selected(true)
                .semantics_identifier("settings.tab.team")
                .on_press(action("settings.select.team")),
            ],
        ),
        TabPanel::new(team_panel_id, team_id, Text::new("Team settings")),
    );
    let (ir, portals) = build_widget(|| layout.into());

    assert!(portals.is_empty());
    let list = semantics_at(&ir, list_id);
    assert_eq!(list.role, Role::TabList);
    let general = semantics_at(&ir, general_id);
    assert_eq!(general.role, Role::Tab);
    assert_eq!(general.selected, Some(false));
    assert!(!general.sequential_focusable);
    assert_eq!(general.controls, vec![general_panel_id]);
    let team = semantics_at(&ir, team_id);
    assert_eq!(team.role, Role::Tab);
    assert_eq!(team.label.as_deref(), Some("Team"));
    assert_eq!(team.selected, Some(true));
    assert!(team.sequential_focusable);
    assert_eq!(team.controls, vec![team_panel_id]);
    let panel = semantics_at(&ir, team_panel_id);
    assert_eq!(panel.role, Role::TabPanel);
    assert_eq!(panel.labelled_by, vec![team_id]);
}

#[test]
fn form_control_layout_relates_custom_label_description_and_error() {
    let field_id = WidgetId::explicit("email.field.layout");
    let input_id = WidgetId::explicit("email.field.input");
    let label_id = WidgetId::explicit("email.field.label");
    let description_id = WidgetId::explicit("email.field.description");
    let error_id = WidgetId::explicit("email.field.error");
    let layout = FormControlLayout::new(
        field_id,
        TextInput {
            id: Some(input_id),
            value: "not-an-email".into(),
            ..Default::default()
        },
    )
    .label(FormLabel::custom("Email address", Text::new("Account email")).id(label_id))
    .description(
        FormDescription::custom("Used for sign in.", Text::new("Sign-in address"))
            .id(description_id),
    )
    .error(
        FormError::custom(
            "Enter a valid email address.",
            Text::new("Email is invalid"),
        )
        .id(error_id),
    )
    .required(true);
    let (ir, portals) = build_widget(|| layout.into());

    assert!(portals.is_empty());
    let group = semantics_at(&ir, ir.root.expect("form-control semantics root"));
    assert_eq!(group.role, Role::Group);
    assert_eq!(group.labelled_by, vec![label_id]);
    assert_eq!(group.described_by, vec![description_id, error_id]);
    assert_eq!(
        semantics_at(&ir, label_id).label.as_deref(),
        Some("Email address")
    );
    assert_eq!(
        semantics_at(&ir, description_id).label.as_deref(),
        Some("Used for sign in.")
    );
    assert_eq!(
        semantics_at(&ir, error_id).label.as_deref(),
        Some("Enter a valid email address.")
    );
    let input = semantics_at(&ir, input_id);
    assert_eq!(input.labelled_by, vec![label_id]);
    assert_eq!(input.described_by, vec![description_id, error_id]);
    assert!(input.required);
    assert_eq!(
        input.validation_message.as_deref(),
        Some("Enter a valid email address.")
    );
}
