use fission_core::action::video::{VideoPause, VideoPlay, VideoSeek, VideoSetRate, VideoStop};
use fission_core::env::{VideoState, VideoStatus};
use fission_core::{ActionEnvelope, Runtime, WidgetId};

#[test]
fn video_actions_update_runtime_state() {
    let mut runtime = Runtime::default();
    let widget_id = WidgetId::explicit("test_video");
    runtime
        .runtime_state
        .video
        .states
        .insert(widget_id, VideoState::default());

    let play_envelope: ActionEnvelope = VideoPlay { target: widget_id }.into();
    runtime
        .dispatch(play_envelope, WidgetId::explicit("button"))
        .unwrap();
    assert_eq!(
        runtime
            .runtime_state
            .video
            .states
            .get(&widget_id)
            .unwrap()
            .status,
        VideoStatus::Playing
    );

    let pause_envelope: ActionEnvelope = VideoPause { target: widget_id }.into();
    runtime
        .dispatch(pause_envelope, WidgetId::explicit("button"))
        .unwrap();
    assert_eq!(
        runtime
            .runtime_state
            .video
            .states
            .get(&widget_id)
            .unwrap()
            .status,
        VideoStatus::Paused
    );

    let seek_envelope: ActionEnvelope = VideoSeek {
        target: widget_id,
        position_ms: 1_234,
    }
    .into();
    runtime
        .dispatch(seek_envelope, WidgetId::explicit("button"))
        .unwrap();
    let video_state = runtime.runtime_state.video.states.get(&widget_id).unwrap();
    assert_eq!(video_state.position_ms, 1_234);
    assert_eq!(video_state.pending_seek, Some(1_234));

    let rate_envelope: ActionEnvelope = VideoSetRate {
        target: widget_id,
        rate: 1.5,
    }
    .into();
    runtime
        .dispatch(rate_envelope, WidgetId::explicit("button"))
        .unwrap();
    let video_state = runtime.runtime_state.video.states.get(&widget_id).unwrap();
    assert_eq!(video_state.rate, 1.5);

    let stop_envelope: ActionEnvelope = VideoStop { target: widget_id }.into();
    runtime
        .dispatch(stop_envelope, WidgetId::explicit("button"))
        .unwrap();
    let video_state = runtime.runtime_state.video.states.get(&widget_id).unwrap();
    assert_eq!(video_state.status, VideoStatus::Stopped);
    assert_eq!(video_state.position_ms, 0);
    assert_eq!(video_state.pending_seek, Some(0));
}
