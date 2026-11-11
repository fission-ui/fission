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

#[test]
fn semantic_key_action_types_are_available_from_the_facade_and_prelude() {
    let action = fission::KeyAction::new(fission::KeyCode::Char('x'), 7);
    let _: fission::core::KeyBinding = action.binding.clone();
    let _: fission::core::event::KeyBinding = action.binding.clone();
    let _: fission::core::event::KeyAction = action.clone();

    use fission::prelude::*;
    let action = KeyAction::new(KeyCode::Char('k'), 8);
    let _: KeyBinding = action.binding.clone();
    assert_eq!(action.binding.key, KeyCode::Char('k'));
}

#[test]
fn generated_gradient_extension_path_is_available_from_the_theme_namespace() {
    let _: fission::theme::GradientExtend = fission::theme::GradientExtend::Pad;
}
