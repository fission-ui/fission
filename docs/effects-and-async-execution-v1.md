# Effects and Async Execution Spec (v1)

This document defines how **asynchronous work** is supported in Fission while preserving
determinism, testability, and a synchronous reducer model.

The core principle is:

> **Reducers are always synchronous and deterministic.  
> Asynchronous work is modeled as Effects → Host execution → EffectResults.**

---

## 1. Design Goals

- Support async workflows (HTTP, file IO, DB, GPU, custom app logic).
- Preserve determinism: same inputs + same effect results → same state.
- Keep reducers synchronous and replayable.
- Allow **system-provided effects** and **user-defined app effects**.
- Make testing easy via effect capture and deterministic result injection.
- Avoid closures, futures, or async code inside reducers.
- **Ergonomic Routing**: Avoid manual matching of request IDs; use "bound continuations" (like `on_press`).

---

## 2. High-Level Model

```
User Input
   ↓
Action (sync)
   ↓
Reducer
   ├─ updates State
   └─ emits Effect(s) (with attached on_ok/on_err envelopes)
        ↓
      Host (async, nondeterministic)
        ↓
   SystemEffectResult (internal)
        ↓
   Runtime Maps Result → ActionEnvelope (using on_ok/on_err)
        ↓
Reducer (continuation)
   ↓
State → UI
```

---

## 3. Effect Types

### 3.1 System Effects (provided by Fission)

Defined in `fission-core`.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SystemEffect {
    HttpGet { url: String }, // req_id is managed by Envelope
    FileRead { path: String },
    // Logical cancellation is handled by state tokens.
    // Best-effort physical cancellation:
    Cancel { req_id: u64 }, 
}
```

### 3.2 Effect Envelope (The Glue)

Instead of just `Effect`, the reducer emits an `EffectEnvelope`.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectEnvelope {
    pub req_id: u64,
    pub effect: Effect,
    pub on_ok: Option<ActionEnvelope>,   // dispatched on success
    pub on_err: Option<ActionEnvelope>,  // dispatched on error
}
```

---

## 4. Reducer API

Reducers interact with an `Effects` helper struct.

```rust
pub struct Effects<S> {
    next_req_id: u64,
    out: Vec<EffectEnvelope>,
    // phantom data for S to allow bind helper
}

impl<S> Effects<S> {
    // Factory methods return a builder
    pub fn http_get(&mut self, url: impl Into<String>) -> EffectBuilder<'_, S> { ... }
    
    // Bind helper (same as BuildCtx)
    pub fn bind<A: Action>(&mut self, action: A, handler: fn(&mut S, A, &mut Effects<S>)) -> ActionEnvelope;
}
```

**Usage Example:**

```rust
fn on_load_chart(state: &mut AppState, _a: LoadChart, fx: &mut Effects<AppState>) {
    fx.http_get("https://api.example.com/chart.json")
      .on_ok(fx.bind(ChartLoaded(vec![]), on_chart_loaded)) // Placeholder payload
      .on_err(fx.bind(ChartLoadFailed("".into()), on_chart_failed))
      .dispatch();
}

// Handler
fn on_chart_loaded(state: &mut AppState, action: ChartLoaded, fx: &mut Effects<AppState>) {
    // action.0 contains the response body
    state.data = parse(action.0);
}
```

---

## 5. Runtime Execution Flow

At the end of each frame:

```rust
fn run_frame(&mut self) {
    self.process_actions(); // Phase 1: UI Actions
    
    let effects = self.effects.take();
    for env in effects {
        self.host.dispatch_effect(env);
    }
    
    // Phase 2: Process any async results that arrived from Host
    // (This might be beginning of NEXT frame depending on loop structure)
    while let Some(result) = self.host.poll_results() {
        self.dispatch_result(result);
    }
    
    self.render();
}
```

### Result Dispatch Logic

When `Runtime` receives `SystemEffectResult` (e.g., `HttpGetOk { req_id, body }`):
1.  Look up the active `EffectEnvelope` for `req_id` (or the host passes it back?).
    *   *Correction*: The Host should return `(req_id, result)`. The Runtime needs to store the map of `req_id -> (on_ok, on_err)`?
    *   *Alternative*: The `EffectEnvelope` is sent to Host. The Host attaches the *relevant* envelope (`on_ok` or `on_err`) to the result it sends back. This keeps Runtime stateless regarding in-flight requests (better for replay).
2.  If `Ok`, take `on_ok` envelope.
3.  Inject `body` into the envelope's payload.
4.  Dispatch the envelope to the Reducer.

---

## 6. Determinism & Replay

### ReqId Generation
`Effects.next_req_id` is seeded deterministically (e.g., `frame_count << 32 | seq_index`).
This ensures that if we replay the same user actions, the same ReqIds are generated.

### Replay Model
**Model A: Result-Driven Replay**
We record the stream of `(Action)` inputs and `(EffectResult)` outputs from the Host.
During replay:
*   We execute Actions.
*   We **skip** sending Effects to the real Host.
*   We inject `EffectResult`s from the log at the correct frame.

---

## 7. Cancellation

**Logical Cancellation:**
The app state stores a "token" or "current_req_id". When `ChartLoaded` fires, check:
`if action.req_id != state.current_req_id { return; }`
(Requires passing `req_id` in the action payload too, or capturing it in closure? No closures. So explicit token in State).

**Physical Cancellation (Best Effort):**
`fx.cancel(req_id)`.
Runtime sends `Cancel` effect to Host. Host drops the task. This is optimization, not correctness.

---

## 8. Summary

This design unifies Actions and Effects under a single "Message Passing" mental model, enabling async IO in a strictly deterministic UI framework.