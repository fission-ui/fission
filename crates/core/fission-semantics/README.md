# fission-semantics

Accessibility semantics for the Fission UI framework.

`fission-semantics` re-exports the semantic types defined in `fission-ir` -- `Role`, `Semantics`,
and `ActionSet` -- so that crates outside the core can depend on the accessibility vocabulary
without pulling in the full IR. These types describe what a node *means* to assistive technology
(screen readers, switch control) and to the event system (focusable, draggable, scrollable), as
opposed to how the node looks or where it is positioned.

## Key types

| Type | Description |
|------|-------------|
| `Role` | Accessibility role enum: `Button`, `Text`, `TextInput`, `Image`, `Checkbox`, `Switch`, `Dialog`, `Slider`, `Input`, `List`, `ListItem`, `Generic`. |
| `Semantics` | Full semantic metadata for a node -- role, label, value, focusability, drag-and-drop flags, scroll flags, toggle state, input mask, focus scope/barrier, and more. |
| `ActionSet` | A collection of `ActionEntry` values that map `ActionTrigger` gestures (tap, drag, hover, focus, change, etc.) to action IDs dispatched by the event system. |

## Usage example

```rust
use fission_semantics::{Role, Semantics, ActionSet};

let sem = Semantics {
    role: Role::Button,
    label: Some("Save".into()),
    focusable: true,
    ..Semantics::default()
};

assert_eq!(sem.role, Role::Button);
assert!(!sem.disabled);
```

## Status

**Supported** -- stable public API consumed by widgets, the accessibility bridge, and test tooling.

## License

MIT -- see the [Fission repository](https://github.com/worka-ai/fission) for full documentation at https://fission.rs.
