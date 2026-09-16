//! Controls that sit side by side in a toolbar share one height scale: a
//! select or text field is exactly as tall as a button of the same size, in
//! every design system and at every density.

use fission_theme::{
    ComponentSize, ComponentState, Density, DesignMode, DesignSystem, FissionCupertinoDesignSystem,
    FissionDefaultDesignSystem, FissionEmberDesignSystem, FissionFluent2DesignSystem,
    FissionGraphiteDesignSystem, FissionLiquidGlassDesignSystem,
    FissionMaterialDesign3DesignSystem, Theme,
};

fn presets(mode: DesignMode) -> Vec<(&'static str, Theme)> {
    vec![
        ("Tidewater", FissionDefaultDesignSystem::theme(mode)),
        ("Graphite", FissionGraphiteDesignSystem::theme(mode)),
        ("Ember", FissionEmberDesignSystem::theme(mode)),
        (
            "Material 3",
            FissionMaterialDesign3DesignSystem::theme(mode),
        ),
        ("Fluent 2", FissionFluent2DesignSystem::theme(mode)),
        ("Cupertino", FissionCupertinoDesignSystem::theme(mode)),
        ("Liquid Glass", FissionLiquidGlassDesignSystem::theme(mode)),
    ]
}

const SIZES: [ComponentSize; 4] = [
    ComponentSize::Sm,
    ComponentSize::Md,
    ComponentSize::Lg,
    ComponentSize::Xl,
];

const DENSITIES: [Density; 3] = [Density::Compact, Density::Comfortable, Density::Spacious];

#[test]
fn select_and_text_input_match_button_height_in_every_preset_size_and_density() {
    for mode in [DesignMode::Light, DesignMode::Dark] {
        for (name, declared) in presets(mode) {
            for density in DENSITIES {
                let theme = declared.with_density(density);
                let components = &theme.components;
                for size in SIZES {
                    let button = components
                        .button
                        .size_style(size)
                        .height
                        .unwrap_or_else(|| panic!("{name} {size:?} button height"));
                    let select = components
                        .select
                        .resolve_trigger(size, ComponentState::Default)
                        .height;
                    let input = components.text_input.size_style(size).height;
                    let context = format!("{name} {mode:?} {density:?} {size:?}");
                    assert_eq!(select, Some(button), "{context}: select vs button");
                    // The trigger's content must fit, or layout grows it past the button.
                    let trigger = components
                        .select
                        .resolve_trigger(size, ComponentState::Default);
                    let padding = trigger.padding.unwrap_or([0.0; 4]);
                    let border = trigger.border.as_ref().map_or(0.0, |border| border.width);
                    let content =
                        trigger.line_height.unwrap_or(0.0) + padding[2] + padding[3] + border * 2.0;
                    assert!(
                        content <= button,
                        "{context}: select content {content} overflows height {button}"
                    );
                    assert_eq!(input, Some(button), "{context}: text input vs button");

                    let recipe_height = |recipe: &str| {
                        components
                            .recipes
                            .get(recipe)
                            .and_then(|recipe| recipe.sizes.get(&size))
                            .and_then(|style| style.height)
                    };
                    if let Some(button) = recipe_height("button") {
                        for recipe in ["select", "input"] {
                            if let Some(height) = recipe_height(recipe) {
                                assert_eq!(height, button, "{context}: {recipe} recipe vs button");
                            }
                        }
                    }
                }
            }
        }
    }
}
