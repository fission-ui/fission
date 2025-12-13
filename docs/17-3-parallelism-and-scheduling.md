# 17.3 Parallelism and Scheduling

This section describes how the framework exploits **parallelism** while preserving strict determinism.
Parallel execution is treated as an *implementation strategy*, never as an observable semantic feature.

If parallelism changes behavior, it is a bug.

---

## 17.3.1 Design Principles

Parallelism and scheduling must:

- preserve deterministic results,
- be independent of thread count and CPU topology,
- avoid data races by construction,
- remain debuggable and inspectable,
- degrade gracefully to single-threaded execution.

The Core Runtime defines *what* happens; the scheduler decides *where* it runs.

---

## 17.3.2 Determinism vs Parallelism

Determinism is defined over **observable outputs**:

- snapshots,
- diffs,
- action traces,
- display lists.

Rules:
- execution order may vary internally,
- outputs must be byte-for-byte identical,
- scheduling decisions are not observable.

Parallelism is invisible to users and tests.

---

## 17.3.3 Parallelizable Stages

The following stages are safe to parallelize:

- independent subtree layout evaluation,
- intrinsic size computation (e.g. text),
- display list command generation,
- diff computation across disjoint regions,
- renderer command submission preparation.

Stages with global ordering constraints remain sequential.

---

## 17.3.4 Task Graph Model

The runtime constructs an explicit task graph per evaluation cycle.

Properties:
- nodes represent pure computations,
- edges represent data dependencies,
- the graph is acyclic and deterministic,
- task identities are stable across runs.

The task graph is inspectable and versioned.

---

## 17.3.5 Scheduling Strategy

Scheduling strategy is pluggable.

Options include:
- single-threaded (debug / CI),
- work-stealing thread pool,
- platform-provided executors.

The same task graph is executed regardless of scheduler.

---

## 17.3.6 Work Partitioning

Work is partitioned deterministically.

Rules:
- partition boundaries are derived from snapshot structure,
- subtree partitioning respects stable node identity,
- partition sizes are bounded.

No heuristic partitioning based on runtime timing exists.

---

## 17.3.7 Synchronization and Barriers

Synchronization points are explicit.

Examples:
- layout completion before paint compilation,
- reducer completion before snapshotting,
- diff completion before invalidation propagation.

Implicit barriers are forbidden.

---

## 17.3.8 Avoiding Shared Mutable State

Shared mutable state is avoided entirely.

Rules:
- tasks operate on immutable inputs,
- outputs are written to isolated buffers,
- merging follows deterministic rules.

Rust’s ownership model enforces this discipline.

---

## 17.3.9 Interaction With Arenas

Parallel tasks allocate from thread-local arenas.

Rules:
- arenas are not shared across threads,
- arena merges are deterministic,
- lifetime boundaries are explicit.

This avoids contention and fragmentation.

---

## 17.3.10 Debugging Parallel Execution

Parallel execution is debuggable.

Tools can:
- force single-threaded execution,
- visualize task graphs,
- record task execution order,
- replay with different schedulers.

Parallelism never obscures behavior.

---

## 17.3.11 Testing and CI Behavior

In CI and tests:

- parallelism may be disabled by default,
- results must match production exactly,
- race conditions are surfaced early.

Tests validate both parallel and serial paths.

---

## 17.3.12 Platform Considerations

Platform shells do not influence scheduling.

Rules:
- Core controls scheduling entirely,
- platform thread models are abstracted,
- background threads never mutate Core state.

This preserves portability.

---

## 17.3.13 Performance Characteristics

Parallelism provides:

- near-linear speedups for large trees,
- bounded overhead for small UIs,
- predictable scaling behavior.

Worst-case behavior remains well-defined.

---

## 17.3.14 Failure Modes and Safeguards

Safeguards include:

- fallback to serial execution on error,
- consistency checks on merged results,
- panic isolation per task.

Correctness is never sacrificed for throughput.

---

## 17.3.15 Summary

Parallelism works in this framework because:

- all computations are pure and explicit,
- dependencies are modeled as data,
- scheduling is decoupled from semantics,
- determinism is enforced at the boundaries.

The runtime scales across cores without becoming nondeterministic.
