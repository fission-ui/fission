# 11.4 Embed Lifecycle and Platform Responsibilities

This section defines the **lifecycle of embeds** (images, video, audio, native surfaces) and the clear division of responsibility between the Core runtime and platform shells.
A strict lifecycle model is required to preserve determinism, portability, and testability.

The Core defines *what* must happen; platforms define *how* it is executed.

---

## 11.4.1 Lifecycle Overview

All embeds follow the same high-level lifecycle:

1. Declaration in Core IR
2. Resource resolution
3. Activation
4. Steady-state operation
5. Deactivation
6. Resource release

Each phase is explicit and observable.

---

## 11.4.2 Declaration Phase

During declaration:

- an embed node appears in Core IR,
- it references a resource identifier,
- sizing and semantics are defined.

Declaration is pure data and has no side effects.

---

## 11.4.3 Resource Resolution Phase

Resource resolution occurs when:

- the Core requests a resource by identifier,
- the platform shell resolves it to a concrete handle.

Rules:
- resolution is deterministic given the same inputs,
- failures are reported explicitly,
- resolution does not affect layout geometry.

Resolution may be asynchronous but results are explicit.

---

## 11.4.4 Activation Phase

Activation occurs when an embed becomes “live”.

Examples:
- image decode completion,
- video surface creation,
- audio stream initialization.

Rules:
- activation transitions are explicit state changes,
- no implicit retries or background magic,
- activation order is deterministic.

Activation does not change layout unless explicitly modeled.

---

## 11.4.5 Steady-State Operation

In steady state:

- embeds respond to actions,
- playback progresses via explicit ticks,
- frames or samples are produced deterministically.

The Core observes state; platforms execute side effects.

---

## 11.4.6 Deactivation Phase

Deactivation occurs when:

- an embed is removed from Core IR,
- visibility changes require suspension,
- resources are explicitly released.

Rules:
- deactivation is explicit and observable,
- resources are not leaked,
- deactivation order is deterministic.

---

## 11.4.7 Resource Release

Resource release is final.

Rules:
- resources are released exactly once,
- release order is deterministic,
- double-release is an error.

Resource lifecycle is traceable for debugging and tests.

---

## 11.4.8 Core Responsibilities

The Core runtime is responsible for:

- declaring embed intent,
- defining lifecycle state transitions,
- owning embed-related state,
- exposing snapshots and inspection APIs,
- driving actions and reducers.

The Core never performs IO or platform calls directly.

---

## 11.4.9 Platform Shell Responsibilities

Platform shells are responsible for:

- loading and decoding resources,
- managing OS or GPU handles,
- executing side effects (playback, decoding),
- reporting results back to the Core.

Platform shells must not:
- infer behavior,
- mutate Core state directly,
- reorder lifecycle events.

---

## 11.4.10 Error Handling Responsibilities

Error handling is split:

- platforms detect and report failures,
- the Core records failures as explicit state,
- fallback behavior is driven by Core logic.

Platforms never silently recover without Core involvement.

---

## 11.4.11 Headless and Test Environments

In headless mode:

- platform shells may be stubbed,
- deterministic fake resources are used,
- lifecycle events are simulated.

The Core lifecycle model remains unchanged.

---

## 11.4.12 Observability and Instrumentation

Lifecycle events are observable.

Instrumentation may:
- log lifecycle transitions,
- attach provenance to events,
- replay lifecycle traces.

Instrumentation must not affect behavior.

---

## 11.4.13 Determinism Guarantees

Embed lifecycle determinism is guaranteed because:

- transitions are explicit,
- ordering is defined,
- platform actions are isolated,
- state is snapshot-driven.

Identical traces produce identical lifecycle behavior.

---

## 11.4.14 Summary

A strict embed lifecycle:

- separates intent from execution,
- makes media and embeds portable,
- enables deterministic testing and replay,
- prevents platform behavior from leaking into semantics.

Platforms execute; the Core decides.

---
