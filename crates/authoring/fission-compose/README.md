# fission-compose

`fission-compose` contains the semantic contracts behind Fission's recommended component-authoring API. It connects concise component declarations to the existing retained widget tree, typed actions, state scopes and effects; it does not introduce a second runtime or a separate widget catalogue.

Application developers should normally depend on the [`fission`](https://crates.io/crates/fission) facade and import `fission::prelude::*`. The facade re-exports Compose, the built-in widgets and the shell selected by your feature flags. Depend on this crate directly only when implementing authoring infrastructure or integration tests.

## What it provides

- `fission_component!` for typed component properties, retained local state and declarative composition.
- `state_scope!` and `Scope` for editing an identified part of application state without copying it into a second authority.
- Typed value/update sources used by control bindings.
- Stable semantic identities for components, repeated children, fields and inline handlers.
- Event contracts for presses, text changes and toggles.
- `#[fission_effect]` and inline `start`/`Ok`/`Err` handling for component-owned async work.

Widget names are resolved by normal Rust imports. Named arguments initialise public struct fields and omitted fields use `Default`; positional arguments call the type's existing `new(...)` constructor. Third-party widgets require no Compose registration.

```rust
use fission::prelude::*;

fission_component! {
    pub Counter<()> {
        #[local_state(default = 0)]
        count: i32,
    } |counter, _state, _cx| =>
    Column(children: [
        Text(counter.count.to_string()),
        Button(
            child: Text("Increment"),
            key: "increment",
            on_press: |counter, _state, _cx| counter.count += 1,
        ),
    ])
}
```

Use ordinary Fission reducers and registered jobs for behaviour shared across components. Component-owned async work can stay beside the control that starts it:

```rust,ignore
#[fission_effect]
async fn save_draft(draft: String) -> Result<String, String> {
    api::save(draft).await
}

Button(
    child: Text("Save"),
    key: "save",
    on_press: effect save_draft(editor.draft.clone()) {
        start { editor.saving = true; }
        Ok(saved) {
            editor.draft = saved;
            editor.saving = false;
        }
        Err(error) {
            editor.error = Some(error);
            editor.saving = false;
        }
    },
)
```

Handwritten `From<YourWidget> for Widget` implementations remain fully interoperable for low-level custom rendering.

Read the [Compose guide](https://fission.rs/compose/) or run the checked-in [`compose` example](https://github.com/fission-ui/fission/tree/main/examples/compose).

## Licence

Apache-2.0.
