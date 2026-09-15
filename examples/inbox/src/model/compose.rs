//! Reducers behind the compose modal.

use super::{
    Category, DiscardCompose, Email, EmailMessage, FileSelected, Folder, InboxState, SendCompose,
    SetComposeBody, SetComposeOpen, SetComposeSubject, SetComposeTo, SetDatePickerOpen,
    SetScheduleDate, SetScheduleTime,
};
use chrono::Local;
use fission::core::ReducerContext;
use std::collections::HashSet;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

/// The edited text from an input event, or `fallback` when there was none.
fn edited_text(cx: &Cx<'_, '_, '_>, fallback: String) -> String {
    cx.input
        .text_change()
        .map(|change| change.new_text.clone())
        .unwrap_or(fallback)
}

pub fn set_compose_open(state: &mut InboxState, action: SetComposeOpen, _: &mut Cx<'_, '_, '_>) {
    state.show_compose = action.0;
}

/// Cancel: closes compose and clears the draft, so the next message starts empty.
pub fn discard_compose(state: &mut InboxState, _: DiscardCompose, _: &mut Cx<'_, '_, '_>) {
    state.show_compose = false;
    state.compose_to.clear();
    state.compose_to_error = None;
    state.compose_subject.clear();
    state.compose_body.clear();
    state.compose_attachments.clear();
    state.schedule_date = None;
    state.schedule_time = None;
    state.is_date_picker_open = false;
}

pub fn set_compose_to(state: &mut InboxState, action: SetComposeTo, cx: &mut Cx<'_, '_, '_>) {
    state.compose_to = edited_text(cx, action.0);
    state.compose_to_error = None;
}

/// Whether a recipient looks like an address: a local part, an `@` and a dotted domain.
fn is_address(recipient: &str) -> bool {
    match recipient.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && domain.contains('.')
                && !recipient.contains(char::is_whitespace)
        }
        None => false,
    }
}

pub fn set_compose_subject(
    state: &mut InboxState,
    action: SetComposeSubject,
    cx: &mut Cx<'_, '_, '_>,
) {
    state.compose_subject = edited_text(cx, action.0);
}

pub fn set_compose_body(state: &mut InboxState, action: SetComposeBody, cx: &mut Cx<'_, '_, '_>) {
    state.compose_body = edited_text(cx, action.0);
}

pub fn set_schedule_date(state: &mut InboxState, action: SetScheduleDate, _: &mut Cx<'_, '_, '_>) {
    state.schedule_date = Some(action.0);
    state.is_date_picker_open = false;
}

pub fn set_schedule_time(state: &mut InboxState, action: SetScheduleTime, _: &mut Cx<'_, '_, '_>) {
    state.schedule_time = Some((action.0, action.1));
}

pub fn set_date_picker_open(
    state: &mut InboxState,
    action: SetDatePickerOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.is_date_picker_open = action.0;
}

/// Adds every file dropped on the modal to the draft's attachments.
pub fn add_dropped_files(state: &mut InboxState, _: FileSelected, cx: &mut Cx<'_, '_, '_>) {
    if let Some(paths) = cx.input.as_drop_paths() {
        state.compose_attachments.extend(paths.iter().cloned());
    }
}

/// Files the draft as a new sent thread, clears the draft, closes the modal and
/// confirms with a toast.
pub fn send_compose(state: &mut InboxState, _: SendCompose, _: &mut Cx<'_, '_, '_>) {
    let invalid: Vec<&str> = state
        .compose_to
        .split(',')
        .map(str::trim)
        .filter(|recipient| !recipient.is_empty() && !is_address(recipient))
        .collect();
    if !invalid.is_empty() {
        state.compose_to_error = Some(format!("Not a valid address: {}", invalid.join(", ")));
        return;
    }
    state.compose_to_error = None;
    let subject = match state.compose_subject.trim() {
        "" => "(no subject)".to_string(),
        subject => subject.to_string(),
    };
    let body = match state.compose_body.trim() {
        "" => "(empty message)".to_string(),
        body => body.to_string(),
    };
    let mut to: Vec<String> = state
        .compose_to
        .split(',')
        .map(|recipient| recipient.trim().to_string())
        .filter(|recipient| !recipient.is_empty())
        .collect();
    if to.is_empty() {
        to.push("team@fission.rs".into());
    }

    let message_id = state.next_message_id;
    state.next_message_id += 1;
    let thread_id = state.next_email_id;
    state.next_email_id += 1;

    let mut email = Email {
        id: thread_id,
        subject,
        sender: "You".into(),
        preview: String::new(),
        folders: HashSet::from([Folder::Sent]),
        is_read: true,
        is_flagged: false,
        labels: vec!["Sent".into()],
        category: Category::Primary,
        size_kb: body.len().div_ceil(1000) as u32,
        messages: vec![EmailMessage {
            id: message_id,
            from: "You".into(),
            to,
            cc: Vec::new(),
            body,
            sent_at: Local::now().naive_local(),
        }],
    };
    email.refresh_preview();
    state.emails.insert(0, email);

    state.compose_to.clear();
    state.compose_subject.clear();
    state.compose_body.clear();
    state.compose_attachments.clear();
    state.schedule_date = None;
    state.schedule_time = None;
    state.is_date_picker_open = false;
    state.show_compose = false;
    state.show_toast = true;
    state.toast_message = Some("Message sent".into());
}
