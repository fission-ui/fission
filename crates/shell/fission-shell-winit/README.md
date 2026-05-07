# fission-shell-desktop

Desktop shell for Fission applications. Provides the native window, GPU-accelerated rendering pipeline, input handling, and platform integrations needed to run a Fission UI on macOS, Windows, and Linux.

## Architecture

```
DesktopApp<S, W>
  |
  +-- Runtime         (fission-core: state management, action dispatch, animation ticking)
  +-- LayoutEngine    (fission-layout: flexbox layout computation)
  +-- Pipeline        (IR diffing, layout, paint, display list generation)
  +-- VelloRenderer   (fission-render-vello: GPU rasterization via Vello + wgpu)
  +-- VelloTextMeasurer (font context, text shaping via Parley + Fontique)
  +-- DesktopClipboard (platform clipboard via arboard)
  +-- DesktopImeHandler (IME composition via winit)
  +-- VideoBackend    (macOS AVFoundation video, or mock on other platforms)
  +-- TestControl     (optional HTTP server for automated UI testing)
```

The main entry point is `DesktopApp::new(root_widget)`, which creates a winit event loop and runs the full build-layout-paint-present cycle on every frame.

## `DesktopApp`

The generic `DesktopApp<S: AppState, W: Widget<S>>` owns the entire application lifecycle.

### Construction

```rust
use fission_shell_desktop::DesktopApp;

DesktopApp::new(MyRootWidget)
    .with_title("My App")
    .with_state_init(|state| {
        state.counter = 42;
    })
    .with_key_handler(|state, key, mods| {
        // Return true if handled, false to pass to framework
        false
    })
    .with_sync_env(|state, env| {
        env.theme = if state.dark_mode { Theme::dark() } else { Theme::default() };
    })
    .with_frame_hook(|state| {
        // Runs on every event loop iteration (e.g., poll LSP).
        // Return true to request a redraw.
        false
    })
    .with_app_effect_handler(|payload, req_id, on_ok, on_err, tx| {
        // Handle Effect::App(...) payloads on a background thread.
    })
    .run()
    .unwrap();
```

### Builder methods

| Method | Purpose |
|--------|---------|
| `with_title(title)` | Set the window title. |
| `with_state_init(f)` | Mutate the initial `S` state before the first frame. |
| `with_key_handler(f)` | Register an app-level key handler that intercepts before the framework. The handler receives `(&mut S, &KeyCode, modifiers)` and returns `true` if consumed. |
| `with_sync_env(f)` | Synchronize `Env` from app state each frame (e.g., theme switching). |
| `with_frame_hook(f)` | Register a callback that runs on every `AboutToWait` event. Return `true` to request a redraw. Useful for polling background services. |
| `with_app_effect_handler(f)` | Handle custom `Effect::App(Vec<u8>)` payloads. The handler should spawn background work and send results through the provided channel. |
| `with_env(env)` | Replace the default `Env`. |
| `register_reducer(id, f)` | Register a single action reducer. |
| `absorb_registry(registry)` | Absorb an entire `ActionRegistry<S>` for bulk reducer registration. |

### Frame cycle

Each `RedrawRequested` event triggers:

1. **Effect drain** -- pending effects from the previous frame are dispatched.
2. **Build** -- `root_widget.build()` produces the `Node` tree; portals are collected.
3. **Lower** -- the `Node` tree is lowered to `CoreIR` (intermediate representation).
4. **Pipeline update** -- IR diff, layout computation, display list generation.
5. **Render** -- Vello rasterizes the display list to a GPU texture.
6. **Present** -- the texture is blitted to the window surface.

## `Pipeline`

The render pipeline (`Pipeline`) manages incremental updates:

- **IR diffing** via `fission-core::diff::diff_ir` identifies structurally changed nodes.
- **Incremental layout** only recomputes dirty subtrees (unless the viewport resized).
- **Paint caching** stores display list segments per node, keyed by content hash.
- **Video surface collection** extracts video embed rects for platform compositing.

### Key fields

| Field | Purpose |
|-------|---------|
| `prev_ir` | The CoreIR from the previous frame (used for diffing). |
| `last_snapshot` | The most recent `LayoutSnapshot` with computed rects for every node. |
| `paint_cache` | Per-node display list cache: `HashMap<NodeId, (hash, Vec<DisplayOp>)>`. |
| `video_surfaces` | Video rects to hand off to the platform video backend. |

## Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `FISSION_MAX_FPS` | `60` | Maximum frame rate (throttled via `WaitUntil`). |
| `FISSION_TEXTINPUT_BLINK` | `true` | Enable/disable cursor blinking in text inputs. |
| `FISSION_TEXTINPUT_BLINK_MS` | `530` | Cursor blink period in milliseconds. |
| `FISSION_USE_SYSTEM_FONTS` | `false` | Include system fonts in the font collection. |
| `FISSION_TEXT_TRACE` | `false` | Enable text input latency tracing to stderr. |
| `FISSION_SCROLL_TRACE` | `false` | Enable scroll event tracing to stderr. |
| `FISSION_TEST_CONTROL_PORT` | (none) | Start an HTTP test control server on this port. |

## Test control

When `FISSION_TEST_CONTROL_PORT` is set, the shell spawns a TCP server that accepts JSON commands from `fission-test-driver::LiveTestClient`. This enables automated UI testing by sending tap, scroll, type, screenshot, and semantic tree queries over HTTP. See the `fission-test-driver` crate for the client API.

## Platform support

- **macOS**: Full support including AVFoundation video playback and IME.
- **Windows/Linux**: Rendering and input fully supported. Video playback uses a mock backend.
