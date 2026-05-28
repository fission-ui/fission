# Animation Gallery

Animation Gallery shows how Fission animates ordinary widget trees without requiring an application-specific render loop. The app groups small cards for opacity, translation, scale, rotation, clipping, scrolling, transitions, and a compositor-driven pulse so you can compare animation properties side by side.

Use this example when you want to learn how a widget asks the runtime to animate a stable `WidgetNodeId`, how transition widgets wrap existing content, and how state changes trigger one-shot transitions.

## Run it

```bash
cargo run -p animation-gallery
```

## What to look at

- [`src/main.rs`](src/main.rs) contains the complete app, including the animation state, reducers, stable node identifiers, and gallery layout.
- The `OPACITY_ID`, `TRANSLATE_ID`, `SCALE_ID`, `ROTATION_ID`, `CLIP_ID`, and `CUSTOM_ID` values show the pattern for assigning stable animation identities.
- The `AnimationGalleryApp::build` implementation shows how `Transition` wraps normal UI and how `ctx.anim_for(...).request(...)` starts an explicit animation.
- The test driver dependency in [`Cargo.toml`](Cargo.toml) is used by screenshot and smoke tests for examples that need live rendering assertions.

## Features exercised

- `AnimationRequest` and `AnimationPropertyId` for explicit animation requests.
- `Transition` for declarative animated changes between build passes.
- `Wrap`, `Scroll`, `Row`, `Column`, `Container`, `Text`, and `Button` for responsive gallery layout.
- `#[fission_reducer]` and `with_reducer!` for compact action/reducer wiring.

## Learning path

Start with the `ToggleScene` reducer and follow `scene_active` through `AnimationGalleryApp::build`. That shows the common pattern: state changes first, the next build computes the target visual value, and Fission animates the node from the previous value to the new value.
