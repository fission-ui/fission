use std::sync::Arc;

use crate::state::{
    filtered_suggestions, menu_picked, set_inline_combobox, set_menu_open, set_multiline,
    set_show_modal, set_single_line, MenuPicked, SetInlineCombobox, SetMenuOpen, SetMultiline,
    SetShowModal, SetSingleLine, TextLabState,
};
use fission::prelude::*;

const MULTILINE_HEIGHT: f32 = 120.0;
const POPUP_MAX_HEIGHT: f32 = 180.0;

const INLINE_OPTIONS: [&str; 6] = [
    "alice@example.com",
    "bob@example.com",
    "carol@example.com",
    "design@fission.rs",
    "ops@fission.rs",
    "team@fission.rs",
];

fission_component! {
/// The text-input test harness: single-line, multiline and combobox fields, a
/// menu, a modal launcher and a status line recording the last event.
    pub(crate) TextLabContent<TextLabState> |_content, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
            spacing: tokens.spacing.m,
            children: [
                LabHeader(),
                SingleLineField(),
                MultilineField(),
                InlineComboboxField(),
                LabActions(),
                StatusLine(),
            ],
        )
}

fission_component! {
    LabHeader<TextLabState> |_header, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
            spacing: tokens.spacing.m,
            children: [
                Text("Text Lab")
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.text_primary),
                Text(
                    "Use this harness to validate text-input behaviour, wrappers, and event latency traces.",
                )
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary),
            ],
        )
}

fission_component! {
    SingleLineField<TextLabState> |_field, state, cx| {} =>
        FormControl(
            label: Some("Single-line input".to_string()),
            helper: Some("Try rapid typing, navigation, and selection.".to_string()),
            child: TextInput(
                id: WidgetId::explicit("text_lab_single_line"),
                semantics_identifier: "text-lab.single-line".to_string(),
                value: state.single_line.clone(),
                placeholder: Some("Type quickly here".into()),
                on_input: with_reducer!(cx.build, SetSingleLine, set_single_line),
            ),
        )
}

fission_component! {
    MultilineField<TextLabState> |_field, state, cx| {} =>
        FormControl(
            label: Some("Multiline input".to_string()),
            helper: Some("Use enter, arrow keys, and drag selection.".to_string()),
            child: TextInput(
                id: WidgetId::explicit("text_lab_multiline"),
                semantics_identifier: "text-lab.multiline".to_string(),
                value: state.multiline.clone(),
                placeholder: Some("Multiline editing area".into()),
                on_input: with_reducer!(cx.build, SetMultiline, set_multiline),
                multiline: true,
                height: MULTILINE_HEIGHT,
            ),
        )
}

fission_component! {
/// Suggests addresses while typing, and closes once the value matches one exactly.
    InlineComboboxField<TextLabState> |_field, state, cx| {
        let value = state.inline_combobox.clone();
        let has_exact = INLINE_OPTIONS
            .iter()
            .any(|option| option.eq_ignore_ascii_case(value.trim()));
        let set_value = with_reducer!(
            cx.build,
            SetInlineCombobox(String::new()),
            set_inline_combobox,
        );
        let pick_suggestion = set_value.clone();
        let select_suggestion: Arc<dyn Fn(String) -> ActionEnvelope + Send + Sync> =
            Arc::new(move |picked| {
                pick_suggestion.with_action(&SetInlineCombobox(picked))
            });
    } =>
        FormControl(
            label: Some("Combobox wrapper".to_string()),
            helper: Some("Type to open suggestions and pick via mouse/keyboard.".to_string()),
            child: Combobox(
                id: WidgetId::explicit("text_lab_inline_combobox"),
                semantics_identifier: "text-lab.inline-combobox".to_string(),
                items: filtered_suggestions(&value, &INLINE_OPTIONS),
                is_open: !value.trim().is_empty() && !has_exact,
                value: value,
                placeholder: Some("Start typing an address".into()),
                max_popup_height: POPUP_MAX_HEIGHT,
                on_input: set_value,
                on_select: select_suggestion,
            ),
        )
}

fission_component! {
/// The actions menu and the button that opens the modal text flow.
    LabActions<TextLabState> |_actions, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let pick = with_reducer!(cx.build, MenuPicked(String::new()), menu_picked);
    } =>
        HStack(
            spacing: tokens.spacing.s,
            children: [
                MenuButton(
                    id: WidgetId::explicit("text_lab_menu_button"),
                    label: "Actions".to_string(),
                    is_open: state.menu_open,
                    on_toggle: with_reducer!(
                        cx.build,
                        SetMenuOpen(!state.menu_open),
                        set_menu_open,
                    ),
                    trigger_semantics_identifier: Some("text-lab.actions".to_string()),
                    items: vec![
                        MenuItem {
                            label: "Mark all as read".to_string(),
                            on_select: Some(
                                pick.with_action(&MenuPicked("mark_all_read".into())),
                            ),
                            semantics_identifier: Some("text-lab.mark-all-read".into()),
                            ..Default::default()
                        },
                        MenuItem {
                            label: "Archive selected".to_string(),
                            on_select: Some(
                                pick.with_action(&MenuPicked("archive_selected".into())),
                            ),
                            semantics_identifier: Some("text-lab.archive-selected".into()),
                            ..Default::default()
                        },
                    ],
                ),
                Button(
                    variant: ButtonVariant::Filled,
                    child: Text("Open modal text flow"),
                    on_press: with_reducer!(
                        cx.build,
                        SetShowModal(true),
                        set_show_modal,
                    ),
                ).semantics_identifier("text-lab.open-modal"),
            ],
        )
}

fission_component! {
/// The last event the harness recorded. Nothing is shown until an event arrives.
    StatusLine<TextLabState> |_line, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        if state.status.is_empty() {
            Spacer()
        } else {
            VStack(
                spacing: tokens.spacing.xs,
                children: [
                    Spacer(),
                    Text(format!("Status: {}", state.status))
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary),
                ],
            )
        }
}
