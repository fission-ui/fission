# ADR 0003: Capability-Based Host Access and Device Integration

- Status: Proposed
- Date: 2026-05-10
- Related docs:
  - `docs/effects-and-async-execution-v1.3.md`
  - `docs/16-platform-integration.md`
  - `docs/11-4-embed-lifecycle-and-platform-responsibilities.md`
  - `docs/16-5-input-and-lifecycle-handling.md`

## Context

Fission now has two overlapping models for asynchronous and host-driven work.

The first model is the older effect surface:

- `SystemEffect::HttpGet`
- `SystemEffect::FileRead`
- `SystemEffect::Alert`
- `SystemEffect::OpenUrl`
- `SystemEffect::Authenticate`
- `SystemEffect::Cancel`
- `SystemEffect::ReleaseResource`

The second model is the newer async runtime surface:

- typed jobs,
- typed services,
- build-declared resources,
- startup actions,
- and shell-owned async execution.

The newer model is structurally better. It separates one-shot work from long-lived work, it keeps reducers pure, and it gives Fission a coherent way to run asynchronous logic without pushing apps back toward startup blocking, frame polling, or ad hoc threads.

The older `SystemEffect` model is now the outlier.

It has three problems.

First, it is too narrow.

`HttpGet` and `FileRead` are not a general model for host access. They solve one or two concrete cases but do not scale to cameras, microphones, geolocation, sensors, file pickers, share sheets, save dialogs, media playback control, downloads, or future device classes.

Second, it bakes in the wrong abstractions.

A path-based `FileRead` is acceptable on a permissive desktop shell and wrong almost everywhere else. Web runtimes do not expose arbitrary local paths. Restricted runtimes may allow a user-granted blob and forbid all path access. Sandboxed environments may allow save/export but not browse/open. The API shape should reflect those realities instead of pretending every shell has a shared filesystem model.

Third, it does not define a proper capability system.

There is no single place where Fission answers these questions:

- what host features are compiled into a shell,
- which of those features are enabled for a given app at runtime,
- how a UI can discover availability before it renders affordances,
- how a shell prompts for access,
- how third parties add new capabilities without changing `fission-core`,
- and how stateful host interactions such as media sessions, camera sessions, or sensor watches fit the same model as one-shot operations.

That gap matters to Worka, but it is not a Worka-specific problem.

Any serious application framework that targets desktop, mobile, web, embedded browsers, restricted runtimes, and future device classes needs a uniform model for host access. Files, network, media devices, location, motion sensors, nearby-device integrations, external authentication, OS intents, and platform dialogs are all instances of the same architectural problem.

Fission therefore needs to stop treating built-in host access as a hard-coded list of special effects and instead treat it as a capability system with typed requests, typed results, explicit policy, and platform-registered providers.

## Current evidence in the repository

The repository already contains the pieces that make this change practical.

- `crates/core/fission-core/src/async_runtime.rs` defines typed jobs, typed services, service slots, service bindings, and shell-owned async execution.
- `crates/core/fission-core/src/context.rs` already exposes `ctx.effects.app(...)`, `ctx.effects.start_service(...)`, `ctx.effects.command(...)`, and `ctx.effects.stop_service(...)`.
- `crates/core/fission-core/src/effect.rs` still carries `SystemEffect` alongside the newer effect variants, which creates duplication at the API boundary.
- `docs/effects-and-async-execution-v1.1.md` already notes that security and capabilities should be permission-gated per shell, but the current public API does not implement that idea.
- `docs/11-4-embed-lifecycle-and-platform-responsibilities.md` and `docs/16-platform-integration.md` already treat shells as the place where platform-specific responsibilities belong.
- Work underway in the shells already assumes a shell-owned executor and typed runtime bindings, which is the correct substrate for capability providers.

The architecture therefore does not need another parallel runtime. It needs a clearer model for host access on top of the runtime it already has.

## Decision

Fission will replace `SystemEffect` with a first-class capability system.

This is a breaking change.

The capability system becomes the only built-in way for reducers and mounted resources to request host access through the framework. Host access will no longer be modeled as a closed `SystemEffect` enum maintained directly inside `fission-core`.

The rest of this ADR defines that system.

### 1. Fission distinguishes application async work from host capabilities

Fission will keep the async runtime concepts that already exist, but it will draw a sharper line between two categories of work.

#### 1.1 Application async work

This is business logic defined by the app or library author.

Examples:

- fetching domain data from the app's backend,
- reconciling a local cache,
- running a parser or compiler,
- talking to a database owned by the app,
- orchestrating a long-lived app-specific connection,
- or managing a subprocess the app conceptually owns.

This continues to use the existing typed async runtime:

- jobs,
- services,
- commands,
- and build-declared resources.

#### 1.2 Host capabilities

This is access to platform or shell functionality that exists outside the app's own domain model.

Examples:

- network access,
- file open/save/import/export,
- camera and microphone,
- geolocation,
- motion sensors,
- external browser and share sheets,
- permission prompts,
- nearby-device APIs,
- platform media playback,
- or future device-specific features.

This will use the new capability system.

The point of the distinction is not to create two runtimes. The point is to make policy, portability, and provider registration explicit for host access while leaving app-specific async orchestration in the existing typed runtime.

### 2. `SystemEffect` is removed

Fission will remove `SystemEffect` from the public model.

The current effect surface conflates three different concerns:

- runtime control,
- app-defined async work,
- and host/device access.

Those concerns need different extension mechanisms and different policy rules.

After this change:

- `HttpGet`, `FileRead`, `Alert`, `OpenUrl`, and `Authenticate` are not built-in special cases,
- `Cancel` and `ReleaseResource` are not application-facing host effects,
- and the older opaque `Effect::App(Vec<u8>)` path is no longer the preferred extension mechanism because typed jobs and services already cover app-defined async work.
- `Effect::App(Vec<u8>)` should therefore be removed in the same breaking change, or deprecated and immediately scheduled for removal if a short compatibility window is needed,
- and host access is always expressed as a capability request or a capability-backed resource/session.

At the data-model level, `Effect` will be reshaped accordingly.

A representative shape is:

```rust
pub enum Effect {
    Runtime(RuntimeEffect),
    Job(JobRequestPayload),
    StartService(ServiceStartPayload),
    ServiceCommand(ServiceCommandPayload),
    StopService(ServiceStopPayload),
    Capability(CapabilityInvocationPayload),
}

pub enum RuntimeEffect {
    Cancel { req_id: ReqId },
    ReleaseResource { resource_id: ResourceId },
}
```

Low-level runtime control such as cancellation and resource release remains in the model, but it becomes an explicit runtime concern rather than being mixed into host access.

This keeps the public API centered on typed jobs, typed services, typed capabilities, and a small runtime-control surface.

### 3. Capabilities are typed specs, not ad hoc shell hooks

A capability is a named, typed contract between app code and a shell provider.

Each capability spec defines:

- a stable namespaced identifier,
- what kind of capability it is,
- the request/config type,
- the success and failure payload types,
- whether it can emit repeated events,
- whether it accepts commands after start,
- and what permission or policy state the shell may expose to the UI.

Fission will introduce a capability module in `fission-core` for these types.

Representative shared metadata:

```rust
pub enum CapabilityKind {
    Operation,
    Resource,
    Session,
}

pub struct CapabilityMetadata {
    pub id: &'static str,
    pub kind: CapabilityKind,
    pub vendor: &'static str,
    pub schema_version: u32,
    pub min_core_version: &'static str,
}

pub struct CapabilityType<C> {
    pub metadata: CapabilityMetadata,
    _marker: PhantomData<C>,
}

pub struct CapabilitySlot<C> {
    pub ty: CapabilityType<C>,
    pub slot_key: Cow<'static, str>,
}
```

The capability type and slot story deliberately mirrors the existing job and service model.

- operation capabilities are typed by `CapabilityType<C>`,
- session capabilities use `CapabilitySlot<C>` the same way services use `ServiceSlot<Svc>`,
- and resource capabilities are declared through `BuildCtx.resources` with a stable `ResourceKey`, matching the runtime resource reconciliation model that already exists.

Fission will support three capability shapes.

#### 3.1 Operation capabilities

These are one-shot request/response interactions.

Examples:

- open a file picker,
- save a blob,
- make a network request,
- open a URL,
- start an external auth session,
- show a native confirmation prompt,
- request a one-off geolocation fix.

Representative trait:

```rust
pub trait OperationCapability {
    type Request: Serialize + DeserializeOwned + Send + 'static;
    type Ok: Serialize + DeserializeOwned + Send + 'static;
    type Err: Serialize + DeserializeOwned + Send + 'static;
    const ID: &'static str;
}
```

Reducer usage:

```rust
ctx.effects
    .capability(fs_cap::PICK_OPEN, PickOpenRequest::single_file())
    .on_ok(FileChosen.into())
    .on_err(FilePickFailed.into());
```

#### 3.2 Resource capabilities

These are mounted subscriptions or watches that emit events over time and are primarily declarative.

Examples:

- watch geolocation,
- watch device orientation,
- monitor connectivity,
- subscribe to ambient light changes,
- subscribe to headset connection state,
- or receive repeated permission-status updates.

Representative trait:

```rust
pub trait ResourceCapability {
    type Config: Serialize + DeserializeOwned + Send + 'static;
    type Event: Serialize + DeserializeOwned + Send + 'static;
    type StartErr: Serialize + DeserializeOwned + Send + 'static;
    const ID: &'static str;
}
```

Resource capabilities are declared from `BuildCtx`, not from reducers.

Representative build usage:

```rust
fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> impl IntoWidget<AppState> {
    ctx.resources.capability(
        CapabilityResource::new(
            ResourceKey::new("location-watch"),
            location_cap::WATCH_POSITION,
            WatchPositionConfig {
                high_accuracy: false,
                max_frequency_hz: 1.0,
            },
        )
        .deps(view.state.location_mode.clone())
        .on_event(LocationEventReceived)
        .on_start_failed(LocationWatchFailed)
        .on_stopped(LocationWatchStopped),
    );

    // ... return node tree
}
```

The runtime reconciles these resources the same way it reconciles other mounted runtime resources:

- mount starts,
- dependency changes restart or update,
- unmount stops,
- stale events are dropped by generation.

#### 3.3 Session capabilities

These are stateful, long-lived host interactions that emit events and also accept commands after start.

Examples:

- media playback,
- camera capture,
- microphone recording,
- screen recording,
- Bluetooth or nearby-device sessions,
- map or navigation sessions,
- external display sessions,
- or a platform-native document viewer.

This is the shape needed for cases such as audio and video where a pure one-shot request or a forward-only stream is not enough.

A media session is not just “play and receive frames.”

It needs commands such as:

- play,
- pause,
- seek,
- change playback rate,
- toggle looping,
- change volume,
- switch tracks,
- request current position,
- or switch output device.

Representative trait:

```rust
pub trait SessionCapability {
    type Config: Serialize + DeserializeOwned + Send + 'static;
    type Command: Serialize + DeserializeOwned + Send + 'static;
    type CommandOk: Serialize + DeserializeOwned + Send + 'static;
    type CommandErr: Serialize + DeserializeOwned + Send + 'static;
    type Event: Serialize + DeserializeOwned + Send + 'static;
    type StartErr: Serialize + DeserializeOwned + Send + 'static;
    const ID: &'static str;
}
```

Reducer usage:

```rust
ctx.effects.start_capability_session(
    media_cap::PLAYER_SESSION.slot("preview"),
    StartPlayer {
        source: selected_track,
        autoplay: false,
    },
)
.on_started(PlayerSessionStarted.into())
.on_start_failed(PlayerSessionStartFailed.into())
.on_event(PlayerSessionEventReceived.into());
```

Later, on a user tap:

```rust
ctx.effects
    .command_capability_session(
        media_cap::PLAYER_SESSION.slot("preview"),
        PlayerCommand::SetPlaybackRate(1.5),
    )
    .on_err(PlayerCommandFailed.into());
```

This is how Fission covers streaming, controllable sessions, and future devices without inventing a separate mechanism for each domain.

#### 3.4 Capability providers are explicit shell registrations

The public extension story needs to be concrete, not implied.

Fission will therefore expose provider traits that mirror the three capability shapes.

Representative provider interfaces:

```rust
pub trait OperationProvider<C: OperationCapability>: Send + Sync + 'static {
    fn invoke(
        &self,
        request: C::Request,
        ctx: CapabilityJobCtx<C>,
    ) -> BoxFuture<Result<C::Ok, C::Err>>;
}

pub trait ResourceProvider<C: ResourceCapability>: Send + Sync + 'static {
    fn start(
        &self,
        config: C::Config,
        ctx: CapabilityResourceCtx<C>,
    ) -> BoxFuture<Result<Box<dyn CapabilityResourceRunner<C>>, C::StartErr>>;
}

pub trait SessionProvider<C: SessionCapability>: Send + Sync + 'static {
    fn start(
        &self,
        config: C::Config,
        ctx: CapabilitySessionCtx<C>,
    ) -> BoxFuture<Result<Box<dyn CapabilitySessionRunner<C>>, C::StartErr>>;
}
```

The runners keep the long-lived part of the capability alive:

```rust
pub trait CapabilityResourceRunner<C: ResourceCapability>: Send + 'static {
    fn on_stop(self: Box<Self>, ctx: CapabilityResourceCtx<C>) -> BoxFuture<()>;
}

pub trait CapabilitySessionRunner<C: SessionCapability>: Send + 'static {
    fn on_command(
        &mut self,
        command: C::Command,
        ctx: CapabilitySessionCtx<C>,
    ) -> BoxFuture<Result<C::CommandOk, C::CommandErr>>;

    fn on_stop(self: Box<Self>, ctx: CapabilitySessionCtx<C>) -> BoxFuture<()>;
}
```

Shell registration should then be symmetrical with the existing async runtime registration model:

```rust
DesktopApp::new(App)
    .with_async(|asyncs| {
        asyncs.register_operation_capability(fs_cap::PICK_OPEN, native_picker_provider());
        asyncs.register_resource_capability(location_cap::WATCH_POSITION, gps_provider());
        asyncs.register_session_capability(media_cap::PLAYER_SESSION, media_player_provider());
    });
```

This gives third-party developers a predictable place to plug in.

They define a spec. They implement one or more providers. A shell opts into those providers explicitly.

#### 3.5 Capability APIs map directly onto the current runtime model

The capability system is not a second async runtime.

It is a typed host-access layer that lowers onto the runtime shapes that Fission already has.

- operation capabilities lower to `Effect::Capability(CapabilityInvocationPayload::Operation { .. })` and reuse the existing `EffectBuilder` request/response flow,
- resource capabilities are declared from `BuildCtx.resources` and reconcile through the same mounted-resource pipeline as today's `JobResource`, `ServiceResource`, and `TimerResource`,
- session capabilities are a typed wrapper over the same start/command/stop lifecycle that services already use,
- and capability registrations extend the existing shell async registry rather than creating a second shell builder.

The new public builder methods are therefore additions to surfaces that already exist:

- `Effects::capability(...)`,
- `Effects::start_capability_session(...)`,
- `Effects::command_capability_session(...)`,
- `Effects::stop_capability_session(...)`,
- `ResourceRegistry::capability(...)`,
- and `AsyncRegistry::{register_operation_capability, register_resource_capability, register_session_capability}`.

That is enough to make the proposal mechanically implementable within the runtime that Fission already ships.

### 4. Files use handles, not portable paths

Fission will not define a portable capability API around arbitrary local paths.

The portable abstraction is a granted handle plus metadata.

Representative types:

```rust
pub struct BlobHandle(pub u64);

pub struct BlobMetadata {
    pub name: String,
    pub mime: Option<String>,
    pub size_bytes: u64,
    pub created_at_unix_ms: Option<u64>,
    pub modified_at_unix_ms: Option<u64>,
}
```

Portable file flows become:

- picker returns one or more granted handles,
- save/export returns a writable handle or completes the export directly,
- reads operate on a handle,
- writes operate on a handle,
- metadata inspection operates on a handle,
- drag-and-drop should follow the same handle model,
- and shells decide how those handles map to real local files, browser `Blob`s, sandboxed documents, or restricted-runtime resources.

This design is intentionally conservative.

It keeps desktop paths out of the portable API surface, which avoids forcing every non-desktop shell into a fake filesystem model.

Desktop-specific path capabilities may still exist as optional capabilities, but they are not the portable default.

Handle lifetime must also be explicit.

- a handle remains valid until it is released, revoked, or the shell session ends,
- apps may release handles early through `RuntimeEffect::ReleaseResource`,
- shells may revoke handles because of permission loss, sandbox expiry, or provider shutdown,
- and any attempt to read or write a revoked handle fails deterministically.

The current `ActionInput::Drop { paths }` payload is therefore legacy. It should be migrated in a follow-up so external drag-and-drop also yields granted handles plus metadata instead of raw paths.

### 5. Capability availability is explicit in both build configuration and runtime policy

Fission needs two layers of control.

#### 5.1 Compile-time provider inclusion

Shells may compile providers in or out using feature flags.

Representative examples:

- `cap-net`
- `cap-fs-picker`
- `cap-fs-save`
- `cap-camera`
- `cap-microphone`
- `cap-location`
- `cap-motion`
- `cap-share-sheet`
- `cap-media-session`

If a provider is compiled out, the shell cannot expose it at runtime.

This matters for binary size, attack surface, distribution targets, and shells that intentionally omit certain host integrations.

#### 5.2 Runtime capability policy

A compiled-in provider is still not automatically available to every app.

Each shell must expose a capability policy layer that can:

- allow a capability,
- deny a capability,
- expose it only after user prompt,
- expose only a constrained subset,
- or report it unavailable because the platform itself does not support it.

Representative types:

```rust
pub enum CapabilityAvailability {
    Unavailable,
    DisabledByPolicy,
    PermissionRequired,
    Granted,
}

#[derive(Default)]
pub struct CapabilityConstraints {
    pub allowed_mime_patterns: Option<Vec<String>>,
    pub max_bytes: Option<u64>,
    pub max_frequency_hz: Option<f32>,
    pub max_resolution: Option<(u32, u32)>,
    pub vendor: BTreeMap<String, serde_json::Value>,
}

pub struct CapabilityState {
    pub availability: CapabilityAvailability,
    pub constraints: CapabilityConstraints,
}
```

Apps must be able to inspect these states through the environment snapshot before they render affordances.

That allows a single app to behave correctly across:

- a desktop shell with local file import enabled,
- a web shell with browser picker support but no path access,
- a restricted runtime that allows network access but forbids file import,
- or a sandbox that permits camera preview but denies microphone recording.

### 6. Capability discovery and policy changes are part of the environment contract

The app must not discover capability availability only by trying an action and failing.

Fission will expose capability availability in the environment snapshot so widgets and apps can render appropriately.

Concretely, `Env` grows a capability catalog owned by the shell. Shells refresh that catalog before each build, and policy changes schedule a rebuild the same way other environment changes do.

Representative policy API:

```rust
pub trait CapabilityPolicy {
    fn state(&self, capability_id: &str) -> CapabilityState;
    fn watch(&self, capability_id: &str) -> CapabilityPolicyStream;
}
```

Representative shell configuration:

```rust
DesktopApp::new(App)
    .with_capability_policy(|policy| {
        policy.allow(net_cap::REQUEST);
        policy.prompt(camera_cap::SESSION);
        policy.disable(fs_cap::PICK_OPEN);
        policy.constrain(
            location_cap::WATCH_POSITION,
            CapabilityConstraints {
                max_frequency_hz: Some(0.5),
                ..CapabilityConstraints::default()
            },
        );
    });
```

Representative usage:

```rust
let state = view.env.capabilities.state(fs_cap::PICK_OPEN.id());
match state.availability {
    CapabilityAvailability::Granted => { /* show active upload CTA */ }
    CapabilityAvailability::PermissionRequired => { /* show CTA + explanation */ }
    CapabilityAvailability::DisabledByPolicy => { /* show disabled state */ }
    CapabilityAvailability::Unavailable => { /* hide or replace affordance */ }
}
```

This is required for good UX on every platform class, including future ones.

Policy changes while the app is running must also be observable.

The contract is:

1. the shell updates `view.env.capabilities`,
2. the runtime schedules a rebuild,
3. active capability resources and sessions receive lifecycle notifications,
4. if a capability is no longer permitted, the runtime stops it with a typed stop reason.

Representative stop reasons:

```rust
pub enum CapabilityStopReason {
    Requested,
    PermissionRevoked,
    PolicyChanged,
    ProviderUnavailable,
    Backgrounded,
    ShellShutdown,
}
```

For long-lived capability resources and sessions, the builder surfaces should therefore expose `on_stopped(...)` and `on_state_changed(...)` bindings in addition to `on_event(...)`.

### 7. Third-party developers can add capabilities without changing `fission-core`

This must be a first-class extension path.

A third-party capability should require three pieces.

#### 7.1 A capability spec crate

This crate defines the capability ID, types, and helper constructors.

Example:

```rust
pub mod thermal_camera_cap {
    use fission::core::capability::SessionCapability;
    use serde::{Deserialize, Serialize};

    pub struct ThermalCameraSession;

    impl SessionCapability for ThermalCameraSession {
        type Config = StartThermalCamera;
        type Command = ThermalCameraCommand;
        type CommandOk = ThermalCameraCommandOk;
        type CommandErr = ThermalCameraCommandErr;
        type Event = ThermalCameraEvent;
        type StartErr = ThermalCameraStartErr;
        const ID: &'static str = "vendor.thermal_camera.session";
    }

    pub const SESSION: CapabilityType<ThermalCameraSession> =
        CapabilityType::new(CapabilityMetadata {
            id: ThermalCameraSession::ID,
            kind: CapabilityKind::Session,
            vendor: "vendor",
            schema_version: 1,
            min_core_version: "0.2",
        });
}
```

#### 7.2 A provider crate for one or more shells

This crate implements the provider against a real platform API.

For example:

- a desktop provider using OS-specific camera APIs,
- a web provider using browser media APIs,
- a mobile provider using native camera frameworks,
- or a test provider using deterministic mock data.

#### 7.3 Shell registration

The shell explicitly registers the provider.

Example:

```rust
DesktopApp::new(App)
    .with_async(|asyncs| {
        asyncs.register_session_capability(
            thermal_camera_cap::SESSION,
            thermal_camera_desktop::provider(),
        );
    })
    .with_capability_policy(|policy| {
        policy.prompt(thermal_camera_cap::SESSION);
    });
```

This makes the extension story concrete.

Third parties do not patch `fission-core`. They define capability specs, write providers, and register those providers in shells that choose to support them.

#### 7.4 Capability authoring checklist

An author adding a new capability should be able to follow a repeatable path.

1. Define a spec crate:
   - choose `OperationCapability`, `ResourceCapability`, or `SessionCapability`,
   - declare a stable namespaced ID,
   - define request, event, command, and error types,
   - publish a `CapabilityType<C>` constant with `CapabilityMetadata`.
2. Implement one or more providers:
   - desktop, mobile, web, headless, or restricted runtime,
   - each provider implements the corresponding provider trait,
   - each provider documents which policy constraints it understands.
3. Register the provider in a shell:
   - use `AsyncRegistry::register_operation_capability(...)`,
   - `register_resource_capability(...)`,
   - or `register_session_capability(...)`.
4. Configure policy:
   - default allow, prompt, deny, or constrained states,
   - policy watch behavior for runtime status changes,
   - permission prompt UX where the platform requires it.
5. Add deterministic tests:
   - a scripted mock provider for operation success/failure,
   - a scripted resource or session provider for event ordering,
   - policy transition tests for prompt, revoke, and constrained operation.

### 8. Capabilities do not replace embeds

Fission already has an embed model for visual surfaces such as video, audio, and 3D integrations.

The capability system does not replace that model. It complements it.

The distinction is:

- embeds describe how a visual or platform-backed surface participates in layout, paint, composition, hit testing, and lifecycle,
- capabilities describe how the app requests access to host functionality and how it controls or observes that functionality over time.

Some features are capability-only:

- file picker,
- geolocation watch,
- network request,
- save/export,
- permission prompt.

Some features are embed-only:

- a pure retained render surface that exposes no host permissions or external hardware.

Many important features use both:

- video playback uses an embed for the visible surface and a media session capability for play/pause/seek/rate/output control,
- camera preview uses an embed for the visible preview and a camera session capability for device selection/capture/flash/zoom/focus,
- a map view uses an embed for the rendered surface and a session capability for camera commands, route updates, and device-location coupling,
- smart-glasses scene rendering may use one or more embeds plus capability sessions for sensors, capture, and spatial input.

This split keeps Fission coherent.

Visual composition remains in the render and embed architecture. Host access remains in the capability architecture.

### 9. The capability model must cover modern and emerging devices without changing shape

The design must survive beyond phones and laptops.

The important observation is that most device integrations still fit one of the three capability shapes:

- one-shot operation,
- resource watch,
- or controllable session.

Examples:

- file picker: operation,
- save/export: operation,
- network request: operation,
- geolocation watch: resource,
- gyro/orientation watch: resource,
- ambient light watch: resource,
- camera preview/capture: session,
- microphone record/monitor: session,
- media playback: session,
- nearby-device discovery: resource or session depending on control needs,
- smart glasses scene capture: session,
- haptic device output: operation for simple feedback or session for complex control,
- AR anchor tracking: resource or session,
- wearable biometric stream: resource,
- external controller or headset route selection: session command,
- and future hardware-specific features can still be expressed as one of these shapes.

This is deliberate.

Fission should not predict every device class in core. It should provide a stable interaction model that lets new device classes adopt the framework without changing the framework's fundamentals.

### 10. Permission prompts, privacy, and lifecycle are part of the capability contract

Host access is never just transport.

Capabilities must account for:

- permission-request flows,
- user denial and later re-enable,
- platform privacy indicators,
- foreground/background transitions,
- app suspension and resume,
- device hot-plugging,
- quality levels and sampling rates,
- battery-sensitive throttling,
- connectivity transitions,
- and platform-specific revocation while a session is active.

That means capability sessions and resources must have explicit lifecycle rules.

Representative consequences:

- camera preview may stop automatically on background and emit an event,
- location watch may downgrade frequency under policy,
- microphone recording may fail mid-session due to OS revocation,
- media route selection may change because a headset disconnects,
- and nearby-device sessions may lose peers asynchronously.

These are capability events, not hidden shell side effects.

For operations, this also means cancellation must remain explicit.

`ctx.effects.cancel(req_id)` stays valid, but it becomes sugar for `RuntimeEffect::Cancel` rather than a host-level system effect. Providers should treat cancellation as best-effort and return a deterministic terminal outcome:

- `on_err` with a typed cancellation error for one-shot operations,
- `on_stopped(CapabilityStopReason::Requested)` for resources and sessions.

### 11. Testability is a requirement, not an afterthought

Every capability provider must support one of two things:

- a deterministic mock provider,
- or a test harness adapter that makes the behavior deterministic enough for CI.

The capability model should therefore make provider substitution normal.

Example:

```rust
HeadlessApp::new(App)
    .with_async(|asyncs| {
        asyncs.register_resource_capability(
            location_cap::WATCH_POSITION,
            scripted_location_provider(),
        );
        asyncs.register_session_capability(
            media_cap::PLAYER_SESSION,
            scripted_player_provider(),
        );
    });
```

This keeps Fission aligned with its determinism and audit goals.

The test harness contract should be explicit.

Representative scripted session provider:

```rust
let provider = ScriptedSessionProvider::<media_cap::PlayerSession>::new()
    .initial_state(CapabilityState {
        availability: CapabilityAvailability::Granted,
        constraints: CapabilityConstraints::default(),
    })
    .step_started()
    .step_event(PlayerEvent::StateChanged { playing: true })
    .step_event(PlayerEvent::PositionChanged {
        position_ms: 1_000,
        duration_ms: Some(60_000),
    })
    .step_state_changed(CapabilityState {
        availability: CapabilityAvailability::Granted,
        constraints: CapabilityConstraints {
            max_resolution: Some((1280, 720)),
            ..CapabilityConstraints::default()
        },
    })
    .step_stopped(CapabilityStopReason::PolicyChanged);
```

The minimum requirements for a deterministic mock provider are:

- scripted start success and start failure,
- scripted events in stable order,
- scripted command success and failure,
- scripted policy transitions,
- and deterministic stop reasons.

## End-to-end examples

The design is easier to evaluate when written as full flows rather than isolated traits.

### Example 0: Migrating today's built-in network request

This example shows how an existing `ctx.effects.http_get(...)` call migrates into the capability model without changing the reducer flow shape.

#### Before

```rust
ctx.effects
    .http_get("https://api.example.com/todos")
    .on_ok(TodosLoaded.into())
    .on_err(TodosFailed.into());
```

#### After

```rust
ctx.effects
    .capability(
        net_cap::REQUEST,
        HttpRequest {
            method: HttpMethod::Get,
            url: "https://api.example.com/todos".into(),
            headers: Vec::new(),
            body: None,
        },
    )
    .on_ok(TodosLoaded.into())
    .on_err(TodosFailed.into());
```

#### Shell registration

```rust
DesktopApp::new(App)
    .with_async(|asyncs| {
        asyncs.register_operation_capability(net_cap::REQUEST, reqwest_provider());
    })
    .with_capability_policy(|policy| {
        policy.allow(net_cap::REQUEST);
    });
```

### Example 1: File import on desktop, web, and restricted runtime

#### App code

The reducer is identical across platforms.

```rust
fn on_attach_document(
    state: &mut AppState,
    _: AttachDocument,
    ctx: &mut ReducerContext<AppState>,
) {
    ctx.effects
        .capability(
            fs_cap::PICK_OPEN,
            PickOpenRequest {
                allow_multiple: false,
                mime_filters: vec!["application/pdf".into(), "image/*".into()],
            },
        )
        .on_ok(DocumentPicked.into())
        .on_err(DocumentPickFailed.into());
}
```

#### Desktop shell

- compiled with `cap-fs-picker`
- provider opens a native file picker
- provider returns a granted `BlobHandle` plus metadata

#### Web shell

- compiled with `cap-fs-picker`
- provider opens the browser file picker
- provider returns a granted `BlobHandle` backed by browser-managed blob data
- no local path is exposed

#### Restricted runtime

- compiled without picker support, or runtime policy disables it
- environment exposes `DisabledByPolicy`
- UI renders a disabled state or an explanation instead of an active CTA

The app code does not branch on platform. It branches only on capability status.

### Example 2: Media playback with seek and playback speed

This example shows why one-shot plus streaming is not enough by itself.

#### Session start

```rust
fn on_open_preview(
    state: &mut AppState,
    action: OpenPreview,
    ctx: &mut ReducerContext<AppState>,
) {
    ctx.effects.start_capability_session(
        media_cap::PLAYER_SESSION.slot(format!("preview:{}", action.asset_id)),
        StartPlayer {
            source: MediaSource::Asset(action.asset_id),
            autoplay: false,
        },
    )
    .on_started(PlayerStarted.into())
    .on_start_failed(PlayerFailedToStart.into())
    .on_event(PlayerEventReceived.into());
}
```

#### User changes playback speed

```rust
fn on_speed_changed(
    state: &mut AppState,
    action: SetPlaybackRate,
    ctx: &mut ReducerContext<AppState>,
) {
    ctx.effects
        .command_capability_session(
            media_cap::PLAYER_SESSION.slot(state.preview_slot_key()),
            PlayerCommand::SetPlaybackRate(action.rate),
        )
        .on_err(PlayerCommandFailed.into());
}
```

#### Provider events

```rust
pub enum PlayerEvent {
    StateChanged { playing: bool },
    PositionChanged { position_ms: u64, duration_ms: Option<u64> },
    RateChanged { rate: f32 },
    Ended,
    RouteChanged { route_name: String },
}
```

This is the correct level of abstraction for audio and video.

### Example 3: Geolocation watch with policy constraints

#### Build-declared resource

```rust
fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> impl IntoWidget<AppState> {
    ctx.resources.capability(
        CapabilityResource::new(
            ResourceKey::new("nearby-delivery-location"),
            location_cap::WATCH_POSITION,
            WatchPositionConfig {
                high_accuracy: true,
                max_frequency_hz: 0.5,
            },
        )
        .deps(view.state.delivery_mode.clone())
        .on_event(LocationEventReceived)
        .on_stopped(LocationWatchStopped),
    );

    // ... return node tree
}
```

#### Runtime policy examples

- mobile shell may grant fine location
- desktop shell may grant coarse location only
- restricted shell may expose `DisabledByPolicy`
- watch-style device shell may force low-frequency updates to save battery

The UI remains deterministic because the resource contract is explicit and policy is surfaced to the app.

### Example 4: Third-party capability for smart glasses gesture input

A third-party vendor wants to expose air-tap and gaze gesture events.

They define a spec crate:

```rust
pub struct SpatialGestureWatch;

impl ResourceCapability for SpatialGestureWatch {
    type Config = SpatialGestureConfig;
    type Event = SpatialGestureEvent;
    type StartErr = SpatialGestureStartErr;
    const ID: &'static str = "vendor.spatial_gesture.watch";
}
```

They implement providers for shells that support the hardware.

An app uses it like any other resource capability:

```rust
fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> impl IntoWidget<AppState> {
    ctx.resources.capability(
        CapabilityResource::new(
            ResourceKey::new("air-gesture-watch"),
            vendor_spatial_cap::WATCH,
            SpatialGestureConfig::default(),
        )
        .deps(view.state.gesture_profile.clone())
        .on_event(SpatialGestureEventReceived)
        .on_start_failed(SpatialGestureStartFailed),
    );

    // ... return node tree
}
```

No Fission core change is required.

### Example 5: Camera preview with both an embed and a capability session

This example shows the relationship between visual surfaces and host control.

#### Build

The UI declares a camera preview embed in the tree:

```rust
CameraPreviewEmbed {
    session_slot: camera_cap::SESSION.slot("front-preview"),
    aspect_ratio: 16.0 / 9.0,
}
```

That node participates in layout and paint like any other embed-backed surface.

#### Session start

The reducer starts the matching capability session:

```rust
ctx.effects.start_capability_session(
    camera_cap::SESSION.slot("front-preview"),
    StartCameraSession {
        lens: CameraLens::Front,
        capture_audio: false,
        preferred_resolution: Some((1920, 1080)),
    },
)
.on_started(CameraStarted.into())
.on_start_failed(CameraStartFailed.into())
.on_event(CameraEventReceived.into());
```

#### Commands

Later the app can issue commands:

```rust
ctx.effects.command_capability_session(
    camera_cap::SESSION.slot("front-preview"),
    CameraCommand::SetZoom(2.0),
);

ctx.effects.command_capability_session(
    camera_cap::SESSION.slot("front-preview"),
    CameraCommand::CaptureStill,
);
```

This is the pattern that scales across phones, tablets, wearables, smart glasses, kiosks, and future camera-bearing devices.

## Rationale

This design does four important things at once.

First, it removes a misleading boundary.

`SystemEffect` suggests that Fission can maintain a universal built-in host API by growing a closed enum. That is the wrong long-term model.

Second, it keeps the runtime coherent.

Jobs, services, resources, and embeds remain the core execution shapes. Capabilities use those shapes rather than inventing another execution system.

Third, it makes portability explicit.

The framework stops pretending that path-based files, always-on network access, or uniformly available hardware make sense across all targets.

Fourth, it gives third parties a real extension path.

That is essential for emerging hardware, special-purpose devices, enterprise shells, and future runtimes.

## Consequences

### Positive

- Fission gets one consistent model for files, network, media devices, sensors, and future host integrations.
- Apps can discover capability availability before rendering UX.
- Web, native, and restricted runtimes can expose the same app-level capability with different provider implementations.
- Controllable sessions such as media playback or camera capture fit naturally without bespoke framework features.
- Third-party device integrations can be added without changing `fission-core`.
- Compile-time features and runtime policy are cleanly separated.
- Visual device surfaces such as cameras, maps, and video remain compatible with the existing embed architecture instead of bypassing it.

### Costs and trade-offs

- This is a breaking change.
- Existing `SystemEffect` users need migration.
- File APIs must be redesigned around handles and metadata rather than raw paths.
- Shells need explicit provider registration and policy configuration.
- Capability discovery adds surface area to the environment contract.
- Native Rust apps can still bypass the framework and call OS libraries directly; framework-level capability enforcement applies to code that chooses to use the framework surface.

## Migration plan

1. Introduce capability traits, payloads, provider registration, policy APIs, and environment capability status.
2. Add first-party capability crates for:
   - network request,
   - file open/save/import/export,
   - open URL,
   - external auth session,
   - alert/confirm,
   - media player session,
   - camera session,
   - microphone session,
   - geolocation watch,
   - motion/orientation watch.
3. Add mock providers and test harness adapters.
4. Migrate shell implementations from `SystemEffect` handling to capability providers.
5. Migrate examples and apps from `ctx.effects.http_get(...)` / `file_read(...)` / `add(SystemEffect::...)` to the capability APIs.
6. Remove `SystemEffect` and related builder helpers.
7. Follow up separately on external file-drop payloads so they also move from raw paths toward granted handles.

## Rejected alternatives

### Keep `SystemEffect` and add more variants

Rejected because it keeps host access as a closed enum maintained in core, which does not scale to diverse device classes or third-party extensions.

### Keep `SystemEffect` as a compatibility layer indefinitely

Rejected because it leaves two overlapping public models in place and makes the framework harder to understand.

### Treat every device integration as an app-defined service instead of a capability

Rejected because it throws away the parts that matter most for host access:

- compile-time inclusion,
- runtime policy,
- environment discovery,
- permission semantics,
- and portable provider substitution.

### Model everything as a stream

Rejected because one-shot operations and controllable sessions are materially different. File pickers, share sheets, auth flows, and media sessions do not become clearer when flattened into one streaming-only API.

## Open questions

These questions do not block the decision, but they need follow-up design and implementation work.

- What exact trait, builder, and envelope names should the public API expose?
- Which parts of `CapabilityConstraints` should be generic in core versus vendor-defined in extension crates?
- What is the best ergonomic API for build-declared capability resources so they remain obviously declarative while still mirroring today's `ResourceRegistry` model?
- Should `Effect::App(Vec<u8>)` be removed in the same breaking change as `SystemEffect`, or one release later?
- Which first-party capabilities belong in separate crates versus the shell crates?
- Which parts of the capability environment should be available in headless tests by default?
