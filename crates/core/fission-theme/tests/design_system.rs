use fission_theme::{
    BadgeTone, ButtonHierarchy, CardPattern, ComponentSize, ComponentState, DesignMode,
    DesignSystem, DesignValue, Fill, FissionCupertinoDesignSystem, FissionDefaultDesignSystem,
    FissionFluent2DesignSystem, FissionLiquidGlassDesignSystem, FissionMaterialDesign3DesignSystem,
    ShadowLayer, Theme, Tokens,
};

fn dimension_token<D: DesignSystem>(path: &str) -> f32 {
    let token = D::tokens()
        .tokens
        .iter()
        .find(|token| token.path == path)
        .unwrap_or_else(|| panic!("missing design token {path}"));
    match &token.value {
        DesignValue::Dimension(value) => *value,
        other => panic!("design token {path} is not a dimension: {other:?}"),
    }
}

fn shadow_token<D: DesignSystem>(path: &str) -> &'static [ShadowLayer] {
    let token = D::tokens()
        .tokens
        .iter()
        .find(|token| token.path == path)
        .unwrap_or_else(|| panic!("missing design token {path}"));
    match &token.value {
        DesignValue::Shadow(layers) => layers,
        other => panic!("design token {path} is not a shadow: {other:?}"),
    }
}

fn assert_component_alias_parity<D: DesignSystem>() {
    let theme = D::theme(DesignMode::Light);

    assert_eq!(
        dimension_token::<D>("component.button.height"),
        theme.components.button.height
    );
    assert_eq!(
        dimension_token::<D>("component.button.padding_horizontal"),
        theme.components.button.padding_horizontal
    );
    assert_eq!(
        dimension_token::<D>("component.button.padding_vertical"),
        theme.components.button.padding_vertical
    );
    assert_eq!(
        dimension_token::<D>("component.button.radius"),
        theme.components.button.radius
    );
    assert_eq!(
        dimension_token::<D>("component.button.text_size"),
        theme.components.button.text_size
    );

    let button_default = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Default,
    );
    let button_hover = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Hover,
    );
    let button_pressed = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Active,
    );
    assert_eq!(
        shadow_token::<D>("component.button.elevation_rest"),
        button_default.shadows.as_slice()
    );
    assert_eq!(
        shadow_token::<D>("component.button.elevation_hover"),
        button_hover.shadows.as_slice()
    );
    assert_eq!(
        shadow_token::<D>("component.button.elevation_pressed"),
        button_pressed.shadows.as_slice()
    );

    assert_eq!(
        dimension_token::<D>("component.text_input.height"),
        theme.components.text_input.height
    );
    assert_eq!(
        dimension_token::<D>("component.text_input.padding_h"),
        theme.components.text_input.padding_h
    );
    assert_eq!(
        dimension_token::<D>("component.text_input.radius"),
        theme.components.text_input.radius
    );
    assert_eq!(
        dimension_token::<D>("component.text_input.font_size"),
        theme.components.text_input.font_size
    );
    assert_eq!(
        dimension_token::<D>("component.text_input.border_width"),
        theme.components.text_input.border_width
    );

    let card = theme
        .components
        .card
        .resolve(theme.components.card.default_pattern, false);
    let hovered_card = theme
        .components
        .card
        .resolve(theme.components.card.default_pattern, true);
    assert_eq!(
        dimension_token::<D>("component.card.padding"),
        theme.components.card.padding
    );
    assert_eq!(
        dimension_token::<D>("component.card.radius"),
        theme.components.card.radius
    );
    assert_eq!(
        dimension_token::<D>("component.card.border_width"),
        card.border.as_ref().map(|border| border.width).unwrap()
    );
    assert_eq!(
        shadow_token::<D>("component.card.elevation_rest"),
        card.shadows.as_slice()
    );
    assert_eq!(
        shadow_token::<D>("component.card.elevation_hover"),
        hovered_card.shadows.as_slice()
    );

    assert_eq!(
        dimension_token::<D>("component.tabs.indicator_height"),
        theme.components.tabs.indicator_height
    );
    assert_eq!(
        dimension_token::<D>("component.modal.radius"),
        theme.components.modal.radius
    );
    assert_eq!(
        dimension_token::<D>("component.modal.max_width"),
        theme.components.modal.max_width
    );
    assert_eq!(
        shadow_token::<D>("component.modal.shadow"),
        theme.components.modal.container_style.shadows.as_slice()
    );

    assert_eq!(
        dimension_token::<D>("component.badge.radius"),
        theme.components.badge.radius
    );
    assert_eq!(
        dimension_token::<D>("component.badge.font_size"),
        theme.components.badge.font_size
    );
    assert_eq!(
        dimension_token::<D>("component.tooltip.radius"),
        theme.components.tooltip.radius
    );
    assert_eq!(
        dimension_token::<D>("component.tooltip.font_size"),
        theme.components.tooltip.font_size
    );
    assert_eq!(
        dimension_token::<D>("component.progress.height"),
        theme.components.progress.height
    );
}

#[test]
fn default_theme_is_generated_from_bundled_dsp() {
    let theme = Theme::default();

    assert_eq!(theme.design_system.info.name, "fission-design-system");
    assert_eq!(theme.design_system.mode, DesignMode::Light);
    assert_eq!(theme.tokens.colors.primary.r, 15);
    assert_eq!(theme.tokens.colors.primary.g, 118);
    assert_eq!(theme.tokens.colors.primary.b, 110);
    assert_eq!(theme.components.button.radius, 10.0);
    assert!(theme
        .design_system
        .tokens
        .tokens
        .iter()
        .any(|token| token.path == "color.teal.700"));
}

#[test]
fn dark_theme_is_generated_from_bundled_dsp() {
    let theme = Theme::dark();

    assert_eq!(theme.design_system.mode, DesignMode::Dark);
    assert_eq!(theme.tokens.colors.background.r, 2);
    assert_eq!(theme.tokens.colors.background.g, 6);
    assert_eq!(theme.tokens.colors.background.b, 23);
    assert_eq!(theme.tokens.colors.primary.r, 45);
    assert_eq!(theme.tokens.colors.primary.g, 212);
    assert_eq!(theme.tokens.colors.primary.b, 191);
}

#[test]
fn default_tokens_and_default_token_themes_use_generated_authority() {
    let light_tokens = Tokens::default();
    let light = Theme::default();
    assert_eq!(light_tokens, light.tokens);
    assert_eq!(
        Theme::from_tokens(light_tokens, DesignMode::Light),
        light,
        "default light tokens must preserve the generated component recipes and metadata"
    );

    let dark_tokens = Tokens::dark();
    let dark = Theme::dark();
    assert_eq!(dark_tokens, dark.tokens);
    assert_eq!(
        Theme::from_tokens(dark_tokens, DesignMode::Dark),
        dark,
        "default dark tokens must preserve the generated component recipes and metadata"
    );
}

#[test]
fn custom_tokens_still_derive_the_compatibility_component_recipe() {
    let mut tokens = Tokens::default();
    tokens.colors.primary = tokens.colors.error;

    let theme = Theme::from_tokens(tokens.clone(), DesignMode::Light);
    let primary = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Default,
    );

    assert_eq!(theme.tokens, tokens);
    assert_eq!(primary.background, Some(Fill::Solid(tokens.colors.primary)));
    assert!(theme.design_system.info.name.is_empty());
}

#[test]
fn default_component_geometry_matches_the_compact_recipe() {
    let theme = Theme::default();

    let button = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Default,
    );
    assert_eq!(button.height, Some(32.0));
    assert_eq!(button.padding_x, Some(10.0));
    assert_eq!(button.padding_y, Some(0.0));
    assert_eq!(button.gap, Some(6.0));
    assert_eq!(button.radius, Some(10.0));
    assert_eq!(button.font_size, Some(14.0));
    assert_eq!(button.font_weight, Some(500));
    assert_eq!(button.line_height, Some(20.0));
    assert_eq!(button.icon_size, Some(16.0));
    assert!(button.shadows.is_empty());
    assert_eq!(theme.components.button.elevation_rest, None);
    assert_eq!(theme.components.button.elevation_hover, None);
    assert_eq!(theme.components.button.elevation_pressed, None);
    assert_eq!(
        theme
            .components
            .button
            .resolve(
                ButtonHierarchy::Primary,
                ComponentSize::Sm,
                ComponentState::Default,
            )
            .height,
        Some(28.0)
    );
    assert_eq!(
        theme
            .components
            .button
            .resolve(
                ButtonHierarchy::Primary,
                ComponentSize::Lg,
                ComponentState::Default,
            )
            .height,
        Some(36.0)
    );
    let focused_button = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Focus,
    );
    assert_eq!(
        focused_button.border.as_ref().map(|border| border.width),
        Some(1.0)
    );
    assert_eq!(focused_button.shadows.len(), 1);
    assert_eq!(focused_button.shadows[0].spread_radius, 3.0);
    assert_eq!(
        theme
            .components
            .button
            .transition
            .as_ref()
            .map(|motion| motion.duration_ms),
        Some(150)
    );

    let input = theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Default);
    assert_eq!(input.height, Some(32.0));
    assert_eq!(input.padding_x, Some(10.0));
    assert_eq!(input.padding_y, Some(4.0));
    assert_eq!(input.radius, Some(10.0));
    assert_eq!(input.font_size, Some(14.0));
    assert_eq!(input.font_weight, Some(400));
    assert_eq!(input.line_height, Some(20.0));
    assert_eq!(
        input.background,
        Some(Fill::Solid(fission_theme::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }))
    );
    assert!(input.shadows.is_empty());
    let focused_input = theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Focus);
    assert_eq!(
        focused_input.border.as_ref().map(|border| border.width),
        Some(1.0)
    );
    assert_eq!(focused_input.shadows.len(), 1);
    assert_eq!(focused_input.shadows[0].spread_radius, 3.0);
    let invalid_input = theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Error);
    assert_eq!(invalid_input.shadows.len(), 1);
    assert_eq!(invalid_input.shadows[0].spread_radius, 3.0);
    assert_eq!(invalid_input.shadows[0].color.a, 51);
    assert_eq!(
        invalid_input.shadows[0].color.r,
        theme.tokens.colors.error.r
    );
    assert_eq!(
        invalid_input.shadows[0].color.g,
        theme.tokens.colors.error.g
    );
    assert_eq!(
        invalid_input.shadows[0].color.b,
        theme.tokens.colors.error.b
    );

    let card = theme.components.card.resolve(CardPattern::Raised, false);
    assert_eq!(theme.components.card.padding, 16.0);
    assert_eq!(theme.components.card.radius, 14.0);
    assert!(card.shadows.is_empty());

    let tab = theme
        .components
        .tabs
        .resolve_tab(ComponentSize::Md, ComponentState::Active);
    assert_eq!(theme.components.tabs.indicator_height, 0.0);
    assert_eq!(tab.height, Some(25.0));
    assert_eq!(tab.padding_x, Some(6.0));
    assert_eq!(tab.padding_y, Some(2.0));
    assert_eq!(tab.gap, Some(6.0));
    assert_eq!(tab.radius, Some(8.0));
    assert_eq!(tab.font_size, Some(14.0));
    assert_eq!(tab.font_weight, Some(500));
    assert_eq!(tab.line_height, Some(20.0));
    assert!(!tab.shadows.is_empty());
    assert_eq!(theme.components.tabs.track_style.radius, Some(10.0));
    assert_eq!(theme.components.tabs.track_style.padding, Some([3.0; 4]));
    assert_eq!(theme.components.tabs.track_style.gap, Some(0.0));

    assert_eq!(theme.components.modal.max_width, 384.0);
    assert_eq!(theme.components.modal.radius, 14.0);
    assert_eq!(theme.components.modal.shadow, None);
    assert_eq!(
        theme.components.modal.container_style.max_width,
        Some(384.0)
    );
    assert!(theme.components.modal.container_style.shadows.is_empty());
    assert_eq!(theme.components.modal.scrim_blur, 4.0);
    assert_eq!(
        theme.components.modal.scrim_style.background,
        Some(Fill::Solid(fission_theme::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 26,
        }))
    );

    assert_component_alias_parity::<FissionDefaultDesignSystem>();
}

#[test]
fn generated_component_colours_follow_light_and_dark_semantics() {
    let light = Theme::default();
    let dark = Theme::dark();

    for theme in [&light, &dark] {
        let button = theme.components.button.resolve(
            ButtonHierarchy::Primary,
            ComponentSize::Md,
            ComponentState::Default,
        );
        assert_eq!(
            button.background,
            Some(Fill::Solid(theme.tokens.colors.primary))
        );
        assert_eq!(button.text_color, Some(theme.tokens.colors.on_primary));
        let focused_button = theme.components.button.resolve(
            ButtonHierarchy::Primary,
            ComponentSize::Md,
            ComponentState::Focus,
        );
        assert_eq!(
            focused_button.border.as_ref().map(|border| &border.fill),
            Some(&Fill::Solid(theme.tokens.colors.focus_ring))
        );

        let destructive = theme.components.button.resolve(
            ButtonHierarchy::Destructive,
            ComponentSize::Md,
            ComponentState::Default,
        );
        assert_eq!(
            destructive.background,
            Some(Fill::Solid(theme.tokens.colors.error.with_alpha(26)))
        );
        assert_eq!(destructive.text_color, Some(theme.tokens.colors.error));

        let input = theme
            .components
            .text_input
            .resolve(ComponentSize::Md, ComponentState::Default);
        assert_eq!(input.text_color, Some(theme.tokens.colors.text_primary));
        assert_eq!(
            input.border.as_ref().map(|border| &border.fill),
            Some(&Fill::Solid(theme.tokens.colors.border))
        );
        let focused_input = theme
            .components
            .text_input
            .resolve(ComponentSize::Md, ComponentState::Focus);
        assert_eq!(
            focused_input.border.as_ref().map(|border| &border.fill),
            Some(&Fill::Solid(theme.tokens.colors.focus_ring))
        );

        let card = theme.components.card.resolve(CardPattern::Raised, false);
        assert_eq!(
            card.background,
            Some(Fill::Solid(theme.tokens.colors.surface))
        );
        assert_eq!(
            card.border.as_ref().map(|border| &border.fill),
            Some(&Fill::Solid(theme.tokens.colors.border))
        );

        let tab = theme
            .components
            .tabs
            .resolve_tab(ComponentSize::Md, ComponentState::Active);
        assert_eq!(
            tab.background,
            Some(Fill::Solid(theme.tokens.colors.surface))
        );
        assert_eq!(tab.text_color, Some(theme.tokens.colors.text_primary));
        assert_eq!(
            theme.components.tabs.track_style.background,
            Some(Fill::Solid(theme.tokens.colors.surface_sunken))
        );

        assert_eq!(
            theme.components.modal.container_style.background,
            Some(Fill::Solid(theme.tokens.colors.surface))
        );
    }

    assert_ne!(light.tokens.colors.primary, dark.tokens.colors.primary);
    assert_ne!(light.tokens.colors.surface, dark.tokens.colors.surface);
}

#[test]
fn generated_design_system_exposes_components_patterns_and_assets() {
    assert!(FissionDefaultDesignSystem::components()
        .iter()
        .any(|component| component.name == "button"));
    assert!(FissionDefaultDesignSystem::patterns()
        .iter()
        .any(|pattern| pattern.name == "marketing_hero"));
    assert!(FissionDefaultDesignSystem::assets()
        .logos
        .iter()
        .any(|asset| asset.id == "wordmark.dark"));
}

#[test]
fn generated_design_system_embeds_declared_font_weights() {
    let fonts = FissionDefaultDesignSystem::font_faces();
    let weights = fonts.iter().map(|font| font.weight).collect::<Vec<_>>();

    assert_eq!(weights, vec![400, 500, 600, 700]);
    assert!(fonts
        .iter()
        .all(|font| font.family == "Inter" && !font.data.is_empty()));
}

#[test]
fn generated_theme_resolves_dsp_component_model() {
    let theme = Theme::default();

    let primary_button = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Default,
    );
    let hover_button = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Hover,
    );
    let destructive_button = theme.components.button.resolve(
        ButtonHierarchy::Destructive,
        ComponentSize::Lg,
        ComponentState::Default,
    );
    assert_ne!(primary_button.background, hover_button.background);
    assert!(destructive_button.background.is_some());
    assert_eq!(destructive_button.height, Some(36.0));

    let focused_input = theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Focus);
    assert_eq!(
        focused_input.border.as_ref().map(|border| border.width),
        Some(1.0)
    );

    let badge = theme
        .components
        .badge
        .resolve(BadgeTone::Success, ComponentSize::Sm);
    assert_eq!(badge.height, Some(20.0));
    assert!(badge.border.is_some());

    let card = theme.components.card.resolve(CardPattern::Raised, false);
    assert!(card.background.is_some());
    assert!(card.shadows.is_empty());

    assert!(theme.tokens.data_visualization.palette.len() >= 4);
}

#[test]
fn generated_dark_input_uses_dark_readable_text_tokens() {
    let theme = Theme::dark();
    let input = theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Default);

    assert_eq!(input.text_color, Some(theme.tokens.colors.text_primary));
    assert_ne!(
        input.text_color,
        Some(theme.tokens.colors.background),
        "dark input text must not collapse to the dark background color"
    );
}

#[test]
fn generated_dark_buttons_use_dark_readable_text_tokens() {
    let theme = Theme::dark();

    let secondary = theme.components.button.resolve(
        ButtonHierarchy::SecondaryGray,
        ComponentSize::Md,
        ComponentState::Default,
    );
    assert_eq!(secondary.text_color, Some(theme.tokens.colors.text_primary));
    assert_eq!(
        secondary.border.as_ref().map(|border| &border.fill),
        Some(&fission_theme::Fill::Solid(theme.tokens.colors.border))
    );

    let tertiary = theme.components.button.resolve(
        ButtonHierarchy::TertiaryGray,
        ComponentSize::Md,
        ComponentState::Default,
    );
    assert_eq!(
        tertiary.text_color,
        Some(theme.tokens.colors.text_secondary)
    );

    let disabled = theme.components.button.resolve(
        ButtonHierarchy::Primary,
        ComponentSize::Md,
        ComponentState::Disabled,
    );
    assert_eq!(disabled.text_color, Some(theme.tokens.colors.text_muted));
    assert_eq!(
        disabled.background,
        Some(fission_theme::Fill::Solid(
            theme.tokens.colors.surface_sunken
        ))
    );
}

#[test]
fn bundled_standard_design_system_presets_generate_themes() {
    let presets: &[(&str, fn(DesignMode) -> Theme)] = &[
        (
            "material-design-3",
            FissionMaterialDesign3DesignSystem::theme,
        ),
        ("fluent-2", FissionFluent2DesignSystem::theme),
        ("liquid-glass", FissionLiquidGlassDesignSystem::theme),
        ("cupertino", FissionCupertinoDesignSystem::theme),
    ];

    for (name, theme_for_mode) in presets {
        let light = theme_for_mode(DesignMode::Light);
        let dark = theme_for_mode(DesignMode::Dark);

        assert_eq!(&light.design_system.info.name, name);
        assert_eq!(light.design_system.mode, DesignMode::Light);
        assert_eq!(dark.design_system.mode, DesignMode::Dark);
        assert_ne!(
            light.tokens.colors.background,
            dark.tokens.colors.background
        );
        assert!(!light.tokens.data_visualization.palette.is_empty());
        assert!(light
            .components
            .button
            .resolve(
                ButtonHierarchy::Primary,
                ComponentSize::Md,
                ComponentState::Default,
            )
            .background
            .is_some());
    }

    assert_component_alias_parity::<FissionMaterialDesign3DesignSystem>();
    assert_component_alias_parity::<FissionFluent2DesignSystem>();
    assert_component_alias_parity::<FissionLiquidGlassDesignSystem>();
    assert_component_alias_parity::<FissionCupertinoDesignSystem>();

    let button_radii = [
        FissionMaterialDesign3DesignSystem::theme(DesignMode::Light)
            .components
            .button
            .radius,
        FissionFluent2DesignSystem::theme(DesignMode::Light)
            .components
            .button
            .radius,
        FissionLiquidGlassDesignSystem::theme(DesignMode::Light)
            .components
            .button
            .radius,
        FissionCupertinoDesignSystem::theme(DesignMode::Light)
            .components
            .button
            .radius,
    ];
    assert_eq!(button_radii, [20.0, 4.0, 18.0, 10.0]);
}
