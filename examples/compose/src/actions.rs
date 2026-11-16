use crate::state::AppState;
use fission::prelude::*;

#[fission_action]
pub struct Navigate(pub String);

pub fn navigate(state: &mut AppState, action: Navigate, cx: &mut ReducerContext<AppState>) {
    state.current_path = action.0.clone();
    cx.effects.navigate(action.0);
}

pub fn shell_route_changed(
    state: &mut AppState,
    action: fission::core::action::ShellRouteChanged,
    _cx: &mut ReducerContext<AppState>,
) {
    state.current_path = action.location.pathname;
}
