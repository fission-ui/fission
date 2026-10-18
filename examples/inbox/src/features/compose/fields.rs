use crate::model::compose::{
    set_compose_body, set_compose_subject, set_compose_to, set_date_picker_open, set_schedule_date,
    set_schedule_time,
};
use crate::model::{
    InboxState, SetComposeBody, SetComposeSubject, SetComposeTo, SetDatePickerOpen,
    SetScheduleDate, SetScheduleTime,
};
use fission::prelude::*;
use std::sync::Arc;

const KNOWN_RECIPIENTS: [&str; 3] = ["alice@example.com", "bob@example.com", "team@fission.rs"];

fission_component! {
    pub(super) RecipientField<InboxState> { width: f32 } |field, state, cx| {
        let value = state.compose_to.clone();
        let query = value.trim().to_lowercase();
        let suggestions: Vec<String> = if query.is_empty() {
            Vec::new()
        } else {
            KNOWN_RECIPIENTS.iter()
                .filter(|recipient| recipient.contains(&query))
                .map(|recipient| recipient.to_string())
                .collect()
        };
        let exact_match = KNOWN_RECIPIENTS.iter()
            .any(|recipient| recipient.eq_ignore_ascii_case(value.trim()));
        let to_action = cx.bind(SetComposeTo(String::new()), reduce_with!(set_compose_to));
        let pick_recipient = to_action.clone();
    } =>
    FormControl(
        label: Some(cx.tr("compose.to_label")),
        required: true,
        error: state.compose_to_error.clone(),
        child: Combobox(
            id: WidgetId::explicit("compose_to"),
            semantics_identifier: Some("compose.to".into()),
            value: value,
            placeholder: Some(cx.tr("compose.to_label").into()),
            items: suggestions,
            is_open: !query.is_empty() && !exact_match,
            width: field.width,
            max_popup_height: 180.0,
            on_input: to_action,
            on_select: Arc::new(move |recipient| {
                pick_recipient.with_action(&SetComposeTo(recipient))
            }) as Arc<dyn Fn(String) -> ActionEnvelope + Send + Sync>,
        ),
    )
}

fission_component! {
    pub(super) SubjectField<InboxState> |_field, state, cx| =>
    FormControl(
        label: Some(cx.tr("compose.subject_label")),
        child: TextInput(
            id: Some(WidgetId::explicit("compose_subject_input")),
            value: state.compose_subject.clone(),
            placeholder: Some(cx.tr("compose.subject_placeholder").into()),
            on_input: cx.bind(
                SetComposeSubject(String::new()),
                reduce_with!(set_compose_subject),
            ),
        ),
    )
}

fission_component! {
    pub(super) ScheduleFields<InboxState> |_fields, state, cx| {
        let date_action = cx.bind(
            SetScheduleDate(chrono::Local::now().date_naive()),
            reduce_with!(set_schedule_date),
        );
        let time_action = cx.bind(SetScheduleTime(0, 0), reduce_with!(set_schedule_time));
    } =>
    Wrap(
        direction: FlexDirection::Row,
        spacing: cx.view.env().theme.tokens.spacing.s,
        children: [
            DatePicker(
                id: WidgetId::explicit("schedule_date"),
                value: state.schedule_date,
                is_open: state.is_date_picker_open,
                on_change: Arc::new(move |date| {
                    date_action.with_action(&SetScheduleDate(date))
                }) as Arc<dyn Fn(chrono::NaiveDate) -> ActionEnvelope + Send + Sync>,
                on_toggle: cx.bind(
                    SetDatePickerOpen(!state.is_date_picker_open),
                    reduce_with!(set_date_picker_open),
                ),
                on_close: cx.bind(
                    SetDatePickerOpen(false),
                    reduce_with!(set_date_picker_open),
                ),
            ),
            TimePicker(
                hour: state.schedule_time.map_or(9, |(hour, _)| hour),
                minute: state.schedule_time.map_or(0, |(_, minute)| minute),
                on_change: Arc::new(move |hour, minute| {
                    time_action.with_action(&SetScheduleTime(hour, minute))
                }) as Arc<dyn Fn(u32, u32) -> ActionEnvelope + Send + Sync>,
            ),
        ],
    )
}

fission_component! {
    pub(super) AttachmentField<InboxState> |_field, state, cx| =>
    FileUpload(
        label: cx.tr("compose.attach_file"),
        selected_file: state.compose_attachments.first().cloned(),
        browse_semantics_identifier: Some("inbox.compose.attach".into()),
    )
}

fission_component! {
    pub(super) MessageField<InboxState> |_field, state, cx| =>
    FormControl(
        label: Some(cx.tr("compose.message_label")),
        required: true,
        helper: Some(cx.tr("compose.message_helper")),
        child: TextInput(
            id: Some(WidgetId::explicit("compose_body_input")),
            value: state.compose_body.clone(),
            placeholder: Some(cx.tr("compose.message_placeholder").into()),
            on_input: cx.bind(SetComposeBody(String::new()), reduce_with!(set_compose_body)),
            multiline: true,
            height: 160.0,
        ),
    )
}
