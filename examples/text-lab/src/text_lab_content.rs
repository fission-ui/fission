use std::sync::Arc;

use crate::state::{
    filtered_suggestions, menu_picked, set_inline_combobox, set_menu_open, set_multiline,
    set_show_modal, set_single_line, MenuPicked, SetInlineCombobox, SetMenuOpen, SetMultiline,
    SetShowModal, SetSingleLine, TextLabState,
};
use fission::prelude::*;

const MULTILINE_HEIGHT: f32 = 120.0;
const POPUP_MAX_HEIGHT: f32 = 180.0;

/// Addresses the inline combobox suggests.
const INLINE_OPTIONS: [&str; 6] = [
    "alice@example.com",
    "bob@example.com",
    "carol@example.com",
    "design@fission.rs",
    "ops@fission.rs",
    "team@fission.rs",
];

/// The text-input test harness: single-line, multiline and combobox fields, a
/// menu, a modal launcher and a status line recording the last event.
pub(crate) struct TextLabContent;

impl From<TextLabContent> for Widget {
    fn from(_content: TextLabContent) -> Self {
        let (_, view) = fission::build::current::<TextLabState>();
        VStack {
            spacing: Some(view.env().theme.tokens.spacing.m),
            children: widgets![
                LabHeader,
                SingleLineField,
                MultilineField,
                InlineComboboxField,
                LabActions,
                StatusLine,
            ],
        }
        .into()
    }
}

struct LabHeader;

impl From<LabHeader> for Widget {
    fn from(_header: LabHeader) -> Self {
        let (_, view) = fission::build::current::<TextLabState>();
        let tokens = &view.env().theme.tokens;
        VStack {
            spacing: Some(tokens.spacing.m),
            children: widgets![
                Text::new("Text Lab")
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.text_primary),
                Text::new(
                    "Use this harness to validate text-input behavior, wrappers, and event latency traces.",
                )
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary),
            ],
        }
        .into()
    }
}

struct SingleLineField;

impl From<SingleLineField> for Widget {
    fn from(_field: SingleLineField) -> Self {
        let (ctx, view) = fission::build::current::<TextLabState>();
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
                on_input: Some(with_reducer!(ctx, SetSingleLine, set_single_line)),
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

struct MultilineField;

impl From<MultilineField> for Widget {
    fn from(_field: MultilineField) -> Self {
        let (ctx, view) = fission::build::current::<TextLabState>();
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
                on_input: Some(with_reducer!(ctx, SetMultiline, set_multiline)),
                multiline: true,
                height: Some(MULTILINE_HEIGHT),
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

/// Suggests addresses while typing, and closes once the value matches one exactly.
struct InlineComboboxField;

impl From<InlineComboboxField> for Widget {
    fn from(_field: InlineComboboxField) -> Self {
        let (ctx, view) = fission::build::current::<TextLabState>();
        let value = view.state().inline_combobox.clone();
        let has_exact = INLINE_OPTIONS
            .iter()
            .any(|option| option.eq_ignore_ascii_case(value.trim()));
        let set_value = with_reducer!(ctx, SetInlineCombobox(String::new()), set_inline_combobox);
        let pick_suggestion = set_value.clone();

        FormControl {
            id: None,
            label: Some("Combobox wrapper".to_string()),
            required: false,
            error: None,
            helper: Some("Type to open suggestions and pick via mouse/keyboard.".to_string()),
            child: Combobox {
                id: WidgetId::explicit("text_lab_inline_combobox"),
                semantics_identifier: Some("text-lab.inline-combobox".into()),
                items: filtered_suggestions(&value, &INLINE_OPTIONS),
                is_open: !value.trim().is_empty() && !has_exact,
                value,
                width: None,
                max_popup_height: Some(POPUP_MAX_HEIGHT),
                on_input: Some(set_value),
                on_select: Some(Arc::new(move |picked| {
                    pick_suggestion.with_action(&SetInlineCombobox(picked))
                })),
                on_toggle: None,
            }
            .into(),
        }
        .into()
    }
}

/// The actions menu and the button that opens the modal text flow.
struct LabActions;

impl From<LabActions> for Widget {
    fn from(_actions: LabActions) -> Self {
        let (ctx, view) = fission::build::current::<TextLabState>();
        let menu_open = view.state().menu_open;
        let pick = with_reducer!(ctx, MenuPicked(String::new()), menu_picked);
        HStack {
            spacing: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                MenuButton {
                    id: WidgetId::explicit("text_lab_menu_button"),
                    label: "Actions".to_string(),
                    is_open: menu_open,
                    on_toggle: Some(with_reducer!(ctx, SetMenuOpen(!menu_open), set_menu_open)),
                    trigger_semantics_identifier: Some("text-lab.actions".into()),
                    items: vec![
                        MenuItem {
                            label: "Mark all as read".to_string(),
                            icon: None,
                            on_select: Some(pick.with_action(&MenuPicked("mark_all_read".into()))),
                            semantics_identifier: Some("text-lab.mark-all-read".into()),
                        },
                        MenuItem {
                            label: "Archive selected".to_string(),
                            icon: None,
                            on_select: Some(
                                pick.with_action(&MenuPicked("archive_selected".into())),
                            ),
                            semantics_identifier: Some("text-lab.archive-selected".into()),
                        },
                    ],
                },
                Button {
                    variant: ButtonVariant::Filled,
                    child: Some(Text::new("Open modal text flow").into()),
                    on_press: Some(with_reducer!(ctx, SetShowModal(true), set_show_modal)),
                    ..Default::default()
                }
                .semantics_identifier("text-lab.open-modal"),
            ],
        }
        .into()
    }
}

/// The last event the harness recorded, below a small gap.
struct StatusLine;

impl From<StatusLine> for Widget {
    fn from(_line: StatusLine) -> Self {
        let (_, view) = fission::build::current::<TextLabState>();
        let tokens = &view.env().theme.tokens;
        VStack {
            spacing: Some(tokens.spacing.xs),
            children: widgets![
                Spacer::default(),
                Text::new(format!("Status: {}", view.state().status))
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary),
            ],
        }
        .into()
    }
}
