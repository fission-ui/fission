# 16. Platform Integration

This section describes how the framework integrates with **platforms** while preserving a single,
deterministic Core Runtime. Platform integration is intentionally thin: platforms host the runtime,
provide IO and surfaces, and forward events—but do not participate in UI logic.

Platforms are shells, not co-authors of behavior.

---

## 16.1 Design Principles

Platform integration follows strict principles:

- **Core-first**: all UI logic lives in the Core Runtime.
- **Thin shells**: platforms provide windows, surfaces, and input only.
- **Determinism preserved**: platform variance must not affect behavior.
- **Replaceable backends**: shells can be swapped without changing the Core.
- **Testable by default**: every platform feature has a headless/mocked equivalent.

---

## 16.2 Responsibilities Split

### Core Runtime Owns
- Core IR evaluation
- State, reducers, and actions
- Layout and display list generation
- Semantics and accessibility model
- Deterministic clock and animation
- Snapshotting and instrumentation

### Platform Shell Owns
- Window creation and lifecycle
- Surface creation (GPU/CPU)
- Event capture and normalization
- Clipboard, IME, system dialogs
- Platform accessibility API bridging
- Renderer selection and setup

No responsibility is shared.

---

## 16.3 Event Flow

1. Platform receives raw input (mouse, touch, key, accessibility).
2. Shell normalizes input into Core events.
3. Core performs hit testing and semantics resolution.
4. Actions are dispatched to reducers.
5. Core produces new snapshots and display lists.
6. Platform submits display list to renderer backend.

The platform never mutates Core state.

---

## 16.4 Rendering Integration

Platforms integrate rendering by:

- creating a rendering surface,
- selecting a renderer backend (e.g. Skia),
- submitting display lists per frame.

Renderers:
- consume display lists only,
- do not compute layout,
- do not own time,
- do not reorder commands.

---

## 16.5 Accessibility Integration

Accessibility is bridged at the shell boundary.

Rules:
- Core defines roles, labels, actions, focus order.
- Platform shells translate semantics to native APIs.
- Native accessibility actions are mapped back to Core actions.

This ensures parity between pointer and accessibility interaction.

---

## 16.6 Platform-Specific Services

Certain services are platform-provided but Core-controlled:

- clipboard (read/write via requests),
- text input / IME (stateful protocol),
- window metrics (DPI, insets),
- system theme and preferences.

All services are accessed via explicit, mockable interfaces.

---

## 16.7 Headless and Mock Platforms

Every platform feature has a headless equivalent.

Uses:
- CI testing,
- deterministic replay,
- LLM-driven inspection,
- offline snapshot analysis.

Headless shells implement the same interfaces as real platforms.

---

## 16.8 Error Isolation and Fault Handling

Platform errors must not corrupt Core state.

Rules:
- platform failures are surfaced as explicit events,
- Core state remains valid,
- retries and fallbacks are deterministic.

Undefined behavior is forbidden.

---

## 16.9 Versioning and Compatibility

Platform shells are versioned independently.

Compatibility rules:
- Core Runtime defines the contract,
- shells declare supported capabilities,
- incompatible features fail explicitly.

This enables gradual evolution.

---

## 16.10 Summary

Platform integration works because:

- the Core Runtime is authoritative,
- shells are thin and replaceable,
- determinism is enforced at the boundary,
- testing does not depend on real platforms.

The same UI runs identically on web, desktop, mobile, CI, and in an LLM’s headless environment.

---
