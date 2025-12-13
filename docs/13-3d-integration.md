# 13. 3D Integration

This section defines how **3D content** integrates into the framework without compromising determinism, testability, or the Core IR’s simplicity.
3D is treated as an embedded scene with explicit state, layout participation, and rendering boundaries—never as an implicit engine.

3D answers *how spatial content is composed and observed*, not *how a GPU is driven*.

---

## 13.1 Design Goals

3D integration must:

- preserve determinism and replayability,
- integrate with the Core IR and snapshots,
- be testable headlessly,
- compose with 2D UI (layout, input, semantics),
- avoid hard-coupling to any single 3D engine.

3D must be optional and sandboxed.

---

## 13.2 3D as a First-Class Embed

3D content is represented as a Core embed node with:

- a referenced 3D scene or graph resource,
- explicit viewport geometry,
- camera configuration,
- interaction and semantics bindings,
- explicit lifecycle state.

There is no implicit global scene.

---

## 13.3 Scene Graph Ownership

The framework distinguishes ownership:

- **Core** owns *what* scene is embedded and its state,
- **3D backend** owns *how* the scene is rendered.

Scene graphs are immutable inputs plus explicit state deltas.

---

## 13.4 Layout Participation

3D embeds participate in 2D layout as bounded viewports.

Rules:
- layout defines the viewport rect,
- clipping is enforced by the 2D pipeline,
- 3D content does not escape its bounds,
- resizing the viewport is deterministic.

3D does not influence surrounding layout.

---

## 13.5 Cameras as Explicit State

Cameras are explicit, serializable state.

Camera parameters include:
- position and orientation,
- projection type and parameters,
- near/far planes,
- field of view.

Camera changes occur via actions and reducers.

---

## 13.6 Transforms and Coordinate Spaces

3D introduces explicit coordinate spaces:

- world space,
- camera/view space,
- viewport space.

Transforms between spaces are explicit and snapshot-visible.
No implicit matrix stacks exist.

---

## 13.7 Input and Hit Testing

Input routing to 3D content is explicit.

Rules:
- 2D hit testing selects the 3D viewport,
- input is transformed into 3D space explicitly,
- picking results are deterministic.

3D picking results are surfaced as actions.

---

## 13.8 Semantics and Accessibility

3D content exposes semantics:

- roles (scene, object, control),
- labels and descriptions,
- actions (activate, rotate, select).

Accessibility never depends on visual rendering.

---

## 13.9 Animation and Time

3D animations integrate with the owned clock.

Rules:
- animation time is explicit,
- animations are driven by actions and ticks,
- no engine-owned timers exist.

3D animations participate in snapshots and replay.

---

## 13.10 Rendering Boundaries

Rendering responsibilities are split:

- Core produces a 3D render description,
- backend produces GPU commands,
- renderer consumes snapshots only.

Renderers never mutate 3D state.

---

## 13.11 Headless Testing of 3D

3D is testable headlessly.

Strategies:
- mock 3D backends,
- deterministic scene evaluators,
- snapshot-based assertions.

Tests may assert:
- camera state,
- object transforms,
- visibility and bounds,
- emitted render descriptors.

---

## 13.12 Snapshots and Inspection

3D snapshots include:

- scene identifiers and versions,
- camera parameters,
- active animations,
- visible object sets,
- error states.

Snapshots make 3D behavior observable and diffable.

---

## 13.13 Performance Considerations

Performance optimizations include:

- backend-level batching,
- scene caching,
- culling driven by explicit data.

Optimizations must not alter observable behavior.

---

## 13.14 Error Handling and Fallbacks

3D errors are explicit states.

Rules:
- load or render failures do not crash UI,
- fallbacks are deterministic (placeholders),
- errors are testable and replayable.

---

## 13.15 Future Extensions

The model supports future capabilities:

- AR/VR viewports,
- spatial UI composition,
- collaborative 3D scenes,
- LLM-driven spatial reasoning.

All extensions must preserve explicit state and determinism.

---

## 13.16 Summary

3D integration works because:

- 3D is embedded, not ambient,
- state and time are explicit,
- rendering is isolated,
- snapshots make behavior observable.

3D becomes just another deterministic system—no special rules required.

---
