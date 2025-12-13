# 2.3 Platform Shells vs Core Runtime

This section clarifies the strict boundary between Platform Shells and the Core Runtime.
This boundary is essential to maintaining determinism, cross-platform consistency, and testability.

The Core Runtime defines *what the UI means*.
Platform Shells define *how the UI is connected to an operating system*.

---

## 2.3.1 Core Runtime: Definition of UI Semantics

### Responsibility

The Core Runtime is the authoritative source of UI behavior.

It is responsible for:
- interpreting authoring intent,
- defining layout and geometry,
- defining semantics and accessibility structure,
- routing input and emitting actions,
- producing display lists and traces.

### Characteristics

- Platform-agnostic
- Deterministic
- Closed-world Core IR
- Fully inspectable and serializable

### Constraints

- Must not call platform APIs
- Must not depend on OS timing or event loops
- Must not allocate platform resources
- Must not render pixels directly

All observable UI behavior originates here.

---

## 2.3.2 Platform Shells: OS Integration Layer

### Responsibility

Platform Shells adapt the Core Runtime to a specific operating system.

They are responsible for:
- creating windows or surfaces,
- collecting raw input events,
- forwarding lifecycle events,
- exposing accessibility information to the OS,
- presenting rendered pixels to the screen.

### Characteristics

- Platform-specific
- Thin and replaceable
- Minimal logic

### Constraints

- Must not implement UI logic
- Must not alter Core decisions
- Must not infer or modify semantics
- Must forward input faithfully

Platform Shells are adapters, not interpreters.

---

## 2.3.3 Direction of Control

Control always flows **into** the Core Runtime.

- Platform Shells push input events to the Core.
- The Core Runtime processes events deterministically.
- The Core Runtime emits actions and updated outputs.
- Platform Shells reflect those outputs to the OS.

Platform Shells never pull state from the Core opportunistically.

---

## 2.3.4 Accessibility Responsibilities

Accessibility is defined by the Core Runtime.

- Semantics trees originate in the Core.
- Platform Shells translate semantics into OS-specific accessibility APIs.

Platform Shells must not:
- invent roles,
- infer labels,
- reorder accessibility nodes.

This ensures accessibility behavior is:
- consistent across platforms,
- testable in headless mode,
- deterministic.

---

## 2.3.5 Input Handling

### Input Flow

1. Platform Shell receives raw input (pointer, keyboard, gesture).
2. Input is normalized into framework input events.
3. Events are forwarded to the Core Runtime.
4. Core Runtime performs hit-testing and routing.
5. Actions are emitted based on Core semantics.

### Guarantees

- Input ordering is preserved.
- Hit-test paths are deterministic.
- All input handling is traceable.

---

## 2.3.6 Lifecycle Management

Platform Shells are responsible for:
- window creation and destruction,
- surface resizing,
- suspend/resume notifications.

The Core Runtime:
- reacts to lifecycle events explicitly,
- does not own lifecycle state implicitly.

Lifecycle events are treated as input signals.

---

## 2.3.7 Headless Mode Implications

In headless mode:
- Platform Shells are replaced by a test harness.
- Input events are synthetic.
- Accessibility translation is skipped or mocked.

The Core Runtime remains unchanged.

This validates that Platform Shells are non-essential for Core correctness.

---

## 2.3.8 Error Handling and Fault Isolation

Errors are isolated by layer:

- Core Runtime errors indicate logic or semantic bugs.
- Platform Shell errors indicate OS integration issues.
- Rendering backend errors indicate pixel-level problems.

This isolation simplifies diagnosis and recovery.

---

## 2.3.9 Why This Boundary Is Strict

Relaxing this boundary would:
- introduce nondeterminism,
- complicate testing,
- couple UI behavior to OS quirks,
- make reasoning about behavior difficult.

Therefore, violations of this boundary are considered architectural errors.

---

## 2.3.10 Summary

The Core Runtime and Platform Shells serve fundamentally different roles.

By keeping their responsibilities strictly separated, the framework ensures:
- deterministic behavior,
- consistent semantics across platforms,
- robust testing and tooling,
- and long-term architectural stability.

---
