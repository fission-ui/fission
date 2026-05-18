use fission_theme::{DesignMode, DesignSystem, FissionDefaultDesignSystem, Theme};

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
