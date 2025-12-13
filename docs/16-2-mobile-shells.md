# 16.2 Mobile Shells

This section describes the **mobile platform shells** for iOS and Android.
Mobile shells host the Core Runtime within native application lifecycles while preserving determinism, testability, and behavioral parity with desktop and web.

Mobile platforms impose constraints; the Core absorbs them without leaking complexity.

---

## 16.2.1 Goals of Mobile Shells

Mobile shells must:

- integrate with native app lifecycles,
- host the Core Runtime efficiently,
- forward input deterministically (touch, keyboard, accessibility),
- support GPU-backed rendering,
- respect platform resource constraints,
- remain thin and replaceable.

They must not encode UI logic.

---

## 16.2.2 Supported Platforms

First-class mobile targets:

- **iOS** (UIKit / SwiftUI host container)
- **Android** (View / SurfaceView / Compose host container)

Each shell implements the same Core-facing contracts as desktop.

---

## 16.2.3 Application Lifecycle Integration

Mobile shells integrate with lifecycle events:

- app launch and termination,
- background / foreground transitions,
- suspension and resume,
- low-memory warnings.

Lifecycle changes are forwarded to Core as explicit, ordered events.
The Core decides how UI state responds.

---

## 16.2.4 Surface and Rendering Integration

Mobile shells provide rendering surfaces:

- Metal-backed surfaces on iOS,
- Vulkan / OpenGL / ANGLE-backed surfaces on Android,
- CPU fallback for testing.

The shell selects and initializes the renderer; the Core submits display lists.

---

## 16.2.5 Frame Scheduling and Timing

Mobile platforms drive frame callbacks, but:

- the Core owns the clock,
- frame requests are explicit,
- animation advancement is deterministic.

Platform frame cadence never implicitly advances Core time.

---

## 16.2.6 Touch and Pointer Input

Mobile shells capture:

- touch down / move / up,
- multi-touch gestures,
- pointer cancellation,
- platform gesture recognizers (normalized).

All input is normalized into Core event types before hit testing.

---

## 16.2.7 Keyboard and Text Input

Text input is mediated through an explicit protocol:

- Core declares focused text fields,
- shells connect to native keyboards / IMEs,
- composition and selection updates are forwarded deterministically,
- committed text produces Core events.

IME behavior is mockable for tests.

---

## 16.2.8 Accessibility Integration

Mobile accessibility is bridged carefully.

- Core defines semantics, roles, labels, actions.
- Shells map semantics to:
  - iOS Accessibility APIs,
  - Android Accessibility Services.
- Accessibility activation dispatches Core actions.

Accessibility parity with touch is mandatory.

---

## 16.2.9 System Gestures and Navigation

Platform-level gestures (e.g. back navigation) are surfaced as events.

Rules:
- shells do not intercept gestures silently,
- Core decides how to handle navigation intents,
- unhandled gestures may be forwarded to the OS explicitly.

This preserves predictability.

---

## 16.2.10 Resource Management

Mobile shells assist with:

- surface recreation,
- GPU context loss,
- memory pressure signaling.

Core state remains valid across recoverable failures.

---

## 16.2.11 Determinism Considerations

Mobile-specific nondeterminism is constrained:

- device DPI differences normalized to logical pixels,
- timing jitter isolated from Core time,
- floating-point normalization applied consistently.

Tests behave identically on desktop and mobile.

---

## 16.2.12 Headless and Simulator Support

Mobile shells support:

- simulator/emulator execution,
- headless modes for CI,
- mock input and lifecycle events.

Real devices are not required for correctness testing.

---

## 16.2.13 Error Handling

Errors such as:

- surface loss,
- background kill,
- permission revocation

are surfaced as explicit events. The Core defines recovery behavior.

---

## 16.2.14 Why Mobile Remains Thin

Despite platform complexity:

- UI logic remains centralized,
- testing does not require devices,
- platform differences are abstracted.

Mobile shells adapt the environment, not the UI.

---

## 16.2.15 Summary

Mobile shells succeed because:

- the Core Runtime remains authoritative,
- lifecycle and input are explicit data,
- rendering is decoupled,
- determinism survives platform constraints.

The same UI behaves identically on phones, tablets, simulators, and CI.

---
