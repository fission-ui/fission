# 16. Platform Integration

This section describes how the framework integrates with **platforms** while preserving a single,
deterministic Core Runtime. Platform integration is intentionally thin: platforms host the runtime,
provide IO and surfaces, and forward events—but do not participate in UI logic.

Platforms are shells, not co-authors of behavior.

Implementation status: Winit currently owns renderer selection and
presentation control flow. It constructs and capability-checks an immutable
interactive frame, then calls the Vello or software encoder directly. The
Fission-owned graphics-session lifecycle exists as an internal contract but is
not yet the production Winit call path.

---

## 16.1 Design Principles

Platform integration follows strict principles:

- **Core-first**: all UI logic lives in the Core Runtime.
- **Thin shells**: platforms provide windows, surfaces, and input only.
- **Determinism preserved**: platform variance must not affect behavior.
- **Replaceable backends**: shells can be swapped without changing the Core.
- **Testable by default**: Core-facing platform behavior has headless or mock
  equivalents where meaningful; native integration still requires target tests.

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
- Presenter attachment and selected graphics-backend setup

In the target ownership model, no responsibility is shared. During the current
transition, the Winit host and renderer stack still split some backend-session
and presentation responsibilities as described below.

---

## 16.3 Event Flow

1. Platform receives raw input (mouse, touch, key, accessibility).
2. Shell normalizes input into Core events.
3. Core performs hit testing and semantics resolution.
4. Actions are dispatched to reducers.
5. Core produces new snapshots and the retained scene; the current Winit frame
   compiler adds viewport metadata and external-surface bindings.
6. The Winit host validates the resulting frame inputs and invokes its selected
   encoder; the target architecture submits them through the selected graphics
   session.

The platform never mutates Core state.

---

## 16.4 Rendering Integration

Platforms integrate rendering by:

- creating a rendering surface,
- selecting and attaching a renderer/presenter,
- validating immutable interactive frame inputs per frame,
- forwarding resize and platform lifecycle events.

The target presenter boundary routes resize, suspend, resume, loss, recovery,
memory pressure, rendering, and presentation through one
`GraphicsBackendSession`. Today those responsibilities remain split across
Winit, Vello/wgpu, and the standalone software rasterizer.

Renderers:
- must ultimately consume Fission-owned frame and resource semantics,
- do not compute layout,
- do not own time,
- do not reorder semantic commands,
- in the target contract, own only the backend lifecycle and derived rendering
  state assigned by the selected profile.

---

## 16.5 Accessibility Integration

Accessibility is bridged at the shell boundary.

Rules:
- Core defines roles, labels, actions, focus order.
- Platform shells translate semantics to native APIs.
- Native accessibility actions are mapped back to Core actions.

This is the required parity model. The current Winit accessibility bridge is a
no-op on WebAssembly and Android, so those adapters still require production
implementation and target qualification before they can claim that parity.

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

Core-facing platform services should have headless or mock equivalents where
that is meaningful for deterministic tests. Target-specific integration still
requires real-platform qualification; a mock is not evidence that a native
accessibility, IME, surface, or lifecycle adapter works.

Uses:
- CI testing,
- deterministic replay,
- LLM-driven inspection,
- offline snapshot analysis.

Headless shells implement the shared interfaces needed by the workflows they
claim to exercise.

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
- Core behavior can be tested without a real platform, while platform adapters
  retain their own target-specific tests.

Production-qualified target profiles must preserve the same UI semantics and
interaction contract on Web, desktop, mobile, CI, and headless environments.
Target and backend profiles may have documented visual and text-metric
differences; incomplete adapters do not yet establish cross-target parity.

---
