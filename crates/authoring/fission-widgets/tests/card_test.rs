use fission_core::internal::BuildCtx;
use fission_core::op::{Color, Fill, Overflow};
use fission_core::ui::{CardPattern, Container, Text};
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget, WidgetId, WidgetIdExt};
use fission_theme::ComponentBorder;
use fission_widgets::Card;

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn build_card(
    env: &Env,
    runtime_state: &RuntimeState,
    id: WidgetId,
    pattern: CardPattern,
    interactive: bool,
    selected: bool,
) -> Widget {
    let state = TestState;
    let view = View::new(&state, runtime_state, env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    build::enter(&mut ctx, &view, || {
        Card {
            child: Text::new("Body").into(),
            pattern,
            interactive,
            selected,
        }
        .id(id)
    })
}

fn card_container(widget: &Widget) -> &Container {
    fission_core::internal::widget_as_container(widget).expect("card container")
}

#[test]
fn non_elevated_patterns_do_not_receive_fallback_shadows() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();

    for pattern in [CardPattern::Plain, CardPattern::Raised, CardPattern::Tinted] {
        let key = format!("card.rest.{pattern:?}");
        let id = WidgetId::explicit(&key);
        let widget = build_card(&env, &runtime_state, id, pattern, false, false);
        let container = card_container(&widget);
        let expected = env.theme.components.card.resolve(pattern, false);

        assert_eq!(container.shadows, expected.outer_shadows());
        assert!(
            container.shadow.is_none(),
            "{pattern:?} must not gain an implicit elevation"
        );
    }
}

#[test]
fn elevated_pattern_uses_only_its_resolved_design_system_shadows() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let id = WidgetId::explicit("card.elevated");
    let widget = build_card(
        &env,
        &runtime_state,
        id,
        CardPattern::Elevated,
        false,
        false,
    );
    let container = card_container(&widget);
    let expected = env
        .theme
        .components
        .card
        .resolve(CardPattern::Elevated, false)
        .outer_shadows();

    assert_eq!(container.shadows, expected);
    assert!(container.shadow.is_none());
}

#[test]
fn interactive_flag_enables_real_hover_treatment_without_forcing_it() {
    let env = Env::default();
    let id = WidgetId::explicit("card.interactive");

    let rest_runtime = RuntimeState::default();
    let rest = build_card(&env, &rest_runtime, id, CardPattern::Plain, true, false);
    assert_eq!(
        card_container(&rest).shadows,
        env.theme
            .components
            .card
            .resolve(CardPattern::Plain, false)
            .outer_shadows(),
        "interactive eligibility must not look permanently hovered"
    );

    let mut hover_runtime = RuntimeState::default();
    hover_runtime.interaction.set_hovered(id, true);
    let hovered = build_card(&env, &hover_runtime, id, CardPattern::Plain, true, false);
    assert_eq!(
        card_container(&hovered).shadows,
        env.theme
            .components
            .card
            .resolve(CardPattern::Plain, true)
            .outer_shadows(),
        "an interactive card should use the hover recipe while actually hovered"
    );

    let inactive = build_card(&env, &hover_runtime, id, CardPattern::Plain, false, false);
    assert_eq!(
        card_container(&inactive).shadows,
        env.theme
            .components
            .card
            .resolve(CardPattern::Plain, false)
            .outer_shadows(),
        "non-interactive cards must ignore pointer hover styling"
    );
}

#[test]
fn card_uses_resolved_surface_geometry_and_clips_its_contents() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let id = WidgetId::explicit("card.geometry");
    let widget = build_card(&env, &runtime_state, id, CardPattern::Plain, false, false);
    let container = card_container(&widget);
    let theme = &env.theme.components.card;
    let expected = theme.resolve(CardPattern::Plain, false);

    assert_eq!(container.id, Some(id));
    assert_eq!(
        container.padding,
        expected.padding_box(theme.padding, theme.padding)
    );
    assert_eq!(
        container.border_radius,
        expected.radius.unwrap_or(theme.radius)
    );
    assert_eq!(container.box_style.overflow, Overflow::Clip);
}

#[test]
fn controlled_selection_uses_the_card_selection_recipe() {
    let selected_color = Color {
        r: 17,
        g: 71,
        b: 199,
        a: 255,
    };
    let mut env = Env::default();
    env.theme.components.card.selected_style.border = Some(ComponentBorder {
        fill: Fill::Solid(selected_color),
        width: 3.0,
    });
    let runtime_state = RuntimeState::default();
    let id = WidgetId::explicit("card.selected");

    let selected = build_card(&env, &runtime_state, id, CardPattern::Plain, false, true);
    let selected = card_container(&selected);
    assert_eq!(selected.border_color, Some(selected_color));
    assert_eq!(selected.border_width, 3.0);

    let unselected = build_card(&env, &runtime_state, id, CardPattern::Plain, false, false);
    assert_ne!(
        card_container(&unselected).border_color,
        Some(selected_color)
    );
}
