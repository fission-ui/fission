use fission_theme::{
    BadgeTone, ButtonHierarchy, CardPattern, ComponentSize, ComponentState, DesignMode,
    DesignSystem, FissionCupertinoDesignSystem, FissionDefaultDesignSystem,
    FissionFluent2DesignSystem, FissionLiquidGlassDesignSystem, FissionMaterialDesign3DesignSystem,
    Theme,
};

#[test]
fn default_theme_is_generated_from_bundled_dsp() {
    let theme = Theme::default();

    assert_eq!(theme.design_system.info.name, "fission-design-system");
    assert_eq!(theme.design_system.mode, DesignMode::Light);
    assert_eq!(theme.tokens.colors.primary.r, 15);
    assert_eq!(theme.tokens.colors.primary.g, 118);
    assert_eq!(theme.tokens.colors.primary.b, 110);
    assert_eq!(theme.components.button.radius, 8.0);
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
    assert_eq!(destructive_button.height, Some(44.0));

    let focused_input = theme
        .components
        .text_input
        .resolve(ComponentSize::Md, ComponentState::Focus);
    assert_eq!(
        focused_input.border.as_ref().map(|border| border.width),
        Some(2.0)
    );

    let badge = theme
        .components
        .badge
        .resolve(BadgeTone::Success, ComponentSize::Sm);
    assert_eq!(badge.height, Some(20.0));
    assert!(badge.border.is_some());

    let card = theme.components.card.resolve(CardPattern::Raised, false);
    assert!(card.background.is_some());
    assert!(!card.shadows.is_empty());

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
}
