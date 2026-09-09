#[test]
fn semantic_relationship_types_are_available_from_the_facade_root() {
    let popup = fission::PopupKind::Menu;
    let orientation = fission::SemanticOrientation::Horizontal;

    assert_eq!(popup, fission::core::PopupKind::Menu);
    assert_eq!(orientation, fission::core::SemanticOrientation::Horizontal);
}

#[test]
fn semantic_relationship_types_are_available_from_the_prelude() {
    use fission::prelude::*;

    assert_eq!(PopupKind::ListBox, fission::PopupKind::ListBox);
    assert_eq!(
        SemanticOrientation::Vertical,
        fission::SemanticOrientation::Vertical
    );
}
