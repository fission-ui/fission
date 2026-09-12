//! Theme cloning cost.
//!
//! A `Theme` is cloned per SSR request and per generated site document, so the
//! recipe set must not be deep-copied with it.

use fission_theme::Theme;

#[test]
fn cloning_a_theme_shares_its_recipes() {
    let theme = Theme::default();
    let clone = theme.clone();
    assert!(
        std::sync::Arc::ptr_eq(&theme.components.recipes, &clone.components.recipes),
        "a theme clone must share the recipe set rather than deep-copy ~140 KB per request"
    );
}

#[test]
fn recipe_and_part_lookups_borrow() {
    // Widgets call these on every build, so they must not clone a 3 KB recipe
    // or a 400 byte style each time.
    let theme = Theme::default();
    let first = theme.recipe("button") as *const _;
    let second = theme.recipe("button") as *const _;
    assert_eq!(first, second, "recipe lookup should borrow from the theme");

    let recipe = theme.recipe("pagination");
    let a = recipe.part("item") as *const _;
    let b = recipe.part("item") as *const _;
    assert_eq!(a, b, "part lookup should borrow from the recipe");

    // A missing name is still a borrow, of one shared empty value.
    let missing_a = theme.recipe("not-a-component") as *const _;
    let missing_b = theme.recipe("also-not-a-component") as *const _;
    assert_eq!(
        missing_a, missing_b,
        "absent recipes should share one empty value rather than allocate"
    );
}
