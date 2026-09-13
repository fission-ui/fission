//! Reducers behind the sidebar's navigation.

use super::{InboxState, SelectFolder, SetContactsOpen, ToggleBrowserDemo, ToggleContactSelection};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

/// Opens a folder and closes the mobile menu that offered it.
pub fn select_folder(state: &mut InboxState, action: SelectFolder, _: &mut Cx<'_, '_, '_>) {
    state.navigate_to(action.0.path());
    state.show_mobile_menu = false;
}

pub fn set_contacts_open(state: &mut InboxState, action: SetContactsOpen, _: &mut Cx<'_, '_, '_>) {
    state.show_contacts = action.0;
}

pub fn set_browser_demo_open(
    state: &mut InboxState,
    action: ToggleBrowserDemo,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_browser_demo = action.0;
}

/// Selects a contact, or clears its selection when it is already selected.
pub fn toggle_contact_selection(
    state: &mut InboxState,
    action: ToggleContactSelection,
    _: &mut Cx<'_, '_, '_>,
) {
    let selected = &mut state.contact_selected_ids;
    if let Some(position) = selected.iter().position(|id| *id == action.0) {
        selected.remove(position);
    } else {
        selected.push(action.0);
    }
}
