use fission_core::authoring::BuildCtx;
use fission_core::op::AlignItems;
use fission_core::ui::{Column, Container, Row, Text, WidgetKind};
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget};
use fission_ir::op::{Color, Fill};
use fission_ir::Role;
use fission_widgets::{Alert, AlertContent, AlertDescription, AlertKind, AlertLayout, AlertTitle};

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn build_alert(env: &Env, kind: AlertKind, description: Option<&str>) -> Widget {
    let state = TestState;
    let runtime_state = RuntimeState::default();
    let view = View::new(&state, &runtime_state, env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    build::enter(&mut ctx, &view, || {
        Alert {
            kind,
            title: "Alert title".into(),
            description: description.map(str::to_owned),
        }
        .into()
    })
}

fn root_container(widget: &Widget) -> &Container {
    let region = match widget.kind() {
        WidgetKind::SemanticsRegion(region) => region,
        other => panic!("expected alert semantics, got {other:?}"),
    };
    assert_eq!(region.role, Role::Alert);
    region
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_container)
        .expect("alert container")
}

fn alert_row(container: &Container) -> &Row {
    let stack = match container.child.as_ref().map(Widget::kind) {
        Some(WidgetKind::ZStack(stack)) => stack,
        _ => panic!("alert surface stack"),
    };
    fission_core::internal::widget_as_row(&stack.children[0]).expect("alert content row")
}

fn alert_layers(container: &Container) -> &[Widget] {
    match container.child.as_ref().map(Widget::kind) {
        Some(WidgetKind::ZStack(stack)) => &stack.children,
        _ => panic!("alert surface stack"),
    }
}

fn leading_icon(row: &Row) -> (&fission_core::ui::Icon, Option<f32>) {
    let icon_region = fission_core::internal::widget_as_container(&row.children[0])
        .expect("alert leading icon region");
    let composite = match icon_region.child.as_ref().map(Widget::kind) {
        Some(WidgetKind::Composite(composite)) => composite,
        _ => panic!("alert leading composite"),
    };
    let icon = match composite.child.kind() {
        WidgetKind::Icon(icon) => icon,
        _ => panic!("alert leading icon"),
    };
    (
        icon,
        composite
            .style
            .translate_y
            .as_ref()
            .map(|translate| translate.base),
    )
}

fn message_column(row: &Row) -> &Column {
    let message_container = row.children.get(1).expect("alert message container");
    let message_container = fission_core::internal::widget_as_container(message_container)
        .expect("alert message container");
    message_container
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_column)
        .expect("alert message column")
}

fn text(widget: &Widget) -> &Text {
    fission_core::internal::widget_as_text(widget).expect("alert text")
}

#[test]
fn alert_tones_use_semantic_design_tokens() {
    let env = Env::default();
    let theme = &env.theme.components.alert;
    let tokens = &env.theme.tokens;

    for (kind, background, tone) in [
        (AlertKind::Info, theme.info_bg, tokens.colors.info),
        (AlertKind::Warning, theme.warning_bg, tokens.colors.warning),
        (AlertKind::Error, theme.error_bg, tokens.colors.error),
        (AlertKind::Success, theme.success_bg, tokens.colors.success),
    ] {
        let widget = build_alert(&env, kind, Some("Supporting detail"));
        let container = root_container(&widget);
        let row = alert_row(container);

        assert_eq!(container.background_fill, Some(Fill::Solid(background)));
        assert_eq!(container.border_color, Some(tokens.colors.border));
        assert_eq!(container.border_width, 1.0);
        assert_eq!(container.border_radius, theme.radius);

        let (icon, translate_y) = leading_icon(row);
        assert_eq!(icon.color, Some(tone));
        assert_eq!(icon.size, Some(tokens.spacing.m));
        assert_eq!(translate_y, theme.icon_style.translate_y);
    }
}

#[test]
fn alert_anatomy_is_compact_and_top_aligned() {
    let env = Env::default();
    let tokens = &env.theme.tokens;
    let typography = &tokens.typography;
    let widget = build_alert(&env, AlertKind::Success, Some("Supporting detail"));
    let container = root_container(&widget);
    let row = alert_row(container);
    let message = message_column(row);

    assert_eq!(row.gap, Some(tokens.spacing.s));
    assert_eq!(row.align_items, AlignItems::Start);
    assert_eq!(message.gap, Some(tokens.spacing.xs / 2.0));
    assert_eq!(message.children.len(), 2);
    assert_eq!(
        container.padding,
        [
            tokens.spacing.s + tokens.spacing.xs / 2.0,
            tokens.spacing.s + tokens.spacing.xs / 2.0,
            tokens.spacing.s,
            tokens.spacing.s,
        ]
    );

    let title = text(&message.children[0]);
    assert_eq!(title.font_size, Some(typography.font_size_base));
    assert_eq!(title.font_weight, Some(typography.font_weight_medium));
    assert_eq!(title.color, Some(tokens.colors.text_primary));

    let description = text(&message.children[1]);
    assert_eq!(description.font_size, Some(typography.font_size_base));
    assert_eq!(description.color, Some(tokens.colors.text_secondary));
}

#[test]
fn alert_without_description_has_no_placeholder_or_phantom_gap() {
    let env = Env::default();
    let widget = build_alert(&env, AlertKind::Info, None);
    let message = message_column(alert_row(root_container(&widget)));

    assert_eq!(message.children.len(), 1);
    assert_eq!(
        fission_core::internal::widget_kind_name(&message.children[0]),
        "Text"
    );
}

#[test]
fn alert_layout_exposes_retained_content_and_action_regions() {
    let env = Env::default();
    let state = TestState;
    let runtime_state = RuntimeState::default();
    let view = View::new(&state, &runtime_state, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let widget = build::enter(&mut ctx, &view, || {
        AlertLayout::new(
            AlertKind::Warning,
            AlertContent::new(vec![
                AlertTitle::new("Review required").into(),
                AlertDescription::new("Check the supplied values.").into(),
            ]),
        )
        .without_leading()
        .action(Text::new("Details"))
        .into()
    });

    let root = root_container(&widget);
    let row = alert_row(root);
    assert_eq!(row.children.len(), 1);
    let content = fission_core::internal::widget_as_container(&row.children[0])
        .expect("retained content region");
    let content = content
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_column)
        .expect("standard alert content");
    assert_eq!(content.children.len(), 2);
    let layers = alert_layers(root);
    assert_eq!(layers.len(), 2);
    let positioned = match layers[1].kind() {
        WidgetKind::Positioned(positioned) => positioned,
        _ => panic!("alert action should be positioned independently of message flow"),
    };
    assert_eq!(positioned.top, Some(8.0));
    // The trailing action is inset from the end edge, so it follows reading
    // order instead of pinning itself to the right in every locale.
    assert_eq!(positioned.end, Some(8.0));
    assert_eq!(positioned.right, None);
    assert_eq!(positioned.width, Some(64.0));
}

#[test]
fn alert_surface_and_tone_resolve_from_component_recipe() {
    let mut env = Env::default();
    let surface = Color {
        r: 17,
        g: 31,
        b: 47,
        a: 255,
    };
    let tone = Color {
        r: 86,
        g: 175,
        b: 214,
        a: 255,
    };
    env.theme.components_mut().alert.surface_style.background = Some(Fill::Solid(surface));
    env.theme.components_mut().alert.surface_style.padding = Some([3.0, 5.0, 7.0, 11.0]);
    env.theme.components_mut().alert.surface_style.min_height = Some(47.0);
    env.theme.components_mut().alert.info_style.background = Some(Fill::Solid(surface));
    env.theme.components_mut().alert.info_style.text_color = Some(tone);
    env.theme.components_mut().alert.icon_style.translate_y = Some(-3.5);

    let widget = build_alert(&env, AlertKind::Info, Some("Supporting detail"));
    let container = root_container(&widget);
    let row = alert_row(container);

    assert_eq!(container.background_fill, Some(Fill::Solid(surface)));
    assert_eq!(container.padding, [3.0, 5.0, 7.0, 11.0]);
    assert_eq!(container.min_height, Some(47.0));
    let (icon, translate_y) = leading_icon(row);
    assert_eq!(icon.color, Some(tone));
    assert_eq!(translate_y, Some(-3.5));
}
