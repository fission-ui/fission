# 13.2 Input Routing to 3D Surfaces

This section defines how **input events are routed to 3D embeds** in a deterministic, testable, and platform-independent manner.
3D input is never raw or ambient; it is explicitly routed, transformed, and surfaced through the same action system as all other input.

3D does not receive input directly from the platform.

---

## 13.2.1 Design Constraints

Input routing to 3D must:

- preserve determinism and replayability,
- integrate with the 2D hit-testing system,
- avoid engine-owned input loops,
- support accessibility and testing,
- produce observable, structured results.

Input is data, not callbacks.

---

## 13.2.2 Hit Testing and Target Selection

All input routing begins with standard 2D hit testing.

Rules:
- the layout snapshot identifies the target node,
- if the target is a 3D embed, routing continues,
- clip bounds are respected,
- z-order is deterministic.

3D surfaces cannot receive input unless selected by 2D hit testing.

---

## 13.2.3 Input Normalization

Platform input is normalized before routing.

Normalized input includes:
- pointer position (logical coordinates),
- button or gesture state,
- modifier keys,
- timestamp (logical time reference).

Normalization removes platform-specific variance.

---

## 13.2.4 Coordinate Transformation

Once a 3D embed is selected:

- input coordinates are transformed from global 2D space
- into embed-local viewport space,
- then into the 3D scene’s coordinate system.

All transforms are explicit and snapshot-visible.

---

## 13.2.5 Picking and Scene Queries

3D picking is modeled as a **pure query**.

Rules:
- the Core issues a pick request with transformed input,
- the 3D backend returns a structured result,
- results include object identifiers and metadata.

Picking does not mutate scene state.

---

## 13.2.6 Surfacing Picking Results

Picking results are surfaced as actions.

Examples:
- `ObjectHovered { id }`
- `ObjectSelected { id }`
- `ObjectActivated { id }`

Reducers decide how these results affect application state.

---

## 13.2.7 Gesture Interpretation

Gesture interpretation is explicit and layered.

Rules:
- raw input → intent actions (drag, rotate, zoom),
- intent actions are routed to reducers,
- reducers update camera or object state.

3D engines do not interpret gestures autonomously.

---

## 13.2.8 Focus and Keyboard Input

3D embeds may receive focus.

Rules:
- focus traversal is explicit,
- keyboard input routes through the same system,
- semantics define expected keyboard behavior.

There is no special keyboard handling for 3D.

---

## 13.2.9 Accessibility Input

Accessibility interactions route identically.

Rules:
- accessibility actions map to the same intent actions,
- picking results are abstracted semantically,
- screen readers do not depend on visual picking.

Accessibility is first-class for 3D.

---

## 13.2.10 Determinism Guarantees

Input routing is deterministic because:

- hit testing is snapshot-based,
- coordinate transforms are explicit,
- picking results are data, not callbacks,
- actions drive all state changes.

Identical input traces produce identical outcomes.

---

## 13.2.11 Headless Testing

3D input routing is testable headlessly.

Tests may:
- synthesize input events,
- assert routed actions,
- verify camera or object state changes.

No GPU or real engine is required.

---

## 13.2.12 Error Handling

Input routing errors include:

- invalid coordinate transforms,
- ambiguous picking results,
- backend query failures.

Errors are explicit, observable, and testable.

---

## 13.2.13 Summary

Input routing to 3D surfaces works because:

- 2D layout owns targeting,
- transforms are explicit,
- picking is a pure query,
- actions drive all behavior.

3D receives intent—not raw input.

---
