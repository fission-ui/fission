use crate::model::compose::{
    set_compose_body, set_compose_subject, set_compose_to, set_date_picker_open, set_schedule_date,
    set_schedule_time,
};
use crate::model::{
    InboxState, SetComposeBody, SetComposeSubject, SetComposeTo, SetDatePickerOpen,
    SetScheduleDate, SetScheduleTime,
};
use fission::core::op::FlexDirection;
use fission::core::ui::Widget;
use fission::core::{reduce_with, ActionEnvelope, WidgetId};
use fission::widgets::{
    Combobox, DatePicker, FileUpload, FormControl, TextInput, TimePicker, Wrap,
};
use std::sync::Arc;

/// Addresses the recipient field suggests.
const KNOWN_RECIPIENTS: [&str; 3] = ["alice@example.com", "bob@example.com", "team@fission.rs"];

/// Who the message goes to, with suggestions while typing.
pub(super) struct RecipientField {
    pub width: f32,
}

impl From<RecipientField> for Widget {
    fn from(field: RecipientField) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let value = view.state().compose_to.clone();
        let query = value.trim().to_lowercase();
        let suggestions: Vec<String> = if query.is_empty() {
            Vec::new()
        } else {
            KNOWN_RECIPIENTS
                .iter()
                .filter(|recipient| recipient.contains(&query))
                .map(|recipient| recipient.to_string())
                .collect()
        };
        let exact_match = KNOWN_RECIPIENTS
            .iter()
            .any(|recipient| recipient.eq_ignore_ascii_case(value.trim()));
        // A picked suggestion carries its own address, so its action is filled in on select.
        let to_action = ctx.bind(SetComposeTo(String::new()), reduce_with!(set_compose_to));
        let to_id = to_action.id;

        FormControl {
            id: None,
            label: Some(view.tr("compose.to_label")),
            required: true,
            error: None,
            helper: None,
            child: Combobox {
                id: WidgetId::explicit("compose_to"),
                semantics_identifier: Some("compose.to".into()),
                value,
                items: suggestions,
                is_open: !query.is_empty() && !exact_match,
                width: Some(field.width),
                max_popup_height: Some(180.0),
                on_input: Some(to_action),
                on_select: Some(Arc::new(move |recipient| ActionEnvelope {
                    id: to_id,
                    payload: serde_json::to_vec(&SetComposeTo(recipient))
                        .expect("an address always serializes"),
                })),
                on_toggle: None,
            }
            .into(),
        }
        .into()
    }
}

/// The subject line.
pub(super) struct SubjectField;

impl From<SubjectField> for Widget {
    fn from(_: SubjectField) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        FormControl {
            id: None,
            label: Some(view.tr("compose.subject_label")),
            required: false,
            error: None,
            helper: None,
            child: TextInput {
                id: Some(WidgetId::explicit("compose_subject_input")),
                value: view.state().compose_subject.clone(),
                placeholder: Some(view.tr("compose.subject_placeholder").into()),
                on_input: Some(ctx.bind(
                    SetComposeSubject(String::new()),
                    reduce_with!(set_compose_subject),
                )),
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

/// When to send the message.
pub(super) struct ScheduleFields;

impl From<ScheduleFields> for Widget {
    fn from(_: ScheduleFields) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        // Dates and times are chosen by the picker, so their actions are filled in on change.
        let date_id = ctx
            .bind(
                SetScheduleDate(chrono::Local::now().date_naive()),
                reduce_with!(set_schedule_date),
            )
            .id;
        let time_id = ctx
            .bind(SetScheduleTime(0, 0), reduce_with!(set_schedule_time))
            .id;

        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(view.env().theme.tokens.spacing.s),
            run_spacing: None,
            children: vec![
                DatePicker {
                    id: WidgetId::explicit("schedule_date"),
                    value: state.schedule_date,
                    is_open: state.is_date_picker_open,
                    width: None,
                    view_year: None,
                    view_month: None,
                    on_navigate: None,
                    on_change: Some(Arc::new(move |date| ActionEnvelope {
                        id: date_id,
                        payload: serde_json::to_vec(&SetScheduleDate(date))
                            .expect("a date always serializes"),
                    })),
                    on_toggle: Some(ctx.bind(
                        SetDatePickerOpen(!state.is_date_picker_open),
                        reduce_with!(set_date_picker_open),
                    )),
                    on_close: Some(
                        ctx.bind(SetDatePickerOpen(false), reduce_with!(set_date_picker_open)),
                    ),
                }
                .into(),
                TimePicker {
                    hour: state.schedule_time.map_or(9, |(hour, _)| hour),
                    minute: state.schedule_time.map_or(0, |(_, minute)| minute),
                    on_change: Some(Arc::new(move |hour, minute| ActionEnvelope {
                        id: time_id,
                        payload: serde_json::to_vec(&SetScheduleTime(hour, minute))
                            .expect("a time always serializes"),
                    })),
                }
                .into(),
            ],
        }
        .into()
    }
}

/// The first attached file.
pub(super) struct AttachmentField;

impl From<AttachmentField> for Widget {
    fn from(_: AttachmentField) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        FileUpload {
            label: view.tr("compose.attach_file"),
            selected_file: view.state().compose_attachments.first().cloned(),
            on_browse: None,
            browse_semantics_identifier: Some("inbox.compose.attach".into()),
        }
        .into()
    }
}

/// The message body.
pub(super) struct MessageField;

impl From<MessageField> for Widget {
    fn from(_: MessageField) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        FormControl {
            id: None,
            label: Some(view.tr("compose.message_label")),
            required: true,
            error: None,
            helper: Some(view.tr("compose.message_helper")),
            child: TextInput {
                id: Some(WidgetId::explicit("compose_body_input")),
                value: view.state().compose_body.clone(),
                placeholder: Some(view.tr("compose.message_placeholder").into()),
                on_input: Some(ctx.bind(
                    SetComposeBody(String::new()),
                    reduce_with!(set_compose_body),
                )),
                multiline: true,
                height: Some(160.0),
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}
