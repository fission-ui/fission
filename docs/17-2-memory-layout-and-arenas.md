# 17.2 Memory Layout and Arenas

This section describes the **memory layout strategy** of the framework, with a focus on
**arena allocation**, cache locality, and predictable memory behavior.
Memory management is explicit, observable, and designed to support determinism and performance at scale.

Memory is a performance feature, not an implementation detail.

---

## 17.2.1 Design Goals

The memory system must:

- minimize allocation overhead on hot paths,
- provide stable addresses where needed,
- support cheap snapshotting and diffing,
- avoid hidden global state,
- make memory usage observable and predictable.

No implicit garbage collection is allowed.

---

## 17.2.2 Arena-Based Allocation Model

Most runtime data is allocated from arenas.

Key properties:
- arenas are phase-scoped (e.g. frame, layout pass),
- allocation is bump-only,
- deallocation happens in bulk,
- allocation order is deterministic.

This eliminates per-node allocation churn.

---

## 17.2.3 Arena Types

The runtime uses multiple arena classes:

- **Frame Arena**: transient data for a single evaluation cycle,
- **Snapshot Arena**: immutable snapshot data,
- **Display List Arena**: paint commands and parameters,
- **Diff Arena**: temporary diff computation storage.

Each arena has a clear lifetime and purpose.

---

## 17.2.4 Stable Identity vs Stable Address

Stable identity does **not** require stable addresses.

Rules:
- node identity is logical, not pointer-based,
- snapshots store indices or IDs, not raw pointers,
- arenas may be compacted or dropped freely.

This enables aggressive memory reuse.

---

## 17.2.5 Data-Oriented Layout

Data is stored in cache-friendly layouts.

Examples:
- SoA (structure-of-arrays) for node properties,
- packed command buffers for display lists,
- contiguous arrays for child indices.

Traversal cost dominates; layouts optimize for it.

---

## 17.2.6 Snapshot Memory Sharing

Snapshots use structural sharing.

Techniques:
- reference-counted arena segments,
- copy-on-write nodes,
- deduplicated strings and IDs.

Multiple snapshots can coexist cheaply.

---

## 17.2.7 String and Identifier Storage

Strings are handled carefully.

Rules:
- interned where identity matters,
- arena-allocated where ephemeral,
- no heap allocation on hot paths.

Action tags, keys, and roles are compact identifiers.

---

## 17.2.8 Diff Memory Behavior

Diff computation uses bounded, temporary memory.

Rules:
- diff arenas are short-lived,
- memory is reclaimed immediately after use,
- worst-case memory is proportional to snapshot size.

Diffing does not leak memory over time.

---

## 17.2.9 Avoiding Fragmentation

Fragmentation is avoided by design:

- no long-lived small heap allocations,
- no interleaving of unrelated lifetimes,
- explicit arena reset points.

Memory usage remains stable over long runs.

---

## 17.2.10 Observability and Diagnostics

Memory usage is observable.

Tooling can report:
- per-arena allocation sizes,
- peak memory per frame,
- snapshot retention costs,
- display list memory usage.

This enables data-driven optimization.

---

## 17.2.11 Safety and Correctness

Safety guarantees include:

- no use-after-free (arenas enforce lifetime),
- no aliasing across incompatible lifetimes,
- explicit ownership of long-lived data.

Rust’s type system enforces these invariants.

---

## 17.2.12 Interaction With Testing and Instrumentation

Instrumentation may allocate additional memory.

Rules:
- instrumentation uses separate arenas,
- production builds can compile it out entirely,
- test builds accept higher memory overhead.

Memory costs are explicit and controllable.

---

## 17.2.13 Scaling to Large Applications

At scale, this model provides:

- predictable memory growth,
- bounded per-frame allocations,
- fast teardown and rebuild,
- suitability for low-memory devices.

Large UIs remain stable and performant.

---

## 17.2.14 Summary

The arena-based memory layout works because:

- lifetimes are explicit,
- allocation is cheap and deterministic,
- snapshots and diffs are memory-efficient,
- memory behavior is observable and debuggable.

This enables both high performance and strong correctness guarantees.
