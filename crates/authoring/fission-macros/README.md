# fission-macros

Procedural macros for the Fission UI framework.

## `#[fission_reducer]`

`#[fission_reducer]` is the recommended application-level shortcut for the common case where an action exists only to call one reducer.

```rust
use fission::prelude::*;

#[fission_reducer(Increment)]
fn increment(state: &mut CounterState) {
    state.count += 1;
}
```

This generates a real action type named `Increment` and a canonical reducer handler named `increment`. The generated action uses the reducer function visibility, so `pub fn` generates a public action and private functions generate private actions. You still bind it normally:

```rust
let on_press = with_reducer!(ctx, Increment, increment);
```

Payload parameters become tuple fields on the generated action:

```rust
#[fission_reducer(SetCount)]
fn set_count(state: &mut CounterState, value: i32) {
    state.count = value;
}

let action = with_reducer!(ctx, SetCount(10), set_count);
```

Reducers that need effects or input can keep an explicit final context parameter:

```rust
#[fission_reducer(SaveDraft)]
fn save_draft(state: &mut EditorState, ctx: &mut ReducerContext<EditorState>) {
    ctx.effects.app(SAVE_DRAFT, state.draft.clone());
}
```

For generated actions whose payloads cannot implement `Eq`, use `no_eq`:

```rust
#[fission_reducer(SetScale, no_eq)]
fn set_scale(state: &mut CanvasState, value: f32) {
    state.scale = value;
}
```

## `#[fission_action]`

`#[fission_action]` remains the manual action definition tool. Use it when an action is shared by multiple reducers, exported as part of your public API, documented independently, or constructed in places where you want the action type spelled out explicitly.

```rust
use fission::prelude::*;

#[fission_action]
struct Increment;

fn increment(
    state: &mut CounterState,
    _action: Increment,
    _ctx: &mut ReducerContext<CounterState>,
) {
    state.count += 1;
}
```

This expands to the standard Fission action implementation plus the common serialization, debug, clone, and equality derives. The generated `Action` implementation computes the deterministic action ID from `module_path!()` and the Rust type name, then caches it with `std::sync::OnceLock`.

For payloads that cannot implement `Eq`, use:

```rust
#[fission_action(no_eq)]
struct SetScale(f32);
```

## `#[derive(Action)]`

The derive macro remains available for lower-level crates that want to choose their own serialization and comparison derives explicitly. Application code should normally prefer `#[fission_reducer]` for one-reducer actions or `#[fission_action]` for manually declared actions.

## `#[derive(Widget)]`

Reserved derive macro for future widget code generation. Currently a no-op that produces an empty token stream.
