# 16.1 Desktop Shells

This section describes the **desktop platform shells** for Windows, macOS, and Linux.
Desktop shells are responsible for hosting the Core Runtime, integrating with native windowing systems, and presenting rendered output—without owning UI behavior.

Desktop platforms differ in APIs, but their contract with the Core is identical.

---

## 16.1.1 Goals of Desktop Shells

Desktop shells must:

- provide native windows and surfaces,
- forward input events deterministically,
- integrate accessibility APIs,
- support GPU and CPU rendering paths,
- remain thin and replaceable.

They must not introduce platform-specific UI logic.

---

## 16.1.2 Supported Platforms

Initial first-class targets:

- **Windows** (Win32 / Windows App SDK)
- **macOS** (AppKit)
- **Linux** (Wayland / X11 via abstraction)

Each shell implements the same Core-facing interfaces.

---

## 16.1.3 Window and Lifecycle Management

Responsibilities include:

- window creation and destruction,
- resize and DPI change notifications,
- focus and visibility changes,
- application lifecycle events (suspend, resume).

Lifecycle events are forwarded to the Core as explicit events.

---

## 16.1.4 Surface Creation

Desktop shells create rendering surfaces:

- GPU-backed surfaces (preferred),
- CPU-backed surfaces (fallback / testing),
- fixed-format surfaces for determinism.

Surface creation details are hidden from the Core.

---

## 16.1.5 Rendering Loop Integration

Typical frame flow:

1. Core produces a display list.
2. Shell submits display list to renderer.
3. Renderer draws into the surface.
4. Shell presents the surface.

The shell does not schedule layout or animation—Core does.

---

## 16.1.6 Input Handling

Desktop shells capture native input:

- mouse movement and clicks,
- touch (where supported),
- keyboard input,
- scroll wheels and trackpads.

Shells normalize input into Core event types before forwarding.

---

## 16.1.7 IME and Text Input

Text input uses an explicit protocol:

- Core declares text fields and focus,
- shell connects to native IME APIs,
- composition updates are forwarded deterministically,
- committed text is emitted as Core events.

IME behavior is testable via mock shells.

---

## 16.1.8 Clipboard and System Integration

Shells provide access to:

- clipboard read/write,
- drag-and-drop (future),
- system cursors,
- window chrome integration.

All access occurs via Core-issued requests.

---

## 16.1.9 Accessibility Integration

Desktop shells bridge Core semantics to native APIs:

- Windows UI Automation,
- macOS Accessibility API,
- Linux AT-SPI.

Rules:
- Core owns roles, labels, actions,
- shells translate and forward,
- native accessibility actions dispatch Core actions.

Parity with pointer input is mandatory.

---

## 16.1.10 Determinism Guarantees

Desktop shells must not affect determinism.

Rules:
- no platform timing is trusted,
- no layout decisions are delegated,
- floating-point differences are normalized.

Shells are forbidden from “helping.”

---

## 16.1.11 Headless Desktop Shell

A headless desktop shell exists for:

- CI testing,
- snapshot generation,
- automated interaction tests.

It implements the same interfaces without creating windows.

---

## 16.1.12 Error Handling

Shell errors are isolated.

Examples:
- surface loss,
- GPU reset,
- window destruction.

Errors are reported to Core explicitly and handled deterministically.

---

## 16.1.13 Why Desktop Is Straightforward

Desktop platforms are well-suited because:

- windowing APIs are stable,
- accessibility APIs are explicit,
- input models are mature,
- headless execution is feasible.

The desktop shell serves as the reference implementation.

---

## 16.1.14 Summary

Desktop shells work because:

- the Core Runtime is authoritative,
- shells are thin adapters,
- determinism is enforced at boundaries,
- testing does not require real windows.

Desktop is the proving ground for the framework’s architecture.

---
