# Effects and Async Execution Spec (v1.3)

This document describes the current Fission async execution model after the move away from
`SystemEffect`.

The core rule has not changed:

- reducers are synchronous and deterministic,
- `build` remains pure and declarative,
- asynchronous and host-driven work runs outside the deterministic core,
- completion is fed back into the reducer pipeline as typed action input.

This document supersedes the public `v1.2` description for current Fission code.

## 1. Core Model

Fission now separates four concerns that were previously mixed together:

1. runtime control,
2. one-shot application jobs,
3. long-lived application services,
4. host capabilities.

The runtime owns execution. Reducers only emit requests.

```text
User input or runtime completion
  -> action dispatch
  -> reducer
  -> state mutation + emitted effects/resources
  -> shell-owned async execution
  -> typed completion
  -> next action dispatch
```

## 2. Public Surfaces

### 2.1 Startup actions

A shell may dispatch a one-shot startup action after the runtime is ready.

Use this for:

- non-blocking startup hydration,
- starting long-lived services,
- kicking off initial jobs,
- setting loading state before the first async turn completes.

Representative builder usage:

```rust
DesktopApp::new(App)
    .with_startup_action(AppStarted)
    .run();
```

This replaces the earlier misuse of synchronous startup hooks for asynchronous work.

### 2.2 Reducer effects

Reducers issue typed work through `ReducerContext::effects`.

Current reducer-side builders are:

- `ctx.effects.app(...)`
- `ctx.effects.start_service(...)`
- `ctx.effects.command(...)`
- `ctx.effects.stop_service(...)`
- `ctx.effects.capability(...)`
- `ctx.effects.cancel(...)`
- `ctx.effects.release_resource(...)`

### 2.3 Build-declared runtime resources

`build` remains pure. It does not start threads or perform I/O.

Instead, widgets may declare runtime resources through `BuildCtx.resources`. The runtime reconciles
those declarations after the build pass.

Current first-party resource kinds are:

- `JobResource`
- `ServiceResource`
- `TimerResource`

This gives Fission lifecycle semantics for mount, dependency change, and teardown without pushing
that work into `build` itself.

## 3. Effect Model

The public effect surface in `fission-core` is now centered on typed jobs, services, capabilities,
and a small runtime-control surface.

Representative shape:

```rust
pub enum Effect {
    Runtime(RuntimeEffect),
    Capability(CapabilityInvocationPayload),
    Job(JobRequestPayload),
    StartService(ServiceStartPayload),
    ServiceCommand(ServiceCommandPayload),
    StopService(ServiceStopPayload),
}

pub enum RuntimeEffect {
    Cancel { req_id: u64 },
    ReleaseResource { resource_id: u64 },
}
```

`SystemEffect` is no longer part of the current model.

## 4. Typed Jobs

Jobs are one-shot asynchronous operations defined by the application.

Use jobs for work such as:

- backend requests owned by the app,
- domain-specific reconciliation,
- parsing or compilation tasks,
- file upload orchestration after a capability grants the input bytes,
- any other request/response unit that belongs to application logic rather than to the host.

Reducer usage:

```rust
ctx.effects
    .app(LOAD_TODOS, LoadTodosRequest { workspace_id })
    .on_ok(ctx.effects.bind(TodosLoaded, on_todos_loaded))
    .on_err(ctx.effects.bind(TodosFailed, on_todos_failed));
```

Shell-side registration uses the async registry.

```rust
app.with_async(|asyncs| {
    asyncs.register_job(LOAD_TODOS, |request, _ctx| async move {
        load_todos_from_backend(request).await
    });
})
```

## 5. Typed Services

Services are long-lived asynchronous components that own lifecycle and may emit repeated events.

Use services for work such as:

- streams,
- subscriptions,
- process/session ownership,
- background sync loops,
- app-specific connections that accept commands over time.

Service start is reducer-driven, but the lifecycle is runtime-owned once started.

```rust
ctx.effects
    .start_service(SHELL_SESSION, ShellSessionConfig { endpoint })
    .on_started(ctx.effects.bind(ShellStarted, on_shell_started))
    .on_start_failed(ctx.effects.bind(ShellStartFailed, on_shell_failed))
    .on_event(ctx.effects.bind(ShellEvent, on_shell_event))
    .on_stopped(ctx.effects.bind(ShellStopped, on_shell_stopped));
```

Commands are sent separately:

```rust
ctx.effects
    .command(SHELL_SESSION, ShellCommand::SendMessage { text })
    .on_ok(ctx.effects.bind(ShellCommandOk, on_command_ok))
    .on_err(ctx.effects.bind(ShellCommandErr, on_command_err));
```

## 6. Typed Capabilities

Host access now uses typed capabilities rather than a built-in `SystemEffect` enum.

Current implemented capability shape is the one-shot **operation capability**.

The core types are:

- `OperationCapability`
- `CapabilityType<C>`
- `CapabilityInvocationPayload::Operation(...)`
- `CapabilityCtx`

Reducer usage:

```rust
ctx.effects
    .capability(PICK_OPEN_FILES, PickOpenFilesRequest {
        allow_multiple: false,
        mime_types: vec!["text/plain".into()],
        extensions: vec!["txt".into()],
    })
    .on_ok(ctx.effects.bind(FileChosen, on_file_chosen))
    .on_err(ctx.effects.bind(FilePickFailed, on_file_pick_failed));
```

Shell-side registration:

```rust
app.with_async(|asyncs| {
    asyncs.register_operation_capability(PICK_OPEN_FILES, |request, _ctx| async move {
        pick_files_from_platform(request).await
    });
})
```

### 6.1 First-party operation capabilities implemented now

Current first-party capabilities in `fission-core` are:

- `OPEN_URL`
- `PICK_OPEN_FILES`

Native alerts, authentication handoffs, payment handoffs, and product-specific device integrations should be modeled as custom operation capabilities registered by the shell that supports them. These are the current operation-capability slice. They do not yet represent the full capability architecture described in ADR 0003.

### 6.2 Current file capability shape

`PICK_OPEN_FILES` currently returns `PickedFile` values with:

- `name`
- `content_type`
- `bytes`

That is the current implemented contract. It is enough for desktop-first file ingress and local
upload flows. It is not yet the final portable handle-based file model described in ADR 0003.

## 7. Action Input And Completions

Completions re-enter reducers through `ActionInput`.

Current async-related input variants include:

- `JobOk` / `JobErr`
- `ServiceStarted` / `ServiceStartFailed`
- `ServiceEvent` / `ServiceStopped`
- `ServiceCommandOk` / `ServiceCommandErr`
- `CapabilityOk` / `CapabilityErr`
- `TimerTick`

This keeps reducers pure while still giving them access to the payloads associated with the most
recent async turn.

## 8. Shell-Owned Execution

The shell owns async execution and provider registration.

Current shell/runtime responsibilities are:

- registering typed jobs,
- registering typed services,
- registering operation capabilities,
- launching work off the deterministic core,
- routing completions back into the runtime,
- reconciling build-declared runtime resources,
- suppressing stale resource completions when a keyed resource has been replaced.

At the public API boundary, applications write async handlers. They do not call `block_on` inside
reducers or `build`.

## 9. Current Implementation Scope

The current implementation covers:

- startup actions,
- typed jobs,
- typed services,
- build-declared job/service/timer resources,
- runtime control (`Cancel`, `ReleaseResource`),
- operation capabilities.

The current implementation does not yet cover the full capability roadmap from ADR 0003.

Not yet implemented:

- resource capabilities,
- session capabilities,
- capability policy and availability state,
- capability constraints,
- compile-time capability feature gating,
- broader first-party capability families such as generic network, camera, microphone,
  geolocation, motion/orientation, or media-session control.

## 10. Guidance

Use the surfaces as follows.

- use `with_startup_action(...)` for one-shot startup dispatch,
- use jobs for app-owned request/response work,
- use services for long-lived app-owned async lifecycles,
- use build-declared resources when the work should exist only while a keyed subtree or screen is
  mounted,
- use capabilities for host/platform access,
- keep reducers synchronous,
- keep `build` pure.

If an async design requires blocking the main thread or performing I/O inside `build`, the design is
wrong.

## 11. Relationship To ADR 0003

ADR 0003 remains the broader capability design document.

This `v1.3` document is narrower. It describes the current code-facing async and capability model as
implemented today, while ADR 0003 also defines the larger target shape for:

- richer capability families,
- capability policy,
- resource and session capabilities,
- and broader device integration across shells.
