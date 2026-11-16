//! Reducers behind the email detail view.

use super::{EmailMessage, Folder, InboxState, ToggleDetails};
use chrono::Local;
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

pub fn toggle_details(state: &mut InboxState, _: ToggleDetails, _: &mut Cx<'_, '_, '_>) {
    state.details_expanded = !state.details_expanded;
}

/// Appends a non-empty reply to the thread, files it under Sent and confirms
/// with a toast. The component's inline handler supplies the stable thread ID.
pub fn send_reply(state: &mut InboxState, thread_id: usize) {
    let body = state.reply_body.trim().to_string();
    if body.is_empty() {
        return;
    }
    let message_id = state.next_message_id;
    if let Some(thread) = state.emails.iter_mut().find(|email| email.id == thread_id) {
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

/// Moves the thread to Trash, or deletes it for good when it is already there,
/// then returns to the folder it was opened from and confirms with a toast.
pub fn delete_email(state: &mut InboxState, email_id: usize) {
    let Some(index) = state.emails.iter().position(|email| email.id == email_id) else {
        return;
    };
    let message = if state.emails[index].folders.contains(&Folder::Trash) {
        state.emails.remove(index);
        "Deleted forever"
    } else {
        let email = &mut state.emails[index];
        email.folders.clear();
        email.folders.insert(Folder::Trash);
        "Moved to Trash"
    };
    state.selected_emails.retain(|id| *id != email_id);
    let folder = state.selected_folder.path();
    state.navigate_to(folder);
    state.toast_message = Some(message.into());
    state.show_toast = true;
}
