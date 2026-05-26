# fission-shell

Shared shell abstractions for the Fission UI framework.

`fission-shell` defines the platform-agnostic traits and types that every Fission shell backend
(desktop, web, mobile, test) implements. It provides the `Platform` enum for identifying the
current target, the `VideoBackend` and `VideoPlayer` traits for native video playback, and the
`VideoSurfaceFrame` struct for compositing video surfaces into the Fission layout tree.

## Key types

| Type / Trait | Description |
|--------------|-------------|
| `Platform` | Enum with variants `Desktop`, `Web`, `Mobile`, and `Test`. Used for platform-conditional logic in widgets and renderers. |
| `VideoBackend` | Trait for creating `VideoPlayer` instances and presenting video surface frames at their laid-out rectangles. |
| `VideoPlayer` | Trait for controlling a single video stream -- play, pause, stop, seek, volume, rate, and polling for `VideoEvent` updates. |
| `VideoSurfaceFrame` | Struct carrying a `WidgetNodeId`, a `surface_id`, and a `LayoutRect` so the compositor knows where to place a native video surface. |
| `VideoEvent` | Enum for events emitted by a `VideoPlayer`: `Ready { duration }`, `Ended`, and `Error(String)`. |

## Usage example

```rust
use fission_shell::{Platform, VideoBackend, VideoPlayer};

fn create_player(backend: &dyn VideoBackend) {
    let mut player = backend.create_player("https://example.com/video.mp4");
    player.play();
    // Poll for readiness
    for event in player.poll_events() {
        println!("{:?}", event);
    }
}
```

## Status

**Supported** -- stable traits consumed by `fission-shell-desktop`, rendering backends, and the
test harness.

## License

MIT -- see the [Fission repository](https://github.com/worka-ai/fission) for full documentation.
