# Inbox

Inbox is a production-style mail client example. It demonstrates how to structure a richer app with multiple feature areas, shared state, nested components, compose flows, settings, contacts, and a responsive reading layout.

Use this example when you want to see recommended module organization for a real Fission desktop app instead of a small single-file demo.

## Run it

```bash
cargo run -p inbox
```

## What to look at

- [`src/main.rs`](src/main.rs) wires the app shell and top-level layout.
- [`src/model/app_state.rs`](src/model/app_state.rs), [`src/model/actions.rs`](src/model/actions.rs), and [`src/model/email.rs`](src/model/email.rs) define the domain model and reducer-facing actions.
- [`src/components/sidebar.rs`](src/components/sidebar.rs), [`src/components/email_list.rs`](src/components/email_list.rs), [`src/components/email_detail.rs`](src/components/email_detail.rs), and [`src/components/right_sidebar.rs`](src/components/right_sidebar.rs) define reusable UI components.
- [`src/features/compose.rs`](src/features/compose.rs), [`src/features/contacts.rs`](src/features/contacts.rs), [`src/features/settings.rs`](src/features/settings.rs), and [`src/features/browser.rs`](src/features/browser.rs) show feature-level modules layered on top of shared state.
- [`src/tests/mod.rs`](src/tests/mod.rs) contains example-specific test helpers and assertions.

## Features exercised

- Multi-pane app layout with sidebar, list, detail, and right rail.
- Compose modal and feature overlays.
- App-level state organization across model, components, and features.
- Fission action/reducer patterns in a non-trivial app.
- Desktop screenshot and driver test dependencies in [`Cargo.toml`](Cargo.toml).

## Learning path

Start with [`src/model/app_state.rs`](src/model/app_state.rs) to understand the domain state. Then trace one visible part of the UI: for example, follow the selected email from the model into [`src/components/email_list.rs`](src/components/email_list.rs) and [`src/components/email_detail.rs`](src/components/email_detail.rs). That is the same approach you should use when building larger apps: keep state and feature modules discoverable.
