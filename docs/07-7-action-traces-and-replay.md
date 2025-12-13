# 7.7 Action Traces and Replay

This section defines how actions are recorded, traced, and replayed deterministically.
Action traces are a foundational capability for testing, debugging, CI reproducibility, and LLM-assisted analysis.

If an interaction cannot be replayed, it is not considered fully specified.

---

## 7.7.1 What Is an Action Trace

An **Action Trace** is an ordered log of dispatched actions.

Each trace entry contains:
- action identity (tag),
- payload (if any),
- target NodeId,
- pre-dispatch state version,
- post-dispatch state version,
- timestamp (logical, not wall-clock).

Action traces are pure data.

---

## 7.7.2 Deterministic Replay Guarantee

Given:
- an initial application state,
- a canonical Core IR version,
- a deterministic reducer set,
- an action trace,

the framework guarantees:
- identical state transitions,
- identical Core IR rebuilds,
- identical semantics and layout outputs.

Replay determinism is a hard requirement.

---

## 7.7.3 Trace Capture Points

Traces may be captured at multiple layers:

- semantic action invocation,
- reducer dispatch entry,
- reducer dispatch exit.

The canonical trace point is **post-validation, pre-reducer**.

This ensures traces represent valid intent.

---

## 7.7.4 Trace Serialization Format

Action traces are serialized in a stable, versioned format.

Properties:
- platform-independent,
- architecture-independent,
- forward-compatible where possible.

Serialization includes:
- action tags,
- payloads,
- NodeIds,
- Core IR and action version metadata.

---

## 7.7.5 Replay Modes

The runtime supports multiple replay modes:

1. **Strict Replay**  
   - all identities must match exactly,
   - any mismatch is an error.

2. **Compatible Replay**  
   - aliases are resolved,
   - compatible Core IR versions allowed.

Replay mode selection is explicit.

---

## 7.7.6 Action Traces in Testing

Action traces enable powerful tests:

- record-once, replay-many tests,
- golden interaction tests,
- regression reproduction.

Example workflow:
1. Record trace during manual interaction.
2. Commit trace as test fixture.
3. Replay trace in CI headlessly.

---

## 7.7.7 Time and Action Traces

Action traces do not depend on wall-clock time.

Rules:
- time-based behavior uses the owned clock,
- time advances are explicit actions,
- traces may include time-advance actions.

This avoids flakiness due to timing.

---

## 7.7.8 Debugging and Time Travel

Action traces enable:

- step-by-step replay,
- time-travel debugging,
- binary search over interaction history.

Because reducers are pure, stepping is safe and deterministic.

---

## 7.7.9 LLM and Tooling Integration

Structured action traces allow:

- automated explanation of bugs,
- LLM-driven reproduction and minimization,
- interaction summarization,
- test generation from real usage.

Traces are a machine-readable interaction contract.

---

## 7.7.10 Error Handling During Replay

Replay errors include:

- unknown action identities,
- incompatible payload schemas,
- missing target nodes,
- reducer mismatches.

Errors are:
- deterministic,
- reported with trace context,
- never silently ignored.

---

## 7.7.11 Storage and Retention

Action traces may be:
- stored short-term for debugging,
- persisted as test artifacts,
- streamed for live inspection.

Retention policy is application-defined.

---

## 7.7.12 Summary

Action traces and replay:

- make interaction behavior reproducible,
- eliminate flaky interaction tests,
- enable powerful debugging and tooling,
- support long-term compatibility guarantees.

They complete the framework’s commitment to determinism and observability.

---
