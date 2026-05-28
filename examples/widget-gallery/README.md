# Widget Gallery

Widget Gallery is the broadest single-screen reference app for Fission widgets. It renders common controls, feedback components, navigation widgets, overlays, form inputs, data display widgets, and layout helpers in one scrollable desktop app.

Use this example when you want to see how a widget is constructed in code before reading deeper reference documentation.

## Run it

```bash
cargo run -p widget-gallery
```

## What to look at

- [`src/main.rs`](src/main.rs) contains the gallery state, actions, reducers, helpers, and widget sections.
- `GalleryState` lists the interactive state needed by controls such as sliders, switches, text input, tabs, accordion, select, menu, modal, drawer, tree view, and toast.
- The action definitions near the top show both `#[fission_action]` and manual payload-bearing actions.
- The section helpers show a simple way to organize many unrelated widget demos in one file.

## Features exercised

- Buttons, checkboxes, switches, sliders, text input, number input, select, combobox-style menus, and pagination.
- Accordion, tabs, segmented control, breadcrumbs, drawer, modal, tooltip, toast, timeline, tree view, card, badge, avatar, stat, skeleton, progress, spinner, and code display.
- State-driven visibility for overlays and feedback components.
- `fission_action`, `reduce_with`, and `ActionEnvelope` usage.

## Learning path

Search [`src/main.rs`](src/main.rs) for the widget you care about, then inspect the matching state field and reducer. The gallery intentionally keeps most demos close together, so it is easy to copy a small pattern into a focused app.
