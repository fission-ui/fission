# Custom title bar qualification

The example is `examples/custom-title-bar`. Build it with:

```sh
cargo build -p custom-title-bar --locked
cargo test -p fission-core --lib --test window_drag_region --test button_focus_click --locked
cargo test -p fission-shell-winit --lib --locked
cargo check -p fission-shell-winit --features tray --locked
cargo check -p fission-shell-winit --target wasm32-unknown-unknown --locked
```

The focused tests cover authoring/lowering and retained hit testing, interactive
and disabled descendants, double-click boundaries, edge resize directions,
shutdown cancellation, default decorations, button capture across focus-ring
changes, and native control keys carrying
text. The focus-ring regression fails on the previous implementation: the press
rebuild renumbers the text hit target, so release cancels the click. The ring now
uses an explicit derived identity. Tab/Enter/Escape must keep their key identity even when a native backend
attaches control text; literal Space must activate an ordinary button while
remapped, printable keyboard-layout output remains authoritative.

## Native GNOME/Wayland probe

Use an isolated Ubuntu desktop VM with GNOME/Wayland, Python 3 and `python3-gi`.
Run the app and probe in the same user/session. The shell must expose Mutter's
RemoteDesktop D-Bus interface. The probe creates and stops its own virtual input
session, starts and reaps its own app, and uses real compositor input for pointer
presses, typing, focus navigation and button activation. LiveTest supplies semantic
queries and screenshots; it does not inject mouse presses or window operations.

For compositor state observation, open GNOME's own developer console in this
isolated session (Alt+F2, `lg`), evaluate `global.context.unsafe_mode = true`, then
close the console/overview. This enables Shell Eval only in the disposable test
session. The probe reads the actual app window by its PID and activates it; after
minimization it asks the compositor to restore that same window. It does not
replace the compositor or infer native success from a reducer/UI label.

```sh
python3 crates/shell/fission-shell-winit/tests/custom_title_bar_wayland.py \
  --binary "$PWD/target/debug/custom-title-bar" \
  --output /tmp/fission-custom-title-bar-results
```

Headless sessions can pass `--session-bus-address-file PATH` containing the
session's D-Bus address. Set `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY` for the same
compositor. The probe creates its virtual input seat before launching the app,
then sends pointer motion from outside the window so the native backend receives
an enter/motion event before any click. It clears background-test mode and X11's
`DISPLAY`, uses an available loopback LiveTest port, disables caret blinking, and
requests the existing software renderer.

A passing `results.json` records the executable SHA-256, OS, actual native bounds
and the completed assertions. Exceptions record the failure and available tree/
screenshot evidence, terminate the app, and stop the owned input session.

Assertions:

1. Native decorations are hidden.
2. Native typing updates the embedded note without moving the window.
3. The ordinary Increment button updates app state.
4. Shift+Tab reaches Maximize; Enter maximizes. The compositor state and app's
   observed state/Restore label agree; a native Restore click leaves maximization.
5. A native title-area press/motion moves the window without resizing it.
6. Native double-clicks maximize and restore.
7. A native corner drag resizes the undecorated window.
8. Minimize sets the compositor's minimized state; compositor restoration makes
   the window usable again.
9. Close exits successfully within ten seconds.

## Coverage boundary

The native interaction probe targets GNOME/Wayland. macOS unit/compile checks do
not qualify AppKit window interaction, and Web/WASM compilation does not qualify
Windows behavior. The bundled winit macOS backend supports native moving but
returns NotSupported for drag-resize; Fission falls through to ordinary input and
does not supply a macOS resize fallback. Window managers may apply their own
maximize/minimize policies. Physical GPU/CPU measurements for #256 are separate
from this feature's compositor-state assertions.

## Verified VM run

Ubuntu 25.10, GNOME/Mutter 49, native Wayland, an ARM64 Linux VM and
Mesa/llvmpipe. All nine assertions passed with real OS input. The initial native
window was 800×600 at (112, 100); keyboard maximization produced 1024×736
at (0, 32). The title drag moved the restored window to (142, 110) without
changing its size. Corner resize produced 840×620, and minimize was observed as
`true` before compositor restoration. Native Close exited with status 0.

Initial, typed, maximized and resized captures were inspected. The probe also
exposed and rejected native Tab/Enter text routing and the focus-ring capture
bug before their corrections. The checked-in tests preserve both regressions.
