# 16.3 Web Shells

This section describes the **web platform shells**.
Web shells host the Core Runtime inside a browser environment while preserving determinism, testability, and behavioral parity with desktop and mobile.

The web shell adapts the browser to the Core—not the other way around.

---

## 16.3.1 Goals of Web Shells

Web shells must:

- run the Core Runtime via WASM or JS bindings,
- provide deterministic rendering paths,
- normalize browser input events,
- integrate with browser accessibility,
- support headless execution for CI,
- remain thin and replaceable.

They must not encode UI logic or layout decisions.

---

## 16.3.2 Execution Model

Two primary execution modes are supported:

- **WASM-first**: Core Runtime compiled to WASM, hosted by JS.
- **JS-hosted Core**: Core logic hosted in JS with a compatible API surface (fallback).

WASM-first is the preferred and authoritative path.

---

## 16.3.3 Rendering Strategies

Web shells may use multiple rendering strategies:

- **Canvas 2D** (baseline, widely supported),
- **WebGL / WebGPU** (future, accelerated),
- **OffscreenCanvas** (for workers and headless execution).

All strategies consume the same display list.

---

## 16.3.4 Deterministic Rendering Constraints

Browser nondeterminism is constrained by:

- fixed canvas size and DPR normalization,
- pinned font loading and shaping,
- explicit color space configuration,
- disabled subpixel variability where possible.

Pixel tests require stricter pinning than other platforms.

---

## 16.3.5 Input Normalization

Web shells capture and normalize:

- pointer events (mouse, touch, pen),
- keyboard events,
- wheel and gesture events,
- accessibility activation events.

All browser events are normalized into Core event types before hit testing.

---

## 16.3.6 Text Input and IME

Text input uses a controlled protocol:

- Core declares focused text fields,
- shell manages hidden DOM inputs if required,
- composition and selection updates are forwarded deterministically,
- committed text emits Core events.

IME behavior is mockable for tests.

---

## 16.3.7 Accessibility Integration

Web accessibility integrates via ARIA:

- Core semantics define roles, labels, actions,
- shell maps semantics to ARIA attributes,
- browser accessibility actions dispatch Core actions.

DOM structure is minimal and semantics-driven.

---

## 16.3.8 Event Loop and Timing

Browser event loops are not trusted for time.

Rules:
- Core owns the clock,
- animation advancement is explicit,
- `requestAnimationFrame` is used only as a scheduling hint.

Timing determinism is preserved.

---

## 16.3.9 Headless Web Execution

Web shells support headless execution via:

- Headless Chrome / Firefox,
- OffscreenCanvas,
- worker-based rendering.

This enables CI, snapshot generation, and LLM-driven inspection.

---

## 16.3.10 Networking and Security Boundaries

Web shells respect browser security models.

Rules:
- networking uses explicit Core APIs,
- sandbox restrictions are surfaced explicitly,
- errors are deterministic and observable.

Security constraints do not leak into UI logic.

---

## 16.3.11 Error Handling

Web-specific failures include:

- context loss,
- font load failure,
- canvas resize,
- worker termination.

All errors are reported to Core as explicit events.

---

## 16.3.12 Performance Considerations

Web performance considerations:

- minimize JS↔WASM crossings,
- batch display list submission,
- reuse buffers where possible.

Performance tuning does not alter semantics.

---

## 16.3.13 Determinism Caveats

Despite constraints, the web has limits:

- font rendering differences across browsers,
- GPU driver variability.

These are mitigated but never hidden.
Tests must pin environments explicitly.

---

## 16.3.14 Why the Web Fits the Model

The web works because:

- display lists map naturally to canvas APIs,
- headless execution is mature,
- accessibility is semantics-driven,
- shells remain thin.

The Core Runtime remains authoritative.

---

## 16.3.15 Summary

Web shells integrate successfully because:

- Core logic is isolated and deterministic,
- browser APIs are treated as IO,
- rendering paths are interchangeable,
- CI and testing remain first-class.

The same UI runs identically in browsers, CI, and headless analysis environments—within pinned constraints.

---
