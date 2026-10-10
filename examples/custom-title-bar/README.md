# Custom desktop title bar

Run from the repository root:

```sh
cargo run -p custom-title-bar
```

The example opts out of native decorations with
`DesktopApp::with_decorations(false)`. `TitleBar` composes ordinary Fission
widgets inside `WindowDragRegion`. Its noninteractive content starts the OS
window move on the primary-button press; double-click toggles maximization.
The note field and window buttons keep their ordinary input behavior.

`WindowButton` binds an app action. Its reducer dispatches the built-in
`WINDOW_CONTROL` capability with `WindowCommand::{Minimize, Maximize, Restore,
Close}`. Commands run on the owned window event loop. Close respects the normal
tray close policy. `Restore` leaves maximization; the compositor/task switcher
restores minimized Wayland windows. Capability completion means the request was
submitted; the window manager can apply its own policy.

The Maximize/Restore label reads `view.env().window.maximized`, the state observed
from the native window. Tab and Shift+Tab navigate controls; Enter or Space
activates the focused button. The example uses built-in Fluent tokens and
English/Spanish translation bundles.

Undecorated, resizable Linux and Windows windows request native edge/corner
resize within a five-logical-pixel border. Keep controls padded inside that
border. The bundled winit backend does not support `drag_resize_window` on
macOS; this example does not add a macOS resize fallback.

Native GNOME/Wayland qualification is described in
[the testing guide](../../docs/testing/custom-title-bar.md). Windows and macOS
window-manager interaction still needs separate runtime qualification.
