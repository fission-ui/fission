use std::sync::Arc;

use crate::state::{
    apply_modal, filtered_suggestions, set_modal_body, set_modal_subject, set_modal_to,
    set_show_modal, ApplyModal, SetModalBody, SetModalSubject, SetModalTo, SetShowModal,
    TextLabState,
};
use fission::prelude::*;

const MODAL_MAX_WIDTH: f32 = 720.0;
const MODAL_BODY_HEIGHT: f32 = 180.0;
const POPUP_MAX_HEIGHT: f32 = 180.0;

pub(crate) struct TextLabModal;

impl From<TextLabModal> for Widget {
    fn from(_modal: TextLabModal) -> Self {
        let (ctx, view) = fission::build::current::<TextLabState>();
        let tokens = &view.env().theme.tokens;

        let set_modal_to = with_reducer!(ctx, SetModalTo(String::new()), set_modal_to);
        let set_modal_to_id = set_modal_to.id;
        let set_modal_subject = with_reducer!(ctx, SetModalSubject, set_modal_subject);
        let set_modal_body = with_reducer!(ctx, SetModalBody, set_modal_body);
        let set_show_modal_id = with_reducer!(ctx, SetShowModal(false), set_show_modal).id;
        let apply_modal = with_reducer!(ctx, ApplyModal, apply_modal);

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
        let close_modal = ActionEnvelope {
            id: set_show_modal_id,
            payload: serde_json::to_vec(&SetShowModal(false)).unwrap(),
        };

        let content: Widget = if view.state().show_modal {
            FocusScope {
                id: None,
                is_barrier: true,
                children: widgets![VStack {
                    spacing: Some(tokens.spacing.s),
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
                                width: None,
                                max_popup_height: Some(POPUP_MAX_HEIGHT),
                                on_input: Some(set_modal_to),
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
                                id: Some(WidgetId::explicit("text_lab_modal_subject")),
                                semantics_identifier: Some("text-lab.modal.subject".into()),
                                value: view.state().modal_subject.clone(),
                                placeholder: Some("Subject".into()),
                                on_input: Some(set_modal_subject),
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
                                id: Some(WidgetId::explicit("text_lab_modal_body")),
                                semantics_identifier: Some("text-lab.modal.body".into()),
                                value: view.state().modal_body.clone(),
                                placeholder: Some("Type a longer message".into()),
                                on_input: Some(set_modal_body),
                                multiline: true,
                                height: Some(MODAL_BODY_HEIGHT),
                                ..Default::default()
                            }
                            .into(),
                        }
                        .into(),
                    ],
                }],
            }
            .into()
        } else {
            Spacer::default().into()
        };

        Modal {
            id: WidgetId::explicit("text_lab_modal"),
            title: "Text Lab Modal".to_string(),
            is_open: view.state().show_modal,
            on_dismiss: Some(close_modal.clone()),
            backdrop_semantics_identifier: None,
            close_semantics_identifier: Some("text-lab.modal.close".into()),
            surface_semantics_identifier: Some("text-lab.modal".into()),
            width: Some(MODAL_MAX_WIDTH),
            actions: vec![
                ModalAction {
                    label: "Cancel".to_string(),
                    on_press: Some(close_modal),
                    is_primary: false,
                    semantics_identifier: Some("text-lab.modal.cancel".into()),
                },
                ModalAction {
                    label: "Apply".to_string(),
                    on_press: Some(apply_modal),
                    is_primary: true,
                    semantics_identifier: Some("text-lab.modal.apply".into()),
                },
            ],
            content,
            motion: None,
        }
        .into()
    }
}
