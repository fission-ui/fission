# Animation Gallery

Animation Gallery is a router-backed Fission motion workbench. It is intentionally calm by default: choose a widget or property, press Play, scrub the timeline, inspect lowered motion, and compare the ergonomic API with custom and verbose native forms.

## Run it

```bash
cargo run -p animation-gallery
```

## Structure

- `src/main.rs` starts the app only.
- `src/app.rs` wires `DesktopApp`, including shell route handling.
- `src/chrome.rs` owns the brand rail, sidebar navigation, and workbench shell.
- `src/pages/` contains overview, property lab, composition, policy, and diagnostics routes.
- `src/widgets/` contains one module per widget page plus shared widget-page helpers.
- `src/state.rs` contains app state and global reducers for navigation, source tabs, policy, and timeline controls.

## What it proves

- Built-in widget motion is explicit opt-in: `motion: None` emits no widget-owned motion.
- Widget-owned motion enums are documented through previewable cases.
- Supported `MotionPropertyId` values can be inspected in isolation.
- Ergonomic examples have equivalent custom widget-motion and verbose native `Motion`/`Presence` examples.
- Routes are handled through Fission `Router`, with shell route changes stored in app state.

## Sections

- Overview: coverage matrix for widgets and motion properties.
- Widgets: per-widget controls, live preview, source tabs, and inspector.
- Properties: one page per property with value type, phase, layout/paint effects, and a raw `MotionTrack`.
- Composition: additive atoms, conflicting atoms, custom overrides, and ordered last-wins behavior.
- Motion Policy: full, reduced, and disabled evaluation without changing source structure.
- Diagnostics: lowered declarations, expressions, timeline values, and deterministic test examples.

## Design target

The implemented layout follows `crates/core/fission-theme/design/animation-gallery.png`: a brand rail, grouped navigation, calm cards, explicit controls, preview/workbench panels, source tabs, and an inspector.
