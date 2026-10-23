//! Reducers behind the settings modal.
//!
//! Each settings control dispatches one of these by name instead of binding
//! an inline closure while the modal builds.

use super::{
    InboxState, SetDensity, SetInboxType, SetLocale, SetSettingsOpen, SetTheme, SetZoomLevel,
};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

pub fn set_settings_open(state: &mut InboxState, action: SetSettingsOpen, _: &mut Cx<'_, '_, '_>) {
    state.show_settings = action.0;
}

pub fn set_locale(state: &mut InboxState, action: SetLocale, _: &mut Cx<'_, '_, '_>) {
    state.locale = action.0;
}

pub fn set_theme(state: &mut InboxState, action: SetTheme, _: &mut Cx<'_, '_, '_>) {
    state.theme_mode = action.0;
    state.show_theme_select = false;
}

pub fn set_density(state: &mut InboxState, action: SetDensity, _: &mut Cx<'_, '_, '_>) {
    state.density_mode = action.0;
    state.show_density_select = false;
}

pub fn set_inbox_type(state: &mut InboxState, action: SetInboxType, _: &mut Cx<'_, '_, '_>) {
    state.inbox_type = action.0;
    state.show_inbox_type_select = false;
}

pub fn set_zoom_level(state: &mut InboxState, action: SetZoomLevel, _: &mut Cx<'_, '_, '_>) {
    state.zoom_level = action.0;
}
