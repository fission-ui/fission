use fission::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub struct WebsiteState {
    pub path: String,
}

impl Default for WebsiteState {
    fn default() -> Self {
        Self { path: "/".into() }
    }
}

impl GlobalState for WebsiteState {}

#[cfg(target_arch = "wasm32")]
pub fn route_changed(
    state: &mut WebsiteState,
    action: fission::core::action::ShellRouteChanged,
    _ctx: &mut ReducerContext<'_, '_, '_, WebsiteState>,
) {
    state.path = action.location.pathname;
}
