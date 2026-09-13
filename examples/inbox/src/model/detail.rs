//! Reducers behind the email detail view.

use super::{
    EmailMessage, Folder, InboxState, Navigate, SelectReplyMode, SendReply, SetReplyBody,
    ToggleDetails, ToggleToast,
};
use chrono::Local;
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

/// Returns to the folder the thread was opened from.
pub fn navigate_back(state: &mut InboxState, action: Navigate, _: &mut Cx<'_, '_, '_>) {
    state.navigate_to(action.0);
}

pub fn select_reply_mode(state: &mut InboxState, action: SelectReplyMode, _: &mut Cx<'_, '_, '_>) {
    state.reply_mode = action.0;
}

/// Takes the reply text from the input event, falling back to the payload when
/// the action was dispatched without one.
pub fn set_reply_body(state: &mut InboxState, action: SetReplyBody, cx: &mut Cx<'_, '_, '_>) {
    state.reply_body = cx
        .input
        .text_change()
        .map(|change| change.new_text.clone())
        .unwrap_or(action.0);
}

/// Appends the reply to the thread, files it under Sent and confirms with a toast.
/// An empty reply does nothing.
pub fn send_reply(state: &mut InboxState, action: SendReply, _: &mut Cx<'_, '_, '_>) {
    let body = state.reply_body.trim().to_string();
    if body.is_empty() {
        return;
    }
    let message_id = state.next_message_id;
    if let Some(thread) = state.emails.iter_mut().find(|email| email.id == action.0) {
        state.next_message_id += 1;
        thread.messages.push(EmailMessage {
            id: message_id,
            from: "You".into(),
            to: vec![thread.sender.clone()],
            cc: Vec::new(),
            body,
            sent_at: Local::now().naive_local(),
        });
        thread.folders.insert(Folder::Sent);
        thread.is_read = true;
        thread.refresh_preview();
    }
    state.reply_body.clear();
    state.show_toast = true;
    state.toast_message = Some("Reply sent".into());
}

/// Shows or hides the toast.
pub fn set_toast_visible(state: &mut InboxState, action: ToggleToast, _: &mut Cx<'_, '_, '_>) {
    state.show_toast = action.0;
}

pub fn toggle_details(state: &mut InboxState, _: ToggleDetails, _: &mut Cx<'_, '_, '_>) {
    state.details_expanded = !state.details_expanded;
}
