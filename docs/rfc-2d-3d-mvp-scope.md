# Fission 2D and 3D MVP Scope

- Status: Approved implementation scope
- API status at first release: Alpha
- Initial qualified targets: Web, macOS, Windows, and Linux
- Related architecture: `docs/13-3d-integration.md`

## Outcome

Fission's first 2D and 3D release must let a developer build a small,
interactive game without writing renderer, shell, input-routing, or test-driver
integration code.

The release is deliberately narrower than a complete game engine. Alpha means
that the supported workflows are functional and qualified, while public API
details may still change between alpha releases. It does not mean that blank
surfaces, renderer-only demonstrations, placeholder interaction, or untested
platform claims are acceptable.

The 2D and 3D capabilities ship together once both satisfy this document. A
primitive-only 3D embed is not sufficient for the combined MVP.

## Architectural boundaries

The scene APIs are useful independently of the game runtime:

- `fission-scene` owns shared scene identity, asset handles, diagnostics,
  capabilities, and renderer-neutral values.
- `fission-scene2d` owns the general retained 2D scene, its closed IR, and its
  Fission widget adapter.
- `fission-scene3d` owns the general retained 3D scene, its closed IR, and its
  Fission widget adapter.
- `fission-game` owns deterministic simulation, semantic input, snapshots,
  replay, and the bridge from game state to either scene dimension.
- Rapier providers are optional adapters behind independent Cargo features.

Neither scene crate depends on `fission-game`. A product configurator,
visualisation, editor, or ordinary application can use either scene widget
without starting a game runtime.

Both dimensions use the same game authority when they are used by a game. The
runtime can present either closed scene form without creating a second state or
clock:

```rust,ignore
pub enum ScenePresentation {
    Scene2D(Scene2DIR),
    Scene3D(Scene3DIR),
}
```

The exact ownership shape may change during implementation, but it must retain
one fixed-step runtime, one typed input stream, and one authoritative game
state. Rendering, picking, and physics providers never become authorities for
game state.

Scene content lowers to closed, renderer-neutral IR. Backend resources, GPU
handles, Rapier types, callbacks, and platform SDK types do not enter that IR.

## Public API and alpha policy

Alpha APIs use their intended long-term import paths from the first release:

```rust,ignore
use fission::game::*;
use fission::scene2d::*;
use fission::scene3d::*;
```

There is no temporary `experimental` namespace. Stabilisation must not force
applications to rewrite otherwise unchanged imports. Types are not marked
`#[non_exhaustive]` merely because the release is alpha.

Alpha status is communicated through all of the following:

1. New scene and game crates use explicit prerelease versions such as
   `0.1.0-alpha.1`.
2. The Fission facade keeps the capabilities opt-in through normally named
   features such as `scene2d`, `scene3d`, `game`, `physics-rapier2d`, and
   `physics-rapier3d`. The feature names remain valid after stabilisation.
3. Each package declares tool-readable metadata:

   ```toml
   [package.metadata.fission]
   api-status = "alpha"
   ```

4. Crate pages, API documentation, guides, examples, and changelogs state the
   compatibility policy and current limitations.
5. `cargo-fission` and the Fission crate catalogue surface the metadata when a
   developer discovers or enables an alpha capability.

Examples and documentation use exact alpha requirements where a prerelease
update could otherwise change the API unexpectedly:

```toml
fission-scene2d = "=0.1.0-alpha.1"
fission-scene3d = "=0.1.0-alpha.1"
fission-game = "=0.1.0-alpha.1"
```

Breaking changes are allowed between alpha releases, but each change requires
a changelog entry and a short migration note. Snapshot, replay, asset-bundle,
and scene-IR formats remain explicitly versioned and reject unsupported
versions rather than guessing.

## Shared MVP foundation

The following is required by both dimensions.

### Retained identity and scene processing

- Stable scene, node, asset, and presentation identities.
- Deterministic source and layer ordering.
- Validated local and world transforms.
- Explicit viewport, clip, and visibility state.
- Closed and serialisable `Scene2DIR` and `Scene3DIR` values.
- Actionable diagnostics for invalid geometry, assets, transforms, cameras,
  material values, and unsupported renderer capabilities.
- Inspectable pass accounting for considered, rejected, culled, batched, and
  drawn content.

### Runtime and input

- Fixed-step simulation separated from presentation time.
- Typed semantic game messages.
- Keyboard, pointer, and touch input through the same message path.
- Focus-loss and pointer-cancellation handling.
- Versioned snapshots and deterministic replay.
- Explicit deterministic random streams when simulation needs randomness.
- Headless control of ticks and input for tests.
- Fission actions and semantics for activation and accessible alternatives.

Advanced gamepad support, remapping, and hot-plugging may follow after the MVP,
but the public input model must leave room for device-independent axes and
buttons without making platform key codes authoritative.

### Assets

The MVP asset path is intentionally small but real:

- Typed image, model, mesh, material, and texture handles.
- Build-time validation and deterministic runtime bundle identities.
- Images suitable for 2D sprites.
- Static glTF/GLB import for 3D scenes, including referenced textures and
  metallic-roughness material values used by the MVP renderer.
- Explicit loading, ready, and failed states.
- Useful diagnostics which name the source asset and unsupported content.
- Reuse of unchanged decoded and GPU-resident resources.

Blender automation, texture compression, streamed bundles, Aseprite, Tiled,
LDtk, skeletal import, and platform-specific cooking are later capabilities.

### Optional physics

Physics remains absent from the dependency graph unless its feature is enabled.
The public contracts use Fission types rather than Rapier types.

Both providers support:

- fixed, kinematic, and dynamic bodies;
- explicit collision shapes;
- pose and velocity reads and writes;
- overlap and ray queries;
- trigger and contact transitions;
- force and impulse application;
- deterministic stepping from the game clock; and
- snapshot and restore for supported deterministic tests.

The 3D provider additionally includes a basic kinematic character controller
capable of moving through the MVP qualification scene.

## 2D MVP

### Scene model

The retained 2D scene supports:

- stable nodes and parent-child transforms;
- an orthographic camera and explicit viewport;
- translation, rotation, scale, and anchor/pivot selection;
- images and sprites;
- rectangles and renderer-neutral vector paths;
- text using Fission's resolved text pipeline;
- clips, opacity, layers, and deterministic blend ordering;
- repeated image instances and sprite batches;
- viewport and clip culling;
- retained path, text, image, and batch resource reuse; and
- bounds queries and scene-object picking.

Batching must survive lowering into the renderer. Representing an image batch
in scene IR and then expanding it into independent retained image widgets does
not satisfy the batching requirement.

### Interaction

Scene objects can declare tap, drag start/update/end/cancel, and long-press
actions. Coordinate hit testing, keyboard or accessibility activation, and
semantic testing dispatch the same application actions. Decorative content is
pointer-transparent unless it explicitly owns interaction.

### Functional example

The public 2D qualification game is a small top-down experience with:

- a controllable player;
- a camera that follows or traverses a world larger than its viewport;
- static obstacles and collision;
- visible sprite, text, and path content;
- at least one collectible or objective;
- pointer/touch and keyboard paths to the same actions;
- a complete success state; and
- snapshot and replay coverage of a complete run.

It uses only public APIs and packaged public assets.

## 3D MVP

### Scene model and rendering

The retained 3D scene supports:

- stable nodes and parent-child transforms;
- translation, quaternion rotation, and scale;
- perspective and orthographic cameras;
- camera viewport and clear configuration;
- cubes, spheres, application-provided triangle meshes, and static glTF/GLB
  model instances;
- unlit and basic metallic-roughness materials;
- base-colour and emissive textures;
- opaque, masked, and deliberately ordered alpha blending;
- double-sided material policy;
- ambient, directional, and point lights;
- depth testing;
- world bounds and frustum culling;
- retained mesh, texture, pipeline, and material resources; and
- ray and viewport-coordinate picking.

Normal maps, spot lights, shadows, skeletal animation, skinning, morph targets,
animation blending, inverse kinematics, custom shaders, and post-processing are
not required for this MVP.

Visible transform animation is driven by explicit application state or the
owned game clock. A renderer or imported model may not advance a private clock.

### Interaction

The normal Fission hit-test selects the bounded 3D viewport. Local coordinates
produce a renderer-independent pick query, and the selected stable node is
reported through a normal Fission action. Picking must support pointer input,
semantic test input, and an accessible action path that does not require visual
ray selection.

### Functional example

The public 3D qualification game is a small navigable environment with:

- at least one imported textured model;
- a controllable player or camera;
- visible depth and lighting relationships;
- collision against scene geometry or explicit colliders;
- at least one object selected through viewport picking;
- at least one physics-driven or kinematic interaction;
- a complete objective and success state;
- pointer/touch and keyboard paths to the same game messages; and
- snapshot and replay coverage of a complete run.

A single rotating primitive or a test that only observes a `DrawSurface`
operation is not sufficient qualification.

## Platform contract

The alpha's supported game targets are Web, macOS, Windows, and Linux.

For each supported target:

- a scene participates in normal Fission layout, clipping, opacity, and
  composition;
- failure to acquire a required GPU capability produces a deliberate,
  actionable compatibility state rather than a blank surface;
- renderer readiness and first meaningful content are observable;
- suspend, resume, resize, scale-factor changes, and focus loss preserve valid
  runtime state; and
- the public examples run without target-specific application logic.

Android and iOS must continue to compile with the scene features enabled. They
become advertised game targets only after real-device or simulator rendering,
input, lifecycle, and nonblank-frame qualification is added.

The 2D renderer may provide a software fallback. The 3D capability must not
silently select an unusably slow software renderer.

## Performance and dependency contract

- No scene, asset-pipeline, wgpu, glTF, or Rapier dependency is compiled when
  the corresponding feature is disabled.
- Rapier 2D and Rapier 3D are independently selectable.
- Static scenes reuse GPU and decoded resources across Fission rebuilds.
- Culling and batching are observable through diagnostics.
- Per-frame rebuilding or uploading of unchanged model, mesh, sprite, or
  texture resources is a release blocker.
- Representative Web scenes remain within documented CPU, GPU, and memory
  budgets established during qualification.

## Explicitly deferred capabilities

The combined MVP does not include:

- audio;
- skeletal animation or animation blending;
- advanced gamepad configuration;
- custom shader authoring;
- shadows or advanced lighting;
- voxel terrain;
- editor tooling or a general digital-content-creation environment;
- multiplayer or persistent-world services;
- storefront SDKs;
- advanced asset streaming, compression, or source-format integrations; or
- the higher-level everyday fluent game-authoring API.

These omissions must not be represented as silently supported features. They
also must not introduce temporary architecture that prevents their later
implementation behind the same scene and runtime authorities.

## Release qualification

The 2D and 3D alpha is releasable only when all of the following are true:

1. The implementation is integrated on a branch based on current `main`; the
   historical gaming branch is an implementation input, not a branch to merge
   wholesale.
2. Both public qualification games build and run through documented commands.
3. Headless tests prove deterministic fixed-step behavior, input ordering,
   snapshots, replay, scene validation, culling, picking, and physics adapters.
4. Real Web tests drive keyboard and coordinate-based pointer/touch-equivalent
   input, finish both games, and capture meaningful nonblank frames.
5. macOS, Windows, and Linux tests render meaningful frames and exercise the
   primary interaction path.
6. Render tests inspect actual 2D and 3D output; surface allocation alone is not
   evidence of rendered content.
7. Invalid assets and unsupported GPU capabilities produce tested typed errors
   or compatibility UI.
8. CI proves that builds without scene and physics features do not compile or
   link their optional dependencies.
9. Package contents, feature combinations, documentation, and exact
   prerelease dependencies are verified before publication.
10. The published crates are consumed in a clean project and both examples are
    rebuilt against the registry versions as final verification.

Completion of this scope establishes a functional alpha foundation for small
2D and 3D games. Broader engine completeness is delivered through subsequent
capability releases rather than weakening this MVP's functional guarantees.
