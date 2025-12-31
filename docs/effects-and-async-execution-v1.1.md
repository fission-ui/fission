# Effects, Continuations, and Large Payloads Spec (v1.1)

This document refines **Effects and Async Execution v1** to address:
- continuation routing (no “giant match” on results),
- payload injection rules,
- in-flight tracking and snapshot/replay behavior,
- app-defined executor registration,
- registry unification with `bind`,
- deterministic `ReqId` ordering,
- and support for **large payloads / streaming**.

This is intended as the authoritative spec for implementation.

---

## 0. Summary of the Model

Reducers are synchronous and deterministic. Async work is modeled as:

**Action → Reducer → EffectEnvelope(s) → Host executes async → Continuation ActionEnvelope(s) → Reducer**

Key v1.1 improvements:
- Effects carry **bound continuations** (ActionEnvelopes), reusing the `bind` mental model.
- Results are routed by the runtime to these continuations automatically.
- Large payloads are handled via **resource handles / streaming**, not `Vec<u8>`.

---

## 1. Types

### 1.1 ReqId
`ReqId` is a deterministic token used to correlate in-flight effects and results.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReqId(u64);
```

Generation is deterministic (see §6).

### 1.2 System and App Effects

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SystemEffect {
    HttpGet { req_id: ReqId, url: String, headers: Vec<(String,String)> },
    FileRead { req_id: ReqId, path: String },
    Cancel { req_id: ReqId }, // best-effort physical cancellation
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AppEffect {
    // app-defined
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    System(SystemEffect),
    App(AppEffect),
}
```

### 1.3 System and App Effect Results

Results are internal and primarily for:
- diagnostics,
- replay logs,
- tests,
- host boundary.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SystemEffectResult {
    HttpOk { req_id: ReqId, status: u16, body: EffectPayload },
    HttpErr { req_id: ReqId, message: String },
    FileReadOk { req_id: ReqId, bytes: EffectPayload },
    FileReadErr { req_id: ReqId, message: String },
    CancelAck { req_id: ReqId },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AppEffectResult {
    // app-defined
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EffectResult {
    System(SystemEffectResult),
    App(AppEffectResult),
}
```

---

## 2. Continuation Routing: EffectEnvelope

### 2.1 EffectEnvelope
An emitted effect always carries “what to do next”:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectEnvelope {
    pub req_id: ReqId,
    pub effect: Effect,

    // Continuations:
    pub on_ok: ActionEnvelope,
    pub on_err: ActionEnvelope,

    // Policy:
    pub cancel_policy: CancelPolicy,
    pub replay_policy: ReplayPolicy,
}
```

This solves the “callback problem” without closures.

### 2.2 Why ActionEnvelope continuations
This reuses the same concept as UI bindings:
- `ctx.bind(ActionType, handler) -> ActionEnvelope`
- effect completion dispatches an ActionEnvelope

Apps generally do **not** match on `EffectResult` directly.

---

## 3. Payload Injection and Mapping

Your current `ActionEnvelope` has `payload: Vec<u8>` (JSON of the action struct). Effect results often include bytes (HTTP body), which do not match that JSON.

### 3.1 Do NOT overwrite action payload blindly
The runtime must not blindly replace `on_ok.payload` with result bytes.
Doing so forces every action to be `struct X(Vec<u8>)`, which is poor DX and mixes concerns.

### 3.2 Introduce EffectPayload (supports small inline bytes OR large handles)
```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EffectPayload {
    InlineBytes(Vec<u8>),        // small payloads only
    Resource(ResourceId),        // large/streaming payload
    Empty,
}
```

### 3.3 Continuation payload rule (v1.1)
Continuation actions receive **structured payload** in a deterministic way:

- The runtime dispatches the continuation action envelope **with its original action payload** intact.
- The runtime attaches the effect result payload **separately** as an **implicit input** to the action handler through `ReducerContext`.

This avoids double-encoding and supports large payloads.

#### ReducerContext addition
```rust
pub struct ReducerContext<'a> {
    pub effects: &'a mut Effects,
    pub host: &'a mut dyn HostFacade, // read-only APIs like open_resource
    pub input: &'a ActionInput,       // extra input for this dispatch (optional)
}

pub enum ActionInput {
    None,
    EffectOk { req_id: ReqId, payload: EffectPayload, meta: EffectMeta },
    EffectErr { req_id: ReqId, message: String },
}
```

#### Handler signature (v1.1)
```rust
type Handler<S, A> = fn(state: &mut S, action: A, ctx: &mut ReducerContext);
```

Now `ChartLoaded` can be a normal struct with fields, and it can *read the body* from `ctx.input`.

### 3.4 Who deserializes JSON?
- The **app** deserializes the HTTP JSON into domain structs inside the handler (or a helper), because the app owns the schema.
- Provide helpers:

```rust
impl ActionInput {
    pub fn ok_bytes(&self) -> Option<&[u8]>;
    pub fn ok_resource(&self) -> Option<ResourceId>;
}
```

---

## 4. In-Flight Tracking and Snapshot/Restore

### 4.1 Where does ReqId -> EffectEnvelope mapping live?
It lives in the **runtime (transient)**, not app state.

Recommended structure:

```rust
in_flight: BTreeMap<ReqId, InFlightEffect>,

pub struct InFlightEffect {
    pub envelope: EffectEnvelope,
    pub started_at_frame: u64,
}
```

### 4.2 Snapshot/restore semantics (v1)
v1 does not guarantee resuming OS-level async tasks after process restart.

- Snapshots include deterministic runtime state (focus, scroll offsets, animations) + app state.
- Snapshots do not include live OS tasks/download streams.

On restore:
- clear `in_flight`
- emit diagnostic `InFlightDropped`
- app re-emits effects if state still indicates “loading”.

---

## 5. App-Defined Effect Registration

The shell constructs a Host with:
- SystemEffectExecutor
- AppEffectExecutor provided by the app

API:

```rust
DesktopApp::new(root_widget)
  .with_app_effect_executor(MyAppEffects::new())
  .run();
```

Trait:

```rust
pub trait AppEffectExecutor: Send + Sync + 'static {
    fn dispatch(&self, ctx: &HostContext, eff: AppEffect);
}
```

---

## 6. Registry Unification with `bind`

Async continuations use the **same ActionRegistry** as UI actions.

Two entry points:
- `BuildCtx::bind(...) -> ActionEnvelope`
- `Effects::bind(...) -> ActionEnvelope`

Both register into the same runtime/session ActionRegistry.

---

## 7. ReqId Determinism and Ordering

ReqId is derived from:
- frame_no (owned clock / frame loop)
- per-frame sequence counter

```rust
ReqId = (frame_no << 32) | seq_no
```

Rules:
- reducers are invoked deterministically
- `Effects` queue is append-only
- no HashMap iteration order is used in effect emission/dispatch

---

## 8. Cancellation

- Logical cancellation required (ignore stale results).
- Optional physical cancellation via `SystemEffect::Cancel { req_id }`.

Correctness never depends on physical cancel.

---

## 9. Large Payloads and Streaming (100GB files)

### 9.1 ResourceId
Large payloads are returned as resources owned by the host:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(u64);
```

### 9.2 HostFacade access
Reducers may *read* resources via a facade (tests can mock deterministically):

```rust
pub trait HostFacade {
    fn open_resource(&self, id: ResourceId) -> Box<dyn ResourceReader>;
}
```

Reader supports streaming:

```rust
pub trait ResourceReader {
    fn len(&self) -> Option<u64>;
    fn read_chunk(&mut self, max: usize) -> Result<Vec<u8>, String>;
}
```

### 9.3 Streaming download effect
For huge files, prefer an explicit download-to-resource effect:

```rust
SystemEffect::HttpDownloadToFile { req_id: ReqId, url: String }
```

Results include progress and completion:

```rust
SystemEffectResult::HttpProgress { req_id: ReqId, received: u64, total: Option<u64> }
SystemEffectResult::HttpDownloadComplete { req_id: ReqId, resource: ResourceId }
```

Progress can be routed via:
- a dedicated `on_progress` continuation (optional v1.1), or
- multiple `on_ok` dispatches with `ActionInput::EffectOk` meta describing progress vs complete.

---

## 10. Overlooked Items Checklist

- Backpressure and concurrency limits (deterministic scheduling).
- Retries as explicit state/effects (not implicit).
- Timeouts via owned clock + TimerEffect (not OS time).
- Diagnostics events: EffectStarted/Completed/Failed/Canceled.
- Security/capabilities: effects should be permission-gated per shell.

---

## 11. Minimal End-to-End Walkthrough

1) Reducer emits effect + continuations.
2) Runtime stores envelope in `in_flight` and dispatches effect to Host.
3) Host completes and pushes EffectResult with payload (bytes or resource).
4) Runtime routes to `on_ok`/`on_err` and dispatches that ActionEnvelope, attaching `ActionInput` containing the payload.
5) Handler consumes payload from `ctx.input`, updates state deterministically.

---

## 12. Summary

v1.1 provides:
- continuation routing via ActionEnvelope (reuses `bind`),
- correct payload handling (no blind overwrite),
- transient in-flight tracking with clear snapshot semantics,
- deterministic ReqId generation,
- and streaming/large-payload support via ResourceId and chunk readers.

This is the recommended implementation direction.