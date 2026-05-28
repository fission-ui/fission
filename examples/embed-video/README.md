# Video Embed

Video Embed demonstrates a host-backed video surface inside a normal Fission layout. It uses a local demo video asset and bounds the video with explicit width, height, and a themed border.

Use this example when you want to verify media embedding, sizing, and host-surface composition without building a full media player.

## Run it

```bash
cargo run -p embed-video
```

## What to look at

- [`src/lib.rs`](src/lib.rs) defines `VideoEmbedApp` and the `Video` widget configuration.
- [`src/main.rs`](src/main.rs) runs the app through the desktop shell.
- [`assets/demo.mp4`](assets/demo.mp4) is the local video file used by the example.
- [`Cargo.toml`](Cargo.toml) keeps the example small: it only needs the Fission facade plus `serde` and `anyhow`.

## Features exercised

- `Video` widget source, autoplay, looping, width, and height fields.
- Host-backed media surface composition inside `Container` and `Column`.
- Stable widget identity through `WidgetNodeId::explicit(...)`.
- Themed borders around embedded content.

## Learning path

Start with the `Video` struct literal in [`src/lib.rs`](src/lib.rs). The fields there are the minimal set most apps need: an id, source, size, and playback behavior. The surrounding `Container` shows how the app reserves layout space for the host surface.
