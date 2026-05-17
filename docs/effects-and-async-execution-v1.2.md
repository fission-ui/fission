# Effects and Async Execution Spec (v1.2)

This document finalizes the design for **asynchronous work** in Fission, incorporating decisions on:
- Registry lifetime (Global Type-Based, No GC).
- Resource management (Explicit Release).
- Handler unification (Backward-compatible Adapter).
- Payload routing (Context-based injection).

The core principle remains: **Reducers are always synchronous and deterministic.**

---

## 1. Core Model: The Loop

```
User Input OR Effect Completion
        ↓
ActionEnvelope (Sync, Serializable)
        ↓
Runtime Dispatcher
   ┌────────────────────────────────────────────────────────┐
   │ 1. Deserialize Action Payload                          │
   │ 2. Construct ReducerContext (with Effects & Input)     │
   │ 3. Invoke Registered Handler (fn pointer)              │
   └────────────────────────────────────────────────────────┘
        ↓
Reducer (Sync)
   ├─ Updates State
   └─ Emits EffectEnvelope(s) (via ReducerContext)
        ↓
      Runtime Queue (Transient)
        ↓
      Host Execution (Async)
        ↓
   SystemEffectResult
        ↓
Runtime Router
   1. Match ReqId → In-Flight EffectEnvelope
   2. Select Continuation (on_ok / on_err ActionEnvelope)
   3. Attach Payload to next dispatch's ReducerContext
   4. Dispatch Continuation
```

---

## 2. Types & Structures

### 2.1 Effect Definitions

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SystemEffect {
    HttpGet { url: String, headers: Vec<(String,String)> },
    FileRead { path: String },
    Cancel { req_id: u64 },
    ReleaseResource { resource_id: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    System(SystemEffect),
    App(Vec<u8>), // Opaque app-specific payload
}
```

### 2.2 Effect Envelope (The Glue)

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectEnvelope {
    pub req_id: u64,
    pub effect: Effect,
    // Continuations are just ActionEnvelopes
    pub on_ok: Option<ActionEnvelope>,
    pub on_err: Option<ActionEnvelope>,
}
```

### 2.3 Handler Context

```rust
pub struct ReducerContext<'a, S> {
    pub effects: &'a mut Effects<S>,
    pub input: &'a ActionInput, // Payload from effect result
}

pub enum ActionInput {
    None,
    EffectOk { req_id: u64, payload: EffectPayload },
    EffectErr { req_id: u64, message: String },
}

pub enum EffectPayload {
    InlineBytes(Vec<u8>),
    Resource(u64), // Handle to large data
    Empty,
}
```

---

## 3. Handler Signature & Migration

### 3.1 Canonical Signature
The goal is to move all handlers to this signature:

```rust
type Handler<S, A> = fn(&mut S, A, &mut ReducerContext<S>);
```

### 3.2 Backward Compatibility (Adapter)
To avoid breaking existing code (`fn(&mut S, A)`), the `bind` method uses a trait:

```rust
pub trait IntoHandler<S, A> {
    fn call(&self, state: &mut S, action: A, ctx: &mut ReducerContext<S>);
}

// Impl for Legacy (2-arg)
impl<F, S, A> IntoHandler<S, A> for F 
where F: Fn(&mut S, A) {
    fn call(&self, state: &mut S, action: A, _ctx: &mut ReducerContext<S>) {
        (self)(state, action);
    }
}

// Impl for Modern (3-arg)
impl<F, S, A> IntoHandler<S, A> for F 
where F: Fn(&mut S, A, &mut ReducerContext<S>) {
    fn call(&self, state: &mut S, action: A, ctx: &mut ReducerContext<S>) {
        (self)(state, action, ctx);
    }
}
```

---

## 4. Registry Semantics

- **One Handler per Action Type:** `ActionId` is derived from the struct type name (e.g., `fission::core::Navigate`).
- **Global & Static:** Handlers are function pointers registered once per session. No "per-button" closures.
- **Routing:** Different buttons using the same Action Type must distinguish behavior via the **Action Fields** (payload), not by binding different closures.

---

## 5. Resource Management (Streaming)

- **Ownership:** The Host owns resources (files, sockets).
- **Leasing:** The Runtime receives a `ResourceId`.
- **Cleanup:** The App **MUST** explicitly emit `ReleaseResource(id)` when done.
- **Safety:** The Host *may* auto-close resources if the runtime crashes or disconnects, but correctness relies on explicit release.

---

## 6. Implementation Checklist

1.  **Core Types:** `Effect`, `SystemEffect`, `EffectEnvelope`, `EffectPayload`, `ResourceId`.
2.  **Context:** `ReducerContext`, `ActionInput`.
3.  **Effects Builder:** `Effects` struct with methods `http_get(...)`, `bind(...)`.
4.  **Adapter:** `IntoHandler` trait for `bind`.
5.  **Runtime Loop:**
    - `run_frame` processes actions.
    - Collects `effects` from context.
    - Dispatches to `Host`.
    - Polls `Host` results.
    - Routes results to `ActionInput` + `ActionEnvelope` dispatch.
6.  **Mock Host:** For testing determinism.

---

## 7. Example: Fetching Data

```rust
// 1. Define Actions
#[fission_action]
struct LoadChart;

#[fission_action]
struct ChartLoaded; // Empty struct, payload comes from Context!

// 2. Reducer (Trigger)
fn on_load_chart(state: &mut AppState, _: LoadChart, ctx: &mut ReducerContext<AppState>) {
    ctx.effects.http_get("https://api.com/data")
        .on_ok(ctx.effects.bind(ChartLoaded, on_chart_loaded))
        .dispatch();
}

// 3. Reducer (Result)
fn on_chart_loaded(state: &mut AppState, _: ChartLoaded, ctx: &mut ReducerContext<AppState>) {
    if let Some(bytes) = ctx.input.as_bytes() {
        state.data = parse(bytes);
    }
}
```
