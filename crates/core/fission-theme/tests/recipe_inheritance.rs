//! An inheriting design system layers its component recipes over the ones it
//! inherits, so changing one property keeps the rest of the component.

include!(concat!(
    env!("OUT_DIR"),
    "/generated_inheritance_fixture_design_system.rs"
));

use fission_theme::{recipes, DesignMode, DesignSystem, Theme};

#[test]
fn changing_one_recipe_property_keeps_the_inherited_anatomy() {
    let inherited = Theme::default();
    let custom = InheritanceFixtureDesignSystem::theme(DesignMode::Light);
    let base = inherited.recipe(recipes::Button);
    let button = custom.recipe(recipes::Button);

    assert!(
        !base.parts.is_empty(),
        "the default button declares parts to inherit"
    );
    assert_eq!(
        button.base.radius,
        Some(20.0),
        "the declared property applies"
    );
    assert_eq!(button.parts, base.parts, "inherited parts survive");
    assert_eq!(button.sizes, base.sizes, "inherited sizes survive");
    assert_eq!(button.states, base.states, "inherited states survive");
    for (name, value) in &base.scalars {
        if name != "radius" {
            assert_eq!(
                button.scalars.get(name),
                Some(value),
                "scalar {name} survives"
            );
        }
    }
    let mut rest = button.base.clone();
    rest.radius = base.base.radius;
    assert_eq!(rest, base.base, "only the declared base property changes");

    assert_eq!(
        custom.recipe(recipes::Pagination).component_recipe(),
        inherited.recipe(recipes::Pagination).component_recipe(),
        "undeclared components are inherited unchanged"
    );
}
