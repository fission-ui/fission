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

fission_component! {
    pub(crate) TextLabModal<TextLabState> |_modal, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let set_modal_to = with_reducer!(cx.build, SetModalTo(String::new()), set_modal_to);
        let pick_recipient = set_modal_to.clone();
        let select_recipient: Arc<dyn Fn(String) -> ActionEnvelope + Send + Sync> =
            Arc::new(move |value| pick_recipient.with_action(&SetModalTo(value)));
        let close_modal = with_reducer!(cx.build, SetShowModal(false), set_show_modal);
        let modal_options = [
            "alice@example.com",
            "bob@example.com",
            "qa@fission.rs",
            "team@fission.rs",
        ];
        let modal_items = filtered_suggestions(&state.modal_to, &modal_options);
        let modal_has_exact = modal_options
            .iter()
            .any(|value| value.eq_ignore_ascii_case(state.modal_to.trim()));
    } =>
        Modal(
            id: WidgetId::explicit("text_lab_modal"),
            title: "Text Lab Modal".to_string(),
            is_open: state.show_modal,
            on_dismiss: close_modal.clone(),
            close_semantics_identifier: Some("text-lab.modal.close".to_string()),
            surface_semantics_identifier: Some("text-lab.modal".to_string()),
            width: Some(MODAL_MAX_WIDTH),
            actions: vec![
                ModalAction {
                    label: "Cancel".to_string(),
                    on_press: Some(close_modal.clone()),
                    is_primary: false,
                    semantics_identifier: Some("text-lab.modal.cancel".into()),
                },
                ModalAction {
                    label: "Apply".to_string(),
                    on_press: Some(with_reducer!(cx.build, ApplyModal, apply_modal)),
                    is_primary: true,
                    semantics_identifier: Some("text-lab.modal.apply".into()),
                },
            ],
            content: if state.show_modal {
                FocusScope(
                    is_barrier: true,
                    children: [
                        VStack(
                            spacing: tokens.spacing.s,
                            children: [
                                FormControl(
                                    label: Some("To".to_string()),
                                    required: true,
                                    child: Combobox(
                                        id: WidgetId::explicit("text_lab_modal_to"),
                                        semantics_identifier: "text-lab.modal.to".to_string(),
                                        value: state.modal_to.clone(),
                                        placeholder: Some("name@example.com".into()),
                                        items: modal_items,
                                        is_open: !state.modal_to.trim().is_empty()
                                            && !modal_has_exact,
                                        max_popup_height: POPUP_MAX_HEIGHT,
                                        on_input: set_modal_to,
                                        on_select: select_recipient,
                                    ),
                                ),
                                FormControl(
                                    label: Some("Subject".to_string()),
                                    child: TextInput(
                                        id: WidgetId::explicit("text_lab_modal_subject"),
                                        semantics_identifier: "text-lab.modal.subject".to_string(),
                                        value: state.modal_subject.clone(),
                                        placeholder: Some("Subject".into()),
                                        on_input: with_reducer!(
                                            cx.build,
                                            SetModalSubject,
                                            set_modal_subject,
                                        ),
                                    ),
                                ),
                                FormControl(
                                    label: Some("Body".to_string()),
                                    required: true,
                                    helper: Some(
                                        "Exercise multiline and popup interactions here."
                                            .to_string(),
                                    ),
                                    child: TextInput(
                                        id: WidgetId::explicit("text_lab_modal_body"),
                                        semantics_identifier: "text-lab.modal.body".to_string(),
                                        value: state.modal_body.clone(),
                                        placeholder: Some("Type a longer message".into()),
                                        on_input: with_reducer!(
                                            cx.build,
                                            SetModalBody,
                                            set_modal_body,
                                        ),
                                        multiline: true,
                                        height: MODAL_BODY_HEIGHT,
                                    ),
                                ),
                            ],
                        ),
                    ],
                )
            } else {
                Spacer()
            },
        )
}
