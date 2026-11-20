use fission_icons::material;

const CLOSE_ICON: &str = material::navigation::close::regular();

#[test]
fn generated_nested_icon_accessor_resolves_and_returns_svg() {
    assert!(CLOSE_ICON.starts_with("<svg"));
}
