#[test]
fn semantic_relationship_types_are_available_from_core_and_its_public_facade() {
    let core_popup = fission_core::PopupKind::Menu;
    let core_orientation = fission_core::SemanticOrientation::Horizontal;
    let facade_popup = fission_core::public::PopupKind::ListBox;
    let facade_orientation = fission_core::public::SemanticOrientation::Vertical;

    assert_eq!(core_popup, fission_core::PopupKind::Menu);
    assert_eq!(
        core_orientation,
        fission_core::SemanticOrientation::Horizontal
    );
    assert_eq!(facade_popup, fission_core::PopupKind::ListBox);
    assert_eq!(
        facade_orientation,
        fission_core::SemanticOrientation::Vertical
    );
}
