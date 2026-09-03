use std::sync::Arc;

use crate::state::{
    filtered_suggestions, menu_picked, set_inline_combobox, set_menu_open, set_multiline,
    set_show_modal, set_single_line, MenuPicked, SetInlineCombobox, SetMenuOpen, SetMultiline,
    SetShowModal, SetSingleLine, TextLabState,
};
use fission::prelude::*;

const MULTILINE_HEIGHT: f32 = 120.0;
const POPUP_MAX_HEIGHT: f32 = 180.0;

pub(crate) struct TextLabContent;

impl From<TextLabContent> for Widget {
    fn from(_content: TextLabContent) -> Self {
        let (ctx, view) = fission::build::current::<TextLabState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        let set_single_line = with_reducer!(ctx, SetSingleLine, set_single_line);
        let set_multiline = with_reducer!(ctx, SetMultiline, set_multiline);
        let set_inline_combobox =
            with_reducer!(ctx, SetInlineCombobox(String::new()), set_inline_combobox);
        let set_inline_combobox_id = set_inline_combobox.id;
        let set_show_modal_id = with_reducer!(ctx, SetShowModal(false), set_show_modal).id;
        let set_menu_open_id = with_reducer!(ctx, SetMenuOpen(false), set_menu_open).id;
        let menu_picked_id = with_reducer!(ctx, MenuPicked(String::new()), menu_picked).id;

        let inline_options = [
            "alice@example.com",
            "bob@example.com",
            "carol@example.com",
            "design@fission.rs",
            "ops@fission.rs",
            "team@fission.rs",
        ];
        let inline_items = filtered_suggestions(&view.state().inline_combobox, &inline_options);
        let inline_has_exact = inline_options
            .iter()
            .any(|value| value.eq_ignore_ascii_case(view.state().inline_combobox.trim()));

        let menu_toggle = ActionEnvelope {
            id: set_menu_open_id,
            payload: serde_json::to_vec(&SetMenuOpen(!view.state().menu_open)).unwrap(),
        };
        let open_modal = ActionEnvelope {
            id: set_show_modal_id,
            payload: serde_json::to_vec(&SetShowModal(true)).unwrap(),
        };

        VStack {
            spacing: Some(tokens.spacing.m),
            children: vec![
                Text::new("Text Lab")
                    .size(typography.heading_size)
                    .color(tokens.colors.text_primary)
                    .into(),
                Text::new(
                    "Use this harness to validate text-input behavior, wrappers, and event latency traces.",
                )
                .size(typography.body_medium_size)
                .color(tokens.colors.text_secondary)
                .into(),
                FormControl {
                    id: None,
                    label: Some("Single-line input".to_string()),
                    required: false,
                    error: None,
                    helper: Some("Try rapid typing, navigation, and selection.".to_string()),
                    child: TextInput {
                        id: Some(WidgetId::explicit("text_lab_single_line")),
                        semantics_identifier: Some("text-lab.single-line".into()),
                        value: view.state().single_line.clone(),
                        placeholder: Some("Type quickly here".into()),
                        on_input: Some(set_single_line),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some("Multiline input".to_string()),
                    required: false,
                    error: None,
                    helper: Some("Use enter, arrow keys, and drag selection.".to_string()),
                    child: TextInput {
                        id: Some(WidgetId::explicit("text_lab_multiline")),
                        semantics_identifier: Some("text-lab.multiline".into()),
                        value: view.state().multiline.clone(),
                        placeholder: Some("Multiline editing area".into()),
                        on_input: Some(set_multiline),
                        multiline: true,
                        height: Some(MULTILINE_HEIGHT),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some("Combobox wrapper".to_string()),
                    required: false,
                    error: None,
                    helper: Some("Type to open suggestions and pick via mouse/keyboard.".to_string()),
                    child: Combobox {
                        id: WidgetId::explicit("text_lab_inline_combobox"),
                        value: view.state().inline_combobox.clone(),
                        items: inline_items,
                        is_open: !view.state().inline_combobox.trim().is_empty()
                            && !inline_has_exact,
                        width: None,
                        max_popup_height: Some(POPUP_MAX_HEIGHT),
                        on_input: Some(set_inline_combobox),
                        on_select: Some(Arc::new(move |value| ActionEnvelope {
                            id: set_inline_combobox_id,
                            payload: serde_json::to_vec(&SetInlineCombobox(value)).unwrap(),
                        })),
                        on_toggle: None,
                    }
                    .into(),
                }
                .into(),
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: vec![
                        MenuButton {
                            id: WidgetId::explicit("text_lab_menu_button"),
                            label: "Actions".to_string(),
                            is_open: view.state().menu_open,
                            on_toggle: Some(menu_toggle),
                            trigger_semantics_identifier: Some("text-lab.actions".into()),
                            items: vec![
                                MenuItem {
                                    label: "Mark all as read".to_string(),
                                    icon: None,
                                    on_select: Some(ActionEnvelope {
                                        id: menu_picked_id,
                                        payload: serde_json::to_vec(&MenuPicked(
                                            "mark_all_read".to_string(),
                                        ))
                                        .unwrap(),
                                    }),
                                    semantics_identifier: Some("text-lab.mark-all-read".into()),
                                },
                                MenuItem {
                                    label: "Archive selected".to_string(),
                                    icon: None,
                                    on_select: Some(ActionEnvelope {
                                        id: menu_picked_id,
                                        payload: serde_json::to_vec(&MenuPicked(
                                            "archive_selected".to_string(),
                                        ))
                                        .unwrap(),
                                    }),
                                    semantics_identifier: Some("text-lab.archive-selected".into()),
                                },
                            ],
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(Text::new("Open modal text flow").into()),
                            on_press: Some(open_modal),
                            ..Default::default()
                        }
                        .semantics_identifier("text-lab.open-modal")
                        .into(),
                    ],
                }
                .into(),
                Spacer {
                    height: Some(tokens.spacing.xs),
                    ..Default::default()
                }
                .into(),
                Text::new(format!("Status: {}", view.state().status))
                    .size(typography.body_medium_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ],
        }
        .into()
    }
}
