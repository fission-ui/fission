use fission_icons::material;

const CLOSE_ICON: &str = material::navigation::close::regular();

#[test]
fn generated_nested_icon_accessor_resolves_and_returns_svg() {
    assert!(CLOSE_ICON.starts_with("<svg"));
}

#[cfg(feature = "reflection")]
#[test]
fn reflection_still_exposes_independently_generated_icons() {
    let icons = material::all_icons();
    assert!(icons.iter().any(|(category, name, variant, accessor)| {
        *category == "navigation"
            && *name == "close"
            && *variant == "regular"
            && accessor() == CLOSE_ICON
    }));
    assert!(icons
        .iter()
        .all(|(_, _, _, accessor)| accessor().starts_with("<svg")));
}
