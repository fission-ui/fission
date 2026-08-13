# fission-shell-winit

Shared Winit host and interactive renderer runtime for Fission applications.

This crate holds the common event loop, rendering pipeline, input routing, and test-control
transport used by both `fission-shell-desktop` and `fission-shell-mobile`.

## Architecture

```
WinitApp<S, W>
  |
  +-- Runtime         (fission-core: state management, action dispatch, animation ticking)
  +-- LayoutEngine    (fission-layout: flexbox layout computation)
  +-- Pipeline        (IR diffing, layout, paint, display list generation)
  +-- MainRenderer    (Vello, software, Skia raster, or direct native Skia Ganesh)
  +-- VelloTextMeasurer (font context, text shaping via Parley + Fontique)
  +-- DesktopClipboard (or in-memory clipboard fallback on mobile)
  +-- DesktopImeHandler (IME composition via winit)
  +-- VideoBackend    (macOS AVFoundation video, or mock on other platforms)
  +-- TestControl     (optional HTTP server for automated UI testing)
```

Direct native Ganesh currently presents through Vulkan on Linux/Android, Metal
on macOS/iOS, and D3D12 on Windows without constructing a wgpu presentation
path.

The main entry point is `WinitApp::new(root_widget)`, which creates a winit event loop and runs
the full build-layout-paint-present cycle on every frame.

## `WinitApp`

The generic `WinitApp<S, W>` owns the shared application lifecycle.

### Construction

```rust
use fission_shell_winit::WinitApp;

WinitApp::new(MyRootWidget)
    .with_title("My App")
    .with_state_init(|state| {
        state.counter = 42;
    })
    .with_startup_action(AppStarted)
    .with_async(|asyncs| {
        asyncs.register_job(FETCH_JOB, fetch_job_handler);
    })
    .with_key_handler(|state, key, mods| {
        // Return true if handled, false to pass to framework
        false
    })
    .with_sync_env(|state, env| {
        env.theme = if state.dark_mode { Theme::dark() } else { Theme::default() };
    })
    .with_frame_hook(|state| {
        // Runs on every AboutToWait event.
        // Keep this for synchronous polling or platform wakeups only.
        // Return true to request a redraw.
        false
    })
    .with_async(|asyncs| {
        asyncs.register_operation_capability(MY_CAPABILITY, |request, _ctx| async move {
            Ok(MyCapabilityOk::default())
        });
    })
    .run()
    .unwrap();
```

### Builder methods

| Method | Purpose |
|--------|---------|
| `with_title(title)` | Set the window title. |
| `with_state_init(f)` | Mutate the initial `S` state before the first frame. Keep this synchronous and cheap. |
| `with_startup_action(action)` | Dispatch one action after the runtime is ready. Use this to kick off startup jobs or services without blocking first paint. |
| `with_async(f)` | Register typed async jobs, services, and capability handlers on the shell-owned async host. |
| `with_key_handler(f)` | Register an app-level key handler that intercepts before the framework. The handler receives `(&mut S, &KeyCode, modifiers)` and returns `true` if consumed. |
| `with_sync_env(f)` | Synchronize `Env` from app state each frame (e.g., theme switching). |
| `with_frame_hook(f)` | Register a callback that runs on every `AboutToWait` event. Return `true` to request a redraw. Prefer startup actions, async jobs, services, or timer resources for app lifecycle work. |
| `with_env(env)` | Replace the default `Env`. |
| `register_reducer(id, f)` | Register a single action reducer. |
| `absorb_registry(registry)` | Absorb an entire `ActionRegistry<S>` for bulk reducer registration. |

### Frame cycle

Each `RedrawRequested` event triggers:

1. **Effect drain** -- pending effects from the previous frame are dispatched.
2. **Build** -- the root component converts into a `Widget` tree; portals are collected.
3. **InternalLower** -- the `Widget` tree is lowered to `CoreIR` (intermediate representation).
4. **Pipeline update** -- IR diff, layout computation, display list generation.
5. **Render** -- the selected Fission graphics session rasterizes the display list.
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
| `paint_cache` | Per-node display list cache: `HashMap<WidgetId, (hash, Vec<DisplayOp>)>`. |
| `video_surfaces` | Video rects to hand off to the platform video backend. |

## Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `FISSION_MAX_FPS` | `60` | Maximum frame rate (throttled via `WaitUntil`). |
| `FISSION_RENDERER` | `auto` | Select the renderer. Native targets accept `auto`, `native-vello-gpu`, `native-vello-cpu`, `native-skia-raster`, or the compatibility spelling `native-software`, which now selects Skia raster. Web accepts `auto`, `webgpu-vello`, `web-canvaskit` (WebGL with software fallback), `web-canvaskit-webgl` (strict WebGL), or `web-canvaskit-software` via `globalThis.FISSION_RENDERER` or `?fission_renderer=`; `canvas2d-software` remains a compatibility spelling for CanvasKit software raster. |
| `FISSION_VELLO_USE_CPU` | `false` | Native compatibility escape hatch that asks Vello to use its CPU mode while still presenting through the GPU surface. |
| `FISSION_TEXTINPUT_BLINK` | `true` | Enable/disable cursor blinking in text inputs. |
| `FISSION_TEXTINPUT_BLINK_MS` | `530` | Cursor blink period in milliseconds. |
| `FISSION_USE_SYSTEM_FONTS` | `false` | Include system fonts in the font collection. |
| `FISSION_TEXT_TRACE` | `false` | Enable text input latency tracing to stderr. |
| `FISSION_SCROLL_TRACE` | `false` | Enable scroll event tracing to stderr. |
| `FISSION_TEST_CONTROL_PORT` | (none) | Start an HTTP test control server on this port. |

## Test control

When `FISSION_TEST_CONTROL_PORT` is set, the shell spawns a TCP server that accepts JSON commands from `fission-test-driver::LiveTestClient`. This enables automated UI testing by sending tap, scroll, type, screenshot, and semantic tree queries over HTTP. See the `fission-test-driver` crate for the client API.

## Platform support

- **Desktop**: used by `fission-shell-desktop`
- **iOS / Android**: used by `fission-shell-mobile`
- **Web**: used by `fission-shell-web`; WebGPU/Vello is the default renderer when the browser exposes a usable WebGPU adapter, and CanvasKit's optimized software raster surface is selected before context acquisition when WebGPU is unavailable.

## Skia software rendering

Native builds with the `skia` Cargo feature map `FISSION_RENDERER=software`,
`native-software`, and `native-skia-raster` to the same paired Skia raster and
SkParagraph profile; no native production path selects the previous standalone
software renderer. With Skia compiled in, `auto` normally begins with Vello,
selects Skia immediately for Windows CPU/WARP adapters or a failed Vello GPU
initialization, and keeps Vello CPU only as the last initialization fallback.
When a later valid complete frame exceeds Vello's declared capabilities, the
shell validates that whole frame against Skia, switches profiles only when Skia
accepts it, and relayouts with SkParagraph before presenting it.

Skia rasterization is currently read back and uploaded by the existing wgpu
presenter, so this path does not yet qualify as a Skia-only or no-wgpu build.
Unsupported operations fail the Fission capability gate instead of silently
switching to a renderer with different paragraph geometry.

On Web, `software`, `skia`, `web-canvaskit-software`, and the legacy
`canvas2d-software` spelling all select CanvasKit's CPU raster surface. The
standalone Fission software renderer is not part of the production Web shell.
