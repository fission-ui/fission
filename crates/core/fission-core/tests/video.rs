use fission_core::action::video::{VideoPause, VideoPlay};
use fission_core::env::{VideoState, VideoStatus};
use fission_core::{ActionEnvelope, Runtime, WidgetNodeId};

#[test]
fn video_actions_update_runtime_state() {
    let mut runtime = Runtime::default();
    let widget_id = WidgetNodeId::explicit("test_video");
    runtime
        .runtime_state
        .video
        .states
        .insert(widget_id, VideoState::default());

    let play_envelope: ActionEnvelope = VideoPlay { target: widget_id }.into();
    runtime
        .dispatch(play_envelope, WidgetNodeId::explicit("button").into())
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
        .dispatch(pause_envelope, WidgetNodeId::explicit("button").into())
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
}
