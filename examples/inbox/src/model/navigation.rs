//! Reducers behind the sidebar's navigation.

use super::{Folder, InboxState, SelectFolder, SetContactsOpen};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

/// Opens a folder and closes the mobile menu that offered it.
pub fn select_folder(state: &mut InboxState, action: SelectFolder, _: &mut Cx<'_, '_, '_>) {
    let path = match action.0 {
        Folder::Inbox => "/inbox".to_string(),
        Folder::Starred => "/starred".to_string(),
        Folder::Sent => "/sent".to_string(),
        Folder::Drafts => "/drafts".to_string(),
        Folder::Trash => "/trash".to_string(),
        Folder::Custom(label) => format!("/{label}"),
    };
    state.navigate_to(path);
    state.show_mobile_menu = false;
}

pub fn set_contacts_open(state: &mut InboxState, action: SetContactsOpen, _: &mut Cx<'_, '_, '_>) {
    state.show_contacts = action.0;
}
