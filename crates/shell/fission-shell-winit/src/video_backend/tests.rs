use super::create_video_backend;

#[test]
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[should_panic(expected = "no state-only mock fallback exists")]
fn apple_video_backend_without_window_panics() {
    let _ = create_video_backend(None);
}
