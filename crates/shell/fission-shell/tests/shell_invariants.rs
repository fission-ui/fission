use anyhow::Result;
use fission_core::{
    InputEvent, LayoutPoint, LayoutSize, LifecycleEvent, PointerButton, PointerEvent,
};
use fission_shell::{Platform, VideoBackend, VideoEvent, VideoPlayer};
use serde_json;

struct DummyBackend;

struct DummyPlayer {
    surface_id: u64,
    events: Vec<VideoEvent>,
}

impl DummyPlayer {
    fn new() -> Self {
        Self {
            surface_id: 42,
            events: vec![VideoEvent::Ready { duration: 1_000 }],
        }
    }
}

impl VideoPlayer for DummyPlayer {
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn stop(&mut self) {}
    fn position(&self) -> u64 {
        0
    }
    fn duration(&self) -> Option<u64> {
        Some(1_000)
    }
    fn surface_id(&self) -> u64 {
        self.surface_id
    }
    fn poll_events(&mut self) -> Vec<VideoEvent> {
        std::mem::take(&mut self.events)
    }
    fn seek_to(&mut self, _position_ms: u64) {}
    fn set_rate(&mut self, _rate: f32) {}
    fn set_volume(&mut self, _volume: f32) {}
    fn set_muted(&mut self, _muted: bool) {}
}

impl VideoBackend for DummyBackend {
    fn create_player(
        &self,
        _source: &str,
        _audio: &fission_core::ui::VideoAudioOptions,
    ) -> Box<dyn VideoPlayer> {
        Box::new(DummyPlayer::new())
    }

    fn present_surfaces(&self, _frames: &[fission_shell::VideoSurfaceFrame]) {}
}

#[test]
fn test_input_event_serialization() {
    let event1 = InputEvent::Pointer(PointerEvent::Down {
        pointer_id: Default::default(),
        kind: Default::default(),
        point: LayoutPoint { x: 100.0, y: 50.0 },
        button: PointerButton::Primary,
        modifiers: 0,
    });
    let event2 = InputEvent::Lifecycle(LifecycleEvent::Resize {
        size: LayoutSize {
            width: 800.0,
            height: 600.0,
        },
    });

    let json1 = serde_json::to_string(&event1).unwrap();
    let deserialized1: InputEvent = serde_json::from_str(&json1).unwrap();
    assert_eq!(event1, deserialized1);

    let json2 = serde_json::to_string(&event2).unwrap();
    let deserialized2: InputEvent = serde_json::from_str(&json2).unwrap();
    assert_eq!(event2, deserialized2);
}

#[test]
fn test_video_backend_trait_object() -> Result<()> {
    let backend: Box<dyn VideoBackend> = Box::new(DummyBackend);
    let mut player =
        backend.create_player("dummy.mp4", &fission_core::ui::VideoAudioOptions::default());

    player.play();
    player.pause();
    player.stop();
    player.seek_to(500);
    player.set_rate(1.25);
    assert_eq!(player.surface_id(), 42);
    assert_eq!(player.duration(), Some(1_000));

    let events = player.poll_events();
    assert_eq!(events, vec![VideoEvent::Ready { duration: 1_000 }]);
    Ok(())
}

#[test]
fn test_platform_enum_serialization() {
    let platform = Platform::Desktop;
    let json = serde_json::to_string(&platform).unwrap();
    let roundtrip: Platform = serde_json::from_str(&json).unwrap();
    assert_eq!(platform, roundtrip);
}
