use fission_core::authoring::{BuildCtx, LoweringContext};
use fission_core::internal::build_layout_tree;
use fission_core::ui::{Column, Scroll, Text, Widget};
use fission_core::{
    build, ActionEnvelope, ActionId, Env, GlobalState, InputEvent, KeyCode, KeyEvent,
    LayoutDirection, Runtime, RuntimeState, View, WidgetId, WidgetIdExt,
};
use fission_ir::{
    op::{BoxAlignment, Color, Fill, Length},
    CoreIR, LayoutOp, Op, PaintOp, Role, SemanticOrientation, Semantics,
};
use fission_layout::{LayoutEngine, LayoutSize, LayoutSnapshot};
use fission_theme::{
    ComponentBorder, ComponentSize, ResolvedComponentStyle, ShadowLayer, TabPresentation,
};
use fission_widgets::{
    TabItem, TabList, TabPanel, TabTrigger, TabTriggerContent, Tabs, TabsLayout,
};

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

#[test]
fn tabs_scope_active_content_by_selected_tab() {
    let first = active_tab_scroll_id(0);
    let second = active_tab_scroll_id(1);

    assert_ne!(
        first, second,
        "tab-local scroll state must not carry between different active tabs"
    );
}

#[test]
fn implicit_tabs_ids_are_unique_across_sibling_instances() {
    let state = State;
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let node = build::enter(&mut ctx, &view, || {
        Column {
            children: vec![
                Tabs {
                    items: vec![actionable_tab("First", "implicit.first")],
                    ..Default::default()
                }
                .into(),
                Tabs {
                    items: vec![actionable_tab("Second", "implicit.second")],
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    });
    let mut lowering = LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&node, &mut lowering);
    lowering.ir.root = Some(root);
    let ir = lowering.ir;

    let first = semantics_for_identifier(&ir, "implicit.first").0;
    let second = semantics_for_identifier(&ir, "implicit.second").0;
    assert_ne!(first, second);
    assert_eq!(
        ir.nodes
            .values()
            .filter(|node| matches!(&node.op, Op::Semantics(value) if value.role == Role::TabList))
            .count(),
        2
    );
    assert_eq!(
        ir.nodes
            .values()
            .filter(|node| matches!(&node.op, Op::Semantics(value) if value.role == Role::TabPanel))
            .count(),
        2
    );
}

#[test]
fn tabs_lower_stable_roles_relationships_and_roving_focus() {
    let base_id = WidgetId::explicit("settings.tabs");
    let tabs = Tabs {
        active_index: 1,
        items: vec![
            actionable_tab("General", "settings.general"),
            actionable_tab("Team", "settings.team"),
            TabItem {
                title: "Audit".into(),
                content: Text::new("Audit content").into(),
                on_press: None,
                semantics_identifier: Some("settings.audit".into()),
            },
        ],
        ..Default::default()
    };
    let ir = lower_tabs(
        tabs,
        &Env::default(),
        &RuntimeState::default(),
        Some(base_id),
    );

    let (_, tab_list) = semantics_for_role(&ir, Role::TabList);
    assert_eq!(tab_list.orientation, Some(SemanticOrientation::Horizontal));
    assert!(!tab_list.focusable);

    let expected_first_id = WidgetId::derived(base_id.as_u128(), &[0, 0]);
    let expected_second_id = WidgetId::derived(base_id.as_u128(), &[1, 0]);
    let (first_id, first) = semantics_for_identifier(&ir, "settings.general");
    let (second_id, second) = semantics_for_identifier(&ir, "settings.team");
    let (_, actionless) = semantics_for_identifier(&ir, "settings.audit");
    assert_eq!(first_id, expected_first_id);
    assert_eq!(second_id, expected_second_id);

    assert_eq!(first.role, Role::Tab);
    assert_eq!(first.label.as_deref(), Some("General"));
    assert_eq!(first.selected, Some(false));
    assert!(
        first.focusable,
        "inactive tabs remain programmatically focusable"
    );
    assert!(!first.sequential_focusable);
    assert_eq!(first.actions.entries.len(), 1);
    assert_eq!(first.controls.len(), 1);

    assert_eq!(second.role, Role::Tab);
    assert_eq!(second.label.as_deref(), Some("Team"));
    assert_eq!(second.selected, Some(true));
    assert!(second.focusable);
    assert!(second.sequential_focusable);
    assert_eq!(second.actions.entries.len(), 1);
    assert_eq!(second.controls.len(), 1);

    assert!(actionless.focusable);
    assert!(!actionless.sequential_focusable);
    assert!(actionless.actions.entries.is_empty());

    let (panel_id, panel) = semantics_for_role(&ir, Role::TabPanel);
    assert_eq!(panel_id, second.controls[0]);
    assert_eq!(panel.labelled_by, vec![second_id]);
    assert_ne!(panel_id, first.controls[0]);
    assert_eq!(
        ir.nodes
            .values()
            .filter(|node| matches!(&node.op, Op::Semantics(value) if value.role == Role::Tab))
            .count(),
        3
    );
}

#[test]
fn tabs_keep_logical_semantic_order_and_paint_right_to_left() {
    let base_id = WidgetId::explicit("direction.tabs");
    let tabs = Tabs {
        active_index: 0,
        items: vec![
            actionable_tab("General", "direction.general"),
            actionable_tab("Team", "direction.team"),
            actionable_tab("Audit", "direction.audit"),
        ],
        ..Default::default()
    };
    let mut env = Env::default();
    env.layout_direction = LayoutDirection::RightToLeft;
    let ir = lower_tabs(tabs, &env, &RuntimeState::default(), Some(base_id));

    assert_eq!(ir.layout_direction, LayoutDirection::RightToLeft);
    let first = semantics_for_identifier(&ir, "direction.general").0;
    let second = semantics_for_identifier(&ir, "direction.team").0;
    let third = semantics_for_identifier(&ir, "direction.audit").0;
    let tab_list = semantics_for_role(&ir, Role::TabList).0;
    let semantic_order = descendant_ops_with_ids(&ir, tab_list)
        .filter_map(|(id, op)| {
            matches!(op, Op::Semantics(value) if value.role == Role::Tab).then_some(id)
        })
        .collect::<Vec<_>>();
    assert_eq!(semantic_order, vec![first, second, third]);

    let input = build_layout_tree(&ir, &env);
    let mut engine = LayoutEngine::new().with_layout_direction(ir.layout_direction);
    let layout = engine
        .compute_layout(
            &input,
            ir.root.expect("tabs root"),
            LayoutSize::new(320.0, 160.0),
            &|_| 0.0,
        )
        .expect("RTL tabs layout");
    let first_x = layout.get_node_rect(first).expect("first tab layout").x();
    let second_x = layout.get_node_rect(second).expect("second tab layout").x();
    let third_x = layout.get_node_rect(third).expect("third tab layout").x();
    assert!(first_x > second_x && second_x > third_x);
}

#[test]
fn tabs_consume_track_and_active_tab_recipe_without_zero_height_underline() {
    let base_id = WidgetId::explicit("recipe.tabs");
    let track_fill = gradient(Color::BLACK, Color::WHITE);
    let track_stroke_fill = gradient(Color::RED, Color::BLUE);
    let tab_fill = gradient(Color::GREEN, Color::BLUE);
    let tab_stroke_fill = gradient(Color::WHITE, Color::BLACK);
    let track_shadow = ShadowLayer {
        color: Color::BLACK.with_alpha(31),
        offset: (1.0, 2.0),
        blur_radius: 5.0,
        spread_radius: 0.0,
        inset: false,
    };
    let tab_shadow = ShadowLayer {
        color: Color::BLUE.with_alpha(51),
        offset: (0.0, 1.0),
        blur_radius: 3.0,
        spread_radius: 1.0,
        inset: false,
    };

    let mut env = Env::default();
    let tabs_theme = &mut &mut env.theme.components_mut().tabs;
    tabs_theme.indicator_height = 0.0;
    tabs_theme.track_style = ResolvedComponentStyle {
        background: Some(track_fill.clone()),
        border: Some(ComponentBorder {
            fill: track_stroke_fill.clone(),
            width: 1.5,
        }),
        radius: Some(17.0),
        padding: Some([1.0, 2.0, 3.0, 4.0]),
        gap: Some(7.0),
        shadows: vec![track_shadow.clone()],
        ..Default::default()
    };
    tabs_theme.sizes = vec![(
        ComponentSize::Md,
        ResolvedComponentStyle {
            height: Some(31.0),
            padding: Some([4.0, 5.0, 6.0, 7.0]),
            gap: Some(9.0),
            font_size: Some(17.0),
            line_height: Some(23.0),
            letter_spacing: Some(0.3),
            ..Default::default()
        },
    )];
    tabs_theme.states.active = Some(ResolvedComponentStyle {
        background: Some(tab_fill.clone()),
        text_color: Some(Color::WHITE),
        border: Some(ComponentBorder {
            fill: tab_stroke_fill.clone(),
            width: 2.5,
        }),
        radius: Some(11.0),
        font_weight: Some(650),
        shadows: vec![tab_shadow.clone()],
        ..Default::default()
    });

    let tabs = Tabs {
        active_index: 0,
        items: vec![actionable_tab("General", "recipe.general")],
        ..Default::default()
    };
    let ir = lower_tabs(tabs, &env, &RuntimeState::default(), Some(base_id));

    let (tab_list_id, _) = semantics_for_role(&ir, Role::TabList);
    let track_layout_id = ir.nodes[&tab_list_id].children[0];
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &ir.nodes[&track_layout_id].op else {
        panic!("tab list surface should lower a styled track box");
    };
    assert_eq!(style.padding, Some(points([1.0, 2.0, 3.0, 4.0])));
    assert_eq!(style.alignment, BoxAlignment::Start);
    assert!(descendant_ops(&ir, track_layout_id)
        .any(|op| { matches!(op, Op::Layout(LayoutOp::Flex { gap: Some(7.0), .. })) }));
    assert_surface_recipe(
        &ir,
        track_layout_id,
        &track_fill,
        &track_stroke_fill,
        1.5,
        17.0,
        &track_shadow,
    );

    let (tab_id, _) = semantics_for_identifier(&ir, "recipe.general");
    let tab_layout_id = ir.nodes[&tab_id].children[0];
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &ir.nodes[&tab_layout_id].op else {
        panic!("tab trigger should lower a styled box");
    };
    assert_eq!(style.height, Some(Length::Points(31.0)));
    assert_eq!(style.padding, Some(points([4.0, 5.0, 6.0, 7.0])));
    assert_eq!(style.alignment, BoxAlignment::Center);
    assert!(descendant_ops(&ir, tab_layout_id)
        .any(|op| { matches!(op, Op::Layout(LayoutOp::Flex { gap: Some(9.0), .. })) }));
    assert_surface_recipe(
        &ir,
        tab_layout_id,
        &tab_fill,
        &tab_stroke_fill,
        2.5,
        11.0,
        &tab_shadow,
    );

    let label_run = descendant_ops(&ir, tab_layout_id)
        .find_map(|op| match op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                runs.iter().find(|run| run.text == "General")
            }
            _ => None,
        })
        .expect("styled tab label");
    assert_eq!(label_run.style.font_size, 17.0);
    assert_eq!(label_run.style.font_weight, 650);
    assert_eq!(label_run.style.line_height, Some(23.0));
    assert_eq!(label_run.style.letter_spacing, 0.3);
    assert_eq!(label_run.style.color, Color::WHITE);

    assert!(
        !ir.nodes.values().any(|node| {
            matches!(
                &node.op,
                Op::Layout(LayoutOp::Box { height: Some(height), .. }) if *height == 2.5
            ) || matches!(
                &node.op,
                Op::Layout(LayoutOp::StyledBox { style, .. })
                    if style.height == Some(Length::Points(2.5))
            )
        }),
        "a zero indicator recipe must not leave an underline node behind"
    );
}

#[test]
fn tabs_keep_the_legacy_underline_for_themes_that_request_it() {
    let indicator = Color {
        r: 17,
        g: 101,
        b: 233,
        a: 255,
    };
    let mut env = Env::default();
    env.theme.components_mut().tabs.indicator_height = 3.0;
    env.theme.components_mut().tabs.active_color = indicator;

    let tabs = Tabs {
        items: vec![actionable_tab("General", "underline.general")],
        ..Default::default()
    };
    let ir = lower_tabs(
        tabs,
        &env,
        &RuntimeState::default(),
        Some(WidgetId::explicit("underline.tabs")),
    );

    assert!(ir.nodes.values().any(|node| {
        matches!(
            &node.op,
            Op::Layout(LayoutOp::StyledBox { style, .. })
                if style.height == Some(Length::Points(3.0))
        )
    }));
    assert!(ir.nodes.values().any(|node| {
        matches!(
            &node.op,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                ..
            }) if *color == indicator
        )
    }));
}

#[test]
fn named_tab_list_uses_underline_recipe_and_styles_semantic_count_content() {
    let trigger_id = WidgetId::explicit("underline.content.trigger");
    let panel_id = WidgetId::explicit("underline.content.panel");
    let trigger = TabTrigger::new(trigger_id, panel_id, "Agents")
        .content(TabTriggerContent::new("Agents").trailing_count("3"))
        .selected(true)
        .semantics_identifier("underline.content");
    let env = Env::default();
    let ir = lower_widget(
        || {
            TabList::new(WidgetId::explicit("underline.content.list"), vec![trigger])
                .presentation(TabPresentation::Underline)
                .into()
        },
        &env,
        &RuntimeState::default(),
    );

    let (tab_id, semantics) = semantics_for_identifier(&ir, "underline.content");
    assert_eq!(semantics.label.as_deref(), Some("Agents"));
    let runs = descendant_ops(&ir, tab_id)
        .filter_map(|op| match op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => Some(runs.as_slice()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    let label = runs
        .iter()
        .find(|run| run.text == "Agents")
        .expect("recipe-aware label");
    assert_eq!(label.style.color, env.theme.tokens.colors.primary);
    assert_eq!(
        label.style.font_weight,
        env.theme.tokens.typography.font_weight_medium
    );
    assert!(runs.iter().any(|run| run.text == "3"));
    assert!(descendant_ops(&ir, tab_id).any(|op| {
        matches!(
            op,
            Op::Layout(LayoutOp::StyledBox { style, .. })
                if style.height == Some(Length::Points(44.0))
        )
    }));
    assert!(ir.nodes.values().any(|node| {
        matches!(
            &node.op,
            Op::Layout(LayoutOp::StyledBox { style, .. })
                if style.height == Some(Length::Points(2.0))
        )
    }));
}

#[test]
fn older_serialized_tab_trigger_defaults_to_the_existing_presentation() {
    #[derive(serde::Deserialize)]
    struct LegacyPresentation {
        #[serde(default)]
        presentation: TabPresentation,
    }

    let decoded: LegacyPresentation =
        serde_json::from_str("{}").expect("decode payload without presentation");
    assert_eq!(decoded.presentation, TabPresentation::Default);
    let trigger = TabTrigger::new(
        WidgetId::explicit("compat.trigger"),
        WidgetId::explicit("compat.panel"),
        "General",
    );
    let ir = lower_widget(
        || TabList::new(WidgetId::explicit("compat.list"), vec![trigger]).into(),
        &Env::default(),
        &RuntimeState::default(),
    );
    let tab_id = semantics_for_role(&ir, Role::Tab).0;
    assert!(!descendant_ops(&ir, tab_id).any(|op| {
        matches!(
            op,
            Op::Layout(LayoutOp::StyledBox { style, .. })
                if style.height == Some(Length::Points(44.0))
        )
    }));
}

#[test]
fn disabled_tab_trigger_is_visible_but_not_focusable_or_actionable() {
    let trigger_id = WidgetId::explicit("settings.disabled-trigger");
    let panel_id = WidgetId::explicit("settings.disabled-panel");
    let trigger = TabTrigger::new(trigger_id, panel_id, "Unavailable")
        .on_press(ActionEnvelope {
            id: ActionId::from_name("settings.disabled-action"),
            payload: b"null".to_vec(),
        })
        .selected(true)
        .disabled(true)
        .semantics_identifier("settings.disabled");
    let ir = lower_widget(
        || {
            TabsLayout::new(
                TabList::new(WidgetId::explicit("settings.disabled-list"), vec![trigger]),
                TabPanel::new(panel_id, trigger_id, Text::new("Unavailable content")),
            )
            .into()
        },
        &Env::default(),
        &RuntimeState::default(),
    );

    let (_, semantics) = semantics_for_identifier(&ir, "settings.disabled");
    assert!(semantics.disabled);
    assert!(!semantics.focusable);
    assert!(!semantics.sequential_focusable);
    assert!(semantics.actions.entries.is_empty());
    assert_eq!(semantics.selected, Some(false));
}

#[test]
fn tab_list_uses_first_enabled_trigger_when_selected_trigger_is_disabled() {
    let ir = lower_tab_list(vec![
        named_trigger("disabled-selected", true, true),
        named_trigger("first-enabled", false, false),
        named_trigger("second-enabled", false, false),
    ]);

    let (_, disabled_selected) = semantics_for_identifier(&ir, "disabled-selected");
    let (first_enabled_id, first_enabled) = semantics_for_identifier(&ir, "first-enabled");
    let (_, second_enabled) = semantics_for_identifier(&ir, "second-enabled");

    assert_eq!(disabled_selected.selected, Some(false));
    assert!(!disabled_selected.sequential_focusable);
    assert_eq!(first_enabled.selected, Some(true));
    assert!(first_enabled.sequential_focusable);
    assert_eq!(second_enabled.selected, Some(false));
    assert!(!second_enabled.sequential_focusable);
    assert_eq!(sequential_tab_ids(&ir), vec![first_enabled_id]);
}

#[test]
fn tab_list_uses_first_enabled_trigger_when_none_are_selected() {
    let ir = lower_tab_list(vec![
        named_trigger("disabled-first", false, true),
        named_trigger("enabled-first", false, false),
        named_trigger("enabled-second", false, false),
    ]);

    let (enabled_first_id, enabled_first) = semantics_for_identifier(&ir, "enabled-first");
    assert_eq!(enabled_first.selected, Some(true));
    assert!(enabled_first.sequential_focusable);
    assert_eq!(sequential_tab_ids(&ir), vec![enabled_first_id]);
    assert_eq!(selected_tab_ids(&ir), vec![enabled_first_id]);
}

#[test]
fn tab_list_uses_first_enabled_selected_trigger_when_multiple_are_selected() {
    let ir = lower_tab_list(vec![
        named_trigger("unselected", false, false),
        named_trigger("selected-first", true, false),
        named_trigger("selected-second", true, false),
    ]);

    let (_, unselected) = semantics_for_identifier(&ir, "unselected");
    let (selected_first_id, selected_first) = semantics_for_identifier(&ir, "selected-first");
    let (_, selected_second) = semantics_for_identifier(&ir, "selected-second");

    assert_eq!(unselected.selected, Some(false));
    assert_eq!(selected_first.selected, Some(true));
    assert_eq!(selected_second.selected, Some(false));
    assert_eq!(selected_tab_ids(&ir), vec![selected_first_id]);
    assert_eq!(sequential_tab_ids(&ir), vec![selected_first_id]);
}

#[test]
fn compatibility_tabs_resolve_out_of_range_selection_to_a_coherent_first_tab() {
    let base_id = WidgetId::explicit("out-of-range.tabs");
    let ir = lower_tabs(
        Tabs {
            active_index: 99,
            items: vec![
                actionable_tab("General", "out-of-range.general"),
                actionable_tab("Team", "out-of-range.team"),
            ],
            ..Default::default()
        },
        &Env::default(),
        &RuntimeState::default(),
        Some(base_id),
    );

    let expected_first_id = WidgetId::derived(base_id.as_u128(), &[0, 0]);
    assert_eq!(sequential_tab_ids(&ir), vec![expected_first_id]);
    assert_eq!(selected_tab_ids(&ir), vec![expected_first_id]);
    let (_, first) = semantics_for_identifier(&ir, "out-of-range.general");
    let (panel_id, panel) = semantics_for_role(&ir, Role::TabPanel);
    assert_eq!(panel_id, first.controls[0]);
    assert_eq!(panel.labelled_by, vec![expected_first_id]);
}

#[test]
fn tab_list_keyboard_navigation_skips_disabled_triggers_without_changing_selection() {
    let first_id = WidgetId::explicit("keyboard.first.trigger");
    let disabled_id = WidgetId::explicit("keyboard.disabled.trigger");
    let last_id = WidgetId::explicit("keyboard.last.trigger");
    let ir = lower_tab_list(vec![
        named_trigger_with_id(first_id, "keyboard.first", true, false),
        named_trigger_with_id(disabled_id, "keyboard.disabled", false, true),
        named_trigger_with_id(last_id, "keyboard.last", false, false),
    ]);
    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(first_id));

    press_key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(last_id));
    press_key(&mut runtime, &ir, KeyCode::Right);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(first_id));
    press_key(&mut runtime, &ir, KeyCode::Left);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(last_id));
    press_key(&mut runtime, &ir, KeyCode::Home);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(first_id));
    press_key(&mut runtime, &ir, KeyCode::End);
    assert_eq!(runtime.runtime_state.interaction.focused, Some(last_id));

    assert_eq!(
        semantics_for_identifier(&ir, "keyboard.first").1.selected,
        Some(true)
    );
    assert_eq!(
        semantics_for_identifier(&ir, "keyboard.last").1.selected,
        Some(false)
    );
    let disabled = semantics_for_identifier(&ir, "keyboard.disabled").1;
    assert!(disabled.disabled);
    assert!(!disabled.focusable);
    assert!(!disabled.sequential_focusable);
}

#[test]
fn tabs_resolve_hover_style_against_the_stable_trigger_id() {
    let base_id = WidgetId::explicit("hover.tabs");
    let first_id = WidgetId::derived(base_id.as_u128(), &[0, 0]);
    let hover_fill = gradient(Color::WHITE, Color::GREEN);
    let mut env = Env::default();
    env.theme.components_mut().tabs.states.hover = Some(ResolvedComponentStyle {
        background: Some(hover_fill.clone()),
        ..Default::default()
    });
    let mut runtime = RuntimeState::default();
    runtime.interaction.set_hovered(first_id, true);

    let tabs = Tabs {
        active_index: 1,
        items: vec![
            actionable_tab("General", "hover.general"),
            actionable_tab("Team", "hover.team"),
        ],
        ..Default::default()
    };
    let ir = lower_tabs(tabs, &env, &runtime, Some(base_id));
    let (actual_first_id, _) = semantics_for_identifier(&ir, "hover.general");
    assert_eq!(actual_first_id, first_id);
    assert!(descendant_ops(&ir, actual_first_id).any(|op| {
        matches!(
            op,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(fill),
                ..
            }) if fill == &hover_fill
        )
    }));
}

fn active_tab_scroll_id(active_index: usize) -> WidgetId {
    let tabs = Tabs {
        active_index,
        items: vec![tab("One"), tab("Two")],
        ..Default::default()
    };
    let ir = lower_tabs(tabs, &Env::default(), &RuntimeState::default(), None);

    let panel_id = semantics_for_role(&ir, Role::TabPanel).0;
    let scroll_id = descendant_ops_with_ids(&ir, panel_id)
        .find_map(|(id, op)| matches!(op, Op::Layout(LayoutOp::Scroll { .. })).then_some(id))
        .expect("active tab should lower a scroll node");
    scroll_id
}

fn lower_tab_list(tabs: Vec<TabTrigger>) -> CoreIR {
    lower_widget(
        || TabList::new(WidgetId::explicit("test.tabs.list"), tabs).into(),
        &Env::default(),
        &RuntimeState::default(),
    )
}

fn named_trigger(identifier: &str, selected: bool, disabled: bool) -> TabTrigger {
    named_trigger_with_id(
        WidgetId::explicit(&format!("{identifier}.trigger")),
        identifier,
        selected,
        disabled,
    )
}

fn named_trigger_with_id(
    id: WidgetId,
    identifier: &str,
    selected: bool,
    disabled: bool,
) -> TabTrigger {
    TabTrigger::new(
        id,
        WidgetId::explicit(&format!("{identifier}.panel")),
        identifier,
    )
    .selected(selected)
    .disabled(disabled)
    .semantics_identifier(identifier)
}

fn sequential_tab_ids(ir: &CoreIR) -> Vec<WidgetId> {
    ir.nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics)
                if semantics.role == Role::Tab
                    && semantics.sequential_focusable
                    && !semantics.disabled =>
            {
                Some(node.id)
            }
            _ => None,
        })
        .collect()
}

fn selected_tab_ids(ir: &CoreIR) -> Vec<WidgetId> {
    ir.nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics)
                if semantics.role == Role::Tab && semantics.selected == Some(true) =>
            {
                Some(node.id)
            }
            _ => None,
        })
        .collect()
}

fn press_key(runtime: &mut Runtime, ir: &CoreIR, key_code: KeyCode) {
    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers: 0,
            }),
            ir,
            &LayoutSnapshot::new(LayoutSize::new(640.0, 480.0)),
        )
        .expect("keyboard input");
}

fn lower_widget(
    build_widget: impl FnOnce() -> Widget,
    env: &Env,
    runtime: &RuntimeState,
) -> CoreIR {
    let state = State;
    let view = View::new(&state, runtime, env, None);
    let mut ctx = BuildCtx::<State>::new();
    let node = build::enter(&mut ctx, &view, build_widget);
    let mut lowering = LoweringContext::new(env, runtime, None, None);
    let root = fission_core::internal::lower_widget(&node, &mut lowering);
    lowering.ir.root = Some(root);
    lowering.ir
}

fn lower_tabs(tabs: Tabs, env: &Env, runtime: &RuntimeState, id: Option<WidgetId>) -> CoreIR {
    let state = State;
    let view = View::new(&state, runtime, env, None);
    let mut ctx = BuildCtx::<State>::new();
    let node = build::enter(&mut ctx, &view, || match id {
        Some(id) => tabs.id(id),
        None => tabs.into(),
    });
    let mut lowering = LoweringContext::new(env, runtime, None, None);
    let root = fission_core::internal::lower_widget(&node, &mut lowering);
    lowering.ir.root = Some(root);
    lowering.ir
}

fn tab(title: &str) -> TabItem {
    TabItem {
        title: title.into(),
        content: Scroll {
            child: Some(Text::new(format!("{title} content")).into()),
            ..Default::default()
        }
        .into(),
        on_press: None,
        semantics_identifier: None,
    }
}

fn actionable_tab(title: &str, identifier: &str) -> TabItem {
    TabItem {
        title: title.into(),
        content: Text::new(format!("{title} content")).into(),
        on_press: Some(ActionEnvelope {
            id: ActionId::from_name(identifier),
            payload: b"null".to_vec(),
        }),
        semantics_identifier: Some(identifier.into()),
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

fn semantics_for_role(ir: &CoreIR, role: Role) -> (WidgetId, &Semantics) {
    let mut matches = ir.nodes.values().filter_map(|node| match &node.op {
        Op::Semantics(semantics) if semantics.role == role => Some((node.id, semantics)),
        _ => None,
    });
    let result = matches.next().expect("semantic role");
    assert!(matches.next().is_none(), "semantic role must be unique");
    result
}

fn descendant_ops(ir: &CoreIR, root: WidgetId) -> impl Iterator<Item = &Op> {
    let mut stack = vec![root];
    let mut ids = Vec::new();
    while let Some(id) = stack.pop() {
        let node = &ir.nodes[&id];
        ids.push(id);
        stack.extend(node.children.iter().copied());
    }
    ids.into_iter().map(|id| &ir.nodes[&id].op)
}

fn descendant_ops_with_ids(ir: &CoreIR, root: WidgetId) -> impl Iterator<Item = (WidgetId, &Op)> {
    let mut stack = vec![root];
    let mut ids = Vec::new();
    while let Some(id) = stack.pop() {
        let node = &ir.nodes[&id];
        ids.push(id);
        stack.extend(node.children.iter().rev().copied());
    }
    ids.into_iter().map(|id| (id, &ir.nodes[&id].op))
}

fn assert_surface_recipe(
    ir: &CoreIR,
    root: WidgetId,
    fill: &Fill,
    stroke_fill: &Fill,
    stroke_width: f32,
    radius: f32,
    shadow: &ShadowLayer,
) {
    assert!(descendant_ops(ir, root).any(|op| {
        matches!(
            op,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(actual_fill),
                stroke: Some(stroke),
                corner_radius,
                ..
            }) if actual_fill == fill
                && &stroke.fill == stroke_fill
                && stroke.width == stroke_width
                && *corner_radius == radius
        )
    }));
    assert!(descendant_ops(ir, root).any(|op| {
        matches!(
            op,
            Op::Paint(PaintOp::DrawRect {
                shadow: Some(actual),
                corner_radius,
                ..
            }) if actual == &shadow.to_box_shadow() && *corner_radius == radius
        )
    }));
}

fn gradient(start: Color, end: Color) -> Fill {
    Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 1.0),
        stops: vec![(0.0, start), (1.0, end)],
        extend: Default::default(),
    }
}

fn points(values: [f32; 4]) -> [Length; 4] {
    values.map(Length::Points)
}
