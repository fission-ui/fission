//! Foundation token groups: sizing, opacity, breakpoints and layers, the
//! half-step spacing values, and the loading state.
//!
//! Only the default design system declares the newer groups. The other bundled
//! presets are written against the earlier DSP format, so they stand in for any
//! app's own design system: they must keep generating, and they must get
//! Fission's default values for everything they leave out.

use fission_theme::{
    ComponentState, ComponentStateStyles, DesignMode, DesignSystem, FissionCupertinoDesignSystem,
    FissionDefaultDesignSystem, FissionFluent2DesignSystem, FissionLiquidGlassDesignSystem,
    FissionMaterialDesign3DesignSystem, ResolvedComponentStyle, SpacingTokens, Tokens, WindowClass,
};

fn tokens<D: DesignSystem>() -> Tokens {
    D::theme(DesignMode::Light).tokens.clone()
}

#[test]
fn default_design_system_declares_the_foundation_groups() {
    let tokens = tokens::<FissionDefaultDesignSystem>();

    assert_eq!(tokens.sizing.min_pointer_target, 24.0);
    assert_eq!(tokens.sizing.min_touch_target, 48.0);
    // Declared at 36px; Fission starts one density step smaller.
    assert_eq!(tokens.sizing.control_md, 32.0);
    assert_eq!(tokens.sizing.icon_md, 20.0);
    assert_eq!(tokens.opacity.hover_layer, 0.08);
    assert_eq!(tokens.opacity.pressed_layer, 0.12);
    assert_eq!(tokens.breakpoints.compact_max, 600.0);
    assert!(tokens.layers.modal > tokens.layers.dropdown);
    assert!(tokens.layers.tooltip > tokens.layers.toast);
    assert_eq!(
        (tokens.spacing.xxs, tokens.spacing.ms, tokens.spacing.ml),
        (2.0, 12.0, 20.0)
    );
}

#[test]
fn design_systems_without_the_foundation_groups_get_the_defaults() {
    let defaults = tokens::<FissionDefaultDesignSystem>();
    for (name, tokens) in [
        ("material3", tokens::<FissionMaterialDesign3DesignSystem>()),
        ("fluent2", tokens::<FissionFluent2DesignSystem>()),
        ("cupertino", tokens::<FissionCupertinoDesignSystem>()),
        ("liquid-glass", tokens::<FissionLiquidGlassDesignSystem>()),
    ] {
        assert_eq!(
            tokens.sizing.min_touch_target, defaults.sizing.min_touch_target,
            "{name} touch target"
        );
        assert_eq!(
            tokens.sizing.min_pointer_target, defaults.sizing.min_pointer_target,
            "{name} pointer target"
        );
        assert_eq!(tokens.opacity, defaults.opacity, "{name} opacity");
        assert_eq!(
            tokens.breakpoints, defaults.breakpoints,
            "{name} breakpoints"
        );
        assert_eq!(tokens.layers, defaults.layers, "{name} layers");
        assert_eq!(tokens.spacing.ms, 12.0, "{name} spacing.ms");
    }
}

#[test]
fn control_height_follows_a_design_system_that_only_declares_its_button() {
    // Material 3's DSP sizes its medium button but has no sizing.control
    // tokens, so the sizing group takes the button's height rather than
    // Fission's.
    let material = FissionMaterialDesign3DesignSystem::theme(DesignMode::Light);
    let button_md = material
        .components
        .button
        .size_style(fission_theme::ComponentSize::Md)
        .height
        .expect("material3 declares a medium button height");
    assert_eq!(material.tokens.sizing.control_md, button_md);
}

#[test]
fn rust_defaults_match_the_default_design_system() {
    let generated = tokens::<FissionDefaultDesignSystem>();

    assert_eq!(Tokens::default(), generated);
    assert_eq!(SpacingTokens::default(), generated.spacing);
    assert_eq!(fission_theme::ColorTokens::default(), generated.colors);
    assert_eq!(
        fission_theme::ColorTokens::dark(),
        FissionDefaultDesignSystem::theme(DesignMode::Dark)
            .tokens
            .colors
    );
    assert_eq!(fission_theme::RadiusTokens::default(), generated.radii);
    assert_eq!(fission_theme::MotionTokens::default(), generated.motion);
}

#[test]
fn window_classes_follow_the_breakpoints() {
    let breakpoints = tokens::<FissionDefaultDesignSystem>().breakpoints;

    assert_eq!(breakpoints.class_for(390.0), WindowClass::Compact);
    assert_eq!(breakpoints.class_for(600.0), WindowClass::Medium);
    assert_eq!(breakpoints.class_for(1024.0), WindowClass::Expanded);
    assert_eq!(breakpoints.class_for(1440.0), WindowClass::Large);
    assert_eq!(breakpoints.class_for(2560.0), WindowClass::ExtraLarge);
}

#[test]
fn loading_keeps_the_resting_look_unless_a_recipe_styles_it() {
    let rest = ResolvedComponentStyle {
        opacity: Some(1.0),
        ..Default::default()
    };
    let mut states = ComponentStateStyles {
        default: rest.clone(),
        disabled: Some(ResolvedComponentStyle {
            opacity: Some(0.5),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(states.resolve(ComponentState::Loading), rest);

    states.loading = Some(ResolvedComponentStyle {
        opacity: Some(0.9),
        ..Default::default()
    });
    assert_eq!(
        states.resolve(ComponentState::Loading).opacity,
        Some(0.9),
        "a design system's loading recipe wins"
    );
}
