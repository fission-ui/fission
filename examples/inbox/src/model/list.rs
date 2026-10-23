//! Reducers behind the email list.

use super::{InboxState, SelectTab, SetDateFilter, SetFilterMode, SetPage, SetSortOption};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

pub fn set_filter_mode(state: &mut InboxState, action: SetFilterMode, _: &mut Cx<'_, '_, '_>) {
    state.filter_mode = action.0;
}

pub fn set_page(state: &mut InboxState, action: SetPage, _: &mut Cx<'_, '_, '_>) {
    state.page = action.0;
}

pub fn set_sort_option(state: &mut InboxState, action: SetSortOption, _: &mut Cx<'_, '_, '_>) {
    state.sort_option = action.0;
    state.show_sort_menu = false;
}

pub fn set_date_filter(state: &mut InboxState, action: SetDateFilter, _: &mut Cx<'_, '_, '_>) {
    state.date_filter = (action.0, action.1);
    state.date_filter_start_open = false;
    state.date_filter_end_open = false;
    state.page = 1;
}

pub fn select_tab(state: &mut InboxState, action: SelectTab, _: &mut Cx<'_, '_, '_>) {
    state.active_tab = action.0;
    state.page = 1;
}
