use fission_core::authoring::BuildCtx;
use fission_core::op::{AlignItems, Length, TextAlign};
use fission_core::ui::{Column, Container, Icon, Text, WidgetKind};
use fission_core::{build, Env, GlobalState, LayoutSize, RuntimeState, View, Widget};
use fission_ir::op::Color;
use fission_widgets::EmptyState;

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn build_empty_state(env: &Env, description: Option<&str>, action: Option<Widget>) -> Widget {
    build_empty_state_with_icon(env, Text::new("Icon").into(), description, action)
}

fn build_empty_state_with_icon(
    env: &Env,
    icon: Widget,
    description: Option<&str>,
    action: Option<Widget>,
) -> Widget {
    let runtime = RuntimeState::default();
    let state = TestState;
    let view = View::new(&state, &runtime, env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    build::enter(&mut ctx, &view, || {
        EmptyState {
            icon: Some(icon),
            title: "Nothing here".into(),
            description: description.map(str::to_owned),
            action,
        }
        .into()
    })
}

#[test]
fn supplied_icon_uses_empty_state_recipe_defaults_without_losing_explicit_overrides() {
    let mut env = Env::default();
    let recipe_color = Color {
        r: 18,
        g: 52,
        b: 86,
        a: 255,
    };
    env.theme.components.empty_state.icon_style.icon_size = Some(17.0);
    env.theme.components.empty_state.icon_style.text_color = Some(recipe_color);

    let widget = build_empty_state_with_icon(
        &env,
        Icon::path("M0 0h1v1z").into(),
        Some("Create the first item to get started."),
        None,
    );
    let recipe_header = header(sections(panel(&widget)));
    let icon_region = fission_core::internal::widget_as_container(&recipe_header.children[0])
        .expect("empty-state icon region");
    let icon = match icon_region.child.as_ref().map(Widget::kind) {
        Some(WidgetKind::Icon(icon)) => icon,
        _ => panic!("empty-state icon"),
    };
    assert_eq!(icon.size, Some(17.0));
    assert_eq!(icon.color, Some(recipe_color));

    let explicit_color = Color {
        r: 140,
        g: 80,
        b: 20,
        a: 255,
    };
    let widget = build_empty_state_with_icon(
        &env,
        Icon::path("M0 0h1v1z")
            .size(23.0)
            .color(explicit_color)
            .into(),
        None,
        None,
    );
    let explicit_header = header(sections(panel(&widget)));
    let icon_region = fission_core::internal::widget_as_container(&explicit_header.children[0])
        .expect("empty-state icon region");
    let icon = match icon_region.child.as_ref().map(Widget::kind) {
        Some(WidgetKind::Icon(icon)) => icon,
        _ => panic!("empty-state icon"),
    };
    assert_eq!(icon.size, Some(23.0));
    assert_eq!(icon.color, Some(explicit_color));
}

fn panel(widget: &Widget) -> &Container {
    fission_core::internal::widget_as_container(widget).expect("empty-state panel")
}

fn sections(panel: &Container) -> &Column {
    panel
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_column)
        .expect("empty-state sections")
}

fn header(sections: &Column) -> &Column {
    let region = sections
        .children
        .first()
        .and_then(fission_core::internal::widget_as_container)
        .expect("empty-state header region");
    region
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_column)
        .expect("empty-state header")
}

fn text(widget: &Widget) -> &Text {
    fission_core::internal::widget_as_text(widget).expect("empty-state text")
}

#[test]
fn empty_state_uses_a_bounded_token_driven_panel() {
    let mut env = Env::default();
    env.viewport_size = LayoutSize::new(800.0, 600.0);
    let tokens = &env.theme.tokens;
    let widget = build_empty_state(&env, Some("Create the first item to get started."), None);
    let panel = panel(&widget);

    assert_eq!(panel.box_style.width, Some(Length::percent(100.0)));
    assert_eq!(panel.box_style.max_width, None);
    assert_eq!(panel.min_height, Some(160.0));
    assert_eq!(panel.padding, [tokens.spacing.l; 4]);
    assert_eq!(panel.border_color, Some(tokens.colors.border));
    assert_eq!(panel.border_width, 1.0);
    assert_eq!(panel.border_dash.as_deref(), Some([4.0, 4.0].as_slice()));
    assert_eq!(panel.border_radius, tokens.radii.large);
    assert!(panel.background_fill.is_none());
    assert!(panel.shadow.is_none());
    assert!(panel.shadows.is_empty());
}

#[test]
fn empty_state_composes_the_theme_owned_narrow_surface_below_its_breakpoint() {
    let mut env = Env::default();
    env.theme.components.empty_state.surface_style.padding = Some([9.0, 11.0, 13.0, 15.0]);
    env.theme
        .components
        .empty_state
        .narrow_surface_style
        .min_height = Some(127.0);
    env.theme.components.empty_state.narrow_breakpoint = 500.0;
    env.viewport_size = LayoutSize::new(499.0, 700.0);

    let widget = build_empty_state(&env, None, None);
    let narrow_panel = panel(&widget);
    assert_eq!(narrow_panel.min_height, Some(127.0));
    assert_eq!(narrow_panel.padding, [9.0, 11.0, 13.0, 15.0]);

    env.viewport_size = LayoutSize::new(500.0, 700.0);
    let widget = build_empty_state(&env, None, None);
    let regular_panel = panel(&widget);
    assert_eq!(regular_panel.min_height, Some(160.0));
    assert_eq!(regular_panel.padding, [9.0, 11.0, 13.0, 15.0]);
}

#[test]
fn empty_state_has_compact_centered_header_anatomy() {
    let env = Env::default();
    let tokens = &env.theme.tokens;
    let typography = &tokens.typography;
    let widget = build_empty_state(&env, Some("Create the first item to get started."), None);
    let sections = sections(panel(&widget));
    let header = header(sections);

    assert_eq!(sections.gap, Some(tokens.spacing.m));
    assert_eq!(sections.align_items, AlignItems::Center);
    assert_eq!(header.gap, Some(tokens.spacing.s));
    assert_eq!(header.align_items, AlignItems::Center);
    assert_eq!(header.children.len(), 3);
    let header_region = fission_core::internal::widget_as_container(&sections.children[0])
        .expect("empty-state header region");
    assert_eq!(header_region.max_width, Some(384.0));

    let title = text(&header.children[1]);
    assert_eq!(title.font_size, Some(typography.font_size_base));
    assert_eq!(title.font_weight, Some(typography.font_weight_medium));
    assert_eq!(title.color, Some(tokens.colors.text_primary));
    assert_eq!(title.text_align, TextAlign::Center);

    let description = text(&header.children[2]);
    assert_eq!(description.font_size, Some(typography.font_size_base));
    assert_eq!(description.color, Some(tokens.colors.text_secondary));
    assert_eq!(description.text_align, TextAlign::Center);
}

#[test]
fn optional_content_does_not_create_placeholder_children() {
    let env = Env::default();
    let without_optional_content = build_empty_state(&env, None, None);
    let sections_without = sections(panel(&without_optional_content));
    let header_without = header(sections_without);

    assert_eq!(header_without.children.len(), 2, "icon and title only");
    assert_eq!(sections_without.children.len(), 1, "header only");

    let with_action = build_empty_state(&env, None, Some(Text::new("Create first item").into()));
    let sections_with_action = sections(panel(&with_action));
    assert_eq!(sections_with_action.children.len(), 2, "header and action");
    let action = fission_core::internal::widget_as_container(&sections_with_action.children[1])
        .expect("empty-state action region");
    assert_eq!(
        action
            .child
            .as_ref()
            .map(fission_core::internal::widget_kind_name),
        Some("Text")
    );
}
