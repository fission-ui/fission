# Wayland focused-input IME regression

The [reporter's trace for #256](https://github.com/fission-ui/fission/issues/256#issuecomment-6097817578) contains 1,081 redraws, mostly 16 ms apart, with repeated IME-triggered rebuilds and no caret-blink events. The regression was reproduced using his app on Ubuntu 25.10 with GNOME Shell/Mutter 49 and native Wayland text input in Docker Desktop's Linux VM.

Wayland cursor-area updates commit text-input state. GNOME acknowledges the commit, and the winit backend can translate that acknowledgement into an empty preedit event. Handling that event synchronizes the focused input back to the platform, sending the same cursor area again. This creates an idle feedback loop independent of caret blinking.

`DesktopImeHandler` now suppresses identical cursor-area updates on Wayland. A changed position, size, or scale still publishes an update. Window changes, IME enable/disable transitions, and changed text-input configuration clear the cache. Other display backends retain their existing update behavior.

## Run the regression

Use an isolated Ubuntu GNOME/Wayland session with Python's `gi` module, IBus and GNOME's `org.gnome.Mutter.RemoteDesktop` interface available. Run only one GUI probe at a time; another window taking keyboard focus invalidates the sample. Keep compilation stopped while sampling CPU.

The fixture preserves the reporter's app behavior, with formatting and a standalone entry point. Build it from the repository root:

```sh
CARGO_TARGET_DIR=/tmp/fission-wayland-target cargo build --release \
  --manifest-path crates/shell/fission-shell-winit/tests/fixtures/focused-input-reporter/Cargo.toml

python3 crates/shell/fission-shell-winit/tests/focused_input_wayland.py \
  --gnome \
  --binary /tmp/fission-wayland-target/release/fission-focused-input-reporter \
  --output /tmp/fission-wayland-results/blink-off \
  --renderer native-vello-gpu --blink 0
```

Repeat with `--blink 1` and a separate output directory. To measure the generated development profile, omit `--release` and use the `debug` binary. `--session-bus-address-file PATH` supports an isolated GNOME session whose D-Bus address is stored separately; a normal GNOME terminal inherits the session address.

The probe owns a visible app and a native GNOME remote-input session. It uses Fission's LiveTest API for queries, focus, screenshots and counter actions, and sends actual OS keyboard events for typing, cursor movement, selection, Unicode composition and composition cancellation. It also checks an injected preedit/commit independently. End-key navigation activates GNOME's lazy input context before focused sampling; simply setting framework focus without a native event can miss this regression.

Each phase warms for three seconds and takes two ten-second samples without control requests during sampling. It records process and thread CPU ticks, redraw/wakeup counts, the renderer, the binary hash and a screenshot. Unfocused and blink-disabled phases allow at most two settling redraws per sample; ordinary focused blinking allows at most 25. The probe also rejects excessive event-loop wakeups. An unsuccessful exit fails; a ten-second shutdown timeout kills and reaps the app. `--record-baseline` retains measurements while disabling only the idle bounds, so input and shutdown checks still apply.

## Recorded VM evidence

The release comparison used the reporter's app, an 800×600 visible window, Ubuntu 25.10, GNOME/Mutter 49, Vulkan and Mesa llvmpipe (LLVM 20.1.8). The before binary already included #270's earlier renderer and caret changes; the remaining difference is cursor-area synchronization.

| Configuration | Focused redraws / 10 s | Focused process CPU, one core |
| --- | ---: | ---: |
| Before IME fix, explicit GPU, blinking disabled | 597–600 | 124–126% |
| After IME fix, explicit GPU, blinking disabled | 0 | Below tick resolution |
| After IME fix, explicit GPU, ordinary blinking | 19 | 4.9–6.7% |

Both samples in the blink-disabled comparison agree. Unfocused and button-focused phases have zero sustained redraws. The fixed runtime passes native typing, both counter actions, Unicode composition, selection, composition cancellation and injected IME commit checks. The regression probe rejects the before binary and accepts the fixed binary.

100% means one CPU core; multithreaded llvmpipe can exceed that. These traced VM readings are diagnostic evidence, not measurements of the reporter's physical Intel HD Graphics 520. They verify the feedback-loop fix through a real GNOME/Wayland input path. Keep #256 open until the fix is available and physical-device behavior is confirmed.
