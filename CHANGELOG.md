# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.1] - 2026-08-10

### Added

- **Shared Static site and SSR document configuration** - Both HTML shells now use one `DocumentShellConfig` for themes, environments, generated design systems, packaged fonts, favicons, syntax highlighting, custom CSS, and route-filtered document elements.
- **Aligned shell APIs** - SSR gains design-system fonts, favicon links, conditional code highlighting, and retained footers. Static site gains i18n bundles, generic locale resolution, localized metadata, build-time route state, custom JSON-LD, portals, and browser surface registrations.

### Fixed

- **Static locale selection** - Markdown and MDX `locale` front matter now controls translation and document metadata. Static generation no longer contains a documentation-site-specific `/es` path rule.
- **SSR document assets** - Packaged design-system fonts are emitted into SSR route stylesheets and configured favicons are linked explicitly from rendered documents.

### Migration notes

- Update Fission dependencies to `0.10.1`. Existing Static site and SSR builders remain source-compatible; the new APIs are additive.

```toml
fission = { version = "0.10.1", default-features = false, features = ["site", "server"] }
```

## [0.10.0] - 2026-08-02

### Added

- **Generic native-surface extensions** - Desktop and mobile applications can register handlers for opaque custom embed payloads. The shell supplies a lifetime-bound native host, stable widget identity, transformed geometry, effective clipping, opacity, and paint order while keeping extension-specific types out of the shared runtime.
- **Native Windows notifications** - Windows applications can configure App User Model identity and deliver native toast notifications with stable tags, escaped content, sound policy, and honest capability reporting.
- **Unified crate and example directory** - The documentation site now includes a generated Fission crate index backed by validated crates.io metadata, an interactive example showcase, redesigned documentation and blog layouts, and one responsive visual language across the public site.
- **System appearance and initial window policy** - Desktop applications can follow the host light/dark preference, configure initial maximization, and use the shell's public URL-opening path.
- **Localized server document integrations** - Static and server-rendered document shells can provide localized integrations and theme-aware site chrome without duplicating page composition.

### Changed

- **Native presentation carries complete frame context** - Video, web, 3D, and custom native surfaces now share scroll visibility, ancestor clipping, accumulated transforms and opacity, and stable paint order. Fully clipped native surfaces are omitted rather than bleeding over surrounding Fission content.
- **Desktop feature resolution is consistent** - Development, packaging, and release builds resolve desktop variant features through the same locked Cargo configuration.
- **Documentation is a representative Fission product site** - The home, docs, blog, crate directory, and example pages were rebuilt as retained Fission widgets with responsive layouts, reusable branding, syntax highlighting, sequential documentation navigation, and accessible theme controls.
- **Custom host lifetimes are explicit** - Native-surface handlers receive a lifetime-bound `WindowHandle` and a complete attach, present, detach lifecycle, including Android suspend/resume and normal event-loop exit.

### Fixed

- **Linux Wayland presentation stalls** - Wayland defers the eager startup clear, uses `Mailbox` only when the surface advertises it, and avoids installing the frame callback that could indefinitely suppress later redraws on affected software/composited WSI paths.
- **Surface loss and software-render fidelity** - Native rendering recovers from transient acquisition errors, refreshes stale surface configuration, preserves supported alpha modes, repaints complete captures, and aligns software output with display layout.
- **Modal and Markdown sizing** - Modal surfaces retain intrinsic height and Markdown composition no longer introduces an unwanted nested scrolling region.
- **Desktop and Store packaging** - Variant feature locks, macOS private-API checks, package artifact discovery, release paths, and screenshot validation now agree across development and release workflows.
- **Native surface lifecycle and clipping** - Handlers are detached before a host is destroyed, built-in and extension surfaces remain synchronized while scrolling, and partially visible surfaces receive their effective visible bounds.

### Migration notes

- Update Fission dependencies to `0.10.0`:

```toml
fission = { version = "0.10.0", default-features = false, features = ["desktop"] }
```

- Implementations that construct `VideoSurfaceFrame` values directly must also provide `visible_rect`, `transform`, `opacity`, and `paint_order`. Most applications only consume the built-in video widget and require no source change.
- Native extensions should implement `NativeSurfaceHandler`, release host-owned platform resources in `detach_host`, and use `visible_rect` when clipping native child views.
- The release is versioned as `0.10.0` because the `VideoSurfaceFrame` shape is a public API compatibility change under Fission's pre-1.0 SemVer policy.

## [0.9.2] - 2026-07-24

### Added

- **Anchored spotlight layout** - Added a first-class `Spotlight` widget for product tours and guided onboarding. Native, Web, Static site, and SSR output share the same five-region geometry contract, including browser-side repositioning after resize and scroll.
- **Mac App Store delivery** - Added signed PKG packaging, App Store Connect upload and processing checks, macOS build lookup, review submission, App Store screenshot preparation, and cookbook tutorials for Apple signing, notarized direct distribution, and Mac App Store releases.

### Changed

- **Responsive examples are retained and production-shaped** - Reworked the animation gallery, chart gallery, editor, field inspector, inbox, product examples, text lab, widget gallery, and smoke examples around concrete responsive widget components, stable identities, design tokens, semantic LiveTest selectors, and compact layouts that keep controls reachable.
- **Desktop variants use one effective Cargo configuration** - `fission run` and desktop builds now honor variant `cargo_features` and `cargo_no_default_features` with the same overlay semantics used by packaging.
- **Release capture uses the public LiveTest protocol** - Screenshot scenarios now serialize public selector types, honor scenario timeouts, stop failed capture applications, flatten App Store PNG alpha, and validate explicit screenshot display-type directories.
- **Responsive and visual-fidelity documentation is broader** - Expanded Learn, Develop, Cookbook, Test and Debug, Release and Distribute, and Reference coverage for responsive layout, typed lengths, grid, containers, pressables, fonts, golden tests, package metadata, and release screenshots.

### Fixed

- **GPU renderer memory scales with visible work** - Vello caller buffers are sized from encoded visible content instead of retained offscreen document text, rich text is culled against active scroll clips before glyph encoding, packaged software fonts load lazily, and native/WASM image caches share a bounded 50 MB policy.
- **Layout and slider geometry** - Wrapped and aligned controls preserve intrinsic dimensions, stretch retains explicit cross-axis sizing, flyouts clamp using rendered descendant bounds, and slider visuals, hit testing, and semantic values agree at both fixed and typed widths.
- **Retained custom input identity** - Custom render widgets preserve stable identities across rebuilds, and timer/startup state changes schedule the rebuilds needed by editor, terminal, chart, and other interactive examples.
- **App Store Connect mutations** - Existing localization updates omit immutable locale fields, generated artifact paths resolve without duplicated project roots, IPA and PKG flows select the correct Apple platform, and readiness checks use the matching iOS or macOS build number.
- **Desktop package output discovery** - Desktop packaging honors Cargo target overrides, generated browser artifacts are discovered from Cargo compiler output, and macOS notifications avoid native notification APIs when an app is not running from a valid bundle.
- **Responsive LiveTest selection** - Selector resolution prefers the responsive branch that participates in layout when hidden duplicate branches share one semantic identifier.

### Migration notes

- Update Fission dependencies to `0.9.2`:

```toml
fission = { version = "0.9.2", default-features = false, features = ["desktop"] }
```

- Desktop package variants should declare their complete effective `cargo_features` list. A variant list replaces the base list, matching existing package-overlay behavior, and `cargo_no_default_features` can be overridden per variant.
- Mac App Store releases use a macOS package variant, PKG format, and `distribution.app_store.platform = "macos"`. Follow the Apple signing and Mac App Store cookbook pages for the complete setup.
- `Spotlight` is additive and requires no application migration. Terminal applications should use a terminal-specific guided-attention treatment because cell-grid output intentionally rejects spotlight overlays.

## [0.9.1] - 2026-07-23

### Added

- **Design fidelity primitives** - Added typed style and layout values for design-system-driven boxes, responsive variants, grid tracks, packaged fonts, shadows, backdrop filters, and interaction-aware pressable styling.
- **Golden image verification** - Added deterministic golden comparison helpers and documentation so rendered output can be checked against image baselines as part of LiveTest and CI workflows.
- **Runtime inspection API coverage** - Documented the public design-fidelity and golden-testing API surface, including examples for the new style, responsive, and snapshot comparison types.

### Changed

- **Tray app switcher behavior is configurable** - Desktop shells now expose app switcher and Dock/taskbar visibility policy consistently so tray-style apps can opt into or out of switcher presence explicitly.
- **Release checklist smoke command** - The maintainer release checklist now uses the current `fission add-target` smoke-test command.
- **Rust GUI article and author metadata** - Published the Rust GUI lifecycle essay and normalized the public blog author profile URL.

### Fixed

- **Windows ARM64 CLI stack reservation** - The CLI command dispatcher now reserves a larger stack before running nested command flows, avoiding stack exhaustion in deep packaging and release paths.
- **Layout inspection viewport resolution** - `LayoutEngine::inspect_node` resolves styled lengths against the snapshot viewport instead of stale engine state, so inspection results match the captured frame.
- **Design-fidelity docs and tests** - Added regression coverage for snapshot-based inspection and tightened public Rustdoc on newly exposed design-fidelity types.

### Migration notes

- Update Fission dependencies to `0.9.1`:

```toml
fission = { version = "0.9.1", default-features = false, features = ["desktop"] }
```

- Tray apps that need Dock, taskbar, or app-switcher visibility should set the tray switcher policy explicitly. The defaults remain tray-oriented where the shell can hide the app from those switchers.

## [0.9.0] - 2026-07-19

### Added

- **Native packaging variants** - Android, iOS, macOS, Windows, and Linux packaging can now describe package variants, native module Cargo products, target-specific assets, and release-signing overlays from the project manifest.
- **macOS release signing and notarization flow** - macOS package builds can resolve app-owned signing identities, provisioning profiles, entitlements, notarization credentials, and package-artifact decisions without hard-coding shell-specific scripts.
- **Desktop drag and drop** - Fission now tracks drag sessions, external file drops, dropzone state, drag previews, copy/move effects, cancellation, and gallery demonstrations for app-internal and OS-originated drag flows.
- **Selectable text and context menus** - `Text` and `RichText` can opt into selection, expose standard text actions, and use widget-backed context menus so applications can localize and customize menu contents.
- **Tray app switcher policy** - Desktop tray applications can control Dock/taskbar/app-switcher visibility, with tray-style apps hidden from switchers by default where the platform supports it.
- **Accessibility documentation set** - Added Learn, Cookbook, and Reference pages that explain semantics, assistive technology support, platform coverage, and screen-reader-compatible authoring practices.
- **Release maintainer checklist** - Added a framework release checklist covering scope, version bumps, validation, crates.io publishing, tags, GitHub releases, website verification, and post-release smoke testing.

### Changed

- **Native package resolution is metadata-driven** - Package scripts resolve native Cargo products from Cargo metadata targets and use project-relative paths consistently, avoiding stale absolute path assumptions in generated scripts.
- **Desktop package assets are included consistently** - Desktop packages now carry project assets and artifact manifests across the supported package formats instead of relying on shell-specific incidental behavior.
- **Flyouts size to their content** - Tooltip and other flyout surfaces preserve intrinsic sizing instead of expanding to fill the host window.
- **Scroll and widget state are isolated more aggressively** - Route/tab changes prune stale scroll state and avoid carrying slider/scroll positions into unrelated pages.
- **Generated app guidance is identifiable** - Generated `AGENTS.md` content carries Fission-specific guidance markers so future CLI runs can recognize files produced by Fission instead of blindly creating duplicates.
- **Build and platform documentation is broader** - Platform pages, package lifecycle docs, testing docs, selector command references, and framework-transition guides now describe the current CLI and shell behavior in more detail.

### Fixed

- **Reducer-bound effect callback lifetime** - Reducer-bound callbacks stay retained long enough for delayed effects and shell callbacks instead of being dropped while still referenced.
- **Text input pending edits** - Controlled text inputs reconcile pending edits against transformed model values so rendered text, semantics, and reducer state remain aligned.
- **Modal and barrier focus lifecycle** - Focus is captured and restored around modal/barrier lifecycles so closing a flyout or dialog does not lose the previous interaction target unnecessarily.
- **Oversized `ScrollIntoView` targets** - Nearest-alignment scrolling keeps oversized targets anchored at the leading edge instead of choosing unstable offsets.
- **Colour picker and slider QA issues** - Slider thumbs align with their tracks, click positions dispatch the expected values, colour picker swatches are labeled, and route/tab state no longer leaks into unrelated picker pages.
- **Native packaging edge cases** - macOS native output paths, Windows native handles/tool paths/NuGet preflight, Linux native products, nested desktop package selection, and static-site scaffold path detection were hardened.
- **Native notification responses** - macOS notification response callbacks now reach the Fission application layer.

### Migration notes

- Tray-style applications are hidden from the Dock/taskbar/app switcher by default where supported. Set the tray app switcher policy explicitly if an application should remain visible while minimized or closed to the tray.
- Native package configurations should prefer the manifest-driven package variant, asset, signing, and native module fields over ad hoc script paths.
- Update Fission dependencies to `0.9.0`:

```toml
fission = { version = "0.9.0", default-features = false, features = ["desktop"] }
```

## [0.8.0] - 2026-07-14

### Added

- **Shared release workflow** - Direct CLI, guided prompts, Terminal TUI, windowed publish UI, and CI JSON mode now consume the same release plan, requirement model, workflow events, and publish receipts.
- **Provider-aware publish receipts** - Publish flows retain provider request/response summaries, upload plans, per-asset events, uploaded byte totals, provider status, manual follow-up, omitted requirements, and non-overwriting receipt files.
- **Release workflow recipes** - `release-workflow run` supports declarative Fission command recipes with readiness gates, declared inputs/outputs, normalized argv, exit codes, dry-run, confirmation, and timestamped receipts.
- **Selector-driven LiveTest automation** - LiveTest commands and release screenshot scenarios can wait for and act on semantic/test/accessibility/widget-id/role/label selectors without fragile coordinate math.
- **Native video backends and typed sources** - Video sources are now explicit typed values, native backends are available per supported shell, and Linux native video is gated behind the `video` feature.
- **Publish configuration editor** - The publish UI can now open targeted `fission.toml` actions for missing metadata, release assets, package IDs, and related readiness warnings.

### Changed

- **Release readiness is stricter and clearer** - Store-bound flows classify release notes, screenshots, localized metadata, privacy/review information, signing, provider auth, version/build state, and package validation as provider-required, Fission-recommended, optional, or not applicable.
- **Package and artifact validation is more deterministic** - Artifact manifests carry source config facts, icon manifest details, package validation state, signing/notarization state, and secondary artifact coverage so publishing can reject stale or failed release assets before provider mutation.
- **Provider publishing is more complete** - Google Play, App Store Connect, Microsoft Store, GitHub Releases, S3/object storage, Docker registries, and static hosts expose more precise publish/status behavior, idempotency policy, and provider follow-up.
- **Secure release configuration is enforced** - Secret-bearing paths/material stay out of `fission.toml`; local interactive flows use `~/.fission/<app-name>/`, while CI uses env/base64 env/provider-owned credential sources.
- **Generated app guidance is stricter** - `fission init` now emits guidance for responsive mobile/desktop UI, design-system tokens, i18n setup, semantic widgets/regions, shared multi-platform cores, and LiveTest screenshot QA.
- **Generated dependency snippets** - Current examples, documentation snippets, README fragments, and `fission init` templates now point at `0.8.0`.

### Fixed

- **Google Play edit validation** - Empty-body validate calls now send the required content length instead of failing with `411 Length Required`.
- **Duplicate/stale provider mutations** - Play version-code reuse, GitHub duplicate assets, S3 overwrite policy, stale artifact manifests, and provider metadata locks produce explicit diagnostics before mutation where the provider exposes enough state.
- **Guided subprocess visibility** - Guided publishing keeps bounded live subprocess output visible during long-running operations while preserving full logs/events for receipts.
- **Release UI false readiness** - Local UI gates remain `not checked` until backed by deterministic checks or completed workflow events.
- **Interactive semantics** - Interactive widgets expose clearer semantic identifiers and radio controls use first-class radio semantics instead of generic toggle behavior.
- **Text input consistency** - Controlled text inputs keep rendered text, selection state, and editing affordances aligned, including disabled/read-only selection controls.

## [0.7.0] - 2026-07-03

### Added

- **Framework-wide data streams** - Added `FissionDataStream`, `DataStreamId`, `DataStreamRegistry`, `BoxFissionDataStream`, and stream error types so large user-owned payloads move through runtime handles instead of reducer byte buffers.
- **Stream-aware capability and async contexts** - `CapabilityCtx`, `JobCtx`, `ServiceCtx`, and `ServerJobCtx` can register, open, and release runtime-owned streams, making streamed file, media, clipboard, and barcode flows available to native, web, and server-side async hosts.
- **Desktop file picker provider** - macOS, Windows, and Linux now have a default `PICK_OPEN_FILES` provider that opens the native file picker and returns `PickedFile` metadata plus chunked file streams.
- **Upload and async architecture docs** - Added guides for file uploads and for why reducers are synchronous, with explicit guidance on jobs, services, capabilities, resources, and stream handles.

### Changed

- **Large binary capability payloads** - File picker results, camera captures, microphone captures, rich clipboard content, and barcode image decode requests now use `DataStreamId` handles rather than exposing large `Vec<u8>` payloads through reducer-facing data.
- **Server and SSR async parity** - Server job registries now carry the same stream registry contract as native async hosts so server-rendered and SSR workflows can consume large runtime streams without special casing.
- **Generated dependency snippets** - Current examples, documentation snippets, README fragments, and `fission init` templates now point at `0.7.0`.

### Fixed

- **Reducer memory pressure for uploads** - File uploads no longer require reducers or action payloads to hold whole selected files in memory. Reducers store metadata and stream handles, while jobs or services consume chunks asynchronously.
- **Unsupported file-picker behavior** - Hosts without a picker provider report explicit unsupported capability errors instead of pretending a file was selected.

## [0.6.3] - 2026-07-03

### Added

- **ScrollIntoView runtime effect** - Reducers can now call `ctx.effects.scroll_into_view(...)` or `ctx.effects.ensure_visible(...)` to reveal a target widget inside an explicit scroll container or nearest matching scroll ancestor after layout.
- **Programmatic scroll API types** - Added `ScrollIntoViewRequest`, `ScrollAxis`, `ScrollAlignment`, and `ScrollBehavior` to the public API and prelude so document canvases, editors, outlines, tabs, and validation errors can request deterministic runtime scrolling without mutating scroll state during widget conversion.

### Changed

- **Post-layout runtime work** - The winit shell now keeps post-layout hooks active even when the widget tree is unchanged, allowing layout-dependent runtime effects to resolve against the current snapshot and schedule a follow-up layout frame when scroll offsets change.
- **Scroll documentation** - The `Scroll` reference now documents stable IDs and reducer-driven reveal requests for cases such as selecting a page in a document editor.

### Fixed

- **Side-effect-free scroll control** - App code no longer needs to reorder content or reach into runtime scroll maps to reveal a selected child. Scroll offsets are updated by the runtime after layout and clamped to the container's content bounds.

## [0.6.2] - 2026-07-03

### Added

- **Native accessibility bridge** - The winit shell now publishes Fission semantics through AccessKit and platform accessibility APIs so assistive tools can inspect roles, labels, values, editable text, selections, and supported actions.
- **First-class IME composition model** - Text input now models preedit, cancel, and commit separately, tracks the preedit cursor range, and exposes selection/preedit state through semantics.
- **Pointer focus preservation policy** - Interactive widgets can opt into `FocusPolicy::PreserveCurrentOnPointer`, allowing editor ribbons and toolbars to run commands without stealing focus from the active text editor.
- **Static-site tabs** - The static-site documentation renderer now lowers Fission/Docusaurus-style `Tabs` and `TabItem` blocks into native static markup with progressive JavaScript enhancement.
- **Generated AGENTS guidance** - `fission init` now writes Fission app guidelines to the repository root as `AGENTS.md`, or `AGENTS.fission.md` when a repo-level agent file already exists.
- **Framework-transition docs** - Added a new "Fission for framework developers" documentation section, starting with a React guide built around paired React/Fission examples.

### Changed

- **Text input synchronization** - Focus, scroll, text edits, and custom editor events now refresh the shell IME cursor rectangle so candidate windows and mobile keyboard sessions follow the active caret.
- **Editor dogfooding tests** - The editor LiveTest path now exercises a human-like todo-app editing workflow with typing, typo correction, undo/redo, shortcuts, selection, drag, copy/paste, find/replace, save, and file-tree navigation.
- **Controlled widget coverage** - Widget-state tests now cover controlled interactions more directly, including buttons, toggles, sliders, and text-related state surfaces.
- **Native shell resilience** - Native video and web overlay backends now degrade gracefully when a target cannot support them, and AppKit IME configuration avoids panic-prone paths.

### Fixed

- **Remote Android keyboard input on macOS** - Fission now depends on the published `fission-winit 0.30.13-fission.1` fork, fixing the path where remote Android keyboard input could arrive as spaces instead of the typed characters.
- **Text selection precision** - Single-character and partial-range selections now render as the selected text range instead of visually expanding to the entire line.
- **Selection affordance safety** - Text input selection handles no longer advertise drag behavior that Fission cannot yet complete reliably.
- **Rich text fallback** - Rich text preserves font fallback through layout/lowering so mixed font content remains stable.
- **Layout panics** - Layout edge cases that could panic in widget-state and review scenarios have been hardened.

## [0.6.1] - 2026-06-30

### Added

- **Native capability project shells** - `fission add-target` now generates Android Gradle and iOS SwiftPM project shells that can host app-owned native SDK integrations.
- **Generic native module config** - `fission.toml` can describe Android/iOS native sources, dependencies, repositories, permissions, manifest entries, Swift package products, and linked frameworks without making Fission core SDK-specific.

### Changed

- **Fission Vello fork version** - Renderer crates now depend on `fission-vello 0.6.0-fission.2`, which contains the indirect-dispatch fallback API required by the iOS GPU fix.
- **iOS native renderer path** - iOS now stays on the Vello GPU renderer instead of falling back to the software upload renderer when the adapter lacks indirect execution support.
- **Mobile generated targets** - Regenerated `field-inspector`, `mobile-smoke`, and `web-smoke` Android/iOS targets so examples carry the new Gradle, SwiftPM, native-module, and capability-registry structure.
- **i18n documentation** - Standardized the recommended translation workflow on YAML files embedded with `include_str!` and parsed into `TranslationBundle` values at startup.

### Fixed

- **iOS Vello black screen** - Passed adapter indirect-dispatch capability into the Fission Vello fork and direct-dispatched the affected stages when indirect execution is unavailable, keeping iOS GPU rendering usable for image content and interactions.
- **Mobile integration ceiling** - Generated mobile projects no longer force applications that need native SDKs to hand-maintain unsupported Android/iOS project shells outside Fission.
- **Android raw APK packaging** - Legacy/raw Android package scripts now stage generated launch-theme resources before `aapt`, fixing `@style/FissionLaunchTheme` resolution.

## [0.5.1] - 2026-06-30

### Added

- **Fission Vello fork packages** - Prepared `fission-vello-encoding`, `fission-vello-shaders`, and `fission-vello` so the native renderer can use Fission's profiled dynamic-buffer fork through normal crates.io dependencies.
- **Vello memory profile evidence** - Added `docs/rendering/vello-memory-profile.md` with the baseline, fork experiments, final measurements, WGPU memory-hint results, and the parked direct-WGPU prototype comparison.

### Changed

- **Native renderer memory behavior** - `fission-render-vello` and the winit shell now depend on the Fission-owned Vello fork, keeping `vello::...` imports through Cargo package renaming while avoiding Vello's fixed dynamic-buffer memory floor.
- **Android shell stack** - Upgraded the winit shell crates to `winit 0.30.13`, which moves Android startup onto `android-activity 0.6.1`.
- **Documentation and target wording** - Refreshed README and documentation pages to present macOS, Windows, Linux, Web, Android, iOS, Terminal, Static site, and SSR consistently.

### Fixed

- **High native GPU memory floor** - Reduced simple release-build Vello footprints from about 214 MiB for `examples/inbox` and 247 MiB for `examples/counter` to about 50 MiB and 42 MiB respectively on the measured macOS/Metal harness. See issue #81.
- **Android startup crash path** - Avoided the `android-activity 0.5.2` native-activity panic seen by generated/mobile apps by moving the published Fission dependency graph to `winit 0.30.13`.
- **Clipped image memory pressure** - Kept the bounded decoded-image cache work from issue #79 in the release line so clipped scroll content cannot grow the image cache without bound.

## [0.5.0] - 2026-06-27

### Added

- **Widget-owned motion enums** — Built-in widgets that support motion expose local opt-in motion enums such as `ModalMotion`, `AccordionMotion`, `TabsMotion`, and `ButtonMotion`.
- **Composable motion atoms** — Widget motion enums support ordered `+` composition for common built-in motion combinations.
- **Motion workbench gallery** — Replaced the old animation gallery with a router-backed motion workbench covering widgets, properties, composition, policy, diagnostics, and deterministic LiveTests.
- **Per-widget composition builder** — Every widget gallery page has a scoped `Compose...` dialog for building and previewing widget-specific motion compositions.
- **LiveTest motion coverage** — Added live shell tests for widget demos, property demos, composition/policy/diagnostics routes, and duplicate-dispatch prevention in the composition dialog.

### Changed

- **Animation model** — Common widget motion now lowers through widget-owned enums into the native `Motion`, `Presence`, `RippleLayer`, `MotionTrack`, and `MotionExpr` runtime model.
- **Gallery structure** — `examples/animation-gallery` is split into app, chrome, routes, state, page modules, and one module per widget page.
- **Release examples** — Current Fission dependency snippets and generated project templates now reference `0.5.0`.

### Fixed

- **Motion wrapper identity** — Motion wrappers now derive distinct stable motion slot IDs instead of reusing the wrapped widget's `WidgetId`, preventing self-child lowered trees and stack overflows.
- **Composition builder dispatch** — Composer buttons submit full composition vectors so one click adds exactly one atom and one undo removes exactly one atom.
- **Gallery route behavior** — Policy and diagnostics routes now render route-specific content, and route scroll state is isolated per page.


## [0.1.0] - 2026-04-23

### Added

- **Core framework** — Widget-based UI architecture with build/layout/paint pipeline
- **GPU rendering** — Vello + wgpu backend for hardware-accelerated 2D rendering
- **Widget library** — Buttons, text inputs, modals, popovers, menus, tooltips, tabs, accordions, drawers, select, combobox, split view, and more
- **State management** — Deterministic action/reducer architecture with bound-continuation effects system
- **Layout engine** — Constraint-based layout with Box, Flex, Grid, Scroll, ZStack, Positioned, AbsoluteFill
- **Text engine** — Rope-backed text buffer with line index, undo/redo transactions, UTF-8/UTF-16 coordinate mapping
- **Syntax highlighting** — Tree-sitter integration for Rust with cached incremental parsing
- **LSP support** — rust-analyzer integration with diagnostics, completions, and frame-based polling
- **Custom render objects** — Framework escape hatch for complex widgets (editors, charts, 3D) with custom hit-test and event handling
- **Desktop shell** — macOS/Linux/Windows via winit + Vello with GPU screenshot capture
- **Charts** — fission-charts crate with 22 chart types (line, bar, pie, scatter, heatmap, treemap, etc.)
- **3D** — fission-3d crate with basic 3D scene primitives
- **Icon system** — Material Design icons via fission-icons
- **Theming** — Dark/light theme support with design tokens
- **Diagnostics** — Category-based diagnostic system with configurable sinks and sampling
- **Testing** — Headless TestDriver, LiveTestClient with winit event injection, GPU screenshot verification
- **Editor example** — VS Code-style code editor dogfooding the framework: file tree, tabs, terminal, search, git, find/replace, menu bar, command palette, minimap, LSP diagnostics
- **Effects system** — Background thread executor for FileRead, HttpGet with bound continuations
