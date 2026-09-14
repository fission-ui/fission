//! Reducers behind the email list.

use super::{
    Folder, InboxState, Navigate, SelectTab, SetAdvancedFiltersOpen, SetDateFilter,
    SetDateFilterEndOpen, SetDateFilterStartOpen, SetFilterMode, SetMobileMenuOpen, SetPage,
    SetSizeFilter, SetSortMenuOpen, SetSortOption, ToggleEmailSelection, ToggleFlag,
    ToggleLabelFilter, UpdateSearch,
};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

pub fn set_filter_mode(state: &mut InboxState, action: SetFilterMode, _: &mut Cx<'_, '_, '_>) {
    state.filter_mode = action.0;
}

pub fn set_page(state: &mut InboxState, action: SetPage, _: &mut Cx<'_, '_, '_>) {
    state.page = action.0;
}

pub fn set_advanced_filters_open(
    state: &mut InboxState,
    action: SetAdvancedFiltersOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_advanced_filters = action.0;
}

pub fn set_sort_option(state: &mut InboxState, action: SetSortOption, _: &mut Cx<'_, '_, '_>) {
    state.sort_option = action.0;
    state.show_sort_menu = false;
}

pub fn set_sort_menu_open(state: &mut InboxState, action: SetSortMenuOpen, _: &mut Cx<'_, '_, '_>) {
    state.show_sort_menu = action.0;
}

pub fn set_date_filter(state: &mut InboxState, action: SetDateFilter, _: &mut Cx<'_, '_, '_>) {
    state.date_filter = (action.0, action.1);
    state.date_filter_start_open = false;
    state.date_filter_end_open = false;
    state.page = 1;
}

pub fn set_date_filter_start_open(
    state: &mut InboxState,
    action: SetDateFilterStartOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.date_filter_start_open = action.0;
    state.date_filter_end_open &= !action.0;
}

pub fn set_date_filter_end_open(
    state: &mut InboxState,
    action: SetDateFilterEndOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.date_filter_end_open = action.0;
    state.date_filter_start_open &= !action.0;
}

pub fn set_size_filter(state: &mut InboxState, _: SetSizeFilter, cx: &mut Cx<'_, '_, '_>) {
    if let Some(change) = cx.input.range_slider_change() {
        state.size_filter_mb = (change.start, change.end);
        state.page = 1;
    }
}

/// Takes the query from the input event, falling back to the payload when the
/// action was dispatched without one.
pub fn update_search(state: &mut InboxState, action: UpdateSearch, cx: &mut Cx<'_, '_, '_>) {
    state.search_query = cx
        .input
        .text_change()
        .map(|change| change.new_text.clone())
        .unwrap_or(action.0);
}

pub fn toggle_email_selection(
    state: &mut InboxState,
    action: ToggleEmailSelection,
    _: &mut Cx<'_, '_, '_>,
) {
    if let Some(position) = state.selected_emails.iter().position(|id| *id == action.0) {
        state.selected_emails.remove(position);
    } else {
        state.selected_emails.push(action.0);
    }
}

/// Flags or unflags a thread, keeping its membership of the Starred folder in step.
pub fn toggle_flag(state: &mut InboxState, action: ToggleFlag, _: &mut Cx<'_, '_, '_>) {
    if let Some(email) = state.emails.iter_mut().find(|email| email.id == action.0) {
        email.is_flagged = !email.is_flagged;
        if email.is_flagged {
            email.folders.insert(Folder::Starred);
        } else {
            email.folders.remove(&Folder::Starred);
        }
    }
}

/// Opens a thread: navigates to it, closes the mobile menu and marks it read.
pub fn navigate(state: &mut InboxState, action: Navigate, _: &mut Cx<'_, '_, '_>) {
    state.navigate_to(action.0);
    state.show_mobile_menu = false;
    if let Some(id) = state.selected_email_id {
        if let Some(email) = state.emails.iter_mut().find(|email| email.id == id) {
            email.is_read = true;
        }
    }
}

pub fn toggle_label_filter(
    state: &mut InboxState,
    action: ToggleLabelFilter,
    _: &mut Cx<'_, '_, '_>,
) {
    state.label_filter =
        (state.label_filter.as_deref() != Some(action.0.as_str())).then_some(action.0);
    state.page = 1;
}

pub fn select_tab(state: &mut InboxState, action: SelectTab, _: &mut Cx<'_, '_, '_>) {
    state.active_tab = action.0;
    state.page = 1;
}

pub fn set_mobile_menu_open(
    state: &mut InboxState,
    action: SetMobileMenuOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_mobile_menu = action.0;
}
