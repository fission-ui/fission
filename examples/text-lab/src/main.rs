use anyhow::Result;
use fission::core::{with_reducer, ActionEnvelope, GlobalState, WidgetId};
use fission::prelude::fission_reducer;
use fission::prelude::DesktopApp;
use fission::widgets::{
    Button, ButtonVariant, Combobox, Container, FocusScope, FormControl, HStack, MenuButton,
    MenuItem, Modal, ModalAction, SafeArea, Scroll, Spacer, Text, TextInput, VStack, Widget,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TextLabState {
    single_line: String,
    multiline: String,
    inline_combobox: String,
    modal_to: String,
    modal_subject: String,
    modal_body: String,
    show_modal: bool,
    menu_open: bool,
    status: String,
}

impl GlobalState for TextLabState {}

#[fission_reducer(SetSingleLine)]
fn on_set_single_line(state: &mut TextLabState, value: String) {
    state.single_line = value;
}

#[fission_reducer(SetMultiline)]
fn on_set_multiline(state: &mut TextLabState, value: String) {
    state.multiline = value;
}

#[fission_reducer(SetInlineCombobox)]
fn on_set_inline_combobox(state: &mut TextLabState, value: String) {
    state.inline_combobox = value;
}

#[fission_reducer(SetModalTo)]
fn on_set_modal_to(state: &mut TextLabState, value: String) {
    state.modal_to = value;
}

#[fission_reducer(SetModalSubject)]
fn on_set_modal_subject(state: &mut TextLabState, value: String) {
    state.modal_subject = value;
}

#[fission_reducer(SetModalBody)]
fn on_set_modal_body(state: &mut TextLabState, value: String) {
    state.modal_body = value;
}

#[fission_reducer(SetShowModal)]
fn on_set_show_modal(state: &mut TextLabState, value: bool) {
    state.show_modal = value;
}

#[fission_reducer(SetMenuOpen)]
fn on_set_menu_open(state: &mut TextLabState, value: bool) {
    state.menu_open = value;
}

#[fission_reducer(MenuPicked)]
fn on_menu_picked(state: &mut TextLabState, value: String) {
    state.status = format!("Menu action: {}", value);
    state.menu_open = false;
}

#[fission_reducer(ApplyModal)]
fn on_apply_modal(state: &mut TextLabState) {
    state.status = format!(
        "Modal applied: to='{}' subject='{}' body_len={}",
        state.modal_to,
        state.modal_subject,
        state.modal_body.len()
    );
    state.show_modal = false;
}

fn filtered_suggestions(query: &str, values: &[&str]) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    values
        .iter()
        .filter(|value| value.to_lowercase().contains(&q))
        .map(|value| (*value).to_string())
        .collect()
}

#[derive(Clone)]
struct TextLabApp;

impl From<TextLabApp> for Widget {
    fn from(_component: TextLabApp) -> Self {
        let (ctx, view) = fission::build::current::<TextLabState>();
        let viewport_width = view.viewport_size().width.max(0.0);
        let content_width = (viewport_width - 56.0).clamp(280.0, 640.0);
        let modal_field_width = (content_width - 40.0).clamp(260.0, 600.0);

        let set_single_line_id =
            with_reducer!(ctx, SetSingleLine(String::new()), on_set_single_line).id;
        let set_multiline_id = with_reducer!(ctx, SetMultiline(String::new()), on_set_multiline).id;
        let set_inline_combobox_id = with_reducer!(
            ctx,
            SetInlineCombobox(String::new()),
            on_set_inline_combobox
        )
        .id;
        let set_modal_to_id = with_reducer!(ctx, SetModalTo(String::new()), on_set_modal_to).id;
        let set_modal_subject_id =
            with_reducer!(ctx, SetModalSubject(String::new()), on_set_modal_subject).id;
        let set_modal_body_id =
            with_reducer!(ctx, SetModalBody(String::new()), on_set_modal_body).id;
        let set_show_modal_id = with_reducer!(ctx, SetShowModal(false), on_set_show_modal).id;
        let set_menu_open_id = with_reducer!(ctx, SetMenuOpen(false), on_set_menu_open).id;
        let menu_picked_id = with_reducer!(ctx, MenuPicked(String::new()), on_menu_picked).id;
        let apply_modal = with_reducer!(ctx, ApplyModal, on_apply_modal);

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

        let modal_options = [
            "alice@example.com",
            "bob@example.com",
            "qa@fission.rs",
            "team@fission.rs",
        ];
        let modal_items = filtered_suggestions(&view.state().modal_to, &modal_options);
        let modal_has_exact = modal_options
            .iter()
            .any(|value| value.eq_ignore_ascii_case(view.state().modal_to.trim()));

        let menu_toggle = ActionEnvelope {
            id: set_menu_open_id,
            payload: serde_json::to_vec(&SetMenuOpen(!view.state().menu_open)).unwrap(),
        };

        let open_modal = ActionEnvelope {
            id: set_show_modal_id,
            payload: serde_json::to_vec(&SetShowModal(true)).unwrap(),
        };

        let close_modal = ActionEnvelope {
            id: set_show_modal_id,
            payload: serde_json::to_vec(&SetShowModal(false)).unwrap(),
        };

        let single_line_input_id = WidgetId::explicit("text_lab_single_line");
        let body_input_id = WidgetId::explicit("text_lab_multiline");
        let modal_subject_input_id = WidgetId::explicit("text_lab_modal_subject");
        let modal_body_input_id = WidgetId::explicit("text_lab_modal_body");

        let content = VStack {
            spacing: Some(14.0),
            children: vec![
                Text::new("Text Lab")
                    .size(28.0)
                    .into(),
                Text::new("Use this harness to validate text-input behavior, wrappers, and event latency traces.")
                    .size(13.0)
                    .into(),
                FormControl {
                    id: None,
                    label: Some("Single-line input".to_string()),
                    required: false,
                    error: None,
                    helper: Some("Try rapid typing, navigation, and selection.".to_string()),
                    child:
                        TextInput {
                            id: Some(single_line_input_id),
                            value: view.state().single_line.clone(),
                            placeholder: Some("Type quickly here".into()),
                            on_change: Some(ActionEnvelope {
                                id: set_single_line_id,
                                payload: Vec::new(),
                            }),
                            width: Some(content_width),
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
                    child:
                        TextInput {
                            id: Some(body_input_id),
                            value: view.state().multiline.clone(),
                            placeholder: Some("Multiline editing area".into()),
                            on_change: Some(ActionEnvelope {
                                id: set_multiline_id,
                                payload: Vec::new(),
                            }),
                            multiline: true,
                            width: Some(content_width),
                            height: Some(120.0),
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
                    child:
                        Combobox {
                            id: WidgetId::explicit("text_lab_inline_combobox"),
                            value: view.state().inline_combobox.clone(),
                            items: inline_items,
                            is_open: !view.state().inline_combobox.trim().is_empty() && !inline_has_exact,
                            width: Some(content_width),
                            max_popup_height: Some(180.0),
                            on_change: Some(ActionEnvelope {
                                id: set_inline_combobox_id,
                                payload: Vec::new(),
                            }),
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
                    spacing: Some(10.0),
                    children: vec![
                        MenuButton {
                            id: WidgetId::explicit("text_lab_menu_button"),
                            label: "Actions".to_string(),
                            is_open: view.state().menu_open,
                            on_toggle: Some(menu_toggle),
                            items: vec![
                                MenuItem {
                                    label: "Mark all as read".to_string(),
                                    icon: None,
                                    on_select: Some(ActionEnvelope {
                                        id: menu_picked_id,
                                        payload: serde_json::to_vec(&MenuPicked("mark_all_read".to_string()))
                                            .unwrap(),
                                    }),
                                },
                                MenuItem {
                                    label: "Archive selected".to_string(),
                                    icon: None,
                                    on_select: Some(ActionEnvelope {
                                        id: menu_picked_id,
                                        payload: serde_json::to_vec(&MenuPicked("archive_selected".to_string()))
                                            .unwrap(),
                                    }),
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
                        .into(),
                    ],
                }
                .into(),
                Spacer {
                    width: None,
                    height: Some(6.0),
                    ..Default::default()
                }
                .into(),
                Text::new(format!("Status: {}", view.state().status))
                    .size(13.0)
                    .into(),
            ],
        }
        .into();

        let modal_content = if view.state().show_modal {
            FocusScope {
                id: None,
                is_barrier: true,
                children: vec![VStack {
                    spacing: Some(10.0),
                    children: vec![
                        FormControl {
                            id: None,
                            label: Some("To".to_string()),
                            required: true,
                            error: None,
                            helper: None,
                            child: Combobox {
                                id: WidgetId::explicit("text_lab_modal_to"),
                                value: view.state().modal_to.clone(),
                                items: modal_items,
                                is_open: !view.state().modal_to.trim().is_empty()
                                    && !modal_has_exact,
                                width: Some(modal_field_width),
                                max_popup_height: Some(180.0),
                                on_change: Some(ActionEnvelope {
                                    id: set_modal_to_id,
                                    payload: Vec::new(),
                                }),
                                on_select: Some(Arc::new(move |value| ActionEnvelope {
                                    id: set_modal_to_id,
                                    payload: serde_json::to_vec(&SetModalTo(value)).unwrap(),
                                })),
                                on_toggle: None,
                            }
                            .into(),
                        }
                        .into(),
                        FormControl {
                            id: None,
                            label: Some("Subject".to_string()),
                            required: false,
                            error: None,
                            helper: None,
                            child: TextInput {
                                id: Some(modal_subject_input_id),
                                value: view.state().modal_subject.clone(),
                                placeholder: Some("Subject".into()),
                                on_change: Some(ActionEnvelope {
                                    id: set_modal_subject_id,
                                    payload: Vec::new(),
                                }),
                                width: Some(modal_field_width),
                                ..Default::default()
                            }
                            .into(),
                        }
                        .into(),
                        FormControl {
                            id: None,
                            label: Some("Body".to_string()),
                            required: true,
                            error: None,
                            helper: Some(
                                "Exercise multiline and popup interactions here.".to_string(),
                            ),
                            child: TextInput {
                                id: Some(modal_body_input_id),
                                value: view.state().modal_body.clone(),
                                placeholder: Some("Type a longer message".into()),
                                on_change: Some(ActionEnvelope {
                                    id: set_modal_body_id,
                                    payload: Vec::new(),
                                }),
                                multiline: true,
                                width: Some(modal_field_width),
                                height: Some(180.0),
                                ..Default::default()
                            }
                            .into(),
                        }
                        .into(),
                    ],
                }
                .into()],
            }
            .into()
        } else {
            fission::core::ui::widgets::spacer::Spacer::default().into()
        };

        let modal = Modal {
            id: WidgetId::explicit("text_lab_modal"),
            title: "Text Lab Modal".to_string(),
            is_open: view.state().show_modal,
            on_dismiss: Some(close_modal.clone()),
            width: Some((viewport_width - 48.0).clamp(320.0, 720.0)),
            actions: vec![
                ModalAction {
                    label: "Cancel".to_string(),
                    on_press: Some(close_modal),
                    is_primary: false,
                },
                ModalAction {
                    label: "Apply".to_string(),
                    on_press: Some(apply_modal),
                    is_primary: true,
                },
            ],
            content: modal_content,
            motion: None,
        }
        .into();

        SafeArea {
            id: None,
            child: Scroll {
                child: Some(
                    Container::new(VStack {
                        spacing: Some(0.0),
                        children: vec![content, modal],
                    })
                    .padding_all(16.0)
                    .into(),
                ),
                show_scrollbar: true,
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}
fn main() -> Result<()> {
    DesktopApp::<TextLabState, _>::new(TextLabApp).run()
}
