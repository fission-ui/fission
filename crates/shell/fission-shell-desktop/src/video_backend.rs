use fission_shell::{VideoBackend, VideoEvent, VideoPlayer};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct MockVideoBackend;

impl MockVideoBackend {
    pub fn new() -> Self {
        Self
    }
}

impl VideoBackend for MockVideoBackend {
    fn create_player(&self, source: &str) -> Box<dyn VideoPlayer> {
        Box::new(MockPlayer::new(source))
    }

    fn present_surfaces(&self, _frames: &[fission_shell::VideoSurfaceFrame]) {
        // mac implementation pending; placeholder no-op
    }
}

static NEXT_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

struct MockPlayer {
    _source: String,
    state: PlayerState,
    start_time: Instant,
    play_start_time: Option<Instant>,
    accumulated_play_time: u64,
    surface_id: u64,
    duration: u64,
    sent_ready: bool,
    sent_ended: bool,
}

#[derive(PartialEq)]
enum PlayerState {
    Loading,
    Ready,
    Playing,
    Paused,
    Ended,
}

impl MockPlayer {
    fn new(source: &str) -> Self {
        Self {
            _source: source.to_string(),
            state: PlayerState::Loading,
            start_time: Instant::now(),
            play_start_time: None,
            accumulated_play_time: 0,
            surface_id: NEXT_SURFACE_ID.fetch_add(1, Ordering::Relaxed),
            duration: 5000,
            sent_ready: false,
            sent_ended: false,
        }
    }
}

impl VideoPlayer for MockPlayer {
    fn play(&mut self) {
        if self.state == PlayerState::Ready || self.state == PlayerState::Paused {
            self.state = PlayerState::Playing;
            self.play_start_time = Some(Instant::now());
        }
    }

    fn pause(&mut self) {
        if self.state == PlayerState::Playing {
            if let Some(start) = self.play_start_time {
                self.accumulated_play_time += start.elapsed().as_millis() as u64;
            }
            self.state = PlayerState::Paused;
            self.play_start_time = None;
        }
    }

    fn stop(&mut self) {
        self.state = PlayerState::Ready;
        self.play_start_time = None;
        self.accumulated_play_time = 0;
        self.sent_ended = false;
    }

    fn position(&self) -> u64 {
        let current =
            if let (PlayerState::Playing, Some(start)) = (&self.state, self.play_start_time) {
                start.elapsed().as_millis() as u64
            } else {
                0
            };
        self.accumulated_play_time + current
    }

    fn duration(&self) -> Option<u64> {
        Some(self.duration)
    }

    fn surface_id(&self) -> u64 {
        self.surface_id
    }

    fn poll_events(&mut self) -> Vec<VideoEvent> {
        let mut events = Vec::new();
        let elapsed = self.start_time.elapsed().as_millis() as u64;

        if !self.sent_ready && elapsed > 500 {
            if self.state == PlayerState::Loading {
                self.state = PlayerState::Ready;
            }
            self.sent_ready = true;
            events.push(VideoEvent::Ready {
                duration: self.duration,
            });
        }

        if self.state == PlayerState::Playing && self.position() >= self.duration {
            self.state = PlayerState::Ended;
            self.play_start_time = None; // Stop adding time
            if !self.sent_ended {
                self.sent_ended = true;
                events.push(VideoEvent::Ended);
            }
        }

        events
    }
}
