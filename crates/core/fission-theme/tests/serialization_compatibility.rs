use fission_theme::{
    ButtonHierarchy, ComponentStateStyles, EmptyStateTheme, MenuTheme, ResolvedComponentStyle,
    SelectTheme, Theme,
};

#[test]
fn button_hierarchy_additions_preserve_existing_discriminants() {
    let established = [
        ButtonHierarchy::Primary,
        ButtonHierarchy::SecondaryColor,
        ButtonHierarchy::SecondaryGray,
        ButtonHierarchy::TertiaryColor,
        ButtonHierarchy::TertiaryGray,
        ButtonHierarchy::LinkColor,
        ButtonHierarchy::LinkGray,
        ButtonHierarchy::Destructive,
    ];

    for (expected, hierarchy) in established.into_iter().enumerate() {
        assert_eq!(hierarchy as usize, expected);
    }
    assert_eq!(ButtonHierarchy::Outline as usize, established.len());
}

#[test]
fn menu_theme_fields_default_when_deserializing_an_earlier_recipe() {
    let fields = [
        "surface_style",
        "item_states",
        "destructive_item_states",
        "description_style",
        "shortcut_style",
        "metadata_style",
        "group_label_style",
        "separator_style",
        "indicator_style",
        "trigger_sizes",
        "trigger_states",
    ];
    let mut encoded = serde_json::to_value(MenuTheme::default()).unwrap();
    let object = encoded.as_object_mut().unwrap();
    for field in fields {
        assert!(object.remove(field).is_some(), "missing test field {field}");
    }

    let decoded: MenuTheme = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded.surface_style, ResolvedComponentStyle::default());
    assert_eq!(decoded.item_states, ComponentStateStyles::default());
    assert_eq!(
        decoded.destructive_item_states,
        ComponentStateStyles::default()
    );
    assert_eq!(decoded.description_style, ResolvedComponentStyle::default());
    assert_eq!(decoded.shortcut_style, ResolvedComponentStyle::default());
    assert_eq!(decoded.metadata_style, ResolvedComponentStyle::default());
    assert_eq!(decoded.group_label_style, ResolvedComponentStyle::default());
    assert_eq!(decoded.separator_style, ResolvedComponentStyle::default());
    assert_eq!(decoded.indicator_style, ResolvedComponentStyle::default());
    assert!(decoded.trigger_sizes.is_empty());
    assert_eq!(decoded.trigger_states, ComponentStateStyles::default());
}

#[test]
fn component_theme_defaults_new_recipes_from_an_earlier_serialized_theme() {
    let expected_menu = MenuTheme::default();
    let mut encoded = serde_json::to_value(Theme::default()).unwrap();
    let components = encoded
        .get_mut("components")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap();

    assert!(components.remove("menu").is_some());
    assert!(components.remove("select").is_some());
    assert!(components.remove("empty_state").is_some());

    let card = components
        .get_mut("card")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap();
    for field in [
        "sizes",
        "header_style",
        "content_style",
        "footer_style",
        "title_style",
        "description_style",
        "separator_style",
        "selected_style",
    ] {
        assert!(card.remove(field).is_some(), "missing test field {field}");
    }

    let decoded: Theme = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded.components.menu, expected_menu);
    assert_eq!(decoded.components.select, SelectTheme::default());
    assert_eq!(decoded.components.empty_state, EmptyStateTheme::default());
    assert!(decoded.components.card.sizes.is_empty());
    assert_eq!(
        decoded.components.card.header_style,
        ResolvedComponentStyle::default()
    );
    assert_eq!(
        decoded.components.card.content_style,
        ResolvedComponentStyle::default()
    );
    assert_eq!(
        decoded.components.card.footer_style,
        ResolvedComponentStyle::default()
    );
    assert_eq!(
        decoded.components.card.title_style,
        ResolvedComponentStyle::default()
    );
    assert_eq!(
        decoded.components.card.description_style,
        ResolvedComponentStyle::default()
    );
    assert_eq!(
        decoded.components.card.separator_style,
        ResolvedComponentStyle::default()
    );
    assert_eq!(
        decoded.components.card.selected_style,
        ResolvedComponentStyle::default()
    );
}

#[test]
fn empty_state_narrow_recipe_defaults_when_deserializing_an_earlier_theme() {
    let mut encoded = serde_json::to_value(EmptyStateTheme::default()).unwrap();
    let object = encoded.as_object_mut().unwrap();
    assert!(object.remove("narrow_surface_style").is_some());
    assert!(object.remove("narrow_breakpoint").is_some());

    let decoded: EmptyStateTheme = serde_json::from_value(encoded).unwrap();
    assert_eq!(
        decoded.narrow_surface_style,
        ResolvedComponentStyle::default()
    );
    assert_eq!(decoded.narrow_breakpoint, 640.0);
    assert_eq!(decoded.resolve_surface(320.0), decoded.surface_style);
}
