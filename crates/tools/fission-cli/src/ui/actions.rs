use super::commands::{execute_ui_command, UiCommand};
use super::routes::UiRoute;
use super::state::{UiDialog, UiState};
use crate::Target;
use fission::prelude::*;

#[fission_reducer(Navigate)]
pub(crate) fn navigate(state: &mut UiState, route: UiRoute) {
    state.route = route;
}

#[fission_reducer(ToggleTheme)]
pub(crate) fn toggle_theme(state: &mut UiState) {
    state.theme_mode = state.theme_mode.toggle();
}

#[fission_reducer(ToggleCompactMode)]
pub(crate) fn toggle_compact_mode(state: &mut UiState) {
    state.compact_mode = !state.compact_mode;
}

#[fission_reducer(SelectTarget)]
pub(crate) fn select_target(state: &mut UiState, target: Target) {
    state.selected_target = Some(target);
    state.selected_device = state
        .devices
        .iter()
        .find(|device| device.target == target && device.available)
        .map(|device| device.id.clone());
}

#[fission_reducer(SelectDevice)]
pub(crate) fn select_device(state: &mut UiState, id: String) {
    state.selected_device = Some(id);
}

#[fission_reducer(SelectCommandSession)]
pub(crate) fn select_command_session(state: &mut UiState, id: u64) {
    state.select_command_session(id);
}

#[fission_reducer(SetInitName)]
pub(crate) fn set_init_name(state: &mut UiState, value: String) {
    state.init_name = value;
}

#[fission_reducer(SetInitAppId)]
pub(crate) fn set_init_app_id(state: &mut UiState, value: String) {
    state.init_app_id = value;
}

#[fission_reducer(SetInitLocalPath)]
pub(crate) fn set_init_local_path(state: &mut UiState, value: String) {
    state.init_local_path = value;
}

#[fission_reducer(SetHost)]
pub(crate) fn set_host(state: &mut UiState, value: String) {
    state.host = value;
}

#[fission_reducer(SetPort)]
pub(crate) fn set_port(state: &mut UiState, value: String) {
    state.port = value;
}

#[fission_reducer(SetScrollbackLimitInput)]
pub(crate) fn set_scrollback_limit_input(state: &mut UiState, value: String) {
    state.scrollback_limit_input = value.clone();
    if let Some(limit) = parse_scrollback_limit(&value) {
        state.set_scrollback_limit(limit);
    }
}

#[fission_reducer(SetScrollbackLimit)]
pub(crate) fn set_scrollback_limit(state: &mut UiState, limit: usize) {
    state.set_scrollback_limit(limit);
}

#[fission_reducer(ToggleStrict)]
pub(crate) fn toggle_strict(state: &mut UiState) {
    state.strict = !state.strict;
}

#[fission_reducer(ToggleRelease)]
pub(crate) fn toggle_release(state: &mut UiState) {
    state.release = !state.release;
}

#[fission_reducer(ToggleDetach)]
pub(crate) fn toggle_detach(state: &mut UiState) {
    state.detach = !state.detach;
}

#[fission_reducer(ToggleNoOpen)]
pub(crate) fn toggle_no_open(state: &mut UiState) {
    state.no_open = !state.no_open;
}

#[fission_reducer(ToggleHeadless)]
pub(crate) fn toggle_headless(state: &mut UiState) {
    state.headless = !state.headless;
}

#[fission_reducer(ExecuteCommand)]
pub(crate) fn execute_command(state: &mut UiState, command: UiCommand) {
    execute_ui_command(state, command);
}

#[fission_reducer(RequestCommand)]
pub(crate) fn request_command(state: &mut UiState, command: UiCommand) {
    state.request_command_confirmation(command);
}

#[fission_reducer(ConfirmDialog)]
pub(crate) fn confirm_dialog(state: &mut UiState) {
    let Some(dialog) = state.pending_dialog.take() else {
        return;
    };
    match dialog {
        UiDialog::Command { command, .. } => execute_ui_command(state, command),
        UiDialog::Exit { .. } => {
            state.exit_confirmed = true;
        }
    }
}

#[fission_reducer(CancelDialog)]
pub(crate) fn cancel_dialog(state: &mut UiState) {
    state.pending_dialog = None;
}

fn parse_scrollback_limit(value: &str) -> Option<usize> {
    let compact = value.trim().replace('_', "");
    if compact.is_empty() {
        return None;
    }
    let lower = compact.to_ascii_lowercase();
    let (digits, multiplier) = if let Some(digits) = lower.strip_suffix('k') {
        (digits, 1_000usize)
    } else if let Some(digits) = lower.strip_suffix('m') {
        (digits, 1_000_000usize)
    } else {
        (lower.as_str(), 1usize)
    };
    digits
        .parse::<usize>()
        .ok()
        .and_then(|value| value.checked_mul(multiplier))
        .filter(|value| *value > 0)
}
