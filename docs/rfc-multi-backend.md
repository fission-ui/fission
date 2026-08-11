# RFC: Replaceable Interactive Backends and a Production Skia Implementation

Status: accepted
Audience: Fission runtime, rendering, text, shell, platform, tooling, and release maintainers
Scope: interactive graphical rendering and host integration on native and Web targets
Related: `docs/adr/0001-canonical-text-editing-frame-scheduling-and-incremental-ui-updates.md`

## 1. Summary

Fission will refactor its interactive rendering and platform integration so that
Vello, wgpu, Parley, Winit, Skia, and any future implementation sit behind
Fission-owned contracts.

This RFC makes two commitments:

1. Backend replaceability is required architecture and will be implemented.
2. Skia will be engineered as a production-quality backend so that Fission can
   evaluate it fairly under real workloads.

This RFC does **not** commit Fission to removing Vello, wgpu, Parley, Winit, or
any other current dependency. Removal is not a short-term objective. Those
projects may remain supported indefinitely, and Fission may continue to
maintain and improve its forks and contribute improvements upstream. If the
community rallies around Fission and the Rust-native stack, keeping that stack
healthy is a positive outcome rather than a failed migration.

Skia is the obvious next implementation because it offers a mature,
cross-platform combination of software rasterization, GPU rendering, text,
images, SVG, filters, and browser support. It is not privileged in Fission's
public API. The architecture must also permit a future Fission-owned renderer,
text engine, GPU abstraction, window host, or complete platform stack if
existing projects cannot meet Fission's requirements.

The Skia effort is not a disposable prototype. A shallow adapter would mostly
measure integration shortcuts and missing engineering. Fission will implement
the Skia path deeply enough to exercise the same production responsibilities as
the current stack: native and Web presentation, software and GPU rendering,
paragraph layout, images, SVG, effects, retained composition, resources,
external surfaces, lifecycle recovery, accessibility integration, packaging,
diagnostics, memory, binary size, and performance. A fair comparison is an
output of that engineering, not a prerequisite for doing it.

No permanent default-backend decision and no dependency-removal decision is
made by this RFC. Those decisions require evidence from production-grade
implementations and, if proposed, a later RFC.

## 2. Motivation

Fission's current graphical stack has enabled the framework to grow quickly:

- Vello renders the main retained 2D scene;
- wgpu supplies GPU devices, textures, presentation, and the current 3D path;
- Parley and Fontique provide graphical text layout and font selection;
- a software renderer supports environments where the GPU path is unavailable;
- Winit owns the cross-platform window and event loop integration;
- AccessKit adapters expose native accessibility on the targets where the
  current bridge is implemented;
- browser glue presents either WebGPU output or a Canvas 2D software buffer.

The problem is not that these dependencies exist. The problem is that several
Fission responsibilities are currently coupled to particular implementations
and, in some cases, co-located in one shell. That makes it expensive to test a
different renderer, adopt a better text system, use a different window host, or
retain one component while replacing another.

The current abstractions are too narrow to solve this. `fission-render` exposes
a backend-neutral-looking `RenderScene`, but its `Renderer` contract only asks
an implementation to render a scene. A production interactive backend must
also attach to surfaces, resize, present, recover from surface or device loss,
manage resources and budgets, supply the paragraph geometry used by layout and
editing, produce screenshots, and coordinate external content.

At the same time, Skia changes the available design space. It is not merely a
different path rasterizer. A coherent Skia stack can provide:

- mature CPU rasterization;
- Ganesh GPU rendering across established native APIs;
- Graphite as an independently qualified future GPU path;
- SkParagraph, SkShaper, HarfBuzz, Unicode services, and platform font managers;
- image codecs and animated-image metadata;
- SkSVGDOM;
- mature clipping, blending, filtering, color, and layer operations;
- CanvasKit for interactive browser rendering.

Using only the intersection of Vello and Skia would discard much of that value.
Conversely, allowing Skia types to leak through Fission would replace one form
of coupling with another. Fission therefore needs semantic contracts that it
owns, with room for backend-specific implementation quality and capability.

## 3. Decision

### 3.1 Fission owns the contracts

Core UI, layout, semantics, input, and application code must not depend on
Vello, Skia, wgpu, Winit, Parley, or their public types. Fission-owned types
describe what the framework needs:

- a compiled interactive frame;
- paint and composition semantics;
- paragraph layout and editing geometry;
- logical resources and their state;
- platform lifecycle and input;
- a render session and presentation lifecycle;
- external-surface composition and synchronization;
- backend capabilities and diagnostics.

Implementations may use any appropriate library behind those contracts.

### 3.2 Existing backends remain first-class

The current Rust-native stack will be migrated behind the new contracts first
and kept working throughout the refactor. It is not labelled legacy merely
because Skia is added. It remains eligible to be the default, to coexist with
Skia, or to become preferable again as its ecosystem improves.

Fission may maintain forks, contribute fixes upstream, and invest in Vello,
wgpu, Parley, Winit, and AccessKit where that improves Fission and the wider Rust
ecosystem. This is compatible with, not contrary to, backend replaceability.

### 3.3 Skia is a production implementation programme

Fission will build a complete Skia backend rather than a throwaway spike. The
implementation may be released incrementally, but its design, ownership,
testing, packaging, and support model must be suitable for long-term use from
the beginning.

The first credible comparison occurs only after Skia supports representative
Fission applications across native and Web targets and meets the acceptance
criteria in this RFC. An early window containing a few shapes is useful as an
implementation milestone but is not evidence for or against Skia.

### 3.4 Visual identity is not pixel identity

Fission defines shared constraint, interaction, semantics, overflow-policy, and
design-system contracts. It does not require different graphics and text
engines to produce pixel-identical output or identical text-derived dimensions.

Skia and Vello may differ in glyph rasterization, antialiasing, filter output,
gradient interpolation, color handling, and text metrics. Text metrics may
produce backend-profile-dependent valid line breaks, paragraph sizes, and
downstream layout. That variation is allowed only when both engines apply the
same constraints, choose valid break opportunities, enforce the same maximum
line and overflow policy, preserve the same semantic content, and derive all
interaction geometry from the result actually displayed.

Each backend must be visually excellent. Cross-backend tests enforce semantic
and non-text geometric invariants plus the text rules above; backend-specific
layout and visual goldens enforce stability and quality.

### 3.5 Future Fission-owned implementations remain possible

The architecture must not assume that the final choice is always an existing
third-party project. If Skia and the current Rust stack both expose limitations
that materially constrain Fission, Fission may build and maintain its own
renderer, text engine, GPU layer, window/event host, or other backend component.

This RFC authorizes the architecture needed to make that possible. It does not
authorize or schedule such an implementation today.

### 3.6 Skia becomes the production software renderer

The optimized Skia raster pipeline replaces Fission's existing software
renderer in production graphical profiles. Native software rendering uses the
same `fission-render-skia` frame compiler, SkParagraph profile, resource
authority, lifecycle, diagnostics, and readback contracts as native Skia GPU
rendering. Interactive Web software rendering uses Fission's CanvasKit profile
in software mode rather than the existing Canvas 2D pixel path.

This is a renderer migration, not a change to widget APIs or to non-graphical
targets. Static-site HTML, SSR HTML, and terminal cells remain independent of
Skia. The previous software renderer may remain temporarily as a comparison or
conformance implementation while migration is in progress, but no completed
production profile selects it and it is not a fallback beneath a Skia profile.

## 4. Goals

- Make rendering, text, GPU presentation, and platform hosting independently
  replaceable behind Fission-owned contracts.
- Preserve the public Fission application and widget authoring experience.
- Keep Vello/wgpu/Parley/Winit functional while extracting the boundaries.
- Implement Skia to production quality across native and interactive Web
  targets.
- Support both GPU and optimized software rendering in the Skia backend.
- Use Skia's own paragraph, shaping, font, image, SVG, and effect facilities
  where they are the coherent Skia equivalent of the current stack.
- Keep static-site, SSR, and terminal targets independent from graphical
  backend dependencies.
- Permit implementation-specific rendering improvements without reducing all
  backends to the lowest common denominator.
- Make unsupported operations explicit through capabilities and diagnostics;
  accepted operations must never silently disappear.
- Ship Fission-maintained native Skia artifacts by default so application
  developers do not need a C++ toolchain.
- Keep a pinned, reproducible, opt-in source build for developers and unusual
  targets.
- Measure memory, binary size, startup, latency, throughput, quality, and
  robustness using complete single-backend applications.
- Keep the architecture open to community backends and future Fission-owned
  implementations without prematurely stabilizing a broad public plugin ABI.

## 5. Non-goals

- Removing Vello, wgpu, Parley, Winit, or AccessKit in the near term.
- Promising that any current dependency will eventually be removed.
- Selecting Skia as the permanent default before it is fully qualified.
- Making Vello and Skia pixel-identical.
- Designing a lowest-common-denominator renderer.
- Loading every backend into every application binary.
- Exposing Skia, Vello, wgpu, Winit, Ganesh, or Graphite types through Fission's
  public authoring API.
- Replacing static-site HTML, server-rendered HTML, or terminal-cell rendering
  with Skia.
- Treating Skia as a window system, accessibility implementation, video
  decoder, or general-purpose 3D engine.
- Committing to a stable third-party backend ABI in the first implementation.
- Building a Fission-owned renderer or window system as part of this RFC.
- Preserving implementation quirks that are not part of Fission's documented
  behavior.

## 6. Terminology

**Core IR** is Fission's backend-independent representation of widgets,
semantics, layout inputs, and target-independent operations.

**Interactive frame** is the immutable, backend-independent result compiled for
one graphical frame. It includes paint, composition, damage, resources, and
external-surface placement.

**Graphics backend** implements paint, composition, text rasterization, image
decode, and presentation for an interactive graphical target. Vello and Skia
are graphics backends in this sense.

**Paragraph engine** shapes and lays out text and returns the geometry needed by
layout, painting, selection, hit testing, caret movement, and IME.

**Platform host** owns the OS or browser event loop, window/canvas lifecycle,
input, IME transport, accessibility transport, clipboard, drag and drop, system
appearance, and other device integration. Winit is part of a host
implementation; it is not a renderer.

**Presenter** attaches a graphics backend to a platform surface and owns resize,
present, loss, suspend, and resume behavior.

**External surface** is content produced outside the 2D paint backend, such as
video, a web view, or a 3D texture, which must be positioned and synchronized
with the Fission frame.

**Backend profile** is a build-time selection of implementation and features,
for example Skia raster, Skia Ganesh, or Vello/wgpu.

**Reference backend** means the implementation used to define and exercise the
widest intended graphical behavior. It does not mean that other implementations
must reproduce its pixels.

## 7. Current architecture and evidence

The repository already contains useful seams, but they are incomplete.

### 7.1 Render IR

`crates/rendering/fission-render` defines `DisplayOp`, `DisplayList`,
`RenderLayer`, and `RenderScene`. These are valuable Fission-owned types and are
the starting point for the new frame contract.

The current IR can express clips, transforms, opacity layers, cached scenes,
backdrop filters, shapes, text, rich text, images, paths, SVG, and embedded
surfaces. It does not yet define a complete production backend lifecycle,
resource ownership model, damage model, or typed synchronization contract for
external surfaces.

Its `Renderer` trait exposes only `render_scene`. That is appropriate for a
headless scene consumer, but insufficient as the authority for an interactive
surface.

### 7.2 Shell and presentation

`crates/shell/fission-shell-winit` currently contains window lifecycle, input,
Vello scene construction, wgpu surfaces, texture composition, renderer
selection, software fallback, WebGPU initialization, browser Canvas 2D
presentation, video integration, and optional 3D integration.

This proves that Winit is not intrinsically tied to Vello, because the same host
already selects more than one render path. It also shows that the boundaries
are too co-located: replacing the renderer currently requires editing the host
that owns unrelated platform behavior.

`crates/tools/fission-test` also depends directly on `fission-render-vello`, so
the current default test renderer is coupled to the production implementation.
The conformance harness must instead select a backend through the same Fission
contract used by applications.

The current Winit accessibility bridge is a no-op on WebAssembly and Android.
Those are existing target gaps, not renderer capabilities. The multi-backend
work must not hide them behind a generic “native adapter” claim: Android needs
its platform semantics bridge, and interactive Web needs the DOM/ARIA mirror
described by this RFC.

### 7.3 Text

`crates/rendering/fission-render-vello/src/text.rs` uses Parley and Fontique for
production text layout. `fission-layout` exposes `TextMeasurer`, but the
query-oriented trait does not carry one immutable paragraph result through
layout, painting, hit testing, selection, caret placement, and IME. Several
optional methods also have zero or empty defaults, which can hide an incomplete
backend behind apparently valid output.

ADR 0001 already decides that preedit, caret, selection, and hit testing must
share one paragraph-level geometry source. The multi-backend design implements
that decision rather than creating a second text abstraction.

### 7.4 3D and external content

`crates/core/fission-3d` currently owns a real wgpu render pipeline, textures,
depth state, and presentation inputs. Skia does not provide a general 3D engine
that can replace this directly. Video and web surfaces likewise have lifecycles
and synchronization requirements beyond a generic byte payload.

That current crate placement crosses the boundary this RFC establishes. The
neutral 3D scene/model and external-producer contract will remain in a core
crate, while the wgpu pipeline, shaders, textures, and device integration move
to an optional implementation crate, provisionally
`fission-render-wgpu3d`. The final crate name may follow repository conventions,
but no wgpu type remains in the neutral 3D contract.

The repository also contains `fission-render-wgpu2d`, currently outside the
default workspace build. That remains a legitimate backend candidate and is
another reason to describe Fission capabilities rather than encode a choice
between exactly Vello and Skia.

The new architecture must therefore support typed external producers and
interchange surfaces. It must not pretend that all content is a Skia canvas or
silently copy every frame through the CPU.

### 7.5 Non-graphical targets

`fission-shell-site`, `fission-shell-server`, and `fission-shell-terminal` are
distinct output systems. They share Fission widgets, state, layout concepts,
and semantics where appropriate, but they do not need an interactive graphics
backend. The refactor must preserve that separation and must not pull Skia into
their dependency graphs.

## 8. Architectural principles

### 8.1 One owner for each responsibility

Fission owns semantic intent and orchestration. A backend owns implementation
objects and caches. A host owns platform lifecycle. Resource resolution,
paragraph geometry, presentation, and external-surface synchronization each
have one named authority.

### 8.2 Dependency-neutral boundaries

Types crossing a Fission boundary use Fission geometry, identifiers, resource
handles, errors, and capabilities. A dependency's type may exist inside its
adapter, but cannot become required input to core, widgets, layout, semantics,
or another backend.

### 8.3 Do not flatten useful capability

The shared contract describes Fission's needs, not merely today's Vello feature
set. A backend may implement richer filters, color behavior, paragraph
features, or cache strategies. Fission can add semantic operations when they
serve the product even if another backend initially reports them unsupported.

### 8.4 Capability failure is explicit

A selected backend is validated against the target profile's declared
requirements at build time. Dynamic Rust UI states cannot generally be
enumerated during a build. If a dynamic frame requests an unsupported
operation, frame construction or rendering fails with node/source provenance
and follows an explicitly selected fallback policy. Runtime-device limitations
likewise return a structured diagnostic. No accepted operation may become a
no-op.

### 8.5 Layout and paint share text output

Within one backend profile, the paragraph object used to size and wrap text is
the same authority used for glyph placement, hit testing, selection, caret, and
IME geometry. This matters more than forcing two different paragraph engines to
produce identical line breaks.

Backend-dependent text layout is part of the selected profile, much like
platform font rendering. Widgets receive the same constraints and overflow
policy, but must not assume that an unmeasured string occupies identical width,
height, or line count under every paragraph engine.

### 8.6 Backend selection is a build concern by default

Normal applications include one interactive graphics backend. This avoids
shipping Skia, Vello, wgpu, Parley, and duplicate caches in the same artifact
merely to preserve a runtime switch most users do not need.

Multi-backend diagnostic applications may opt in to more than one backend for
side-by-side qualification.

### 8.7 Developer experience remains Fission-shaped

Application authors continue to depend on `fission`, build the same widgets,
use the same design system, and select a target through Fission tooling. They do
not construct Skia contexts, wgpu devices, Vello scenes, or Winit event loops.

### 8.8 Explicit selection and coexistence

Backend selection belongs to a Fission target's build profile. A project that
does not choose one receives the documented Fission default for that target.
Fission tooling may expose a command-line override for testing, but it resolves
to the same build configuration rather than mutating renderer state behind the
application.

The exact manifest and command spelling will be finalized with the tooling
implementation. The contract requires that:

- the selected backend and paragraph engine are visible in build and runtime
  diagnostics;
- an unavailable or incompatible selection fails with a stable diagnostic;
- selecting a backend does not silently select a different backend;
- GPU-to-software recovery occurs only when the target profile declares that
  policy and reports that it happened;
- normal release builds link one interactive backend;
- conformance and developer builds may explicitly bundle several backends;
- enabling 3D or another external producer adds only the general-GPU
  dependencies that producer requires.

## 9. Target architecture

```text
Fission application and widgets
              |
              v
Core runtime + Core IR + layout + semantics
              |
       +------+----------------------+------------------+
       |                             |                  |
       v                             v                  v
Static/SSR HTML                 Terminal cells   Interactive frame compiler
                                                        |
                                      +-----------------+------------------+
                                      |                                    |
                                      v                                    v
                              Paragraph engine                  Graphics backend session
                         (Parley or SkParagraph)              (Vello, Skia, or future)
                                                                          |
                                                                          v
                                                               Platform-neutral target
                                                                          |
                                                                          v
                                                                   Platform host
                                                          (Winit or future host)
```

The paragraph engine is shown beside the graphics session because layout needs
paragraph geometry before the final frame is painted. A backend profile binds a
compatible paragraph engine and graphics implementation; it does not make the
layout crate depend on the backend library.

Static site, SSR, and terminal continue down their own target-specific paths.

## 10. Fission-owned contracts

The following shapes are illustrative. This RFC approves responsibilities and
invariants, not final Rust spelling. Detailed APIs should remain internal until
the implementation has exercised them across both Vello and Skia.

### 10.1 Interactive frame

The frame compiler produces an immutable value similar to:

```rust
pub struct InteractiveFrame {
    pub frame_id: FrameId,
    pub viewport: FrameViewport,
    pub roots: Vec<FrameNode>,
    pub damage: DamageRegion,
    pub resources: ResourceSnapshot,
    pub external_surface_bindings: ExternalSurfaceBindings,
    pub semantics_epoch: SemanticsEpoch,
}
```

It must carry:

- stable node and layer identity;
- logical bounds and device-scale information;
- clips, transforms, opacity, blend, and isolation semantics;
- ordered paint operations;
- damage accumulated from retained changes;
- immutable resource identifiers and readiness state;
- external-surface slot nodes whose position in the ordered frame tree is the
  sole authority for bounds, z-order, clipping, transform, and opacity;
- an immutable side table that binds each slot ID to producer state and
  synchronization without repeating placement;
- provenance sufficient for diagnostics.

It must not carry `vello::Scene`, `wgpu::Texture`, `SkPicture`, `SkImage`, Winit
windows, or other backend objects.

The existing `RenderScene` and `DisplayList` should evolve into this model. A
second competing scene authority must not be introduced merely to stage the
refactor.

### 10.2 Paint semantics

Fission paint operations describe intent:

- geometry and paths;
- solid colors, gradients, images, and shaders exposed by Fission;
- fill, stroke, antialiasing, and blend intent;
- clips and transforms;
- isolated opacity and filter layers;
- paragraph placement;
- image and SVG placement;
- external-surface placement;
- cache and damage hints whose correctness does not depend on honoring them.

The contract must distinguish semantic requirements from optimization hints. A
backend may ignore a cache hint; it may not ignore a clip or blend operation it
claimed to support.

Backend-native escape hatches are not added to normal widget APIs. New
capability should first be expressed as a Fission semantic operation. A tightly
scoped experimental escape hatch, if ever needed, requires a separate design
because it can destroy portability and deterministic testing.

### 10.3 Paragraph engine

The paragraph engine accepts a normalized Fission description containing:

- UTF-8 text and style runs;
- locale and base direction;
- font families, weight, width, slant, size, variations, and features;
- letter and word spacing;
- line height and strut policy;
- width constraint, wrapping, alignment, maximum lines, and overflow;
- inline object placeholders;
- editing selection and preedit annotations where relevant;
- the active Fission font catalog and fallback policy.

It returns one immutable paragraph result containing:

- total and intrinsic dimensions;
- baselines and per-line metrics;
- positioned glyph or backend-owned draw data behind a Fission handle;
- grapheme, cluster, word, and bidirectional mapping;
- coordinate-to-text hit testing;
- caret stops and rectangles;
- selection rectangles;
- inline-object boxes;
- unresolved glyph diagnostics;
- a stable cache key tied to all shaping inputs.

The result is the geometry authority for layout, painting, editing, input, IME,
and accessibility. The default contract cannot return fabricated zeros for
required operations. A profile either implements the required paragraph
capabilities or fails validation.

The Vello profile may implement this with Parley and Fontique. The Skia profile
implements it with SkParagraph, SkShaper, HarfBuzz, SkUnicode, and SkFontMgr.
Fission's canonical editing buffer and transactions remain in
`fission-text-engine`; neither Parley nor SkParagraph becomes the owner of
editing state.

Different paragraph engines may produce different glyph positions, valid line
break choices, paragraph dimensions, and therefore some downstream layout.
Each must satisfy the shared constraint, valid-break, maximum-line, overflow,
content, and interaction invariants. Each profile must use its own paragraph
result consistently from measurement through presentation and keep its own
layout goldens stable.

### 10.4 Resource service

Fission owns logical resource identity and acquisition:

- asset, file, memory, data, and network source policy;
- request deduplication;
- source-byte caching;
- loading, ready, failed, and invalidated state;
- cancellation and frame wake-up;
- stable content identity;
- test substitution and diagnostics.

A backend owns derived objects:

- decoded images or SVG documents;
- typefaces and paragraph caches;
- uploaded textures and atlases;
- backend command or picture caches;
- GPU resource budgets and eviction.

Renderers must not independently fetch URLs or create a second global resource
authority. Source data is delivered through the Fission resource service. The
backend may decode lazily and report its derived-cache memory separately.

### 10.5 Graphics backend session

An interactive backend session must cover the full lifecycle, conceptually:

```rust
pub trait GraphicsBackendSession {
    fn capabilities(&self) -> &GraphicsCapabilities;
    fn attach(&mut self, target: &SurfaceTarget, size: PhysicalSize) -> Result<()>;
    fn resize(&mut self, size: PhysicalSize, scale: ScaleFactor) -> Result<()>;
    fn render(&mut self, frame: &InteractiveFrame) -> Result<RenderReport>;
    fn present(&mut self) -> Result<PresentReport>;
    fn readback(&mut self, request: ReadbackRequest) -> Result<Readback>;
    fn suspend(&mut self) -> Result<()>;
    fn resume(&mut self, target: &SurfaceTarget) -> Result<()>;
    fn recover(&mut self, loss: DeviceOrSurfaceLoss) -> Result<Recovery>;
    fn trim_memory(&mut self, pressure: MemoryPressure);
    fn diagnostics(&self) -> BackendDiagnostics;
}
```

The final API may combine render and present where a platform requires atomic
behavior. It may use generics internally where static dispatch materially helps
performance. The required point is lifecycle completeness without exposing a
particular GPU API.

`SurfaceTarget` is a Fission descriptor containing only the raw native-handle
or browser-canvas identity needed by a selected implementation, with explicit
ownership and thread-affinity rules. It does not contain Winit, wgpu, or Skia
objects.

### 10.6 Platform host

The platform host owns:

- process, application, activity, scene, window, and canvas lifecycle;
- event-loop integration and frame scheduling;
- viewport size, scale, safe areas, and system appearance;
- pointer, keyboard, touch, gesture, and gamepad transport;
- focus and IME transport;
- accessibility tree transport;
- clipboard, drag and drop, cursors, menus, and system dialogs;
- native child-view and external-surface attachment;
- platform capabilities and diagnostics.

Winit remains the initial host implementation. Renderer selection moves out of
Winit-specific control flow so the same host can drive Vello or Skia. A future
SDL, direct-native, browser-specific, or Fission-owned host can implement the
same Fission responsibilities without changing app code.

Host and backend may need a target-specific handshake to create a Metal layer,
Vulkan surface, Direct3D swap chain, OpenGL context, or browser canvas. That
handshake is isolated in the presenter boundary rather than leaking into core.

### 10.7 Compositor and retained layers

Fission owns layer semantics, ordering, clipping, transforms, opacity,
isolation, damage, and external-surface placement. The backend owns how those
semantics are cached and executed.

For example:

- Vello may retain scene fragments and composite wgpu textures;
- Skia may retain `SkPicture`, `SkImage`, display-list, or backend-texture
  objects internally;
- a software backend may repaint a bounded region into a CPU bitmap;
- a future renderer may use an entirely different retained representation.

Fission does not prescribe one cache shape. Cache keys are hints associated with
stable Fission identity, and cache eviction must never change output.

### 10.8 Accessibility and IME

Accessibility and IME are renderer-independent responsibilities built from
Fission semantics and paragraph geometry.

Native hosts continue to adapt the Fission semantics tree to platform
accessibility APIs. An interactive Web canvas backend requires a maintained
DOM/ARIA semantics mirror and a hidden native text-control bridge for browser
IME, selection, and assistive technology. Skia or CanvasKit does not provide
that integration automatically.

The platform adapter may consume paragraph caret and selection geometry, but it
must not query private Skia or Parley objects directly.

### 10.9 External surfaces, video, web content, and 3D

External content uses typed producers rather than an unstructured custom byte
payload. One `ExternalSurfaceSlot` in the ordered frame tree is the sole
placement authority. Its normal ancestors and fields determine logical bounds,
clipping, transform, opacity, and z-order. A producer or side table must not
repeat or override those values.

The `ExternalSurfaceBindings` side table is keyed by slot ID and describes only:

- producer kind and stable identity;
- color space and alpha semantics;
- current frame, interchange image, or native child-view handle;
- ownership and lifetime;
- acquire and release synchronization;
- damage and frame-readiness notification;
- whether zero-copy composition is available.

Skia can composite supplied images and some native GPU textures, but it does not
decode or schedule video and it is not a full 3D engine. The existing wgpu 3D
path may remain for a long time, including in applications whose 2D backend is
Skia. Cross-API texture sharing must be proven per target. CPU readback is a
diagnostic fallback, not the assumed production design.

The neutral Fission 3D scene/model produces work through an optional general-GPU
adapter. Initially that adapter contains the current wgpu renderer. Its output
enters the frame through an external-surface binding, so a future GPU
implementation can replace wgpu without changing the scene model or Skia/Vello
2D contracts.

Because Skia's Dawn integration and Rust wgpu are not automatically the same
device, the architecture cannot assume that selecting Graphite makes existing
wgpu textures directly shareable.

## 11. Backend capabilities and conformance

### 11.1 Capability model

Capabilities are structured Fission data, not stringly typed backend names.
They cover at least:

- CPU and GPU rendering modes;
- supported surface and color formats;
- text shaping, bidi, variable fonts, features, and inline objects;
- image formats and animated-image behavior;
- SVG profile;
- blend modes, filters, backdrop filters, and color spaces;
- readback and deterministic headless rendering;
- external-image import and synchronization modes;
- device-loss recovery;
- target and architecture support.

Application code should rarely branch on capabilities. Fission target
validation and built-in widgets use them to prove that a selected build profile
can honor its declared behavior.

A production graphical profile must implement the complete operation baseline
used by Fission's current built-in widgets. Capability rejection is available
for genuinely optional or future operations; it is not a route to label a
backend production-ready while omitting existing behavior. An existing
operation can leave the baseline only through an explicit Fission migration in
which all producers move to an equivalent supported semantic operation.

### 11.2 Conformance layers

Fission uses three different forms of conformance:

1. **Semantic conformance** is shared across backends: widget behavior,
   constraints, focus, actions, accessibility, editing transactions, ordering,
   clipping intent, and error behavior.
2. **Geometric conformance** holds backend-independent constraints, ordering,
   non-text geometry, damage, and external-surface alignment to shared
   invariants. Text-derived bounds, baselines, hit tests, carets, and downstream
   layout use profile-specific goldens plus the paragraph rules in section 3.4;
   they are not forced into a single cross-engine metric tolerance.
3. **Visual conformance** is backend-specific: screenshot and pixel-diff
   goldens catch regressions within one engine and platform profile.

A cross-backend screenshot comparison is useful for investigation, but it is
not a release requirement for identical pixels.

The initial operation inventory has explicit conformance cases for `Save`,
`Restore`, `ClipRect`, `ClipRoundedRect`, `OpacityLayer`, `Translate`,
`Transform`, `CachedScene`, `BackdropFilter`, `DrawRect`, `DrawText`,
`DrawRichText`, `DrawImage`, `DrawPath`, `DrawSvg`, and `DrawSurface`. The list is
generated or checked against the actual `DisplayOp` enum so adding an operation
cannot silently omit its backend disposition and tests.

### 11.3 Documentation consistency

Some existing architecture documents say that identical display lists must
produce identical pixels across different renderers, including
`docs/15-4-layout-and-display-list.md`,
`docs/18-1-adding-a-pure-rust-renderer.md`, and
`docs/18-2-backwards-compatibility-strategy.md`.

This RFC replaces that cross-renderer requirement. Pinned inputs and a pinned
backend profile must remain deterministic within the tolerances of that
profile. Different backends must preserve semantic and backend-independent
geometric contracts, follow the paragraph-layout rules in section 3.4, and meet
their profile-specific layout and perceptual goldens; they are not required to
rasterize the same pixels or produce identical text metrics. Those documents
must be updated when the multi-backend contract is implemented so they do not
state an impossible compatibility guarantee.

### 11.4 Backend identity

Backend identity is exposed in diagnostics, build reports, crash reports, and
test artifacts. It is not used as a substitute for capabilities in core logic.

## 12. Production Skia implementation

### 12.1 Crate shape

The intended implementation units are:

```text
crates/rendering/fission-skia-sys
crates/rendering/fission-render-skia
```

`fission-skia-sys` owns the pinned Skia source revision, C++ bridge, artifact
selection, linking, and low-level ABI declarations. It is the only crate that
touches Skia C or C++ headers directly and declares one Cargo native-link
authority with `links = "fission_skia"`.

`fission-render-skia` owns safe Rust RAII wrappers, conversion from Fission
frames and paragraph descriptions, cache policy, surface lifecycle, diagnostics,
and the implementation of Fission backend contracts.

The exact crate split is created when code requires it; the architecture does
not require empty placeholder crates.

### 12.2 Direct Skia integration

Fission will interface directly with Skia rather than adopting another Rust
wrapper as an architectural dependency. On native targets, Skia has no
supported stable C API, so “direct” means a small Fission-owned C++ bridge
exposing an `extern "C"` ABI to Rust. It does not mean binding arbitrary C++
headers with layout-dependent types. On Web, it means Fission-owned bindings to
the exact Fission-built CanvasKit module as described in section 12.6.

The ABI follows these rules:

- opaque handles for Skia-owned objects;
- fixed-width Fission-owned POD structs and integer values;
- explicit create, clone where required, release, and destruction operations;
- explicit thread ownership and affinity;
- status codes plus retrievable structured diagnostics;
- ABI version, pinned Skia revision, build profile, and feature bits;
- no STL containers, exceptions, RTTI contracts, `sk_sp`, or Skia enum layout
  across the boundary;
- batched paths, glyph data, paint operations, and resource updates rather than
  one FFI call for every primitive;
- tests for invalid handles, lifetime, panic/error translation, and concurrent
  misuse.

The bridge is deliberately narrower than Skia. New functions are added only to
implement a Fission contract or a measured performance requirement.

### 12.3 Native rendering profile

The initial reference native profile contains:

- raster `SkSurface` and `SkCanvas` for optimized software rendering;
- Ganesh for production GPU rendering;
- platform GPU integrations appropriate to macOS, iOS, Windows, Linux, and
  Android;
- SkParagraph, SkShaper, HarfBuzz, SkUnicode with ICU, and SkFontMgr;
- platform font managers plus Fission-packaged fonts;
- selected SkCodec decoders required by Fission's image contract;
- SkSVGDOM for the declared Fission SVG profile;
- filters, shaders, blend modes, color spaces, and readback required by the
  Fission paint contract.

Ganesh is the initial production GPU path because it has the broadest mature
platform coverage. Graphite is continuously built and qualified behind the same
Fission ABI, but it is not the initial default. Moving a platform or the whole
backend to Graphite is a measured implementation decision that does not alter
the public Fission contracts.

### 12.4 Text and fonts

SkParagraph is used as the foundation of a complete Fission paragraph
implementation, not solely as a glyph painter. The backend must implement
shaping, wrapping, line metrics, selection rectangles, hit testing, caret
geometry, inline placeholders, font fallback, locale and direction, variable
fonts, font features, and unresolved-glyph diagnostics required by the Fission
paragraph contract.

SkParagraph exposes relevant geometry and editing queries, but it is not itself
Fission's canonical editing result. The adapter must normalize its mixed UTF-8
and UTF-16 indexing, derive stable grapheme-aware caret stops and rectangles,
and implement visual bidirectional navigation in the Fission paragraph result.
Those derived values are tested together with selection and hit-test geometry.

Native system fonts are resolved through the appropriate SkFontMgr
implementation. Application and framework fonts enter through the Fission font
catalog as bytes with stable identities. Tests must cover Latin, Arabic,
Devanagari, CJK, emoji, combining marks, bidirectional text, variable fonts,
fallback, and missing glyphs.

### 12.5 Images, SVG, effects, and color

The Skia backend uses SkCodec and SkImage for runtime decode and image objects,
subject to the explicitly built codec profile. The presence of an upstream enum
value is not treated as proof that a codec is enabled.

SkSVGDOM implements Fission's tested SVG profile. Fission does not claim full
browser SVG, scripting, animation, `foreignObject`, or arbitrary CSS behavior
without corpus evidence.

Blend modes, filters, backdrop filters, clipping, color spaces, and wide-gamut
behavior are exercised through product examples and backend-specific goldens.
Where Skia supports a materially better operation, Fission may extend its paint
semantics rather than artificially matching a current Vello limitation.

### 12.6 Web and CanvasKit

Interactive Web is part of the production Skia implementation, not a later
demo. Fission will build and version its own CanvasKit profile so compiled
features, codecs, text, fonts, WebGL/WebGPU choice, and memory settings match
the framework contract.

The current Rust Web target and CanvasKit use different Wasm toolchain models.
The initial integration may run CanvasKit as a second Wasm module controlled by
Fission's JavaScript shell. If so, Rust sends batched frame and resource updates
through a compact Fission protocol. It must not make thousands of
Rust-Wasm-to-JavaScript-to-CanvasKit calls per frame.

This is an intentional transport exception to the native C ABI, not a second
rendering authority. `fission-skia-sys` owns the low-level CanvasKit artifact,
bindings, and protocol; `fission-render-skia` remains the only Rust-facing safe
backend. Native and Web consume the same Fission frame, resource, paragraph,
capability, lifecycle, and conformance contracts. CanvasKit-specific objects or
JavaScript APIs cannot leak above that adapter.

A combined Emscripten build or another single-module topology may replace this
later if it materially improves startup, size, debugging, or performance. The
wire representation remains an internal Fission implementation detail until
there is evidence that third-party stability is useful.

The Web profile must include:

- hardware rendering with the chosen supported browser API;
- optimized software fallback;
- explicit destruction of CanvasKit objects;
- Fission-managed font download or embedding and deterministic fallback;
- resource and module caching through the Fission Web shell;
- resize, device-pixel-ratio, context-loss, suspend, and resume behavior;
- a DOM/ARIA semantics mirror and browser IME bridge;
- download size, compile/startup time, Wasm memory, frame time, and bridge-cost
  measurements.

Standard CanvasKit builds are not assumed to match Fission's native profile.
Fission's custom build and its tests define the actual Web capability set.

In particular, current CanvasKit production exports do not include SkSVGDOM in
the way the native Skia build does, and PDF is disabled. Fission must either add
and maintain the required SkSVGDOM sources, Expat configuration, and Web
bindings, or keep SVG on a backend-neutral lowering path that the CanvasKit
renderer consumes. Merely enabling a nominal Skia build flag is not accepted as
Web SVG support.

If the Web profile uses Graphite through Dawn, GPU completion depends on browser
event-loop progress. The implementation must choose and test an explicit
browser tick/Asyncify strategy or a fully asynchronous readback and teardown
design. It must not perform a forbidden synchronous CPU wait, and it must prove
GPU completion before destroying the Graphite context.

### 12.7 Software rendering

Skia raster is a supported backend mode, not only a last-resort debugging path.
It must support deterministic headless tests, screenshots, GPU-unavailable
systems, remote or virtualized environments, and explicit user selection.

Software presentation is still target-specific. Native hosts upload or present
the raster buffer efficiently; the browser profile transfers or paints it
without unnecessary per-frame copies where practical.

### 12.8 Lifecycle and recovery

The Skia implementation is incomplete until it handles:

- initial attachment and first frame;
- resize and device-scale changes;
- window occlusion and zero-size surfaces;
- mobile suspend, resume, and context recreation;
- browser context loss;
- recoverable surface errors;
- unrecoverable device errors with clear diagnostics;
- screenshot/readback;
- explicit cache trim on memory pressure;
- clean shutdown without leaked GPU, Wasm, or C++ objects.

## 13. Native build and artifact distribution

### 13.1 Default experience

The normal Fission developer does not build Skia from source. Fission publishes
versioned native artifacts for supported target triples and profiles. The
`fission-skia-sys` build selects the matching artifact, verifies its
cryptographic digest against the immutable manifest published in the crate,
verifies the manifest's Fission release signature or provenance attestation,
and only then links it. The release pipeline produces that signed/attested
manifest from the same build that produced the archive.

Artifacts are built by Fission's release pipeline from the exact pinned Skia
revision, bridge source, toolchain, and GN arguments. Release provenance and
license material are retained with the build. The `.crate` contains Rust and
bridge source plus selection metadata, not every platform archive; crates.io's
package size limit makes a single universal binary crate unsuitable.

Each release profile is self-contained for its declared deployment baseline.
It either builds required third-party libraries into the artifact by selecting
the corresponding non-system Skia dependencies, or packages and versions every
required dynamic library. The manifest pins the C/C++ runtime, minimum OS,
SDK/NDK or CRT, and Linux libc baseline as applicable. A generic target triple
alone is not treated as proof that an archive is portable.

The artifact cache key includes the Fission crate version, pinned Skia
revision, bridge ABI, target, profile, toolchain/runtime ABI, deployment
baseline, and digest. Offline mode uses only an exact verified cache entry or an
explicit local/vendor override. A missing, corrupt, stale, or merely compatible
artifact fails with an actionable diagnostic; it is never substituted with a
different profile or an unverified download.

The initial artifact host may be GitHub Releases. The artifact URL is an
implementation detail and may later move without changing the Rust API.

### 13.2 Source build

An explicit feature, provisionally `skia-build-from-source`, builds the pinned
source with Fission's supported GN and Ninja configuration. It exists for new
targets, downstream patching, distribution policies, and backend development.

The source path must use the same feature manifest and ABI tests as the prebuilt
artifact. It is not allowed to drift into a subtly different backend.

The build also supports an explicit local artifact/source override for offline
and distribution builds. It must never silently fall back from a missing
prebuilt artifact to a lengthy network source build.

### 13.3 Profiles

Fission publishes a small, intentional profile matrix rather than every
possible Skia GN combination. At minimum the design distinguishes:

- native raster;
- native Ganesh plus raster fallback;
- native Graphite qualification;
- CanvasKit production;
- CanvasKit software qualification.

Codec, SVG, paragraph, Unicode, and font features are part of a profile's
manifest. Applications should not accidentally combine incompatible archives
through independent Cargo features.

The release matrix covers the architectures and deployment variants Fission
actually supports. It distinguishes, where applicable, Windows MSVC
architectures, macOS architectures, iOS device and simulator slices, Android
ABIs/NDK levels, and Linux libc/architecture combinations. An unbuilt
combination fails explicitly or uses the opt-in source build; it never selects
a merely similar archive.

### 13.4 Version and roll policy

Fission pins an exact Skia revision. Rolls are reviewed changes with ABI,
conformance, visual, memory, size, and performance results. Fission does not
consume Skia tip-of-tree implicitly.

The C ABI has its own version and feature query so a mismatched archive fails
with an actionable build error rather than undefined behavior.

## 14. Existing Rust backend strategy

### 14.1 Refactor, do not freeze

The Vello/wgpu/Parley backend is the first consumer of the new contracts. Moving
it behind a boundary must preserve behavior and create room to improve it. The
refactor is not a code freeze and is not preparation for automatic deletion.

The current Fission forks of Vello, Winit, and AccessKit integration demonstrate
that Fission is already willing to carry targeted ecosystem work. That can
continue when the maintenance cost and wider value justify it.

### 14.2 Community outcomes

Several outcomes are valid:

- Skia becomes the default while the Rust backend remains supported;
- the Rust backend remains the default and Skia serves demanding workloads;
- defaults differ by platform or deployment profile;
- both backends remain equal supported choices;
- Rust ecosystem improvements close gaps and reduce the need for Skia;
- Fission replaces one component, such as text or presentation, while retaining
  others;
- Fission eventually implements an owned component where neither stack meets
  the contract.

This RFC deliberately preserves those options.

### 14.3 Removal policy

Removing a current backend dependency requires a later decision based on:

- supported-target and feature coverage;
- real application compatibility;
- visual and text quality;
- accessibility and IME correctness;
- performance, memory, and binary evidence;
- packaging and operational reliability;
- maintenance cost and upstream/community health;
- migration impact for downstream users.

Completing the replaceability refactor or shipping Skia is not, by itself,
authorization to remove anything.

## 15. Winit, wgpu, and future hosts

### 15.1 Winit

Winit is a platform-host component, not a Vello component. The first refactor
keeps Winit and makes it capable of hosting either graphics backend.

Longer term, Fission may qualify SDL, direct platform implementations, browser
APIs, or an owned event/window layer. The host contract must therefore avoid
Winit types above the adapter, but this RFC does not schedule a Winit
replacement.

### 15.2 wgpu

wgpu currently serves more than Vello. It participates in presentation,
composition, WebGPU, and Fission's 3D renderer. A Skia 2D backend therefore does
not imply immediate removal of wgpu.

The refactor separates these roles so Fission can later choose among:

- wgpu retained for 3D and external texture production;
- native Metal, Vulkan, or Direct3D integration shared with Skia;
- Graphite through Dawn;
- another cross-platform GPU layer;
- an owned, focused GPU abstraction;
- different choices by platform.

Any claim of zero-copy interoperation must include explicit ownership,
synchronization, format, color-space, and device-loss tests on each platform.

### 15.3 No hidden replacement schedule

Winit and wgpu may remain indefinitely. Making them replaceable reduces
architectural risk and permits better choices; it is not a promise to exercise
every option immediately.

## 16. Memory, binary size, and performance

### 16.1 Measurement principle

Dependency-level anecdotes do not decide framework architecture. Fission will
measure complete application artifacts with exactly one selected backend, the
same app content, the same fonts and assets, comparable optimization settings,
and equivalent capabilities.

During development, multi-backend binaries are useful for diagnosis but are not
used for product size or steady-state memory comparisons.

### 16.2 Memory

Skia has configurable caches whose upstream defaults can be large for a UI
framework. Current upstream reference values include a 32 MiB CPU resource
cache, a 2 MiB and 2,048-entry global strike cache, a 256 MiB Ganesh GPU cache,
a 256 MiB Graphite context budget, and an independent 256 MiB default budget
for every Graphite recorder. These are ceilings rather than immediate
allocations, but Fission must not accept them blindly.

Fission defines desktop, mobile, Web, and constrained-device budgets, reports
actual cache use, trims on memory pressure, and avoids creating multiple
independently budgeted contexts or recorders without need. A 3840 x 2160 RGBA
buffer alone is approximately 31.6 MiB before buffering, layers, decoded
images, glyph caches, and driver allocation.

Measurements include:

- cold process resident memory before the first frame;
- first-frame and warm idle resident memory;
- CPU source and decoded-resource caches;
- paragraph, glyph, and font caches;
- GPU allocations and cache limits;
- peak memory during scroll, animation, filtering, image decode, and resize;
- memory after pressure notification and cache trim;
- Web Wasm linear-memory commit and growth;
- leaks across repeated create, suspend, resume, and destroy cycles.

### 16.3 Binary and download size

Skia is expected to add several MiB in a focused native configuration, and a
standard CanvasKit Wasm artifact is several MiB before application code and
fonts. Those observations are scale indicators, not Fission results.

The fair number is the net application difference after excluding components a
single-backend Skia build no longer needs, including duplicate raster, image,
text, Unicode, GPU, or font machinery where applicable.

Measurements include:

- stripped native executable/package size per target;
- packaged dynamic libraries and platform frameworks;
- debug-symbol size reported separately;
- compressed and uncompressed Web Wasm, JavaScript, fonts, and support assets;
- cold download and warm-cache transfer;
- package-store and build-cache footprint;
- incremental and clean build time.

### 16.4 Runtime performance

The qualification suite records at least:

- process startup, backend initialization, and first-contentful frame;
- p50, p95, and p99 input-to-present latency;
- p50, p95, and p99 frame build, paragraph, render, and present time;
- retained-scene and damage effectiveness;
- scroll, resize, animation, opacity, filter, image, SVG, and text workloads;
- large lists, editor documents, charts, and complex product pages;
- CPU and GPU utilization at idle and under load;
- browser bridge serialization and call cost;
- surface-loss and recovery duration;
- screenshot/readback performance where supported.

Quality regressions cannot be traded for speed without an explicit decision.

### 16.5 Frozen qualification budgets

Before product qualification begins, Fission checks in a qualification manifest
that fixes:

- Linux, macOS, Windows, Android, iOS, and interactive Web target variants;
- the required browser and device/driver matrix, including Chromium, Firefox,
  and Safari/WebKit families unless a pre-existing support policy explicitly
  excludes one;
- benchmark applications, scenes, fonts, assets, build profiles, and toolchains;
- numeric ceilings for cold/warm/peak process memory and observable GPU memory;
- stripped native package and compressed/uncompressed Web download size;
- startup and first-contentful-frame time;
- p50, p95, and p99 frame and input-to-present latency;
- recovery, teardown, and idle-use requirements.

The budgets are target-specific and are based on current Fission baselines plus
the product requirements for that target; Skia is not required to beat Vello on
every metric. Once qualification runs begin, a budget or matrix entry cannot be
relaxed merely to turn a failure green. A material change requires an explicit
review with the reason, the old and new values, and their product consequence.

Missing data or a missed required budget means that profile is not production
qualified. It may remain available as an experimental profile, but cannot be
called production-ready or become the default for that target.

## 17. Migration plan

The phases are capability milestones. They do not imply a date for removing any
dependency.

There is deliberately no “remove Vello”, “remove Parley”, “remove wgpu”, or
“remove Winit” phase.

### Phase 0: Baselines and contract inventory

- Record representative application, widget, text, Web, mobile, 3D, video, and
  external-surface workloads.
- Capture current backend-specific visual goldens and semantic snapshots.
- Record size, memory, startup, frame, and recovery baselines.
- Freeze the required native target/browser matrix and the target-specific
  qualification budgets described in section 16.5 before comparative
  qualification begins.
- Classify current shell responsibilities by frame compiler, graphics backend,
  presenter, host, resources, accessibility, and external producer.
- Make missing capability behavior explicit, especially current silent or
  placeholder paths.

Exit gate: Fission can state what behavior is being preserved, how it is
measured, and the budgets a production profile must meet.

### Phase 1: Extract Fission-owned contracts through the current backend

- Evolve the existing render scene into the interactive frame authority.
- Extract presentation lifecycle from Winit-specific renderer selection.
- Introduce complete graphics-session, capability, resource, and diagnostic
  contracts.
- Introduce the paragraph result required by ADR 0001.
- Split neutral 3D scene/model and producer contracts from the current optional
  wgpu renderer implementation.
- Route Vello, wgpu, Parley, transitional software presentation, and Winit
  through those contracts.
- Keep static, SSR, and terminal dependency graphs unchanged.

Exit gate: the existing backend passes its prior functional, visual, text,
accessibility, platform, and performance checks with no dependency type leaking
through the new core boundaries.

### Phase 2: Build the production Skia foundation

- Add `fission-skia-sys` and its versioned C ABI.
- Establish pinned source and reproducible native artifact builds.
- Add safe Rust ownership and thread-affinity wrappers.
- Implement raster surfaces, basic frame execution, resources, readback,
  diagnostics, and deterministic teardown.
- Bring up SkParagraph and the full Fission paragraph result early so layout and
  paint develop against one geometry authority.

Exit gate: the foundation is supportable production code with ABI, ownership,
artifact, error, and lifecycle tests. A shapes demo alone does not satisfy it.

### Phase 3: Complete native Skia behavior

- Implement Ganesh presentation on macOS, Windows, Linux, iOS, and Android.
- Complete text, fonts, images, SVG, filters, color, retained layers, damage,
  cache policy, screenshots, and the production Skia raster path.
- Replace every native production selection and fallback path that uses the old
  software renderer with Skia raster and its paired SkParagraph profile.
- Implement suspend/resume, resize, scale change, memory pressure, surface loss,
  and device-loss behavior.
- Integrate accessibility, IME geometry, video, web views, and 3D external
  surfaces through Fission contracts.
- Continuously qualify Graphite without making it the default.

Exit gate: representative Fission applications are fully usable on supported
native targets without backend-specific application code.

### Phase 4: Complete interactive Web Skia behavior

- Produce the Fission CanvasKit profile and release artifacts.
- Implement batched frame/resource transfer and lifecycle management.
- Complete Web text/font loading, images, the declared Web SVG path, CanvasKit
  software rendering, semantics mirror, IME bridge, resize, context loss, and
  caching.
- Remove the old Canvas 2D software-buffer renderer from production selection
  after CanvasKit software conformance passes.
- Exercise the chosen Graphite/Dawn event-loop, asynchronous readback, and
  teardown rules when that qualification lane is enabled.
- Qualify WebGL/Ganesh and the selected Graphite/WebGPU lane separately.

Exit gate: representative applications meet Fission's Web correctness,
accessibility, startup, download, memory, and frame-time requirements.

### Phase 5: Product qualification

- Run semantic conformance and backend-specific visual suites.
- Run the same complex applications against single-backend builds.
- Publish the measured feature, target, quality, memory, size, build, and
  performance comparison.
- Exercise real development, packaging, CI, release, offline, and source-build
  workflows.
- Resolve any architecture leaks discovered by full implementations rather than
  documenting them as permanent backend exceptions.

Exit gate: Fission has enough evidence to support Skia long term and to make a
separate default-backend decision.

### Phase 6: Default selection, if warranted

A later RFC may choose a default globally or by target. It may also choose to
keep the current default. Selecting a default does not imply removal of another
backend.

## 18. Validation and acceptance criteria

### 18.1 Replaceability acceptance

The architectural refactor is complete when:

- core, layout, semantics, and authoring crates contain no direct dependency on
  Skia, Vello, wgpu, Winit, or Parley;
- the neutral 3D model and producer contract expose no wgpu type; the current
  wgpu 3D renderer is an optional backend implementation;
- the Winit host can run at least the Vello and Skia graphics sessions without
  duplicating app runtime logic;
- feature-matrix builds pass for Vello-only, Skia-only, both-backend diagnostic,
  standalone-software, and Skia-plus-3D profiles;
- a Skia-only 2D application does not link Vello, Parley, or wgpu; wgpu remains
  permitted when 3D or another explicitly selected wgpu backend is enabled;
- a Vello-only application does not link Skia, and a standalone-software
  application links the focused Skia raster profile without Vello, Parley, or
  wgpu; any platform dependency used solely to upload or present its raster
  buffer is reported separately;
- `fission-skia-sys` is the only crate that directly touches Skia's C/C++ ABI,
  and safe-wrapper tests cover ownership, destruction, errors, and thread
  affinity;
- `fission-skia-sys` also owns the direct CanvasKit artifact/bindings on Web;
  the transport may differ from native, but there is no second Rust-facing
  frame, resource, capability, or lifecycle authority;
- the production Skia profile implements every current `DisplayOp`, including
  cached scenes, backdrop filters, and external surfaces, or the operation has
  been explicitly replaced in the Fission contract and all producers have
  migrated; preview milestones may reject unfinished operations with provenance,
  but no operation silently disappears;
- paragraph measurement, paint, hit testing, selection, caret, and IME consume
  one result per backend profile;
- resource acquisition is independent of renderer caches;
- external surfaces use typed lifecycle and synchronization contracts;
- backend capability gaps fail explicitly;
- static, SSR, and terminal builds do not acquire interactive graphics
  dependencies;
- adding a test backend does not require changes to app widgets or core layout
  semantics;
- the required Vello suites do not regress, and the migrated Skia raster path
  passes the prior software-renderer behavior and platform suites before the
  old production path is removed; no Rust-native backend is marked deprecated
  as a consequence of this RFC.

### 18.2 Skia production acceptance

Skia is ready for a fair keep/default decision when:

- Linux, macOS, Windows, Android, iOS, and interactive Web satisfy the
  production qualification requirements in the frozen matrix;
- the native target and browser matrix frozen under section 16.5 has not been
  narrowed after seeing results without an explicit amendment;
- raster and GPU paths run representative Fission applications;
- text editing, selection, caret, hit testing, bidi, complex scripts, emoji,
  fallback, and IME pass the shared contract suite;
- images, the declared SVG profile, filters, color, clipping, transforms, and
  retained layers pass backend goldens;
- accessibility works through native adapters and the Web semantics mirror;
- ordinary pointer/mouse, wheel, touch, keyboard, focus traversal, hit testing,
  viewport scale, and DPI-change suites pass through the shared host path on
  every declared target;
- Android accessibility and IME suites pass through an implemented platform
  bridge rather than the current no-op, and each declared Web browser passes
  DOM/ARIA, focus, keyboard, pointer, and hidden-text-control IME suites;
- resize, scale change, suspend/resume, context loss, device loss, memory
  pressure, and teardown are exercised;
- external video, web, and 3D surfaces remain aligned during scroll, transform,
  clipping, and resize;
- prebuilt and source build workflows are reproducible on supported targets;
- every prebuilt native artifact passes digest, release provenance, ABI,
  deployment-baseline, cache/offline, and local/vendor-override tests before it
  can be linked or published;
- any required workspace, backend, profile, target, or platform-suite failure
  blocks the entire Fission release containing this work; an experimental
  profile may be outside the supported matrix, but every test required by the
  behavior actually shipped and advertised must pass;
- measured memory, binary size, startup, latency, and frame performance are
  available for equivalent Vello-only, Skia-only, both-backend diagnostic, and
  Skia-plus-3D builds;
- every production Skia profile meets its frozen memory, package/download size,
  startup, frame-time, input-latency, and recovery budgets;
- no application code needs Skia-specific setup for ordinary Fission behavior.

### 18.3 Release states

Individual milestones may ship as experimental or preview features while the
implementation matures. Those labels describe support level; they do not turn
the work into disposable prototype code. Every shipped milestone follows the
long-term architecture, has ownership and cleanup semantics, and carries tests
appropriate to its exposed behavior.

## 19. Risks and mitigations

### 19.1 The abstraction becomes a lowest common denominator

**Risk:** Fission exposes only operations implemented identically by Vello and
Skia, preventing progress.

**Mitigation:** define semantic operations from product requirements, expose
capabilities, and permit backend-specific visual realization. Unsupported
profiles fail honestly.

### 19.2 Text layout becomes split-brain

**Risk:** layout measures with one engine while paint or IME uses another.

**Mitigation:** the immutable paragraph result is mandatory and is shared by
layout, paint, hit testing, selection, caret, and IME within a profile.

### 19.3 The C ABI grows into a second Skia API

**Risk:** the bridge mirrors Skia and inherits its churn and ownership hazards.

**Mitigation:** expose only Fission contracts, use opaque handles and batched
data, and require a concrete Fission caller for every ABI addition.

### 19.4 Native artifacts become an operational burden

**Risk:** target/profile combinations, toolchains, licenses, and security rolls
make releases unreliable.

**Mitigation:** keep a small profile matrix, pin inputs, automate build and ABI
tests, retain source-build parity, and fail clearly when no supported artifact
exists.

### 19.5 Web bridge cost erases rendering gains

**Risk:** two Wasm modules and JavaScript crossings inflate startup and frame
cost.

**Mitigation:** batch immutable frame/resource updates, measure serialization
separately, and retain the option to move to a combined toolchain build.

### 19.6 Transition builds duplicate memory and code

**Risk:** carrying both stacks makes comparisons and applications appear much
larger.

**Mitigation:** normal builds select one backend; only diagnostic builds link
both. Report net single-backend artifacts.

### 19.7 External GPU interoperation is not portable

**Risk:** video or 3D textures require copies or cannot synchronize with Skia.

**Mitigation:** define explicit interchange and synchronization, qualify it per
platform, preserve wgpu where useful, and never assume Dawn and wgpu devices are
interchangeable.

### 19.8 Work fragments the ecosystem

**Risk:** investing in Skia reduces effort available for Rust-native projects.

**Mitigation:** keep the current backend first-class, upstream separable fixes,
publish precise missing capabilities, and let evidence and community health
inform later investment.

### 19.9 Production scope delays comparison

**Risk:** a fair Skia implementation takes substantially longer than a demo.

**Mitigation:** use independently valuable milestones and continuous metrics,
but do not draw final conclusions from incomplete integration. The cost of
doing enough engineering is part of the comparison.

## 20. Alternatives considered

### 20.1 Add Skia behind the current `Renderer::render_scene` trait

Rejected as the complete solution. It can draw a frame but leaves text geometry,
surface lifecycle, presentation, resources, recovery, external surfaces, and
host coupling unresolved.

### 20.2 Build a small Skia prototype and decide immediately

Rejected. A shallow prototype systematically disadvantages Skia by excluding
the integration and optimization work needed to use its mature subsystems. It
would compare a production Rust stack against demo-quality Skia glue.

### 20.3 Replace Vello, Parley, wgpu, and Winit immediately

Rejected. The dependencies serve different responsibilities, Skia does not
replace all of them, and removal before qualification would increase risk while
discarding useful Rust ecosystem investment.

### 20.4 Standardize on Skia types throughout Fission

Rejected. It would make Skia easy to use once and expensive to replace forever,
and would couple non-graphical targets to an interactive graphics library.

### 20.5 Use a third-party Rust Skia wrapper

Rejected for the backend foundation. Fission needs a deliberately narrow,
versioned ABI, exact build profiles, native artifact delivery, and ownership of
the contract. Another wrapper's API and release policy would become an
additional architectural dependency.

### 20.6 Generate Rust bindings directly over Skia C++ APIs

Rejected. C++ ABI, templates, smart pointers, exceptions, compiler settings,
and upstream churn must not cross into safe Rust or public Fission types.

### 20.7 Force pixel parity across all backends

Rejected. Different text rasterizers, GPU pipelines, and effect implementations
will differ. Semantic correctness and backend-specific visual excellence are
the useful guarantees.

### 20.8 Link every backend and select at runtime

Rejected as the default. It distorts binary and memory costs and makes normal
applications pay for unused native stacks. Opt-in diagnostic builds can still
do this.

### 20.9 Build Skia from source for every application

Rejected as the default developer experience. It adds a large C++ toolchain and
build cost to ordinary Rust applications. Reproducible source builds remain an
explicit supported option.

### 20.10 Use Skia for static, SSR, and terminal output

Rejected. Those targets have native semantic output formats and do not benefit
from acquiring a graphical dependency.

## 21. Provisional Skia and Vello comparison

This table records the reason to perform the work, not a preordained winner.
“Fission result” means evidence that must come from the production
implementation and qualification suite.

| Property | Skia | Current Vello-centered stack | Fission implication |
|---|---|---|---|
| Maturity | Long-lived production engine used across major products | Younger Rust-native renderer with rapidly evolving APIs | Skia reduces feature-maturity risk; Vello offers a smaller, more influenceable ecosystem |
| 2D feature breadth | Broad paths, clips, blends, filters, layers, color, images, and effects | Strong modern vector pipeline, with gaps Fission has encountered as requirements expanded | Do not constrain Skia to Vello's current feature set |
| GPU architecture | Ganesh is mature; Graphite is the future-facing path and still evolving | Vello is designed around compute-oriented GPU rendering through wgpu | Qualify Ganesh first and Graphite continuously; retain Vello's architectural advantages where they are real |
| Software rendering | Mature optimized raster pipeline in the same graphics system | Fission currently uses a separate software implementation | Skia can reduce duplicated behavior if its end-to-end results qualify |
| Text | SkParagraph + SkShaper + HarfBuzz + Unicode + platform font managers | Parley + Fontique, integrated with Fission's Vello renderer | Both must implement one Fission paragraph contract; Skia offers a more vertically integrated stack |
| Images and codecs | Mature decode, color conversion, animated metadata, and GPU integration according to build profile | Separate Rust image/resource path and renderer upload | Skia may consolidate backend-derived resources; Fission remains the source-data authority |
| SVG | SkSVGDOM provides a substantial tested subset, not a browser | Fission currently lowers SVG through its existing render path and supporting libraries | Define and test a Fission SVG profile rather than claiming arbitrary browser parity |
| Native platforms | Mature integrations for the graphics APIs used on Fission targets | wgpu/Winit provide broad Rust-native portability | Both need Fission-owned host/presenter boundaries and per-platform recovery tests |
| Interactive Web | CanvasKit is mature but introduces Emscripten/Wasm integration and font/resource work | Current Fission Web path uses Vello/WebGPU with Canvas 2D software fallback | Skia Web is a full product stream, not proof that native success automatically transfers |
| Windowing/input | Not provided | Winit and Fission shell integration provide it | Skia does not replace Winit; host replaceability is a separate long-term option |
| Accessibility/IME | Not provided as a Fission integration | Fission + Winit/AccessKit/browser glue provide it | Keep semantics and IME in Fission-owned host contracts |
| General 3D | Not a replacement for Fission's 3D engine | Fission currently uses wgpu directly | wgpu may remain with either 2D backend; prove texture interchange rather than assuming it |
| Video/web surfaces | Can composite supplied content but does not own decode, clocks, or native web views | Current shell has dedicated integrations around the wgpu/Vello compositor | Move lifecycle and synchronization into typed external-surface contracts |
| Rust safety and contribution path | Requires a Fission-owned C++ ABI and unsafe boundary | Rust-native stack is easier for Rust contributors to inspect and improve | Treat bridge safety and artifact provenance as product responsibilities; keep investing in Rust where useful |
| Build complexity | Large C++20 GN/Ninja build with target-specific GPU options | Mostly Cargo plus shader/toolchain requirements and maintained forks | Ship tested prebuilt Skia artifacts by default and keep source builds opt-in |
| Native binary size | Expected to be several MiB in a focused build; exact net cost is profile- and target-dependent | Existing cost is distributed across Vello, wgpu, Parley, software raster, image, and related code | Compare stripped single-backend applications after unused dependencies are excluded |
| Web download size | CanvasKit adds a several-MiB Wasm/JS payload before fonts and app code | Current app already carries Rust Wasm and renderer dependencies | Measure compressed total, startup, caching, and capability-equivalent builds |
| Memory | Mature configurable caches, with upstream defaults too generous for some Fission targets | Multiple Rust-side renderer, text, image, and GPU caches with different policies | Fission must own budgets and diagnostics; compare cold, warm, peak, GPU, and post-trim memory |
| Rendering differences | Established rasterization and platform behavior | Different antialiasing, text, filter, and GPU behavior | Differences are allowed; semantic conformance and per-backend visual quality are required |
| Ecosystem control | Broad upstream project, but Fission is a consumer with a maintained bridge | Smaller Rust projects that Fission can fork, influence, and improve directly | Maintain optionality; community momentum may make the Rust backend strategically preferable |
| Long-term role | Obvious candidate for a reference backend | Current production backend and a credible long-term implementation | Decide defaults only after full qualification; neither is scheduled for removal |

## 22. Open questions

These questions do not block the replaceability refactor or the production Skia
programme:

- Which backend, if any, becomes the default globally or per target?
- Does Graphite become preferable to Ganesh, and on which platforms?
- Which native GPU API is used for each initial Ganesh target profile?
- Is the long-term Web topology two Wasm modules, one Emscripten module, or
  another integration?
- What is the smallest supported native artifact profile matrix?
- Which codecs and exact SVG features belong to Fission's guaranteed profile?
- Which external-texture paths can be zero-copy on each supported platform?
- Should third-party backend contracts eventually become stable public APIs, or
  remain versioned internal integration points?
- Under what evidence would Fission build its own renderer, text engine, GPU
  layer, or platform host?
- How should project and community health be weighted alongside technical
  measurements in a future default-backend decision?

## 23. Decision summary

Fission will become genuinely multi-backend by owning the semantic boundaries
between applications, frames, text, resources, graphics sessions, presenters,
platform hosts, and external surfaces.

The near-term architectural commitment is replaceability. The near- and
medium-term implementation commitment is a complete, long-lived Skia backend
that earns a fair comparison through production engineering.

Removing Vello, wgpu, Parley, Winit, or other Rust ecosystem components is a
separate, longer-term question and may never be desirable. Fission can support
and improve them while also shipping Skia. If neither direction ultimately
meets the framework's needs, the same boundaries leave room for Fission to build
the missing components itself.

## References

- Skia build requirements and GN/Ninja workflow: https://skia.org/docs/user/build/
- Skia release policy: https://skia.org/docs/user/release/
- SkCanvas and GPU surface integration: https://skia.org/docs/user/api/skcanvas_creation/
- CanvasKit overview: https://skia.org/docs/user/modules/canvaskit/
- CanvasKit text and font quickstart: https://skia.org/docs/user/modules/quickstart/
- Skia CPU resource-cache defaults: https://skia.googlesource.com/skia/+/refs/heads/main/src/core/SkResourceCache.cpp
- Skia strike-cache defaults: https://skia.googlesource.com/skia/+/refs/heads/main/src/core/SkStrikeCache.h
- Ganesh GPU resource-cache defaults: https://skia.googlesource.com/skia/+/refs/heads/main/src/gpu/ganesh/GrResourceCache.h
- Graphite context options and resource budget: https://skia.googlesource.com/skia/+/refs/heads/main/include/gpu/graphite/ContextOptions.h
- Graphite recorder options and resource budget: https://skia.googlesource.com/skia/+/refs/heads/main/include/gpu/graphite/Recorder.h
- SkParagraph API: https://skia.googlesource.com/skia/+/refs/heads/main/modules/skparagraph/include/Paragraph.h
- SkParagraph build configuration: https://skia.googlesource.com/skia/+/refs/heads/main/modules/skparagraph/BUILD.gn
- SkUnicode API: https://skia.googlesource.com/skia/+/refs/heads/main/modules/skunicode/include/SkUnicode.h
- SkFontMgr API: https://skia.googlesource.com/skia/+/refs/heads/main/include/core/SkFontMgr.h
- SkCodec API: https://skia.googlesource.com/skia/+/refs/heads/main/include/codec/SkCodec.h
- SkSVGDOM API: https://skia.googlesource.com/skia/+/refs/heads/main/modules/svg/include/SkSVGDOM.h
- Ganesh and Graphite build defaults: https://skia.googlesource.com/skia/+/refs/heads/main/gn/skia.gni
- Current CanvasKit build profiles: https://skia.googlesource.com/skia/+/refs/heads/main/modules/canvaskit/compile.sh
- CanvasKit build and exported module configuration: https://skia.googlesource.com/skia/+/refs/heads/main/modules/canvaskit/BUILD.gn
- Graphite Dawn/Emscripten lifecycle constraints: https://skia.googlesource.com/skia/+/refs/heads/main/include/gpu/graphite/dawn/DawnBackendContext.h
- Cargo package size limit: https://doc.rust-lang.org/cargo/reference/publishing.html#packaging-a-crate
- GitHub Release assets: https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases
- Removal of Skia's experimental C API: https://skia.googlesource.com/skia/+/refs/heads/main/RELEASE_NOTES.md
- Fission text, editing, and frame scheduling decision: `docs/adr/0001-canonical-text-editing-frame-scheduling-and-incremental-ui-updates.md`
