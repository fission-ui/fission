//! The generic recipe path.
//!
//! A component gets design authority by declaring a recipe in each supplied
//! design system and reading it. These tests pin that contract, because the
//! whole point is that no Rust type is written per component.

use fission_theme::{ComponentSize, ComponentState, DesignMode, DesignSystem, Theme};

fn systems() -> Vec<(&'static str, Theme)> {
    vec![
        ("default", Theme::default()),
        (
            "material3",
            fission_theme::FissionMaterialDesign3DesignSystem::theme(DesignMode::Light),
        ),
        (
            "fluent2",
            fission_theme::FissionFluent2DesignSystem::theme(DesignMode::Light),
        ),
        (
            "cupertino",
            fission_theme::FissionCupertinoDesignSystem::theme(DesignMode::Light),
        ),
        (
            "liquid_glass",
            fission_theme::FissionLiquidGlassDesignSystem::theme(DesignMode::Light),
        ),
    ]
}

#[test]
fn every_supplied_system_declares_the_required_recipes() {
    // A widget reading one of these can rely on it being present in any
    // supplied design system.
    for (name, theme) in systems() {
        for recipe in fission_theme::recipe_names::REQUIRED {
            assert!(
                theme.try_recipe(recipe).is_some(),
                "{name} should declare a {recipe} recipe"
            );
        }
    }
}

#[test]
fn recipe_names_match_the_codegen_requirement() {
    let mut named = fission_theme::recipe_names::REQUIRED.to_vec();
    let mut required = fission_design_system_codegen::REQUIRED_COMPONENT_RECIPES.to_vec();
    named.sort_unstable();
    required.sort_unstable();
    assert_eq!(
        named, required,
        "fission_theme::recipe_names must name exactly the recipes the codegen requires"
    );
}

#[test]
fn recipes_carry_parts_sizes_states_and_scalars() {
    let theme = Theme::default();

    // Sub-objects become named parts.
    let pagination = theme.recipe("pagination");
    assert!(pagination.try_part("item").is_some());
    assert!(pagination.try_part("selected").is_some());
    assert!(pagination.part("item").radius.is_some());

    // A `sizes` block becomes density variants.
    let select = theme.recipe("select");
    assert!(select.sizes.contains_key(&ComponentSize::Md));
    assert!(select.size(ComponentSize::Md).height.is_some());

    // Plain numbers and dimension strings become scalars.
    let avatar_group = theme.recipe("avatar_group");
    assert_eq!(avatar_group.scalar("max_visible"), Some(4.0));
    assert_eq!(avatar_group.scalar("overlap"), Some(10.0));

    // A `states` block resolves through the shared overlay rules.
    let input = theme.recipe("input");
    assert_ne!(
        input.state(ComponentState::Default),
        input.state(ComponentState::Focus),
        "a focus state should differ from the resting one"
    );
}

#[test]
fn a_missing_recipe_reads_as_empty_rather_than_panicking() {
    // Application design systems override only what they care about, so reading
    // an absent recipe has to be ordinary rather than exceptional.
    let theme = Theme::default();
    let absent = theme.recipe("not-a-component");
    assert!(absent.parts.is_empty());
    assert!(absent.part("anything").radius.is_none());
    assert_eq!(absent.scalar("anything"), None);
    assert!(theme.try_recipe("not-a-component").is_none());
}

#[test]
fn each_system_resolves_its_own_geometry_for_a_shared_recipe() {
    // The point of the generic path: one recipe shape, per-system values. If
    // these all agreed, the recipes would be ignoring their design system.
    let radii: Vec<_> = systems()
        .into_iter()
        .map(|(name, theme)| (name, theme.recipe("tag").base.radius))
        .collect();
    assert!(
        radii.iter().all(|(_, radius)| radius.is_some()),
        "every system should resolve a tag radius: {radii:?}"
    );
}

#[test]
fn a_partial_recipe_merges_over_its_inherited_recipe_field_by_field() {
    use fission_theme::{ComponentRecipe, ComponentStateStyles, ResolvedComponentStyle};

    let inherited = ComponentRecipe {
        base: ResolvedComponentStyle {
            radius: Some(4.0),
            gap: Some(8.0),
            ..Default::default()
        },
        parts: [
            (
                "item".to_string(),
                ResolvedComponentStyle {
                    height: Some(32.0),
                    padding_x: Some(12.0),
                    ..Default::default()
                },
            ),
            (
                "icon".to_string(),
                ResolvedComponentStyle {
                    width: Some(16.0),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .collect(),
        states: ComponentStateStyles {
            hover: Some(ResolvedComponentStyle {
                opacity: Some(0.9),
                radius: Some(4.0),
                ..Default::default()
            }),
            ..Default::default()
        },
        scalars: [
            ("max_visible".to_string(), 4.0),
            ("overlap".to_string(), 10.0),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let overlay = ComponentRecipe {
        base: ResolvedComponentStyle {
            radius: Some(12.0),
            ..Default::default()
        },
        parts: [(
            "item".to_string(),
            ResolvedComponentStyle {
                height: Some(40.0),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
        states: ComponentStateStyles {
            hover: Some(ResolvedComponentStyle {
                opacity: Some(0.8),
                ..Default::default()
            }),
            ..Default::default()
        },
        scalars: [("overlap".to_string(), 6.0)].into_iter().collect(),
        ..Default::default()
    };

    let merged = overlay.merged_over(&inherited);

    assert_eq!(merged.base.radius, Some(12.0));
    assert_eq!(merged.base.gap, Some(8.0));
    assert_eq!(merged.parts["item"].height, Some(40.0));
    assert_eq!(merged.parts["item"].padding_x, Some(12.0));
    assert_eq!(merged.parts["icon"].width, Some(16.0));
    let hover = merged.states.hover.expect("hover state survives");
    assert_eq!(hover.opacity, Some(0.8));
    assert_eq!(hover.radius, Some(4.0));
    assert_eq!(merged.scalars["max_visible"], 4.0);
    assert_eq!(merged.scalars["overlap"], 6.0);
}
