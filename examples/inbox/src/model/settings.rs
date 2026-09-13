//! Reducers behind the settings modal.
//!
//! Each settings control dispatches one of these by name instead of binding
//! an inline closure while the modal builds.

use super::{
    InboxState, LabelDropped, SetAutoAdvanceEnabled, SetDensity, SetDensitySelectOpen,
    SetDragInProgress, SetInboxType, SetInboxTypeSelectOpen, SetLocale, SetOfflineEnabled,
    SetQuickTipOpen, SetSettingsOpen, SetSignature, SetSignatureEditing, SetSmartComposeEnabled,
    SetTheme, SetThemeSelectOpen, SetZoomLevel,
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

/// Takes the edited text from the input event, falling back to the payload
/// when the action was dispatched without one.
pub fn set_signature(state: &mut InboxState, action: SetSignature, cx: &mut Cx<'_, '_, '_>) {
    state.signature = cx
        .input
        .text_change()
        .map(|change| change.new_text.clone())
        .unwrap_or(action.0);
}

pub fn set_signature_editing(
    state: &mut InboxState,
    action: SetSignatureEditing,
    _: &mut Cx<'_, '_, '_>,
) {
    state.signature_editing = action.0;
}

pub fn set_smart_compose_enabled(
    state: &mut InboxState,
    action: SetSmartComposeEnabled,
    _: &mut Cx<'_, '_, '_>,
) {
    state.smart_compose_enabled = action.0;
}

pub fn set_offline_enabled(
    state: &mut InboxState,
    action: SetOfflineEnabled,
    _: &mut Cx<'_, '_, '_>,
) {
    state.offline_enabled = action.0;
}

pub fn set_auto_advance_enabled(
    state: &mut InboxState,
    action: SetAutoAdvanceEnabled,
    _: &mut Cx<'_, '_, '_>,
) {
    state.auto_advance_enabled = action.0;
}

/// Pins the label that was dropped. A drag carries the label as its payload;
/// a dispatch without one pins the action's label.
pub fn label_dropped(state: &mut InboxState, action: LabelDropped, cx: &mut Cx<'_, '_, '_>) {
    let dropped = cx
        .input
        .as_internal_drop()
        .and_then(|payload| String::from_utf8(payload.to_vec()).ok());
    state.last_drag_label = Some(dropped.unwrap_or(action.0));
    state.drag_in_progress = false;
}

pub fn set_drag_in_progress(
    state: &mut InboxState,
    action: SetDragInProgress,
    _: &mut Cx<'_, '_, '_>,
) {
    state.drag_in_progress = action.0;
}

pub fn set_quick_tip_open(state: &mut InboxState, action: SetQuickTipOpen, _: &mut Cx<'_, '_, '_>) {
    state.show_quick_tip = action.0;
}

/// Opening one settings select closes the others, so only one list is open.
pub fn set_inbox_type_select_open(
    state: &mut InboxState,
    action: SetInboxTypeSelectOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_inbox_type_select = action.0;
    if action.0 {
        state.show_theme_select = false;
        state.show_density_select = false;
    }
}

pub fn set_theme_select_open(
    state: &mut InboxState,
    action: SetThemeSelectOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_theme_select = action.0;
    if action.0 {
        state.show_inbox_type_select = false;
        state.show_density_select = false;
    }
}

pub fn set_density_select_open(
    state: &mut InboxState,
    action: SetDensitySelectOpen,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_density_select = action.0;
    if action.0 {
        state.show_inbox_type_select = false;
        state.show_theme_select = false;
    }
}
