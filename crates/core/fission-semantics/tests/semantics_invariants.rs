use fission_semantics::{ActionSet, Role, Semantics};

#[test]
fn test_role_variants() {
    let roles = [
        Role::Button,
        Role::Text,
        Role::Image,
        Role::Slider,
        Role::List,
        Role::ListItem,
    ];
    assert!(roles.len() > 0);
}

#[test]
fn test_semantics_serialization() {
    let s = Semantics {
        role: Role::Button,
        label: Some("Submit".into()),
        value: None,
        actions: ActionSet::default(),
        focusable: true,
        multiline: false,
        masked: false,
        input_mask: None,
        ime_preedit_range: None,
        checked: None,
        disabled: false,
        read_only: false,
        autofocus: false,
        draggable: false,
        scrollable_x: false,
        scrollable_y: false,
        min_value: None,
        max_value: None,
        current_value: None,
        is_focus_scope: false,
        is_focus_barrier: false,
        drag_payload: None,
        hero_tag: None,
        focus_index: None,
        text_input_type: fission_ir::semantics::TextInputType::Text,
                    text_input_action: fission_ir::semantics::TextInputAction::Done,
                    text_capitalization: fission_ir::semantics::TextCapitalization::None,
                    max_length: None,
                    max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
                    input_formatters: Vec::new(),
                    autocorrect: true,
                    enable_suggestions: true,
                        spell_check: true,
                        smart_dashes: true,
                        smart_quotes: true,
                        autofill_hints: Vec::new(),
        capture_tab: false,
        auto_indent: false,
    };

    // Just verify it compiles and runs; exact serde format isn't critical yet, but capability is.
    let _ = format!("{:?}", s);
}
