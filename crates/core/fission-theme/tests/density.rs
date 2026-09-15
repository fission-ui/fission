//! Density moves control heights one step from a design system's declared,
//! comfortable sizes, for Fission's own design system and any other.

use fission_theme::{
    ComponentSize, Density, DesignMode, DesignSystem, FissionDefaultDesignSystem,
    FissionMaterialDesign3DesignSystem, Theme,
};

fn button_md(theme: &Theme) -> f32 {
    theme
        .components
        .button
        .size_style(ComponentSize::Md)
        .height
        .expect("medium button height")
}

fn input_md(theme: &Theme) -> f32 {
    theme
        .components
        .text_input
        .size_style(ComponentSize::Md)
        .height
        .expect("medium input height")
}

#[test]
fn the_default_design_system_is_comfortable() {
    let theme = FissionDefaultDesignSystem::theme(DesignMode::Light);

    assert_eq!(theme.tokens.density, Density::Comfortable);
    assert_eq!(button_md(&theme), 36.0);
    assert_eq!(input_md(&theme), 36.0);
    assert_eq!(theme.tokens.sizing.control_md, 36.0);
    assert_eq!(theme.tokens.sizing.density_step, 4.0);
}

#[test]
fn compact_and_spacious_move_controls_one_step() {
    let comfortable = FissionDefaultDesignSystem::theme(DesignMode::Light);
    let compact = comfortable.with_density(Density::Compact);
    let spacious = comfortable.with_density(Density::Spacious);

    assert_eq!(compact.tokens.density, Density::Compact);
    assert_eq!(button_md(&compact), 32.0);
    assert_eq!(input_md(&compact), 32.0);
    assert_eq!(compact.tokens.sizing.control_md, 32.0);

    assert_eq!(button_md(&spacious), 40.0);
    assert_eq!(input_md(&spacious), 40.0);
    assert_eq!(spacious.tokens.sizing.control_xl, 52.0);

    // Moving again starts from the current density, not the original.
    assert_eq!(button_md(&compact.with_density(Density::Spacious)), 40.0);
    assert_eq!(
        compact.with_density(Density::Comfortable).components.button,
        comfortable.components.button
    );
}

#[test]
fn heights_never_drop_below_the_pointer_target() {
    let mut theme = FissionDefaultDesignSystem::theme(DesignMode::Light);
    theme.tokens.sizing.density_step = 40.0;

    let compact = theme.with_density(Density::Compact);

    assert_eq!(
        button_md(&compact),
        theme.tokens.sizing.min_pointer_target,
        "a huge step still leaves a hittable control"
    );
}

#[test]
fn display_components_keep_their_declared_size() {
    let comfortable = FissionDefaultDesignSystem::theme(DesignMode::Light);
    let compact = comfortable.with_density(Density::Compact);

    assert_eq!(compact.components.badge, comfortable.components.badge);
    assert_eq!(compact.components.progress, comfortable.components.progress);
    assert_eq!(compact.components.avatar, comfortable.components.avatar);
    for name in ["badge", "progress_bar", "divider", "empty_state", "alert"] {
        assert_eq!(
            compact.components.recipes.get(name),
            comfortable.components.recipes.get(name),
            "{name} recipe"
        );
    }
}

#[test]
fn another_design_system_gets_density_from_its_own_sizes() {
    let material = FissionMaterialDesign3DesignSystem::theme(DesignMode::Light);
    let declared = button_md(&material);

    let compact = material.with_density(Density::Compact);

    assert_eq!(button_md(&compact), declared - 4.0);
}

#[test]
fn the_same_theme_at_the_same_density_is_shared() {
    let theme = FissionDefaultDesignSystem::theme(DesignMode::Light);

    let first = theme.with_density(Density::Compact);
    let second = theme.with_density(Density::Compact);

    assert!(
        std::sync::Arc::ptr_eq(&first.components, &second.components),
        "reapplying density every frame must not rebuild the component themes"
    );
    assert!(std::sync::Arc::ptr_eq(
        &theme.with_density(Density::Comfortable).components,
        &theme.components
    ));
}
