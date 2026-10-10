use fission::prelude::*;

#[derive(Debug, Default)]
pub struct AppState {
    pub note: String,
    pub count: u32,
    pub window_error: bool,
}
impl GlobalState for AppState {}

#[fission_action]
pub struct ControlWindow(pub WindowCommand);
#[fission_action]
pub struct WindowFailed;
#[fission_action]
pub struct UpdateNote;
#[fission_action]
pub struct Increment;

pub fn control_window(
    state: &mut AppState,
    action: ControlWindow,
    cx: &mut ReducerContext<AppState>,
) {
    state.window_error = false;
    let failure = cx.effects.bind(WindowFailed, reduce_with!(window_failed));
    cx.effects
        .capability(WINDOW_CONTROL, action.0)
        .on_err(failure)
        .dispatch();
}

fn window_failed(state: &mut AppState, _: WindowFailed, _: &mut ReducerContext<AppState>) {
    state.window_error = true;
}

pub fn update_note(state: &mut AppState, _: UpdateNote, cx: &mut ReducerContext<AppState>) {
    if let Some(change) = cx.input.text_change() {
        state.note = change.new_text.clone();
    }
}

pub fn increment(state: &mut AppState, _: Increment, _: &mut ReducerContext<AppState>) {
    state.count += 1;
}
