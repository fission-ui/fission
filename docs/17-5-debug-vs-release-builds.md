# 17.5 Debug vs Release Builds

This section defines the **build profiles** used by the framework and how they differ in
instrumentation, safety checks, performance characteristics, and intended use.
Debug and release builds are not different systems—they are different *lenses* on the same system.

Correctness must not depend on build mode.

---

## 17.5.1 Guiding Principles

Build profiles must:

- preserve identical semantics,
- produce identical observable behavior,
- differ only in diagnostics, checks, and instrumentation,
- make performance costs explicit and intentional.

If a bug disappears in release mode, it is a bug in the framework.

---

## 17.5.2 Debug Builds

Debug builds prioritize visibility and safety.

Characteristics:
- full instrumentation enabled by default,
- snapshots and diffs available,
- extensive assertions and invariants checked,
- single-threaded execution optional,
- verbose error messages and diagnostics.

Debug builds are optimized for development and CI.

---

## 17.5.3 Release Builds

Release builds prioritize performance and stability.

Characteristics:
- instrumentation disabled by default,
- assertions compiled out (except critical invariants),
- parallel scheduling enabled,
- minimal memory overhead,
- predictable performance profiles.

Release builds are suitable for production deployment.

---

## 17.5.4 Feature Flag Matrix

Build behavior is controlled via feature flags.

Examples:

- `instrumentation`
- `snapshots`
- `debug-metadata`
- `action-tracing`
- `parallel`

Rules:
- debug builds enable most flags by default,
- release builds enable only required flags,
- flags are orthogonal and composable.

There is no implicit behavior change.

---

## 17.5.5 Assertions and Invariants

Assertions differ by build mode.

Debug builds include:
- structural invariants,
- identity collision checks,
- layout consistency assertions,
- diff consistency checks.

Release builds retain:
- critical safety assertions,
- bounds checks required for correctness.

Assertions never affect outputs.

---

## 17.5.6 Performance Differences

Performance differences are intentional and bounded.

Examples:
- additional allocations for snapshots in debug,
- extra passes for diff validation,
- reduced batching in debug for clarity.

Release builds remove these costs entirely.

---

## 17.5.7 Debugging Production Issues

Production issues can be debugged safely.

Strategies:
- reproduce with debug builds using recorded traces,
- enable scoped instrumentation in release builds,
- replay deterministic inputs locally.

There is no need to debug live state imperatively.

---

## 17.5.8 Binary Size Considerations

Debug builds are larger due to:

- retained debug symbols,
- instrumentation code,
- diagnostic helpers.

Release builds:
- strip unused features,
- eliminate dead code,
- minimize binary size.

Binary size trade-offs are explicit.

---

## 17.5.9 CI and Build Matrix

Typical CI configuration:

- debug + full instrumentation (correctness),
- release + minimal instrumentation (performance),
- optional release + instrumentation (parity testing).

CI validates both correctness and production behavior.

---

## 17.5.10 Avoiding Heisenbugs

The architecture avoids Heisenbugs because:

- instrumentation is passive,
- scheduling is deterministic,
- semantics do not depend on timing.

Build mode does not change behavior.

---

## 17.5.11 Testing Guarantees Across Builds

Guarantees:
- all tests must pass in both debug and release,
- snapshot tests are debug-only but validate release behavior,
- pixel tests validate renderer parity.

Differences are transparent and controlled.

---

## 17.5.12 Summary

Debug vs release builds differ only in *observability*, not *meaning*.

Because:
- Core semantics are immutable,
- instrumentation is optional and gated,
- performance optimizations preserve outputs,
- failures are reproducible across builds.

The same UI is correct everywhere—it is just easier to see why in debug mode.
