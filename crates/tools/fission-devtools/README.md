# fission-devtools

`fission-devtools` is the small public application-side SDK used by Fission
Developer. It turns an ordinary Fission root widget and serializable global
state into a generation worker that can be compiled, probed, and replaced by a
resident development host.

Applications keep their normal target entrypoints. A dedicated development
entrypoint can be as small as:

```rust,ignore
fission_devtools::devtools_main!(my_app::devtools);
```

The referenced function returns a `DevtoolsApp` configured with the same root,
environment synchronization, and persistent reducers as the normal app. The
SDK owns protocol negotiation, state snapshots, action dispatch, Core IR
generation, and orderly shutdown.

This crate is intended for development builds. It does not start a renderer or
platform shell, and applications do not need to include it in release targets.

The current MVP snapshots serializable application `GlobalState`. The resident
host retains interaction, scroll, focus, and text-editing state by stable widget
identity. Generation-owned local component state and host effects are not yet
transferred across the worker boundary. Generation-local custom render-object
hooks are omitted with a diagnostic while their serializable Core IR remains
active; ordinary built-in widgets and serializable Core IR are supported.
