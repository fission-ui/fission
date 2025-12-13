# 16.5 Input and Lifecycle Handling

This section defines how **input events** and **application lifecycle events** are handled across all platforms.
The guiding principle is that *platforms emit signals*, while the **Core Runtime interprets them deterministically**.

Input and lifecycle are data streams, not callbacks.

---

## 16.5.1 Design Principles

Input and lifecycle handling must:

- be fully deterministic and replayable,
- normalize platform-specific details early,
- avoid hidden state in shells,
- unify pointer, keyboard, accessibility, and system signals,
- be testable headlessly.

No platform is allowed to inject behavior.

---

## 16.5.2 Input Categories

The Core Runtime recognizes normalized input categories:

- **Pointer input**: mouse, touch, pen
- **Keyboard input**: key press/release, modifiers
- **Scroll input**: wheel, trackpad, gesture scroll
- **Text input**: IME composition and commits
- **Accessibility input**: activate, increment, focus
- **System input**: back navigation, app commands

All platform events map into these categories.

---

## 16.5.3 Input Normalization

Platform shells normalize raw input into Core events.

Normalization includes:
- coordinate normalization to logical pixels,
- consistent button and modifier mapping,
- explicit pointer identity for multi-touch,
- canonical scroll units and directions.

After normalization, platform origin is irrelevant.

---

## 16.5.4 Event Dispatch Flow

The canonical dispatch flow is:

1. Platform emits a raw event.
2. Shell normalizes it into a Core event.
3. Core performs hit testing and focus resolution.
4. Core maps the event to semantic actions.
5. Actions are dispatched to reducers.
6. New snapshots are produced.

This flow is identical across platforms.

---

## 16.5.5 Hit Testing and Focus Resolution

Hit testing and focus are Core responsibilities.

Rules:
- hit testing uses the layout snapshot,
- z-order and clipping are respected,
- focus changes are explicit state updates,
- accessibility and pointer share resolution logic.

Platforms do not perform hit testing.

---

## 16.5.6 Text Input and IME Lifecycle

Text input follows an explicit protocol:

- Core declares editable nodes and focus,
- shells open and manage native IME sessions,
- composition updates are forwarded as events,
- commit/cancel events are explicit and ordered.

IME behavior is replayable and mockable.

---

## 16.5.7 Gesture Recognition

Gesture recognition policy:

- shells may provide low-level gesture signals,
- Core decides semantic interpretation,
- gestures never bypass the action system.

This avoids platform divergence.

---

## 16.5.8 Application Lifecycle Events

Lifecycle events are normalized:

- launch / initialize
- foreground / background
- suspend / resume
- low-memory warnings
- terminate / shutdown

Events are ordered, explicit, and replayable.

---

## 16.5.9 Lifecycle Effects on Core State

The Core decides lifecycle effects.

Examples:
- pause animations on background,
- persist state on suspend,
- refresh resources on resume.

Platforms never mutate Core state directly.

---

## 16.5.10 Deterministic Time and Lifecycle

Lifecycle does not advance time implicitly.

Rules:
- Core clock pauses/resumes explicitly,
- background time does not elapse unless requested,
- tests control lifecycle transitions explicitly.

This avoids timing flakiness.

---

## 16.5.11 Error and Cancellation Handling

Input cancellation (e.g. touch cancel) and lifecycle interruption are explicit.

Rules:
- cancellations generate Core events,
- partial interactions are resolved deterministically,
- reducers handle incomplete sequences explicitly.

Undefined behavior is forbidden.

---

## 16.5.12 Testing Input and Lifecycle

Input and lifecycle are testable headlessly.

Tests can:
- inject normalized input events,
- simulate lifecycle transitions,
- assert resulting actions and snapshots,
- replay complex interaction sequences.

Real devices are not required.

---

## 16.5.13 Security and Isolation

Input handling respects platform security:

- permission denial surfaces as explicit events,
- sandbox restrictions are observable,
- failures do not corrupt Core state.

Security constraints remain visible.

---

## 16.5.14 Summary

Input and lifecycle handling works because:

- all signals are normalized early,
- the Core interprets everything deterministically,
- actions are the only mutation mechanism,
- tests can replay reality exactly.

Platforms deliver signals; the Core defines behavior.
