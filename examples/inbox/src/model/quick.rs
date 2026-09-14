//! Reducers behind the right sidebar's quick tools.

use super::{InboxState, SetCalendarSelected, SetMeetCameraOn, SetMeetMicOn, ShowToast};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, InboxState>;

pub fn set_meet_camera_on(state: &mut InboxState, action: SetMeetCameraOn, _: &mut Cx<'_, '_, '_>) {
    state.meet_camera_on = action.0;
}

pub fn set_meet_mic_on(state: &mut InboxState, action: SetMeetMicOn, _: &mut Cx<'_, '_, '_>) {
    state.meet_mic_on = action.0;
}

pub fn set_calendar_selected(
    state: &mut InboxState,
    action: SetCalendarSelected,
    _: &mut Cx<'_, '_, '_>,
) {
    state.calendar_selected = Some(action.0);
}

pub fn show_toast(state: &mut InboxState, action: ShowToast, _: &mut Cx<'_, '_, '_>) {
    state.toast_message = Some(action.0);
    state.show_toast = true;
}
