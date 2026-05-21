# RFC: Developer Tooling and Ecosystem

Status: proposal
Audience: Fission runtime, shells, compiler, CLI, documentation, testing, and IDE integration implementers
Scope: developer-facing tools that make Fission applications easier to build, inspect, debug, test, profile, package, and maintain

## 1. Summary

Fission needs a coherent developer tooling story that makes the framework feel like a complete application platform, not only a rendering library. A developer should be able to create an app, run it on any supported target, inspect what the framework built, diagnose layout and performance problems, record tests, check accessibility, package releases, and work from their preferred editor without learning disconnected tools.

The core design should be protocol-first. Fission should expose one stable developer tooling protocol from running development builds, and every surface should consume that protocol:

- the standalone `fission devtools` UI;
- IDE plugins;
- the terminal `fission ui`;
- test recorders;
- CI trace viewers;
- future visual design tools.

This keeps the tooling architecture aligned with Fission's existing optional instrumentation model. The app and shells expose observable snapshots and event streams only when developer tooling is enabled. Production builds should not pay for this instrumentation unless the application explicitly opts in.

The developer tooling must be honest about Fission's architecture. It should inspect the authored widget tree, lowered Core IR, layout, display list, semantics, actions, reducers, jobs, services, capabilities, resources, shell events, and generated assets. It must not invent a separate UI model that bypasses `Widget::build`, reducers, the router, design-system tokens, or the shell pipeline.

## 2. Goals

- Provide a first-class inspection and debugging workflow for desktop, web, Android, iOS, terminal, and static site targets.
- Build one Fission developer tooling protocol that can be consumed by CLI, IDE, browser, and CI tools.
- Make the widget-to-output pipeline explainable: authored widget tree, Core IR, layout, paint/display list, semantics, hit testing, and shell output.
- Provide action, reducer, state, effect, job, service, capability, resource, and network timelines.
- Provide profiling tools for frame time, CPU spans, memory/resource growth, app size, rendering costs, and shell-specific bottlenecks.
- Provide a visual preview/designer workflow that edits Rust, DSP design-system data, `fission.toml`, Markdown content, or other real project files instead of becoming a separate source of truth.
- Integrate with established editor protocols where possible: Language Server Protocol for language/project intelligence and Debug Adapter Protocol for debugger integration.
- Support VS Code-family editors and JetBrains IDEs as first-class rich integrations.
- Keep Neovim, Helix, Emacs, Zed, and other editors usable through CLI, LSP, DAP, and machine-readable diagnostics.
- Keep instrumentation opt-in, bounded, secure, and removable from production builds.

## 3. Non-goals

- Do not build a full Rust IDE or replace `rust-analyzer`.
- Do not make the visual designer the authoritative app format.
- Do not require a hosted service for local development.
- Do not require a specific commercial IDE.
- Do not pretend that arbitrary network traffic can be inspected unless it flows through Fission-owned APIs or explicit instrumentation.
- Do not make release builds expose inspection ports by default.
- Do not put JSON or other debug serialization on the runtime hot path when developer tooling is disabled.
- Do not copy external tool workflows exactly. Fission should provide equivalent capability in a way that fits Rust, Fission's architecture, and Fission's CLI.

## 4. Research baseline

Mature cross-platform application frameworks provide more than a run command. The useful pattern is a layered toolchain:

- a command-line tool that owns project creation, run, device selection, diagnostics, test, build, and package workflows;
- a browser or desktop devtools UI for runtime inspection, profiling, logs, networking, memory, and app-size analysis;
- IDE plugins that integrate the same tooling into the editor;
- preview and property-editing tools that shorten the UI iteration loop;
- common protocols so language and debugging features are not rebuilt for each editor.

The following external references are relevant design inputs, not compatibility targets:

- Flutter DevTools lists UI/state inspection, jank diagnosis, CPU profiling, network profiling, source debugging, memory debugging, logs, app size, and deep-link validation as core capabilities [R1].
- Flutter editor support focuses on VS Code, Android Studio, and IntelliJ plugins, while still allowing other editors through command-line tooling plus LSP/DAP-style integration [R2].
- Flutter's Property Editor and Widget Previewer show that property editing and isolated widget previews materially improve UI iteration, but they also demonstrate the need for careful source mapping and runtime integration [R3][R4].
- LSP exists to reuse language intelligence across editors instead of implementing editor-specific analyzers repeatedly [R5].
- DAP exists to reuse debugger integration across editors through debug adapters [R6].
- VS Code webviews are suitable for custom UI, but the platform guidance is to use them only when normal editor UI is insufficient and to keep them themeable and accessible [R7].
- JetBrains tool windows are the correct extension surface for persistent project/run/debug tooling inside JetBrains IDEs [R8].
- Chrome DevTools Protocol is relevant for the web shell because it provides browser instrumentation domains such as network, performance, runtime, accessibility, and tracing [R9].
- `rust-analyzer` is already the Rust language server and provides IDE features such as go-to-definition, references, refactoring, completion, formatting, and diagnostics [R10].

## 5. Tooling architecture

### 5.1 Fission Developer Tooling Protocol

Fission should define a versioned Fission Developer Tooling Protocol, abbreviated FDTP in this RFC. FDTP is the contract between a running development app and tooling clients.

FDTP is not an app runtime ABI. It is a developer-only observation and control protocol. The protocol can use a developer-friendly encoding such as JSON-RPC or a compact binary encoding, but the encoding choice must not affect production runtime behavior. When FDTP is disabled, the app should not allocate snapshots, retain trace history, open sockets, or perform serialization.

FDTP should support:

- capability discovery;
- session negotiation;
- target and shell identification;
- frame snapshots;
- incremental diffs;
- event streams;
- source provenance;
- diagnostic events;
- bounded trace capture;
- commands for selecting nodes, highlighting nodes, toggling overlays, requesting profiles, and recording tests.

The protocol should be implemented by a small set of crates:

```text
crates/tools/fission-devtools-protocol
crates/tools/fission-devtools-server
crates/tools/fission-devtools-ui
crates/tools/fission-lsp
crates/tools/fission-dap
```

The exact crate names can change, but the responsibility split should remain:

- protocol schemas are shared and stable;
- shell/runtime servers expose live sessions;
- the UI is reusable by CLI, browser, and IDE plugins;
- editor language services complement `rust-analyzer`;
- debug adapters integrate Fission-specific runtime debugging without replacing native Rust debugging.

### 5.2 Transport

Default local transport should be:

- localhost WebSocket for desktop and web development;
- Unix domain socket on Unix-like systems where appropriate;
- named pipe on Windows where appropriate;
- CLI-mediated bridge for Android and iOS devices or simulators;
- in-process channel for the terminal `fission ui` where it owns the child session.

Remote inspection must be explicit:

```text
fission run --target ios --devtools
fission devtools attach --device "iPhone 16 Pro" --session <id>
```

Development sessions should use random session tokens and bind to localhost by default. Remote device attach should tunnel through the CLI instead of exposing unauthenticated ports on a network.

### 5.3 Capability discovery

Every shell should report what it can expose.

```rust
struct DevtoolsCapabilities {
    widget_tree: bool,
    core_ir: bool,
    layout: bool,
    display_list: bool,
    semantics: bool,
    hit_test: bool,
    actions: bool,
    reducers: bool,
    effects: bool,
    resources: bool,
    jobs: bool,
    services: bool,
    capabilities: bool,
    network: bool,
    performance: bool,
    memory: bool,
    app_size: bool,
    screenshots: bool,
    test_recording: bool,
    visual_preview: bool,
    shell_specific: Vec<ShellToolCapability>,
}
```

This prevents tools from pretending every target supports the same output. A static site route can expose HTML, CSS, metadata, link validation, and search-index diagnostics. A terminal app can expose cell buffers, focus traversal, semantics, and terminal input events. A desktop app can expose GPU layers, native window metadata, and device scale. The common protocol should make target differences visible instead of hiding them behind weak abstractions.

### 5.4 Snapshot model

FDTP snapshots should be frame-scoped and source-linked.

```rust
struct DevFrame {
    session_id: DevSessionId,
    frame_id: FrameId,
    sequence: u64,
    shell: ShellTarget,
    viewport: DevViewport,
    widget_tree_ref: SnapshotRef,
    core_ir_ref: SnapshotRef,
    layout_ref: SnapshotRef,
    display_list_ref: SnapshotRef,
    semantics_ref: SnapshotRef,
    diagnostics_ref: SnapshotRef,
}

struct SourceProvenance {
    crate_name: String,
    module_path: String,
    file: String,
    line: u32,
    column: u32,
    symbol: Option<String>,
}
```

Snapshots should be immutable after capture. Historical retention should be bounded by configuration. Tooling clients can request full snapshots, diffs, or summary views depending on the pane they are showing.

## 6. Required developer tool categories

### 6.1 Project and target manager

Developers need a single view of the project before debugging begins.

Capabilities:

- inspect `fission.toml`;
- list enabled targets;
- add, remove, or repair targets;
- list connected devices, simulators, browsers, and terminal targets;
- validate SDKs, NDKs, signing identities, browser drivers, and shell dependencies;
- surface feature-gating problems, such as Android dependencies being pulled into a static site build;
- launch `run`, `test`, `site serve`, `package`, and `distribute` operations.

CLI commands:

```text
fission doctor
fission devices
fission run
fission ui
fission devtools
```

IDE integrations should call these commands or use the same internal libraries. The IDE should not become a second implementation of target discovery.

### 6.2 Runtime app inspector

The app inspector is the starting point for a live session.

It should show:

- app name, version, target, shell, PID/process/session, device, viewport, scale factor, theme mode, locale, and active route;
- feature flags and enabled instrumentation categories;
- Fission crate versions and shell/runtime versions;
- WOF or static-site artifact identity when applicable;
- design-system identity and loaded token package;
- active handles for resources, services, jobs, capabilities, media, embeds, and host integrations.

This answers the basic question: "What exactly am I looking at?"

### 6.3 Widget tree inspector

The widget inspector should show the authored Fission widget tree, not only the final draw output.

Capabilities:

- tree view of widgets with stable node IDs;
- selected widget highlight in the running app;
- source jump to the `Widget::build` implementation or constructor call where possible;
- display of widget properties, theme values, environment inputs, and selector outputs;
- parent/child traversal;
- filter to show app widgets, framework widgets, or all widgets;
- explain rebuild reasons;
- show whether a widget produced Core IR directly, used a custom lowerer, or provided semantic fallback.

The widget tree is useful because it matches the developer's mental model. It must preserve source provenance so selecting a widget in DevTools can open the source location in the IDE.

### 6.4 Core IR inspector

Fission's architecture depends on lowering widgets into a framework-owned intermediate representation. The Core IR inspector should make that lowering visible.

Capabilities:

- show Core nodes and operations;
- map each Core node back to source provenance and widget provenance;
- diff Core IR between frames;
- detect unstable node IDs;
- flag unsupported target operations before the shell falls back or fails;
- expose where custom render nodes enter the pipeline.

This pane is required to keep Fission honest. If a shell output is wrong, developers need to know whether the problem is in widget construction, lowering, layout, paint, shell adaptation, or platform output.

### 6.5 Layout, constraints, and hit-test inspector

Layout bugs are one of the most expensive UI problems to diagnose. The layout inspector should expose the constraints and computed geometry that produced the visible screen.

Capabilities:

- view constraints, measured size, final rect, padding, margin, alignment, flex/grid values, scroll extents, clipping, transforms, and z-order;
- highlight overflow;
- show independent scroll regions and scroll offsets;
- explain why a node took a given size;
- show baseline and text layout metrics;
- show hit-test path for the last pointer/click/touch event;
- replay a hit test at a chosen coordinate;
- compare logical coordinates, device pixels, CSS pixels, terminal cells, and platform coordinates where relevant.

This directly addresses issues such as iOS simulator hit-test offsets, desktop scale-factor mismatches, and terminal scroll handling problems.

### 6.6 Paint, display list, layer, and output inspector

The paint inspector should explain what was drawn and in what order.

Capabilities:

- display list tree with draw operation bounds;
- layer tree and compositing reasons where the shell has layers;
- clip, transform, opacity, shadow, gradient, image, text, embed, and custom-render operations;
- overdraw visualization;
- repaint region visualization;
- shell-specific output:
  - desktop/mobile: GPU layers and surface scale;
  - web: DOM/canvas/WebGPU/WebGL output details as applicable;
  - static site: generated HTML and extracted CSS;
  - terminal: terminal cell buffer, style spans, focus cells, and mouse regions.

The inspector should not force a pixel renderer abstraction onto all targets. It should show the target's real output model.

### 6.7 Semantics and accessibility inspector

Accessibility support should be testable while building the UI, not after the app is complete.

Capabilities:

- semantics tree with roles, names, descriptions, states, values, actions, and focus order;
- platform accessibility mapping;
- missing labels and ambiguous names;
- keyboard navigation order;
- focus traps;
- contrast checks based on resolved design-system colors;
- target-size checks;
- terminal supportability diagnostics based on semantics fallback;
- screen-reader preview where shell/platform support exists.

This inspector should share data with accessibility tests and CI checks.

### 6.8 Design-system and style inspector

Fission now supports design-system packages and generated theme data. Developers need tooling to make token resolution explainable.

Capabilities:

- inspect the active design system, theme mode, resolved component variants, and state styles;
- show where a color, typography value, spacing value, radius, border, shadow, transition, or component token came from;
- show overridden tokens and fallback paths;
- preview light/dark/high-contrast modes;
- edit DSP JSON or generated theme source through safe code actions;
- validate missing tokens, inaccessible contrast, and inconsistent component sizes;
- compare app output against the design-system source package.

This should be one of Fission's differentiators. The developer should not have to guess why a button or text field looks wrong.

### 6.9 Action, reducer, and state timeline

Fission's application model uses actions and reducers. The timeline should make that model debuggable.

Capabilities:

- list dispatched actions in order;
- show action source: pointer, keyboard, text input, command, job, service, capability, timer, route, or test;
- show reducer function and source location;
- show state before/after where state is serializable or diffable;
- show selector recomputation and outputs;
- show route changes;
- support breakpoints on action type, reducer, selector, route, or state predicate where feasible;
- support replay/time-travel only when the state and effects declare deterministic replay support.

Time travel must be honest. If an action caused an external effect that cannot be replayed deterministically, DevTools should mark that boundary instead of pretending replay is exact.

### 6.10 Effects, async, resources, jobs, services, and capabilities inspector

Modern applications fail around asynchronous work as often as they fail around layout.

Capabilities:

- show resource invocations, in-flight state, cache state, retries, failures, cancellation, and result delivery;
- show command lifecycle, suspend/resume, and reducer callbacks;
- show job start/progress/completion/cancellation;
- show service start/stop/events;
- show capability calls and permission decisions;
- show dependency graph between user action, effect, result, and UI update;
- flag long-running synchronous work on the UI loop.

This is the correct place to debug `SHOW_ALERT`, future file pickers, authentication, camera/media sessions, platform permissions, background jobs, and user-defined capabilities.

### 6.11 Network inspector

The network inspector should cover traffic Fission can actually observe.

Supported sources:

- Fission resource APIs that perform HTTP/WebSocket work;
- Fission capability APIs that perform network work;
- Fission-provided HTTP clients;
- shell-provided fetch layers;
- browser network events for the web shell when a Chromium-backed test or development browser is used;
- explicit user instrumentation adapters for external clients.

Capabilities:

- request timeline;
- method, URL, status, timing, size, retries, redirects, cache behavior;
- request and response headers with redaction;
- body preview with redaction and size limits;
- WebSocket open/message/close events;
- correlation to action/reducer/effect timeline;
- export as trace artifact.

The inspector should not claim to capture arbitrary `reqwest`, platform SDK, or third-party native network traffic unless those clients are routed through Fission instrumentation or an explicit adapter.

### 6.12 Performance, frame, and jank profiler

Fission needs a frame-oriented profiler.

Capabilities:

- timeline for input, action dispatch, reducer time, build time, lowering, diff, layout, paint, raster/output, shell present, async callbacks, and idle time;
- frame budget markers;
- jank detection;
- animation timing and dropped-frame analysis;
- slow build/lower/layout/paint nodes;
- expensive selector/reducer detection;
- shell-specific timing:
  - browser: browser performance/CDP trace integration;
  - mobile: device frame timing where platform APIs expose it;
  - terminal: repaint diff size and terminal write time;
  - static site: build time per route and markdown/render/minify time.

Native CPU profiling should reuse platform profilers where possible. Fission should annotate spans and correlate them with app concepts instead of replacing every profiler.

### 6.13 Memory and resource inspector

Memory tooling should answer whether the app is retaining too much and where.

Capabilities:

- app state size estimates where available;
- node/Core IR/layout/display-list snapshot sizes;
- image, font, glyph, chart, media, and embed caches;
- host handles and VM handles;
- resource/job/service/capability lifetimes;
- retained snapshot and trace buffers;
- leak warnings for handles that survive expected lifecycle boundaries;
- object allocation sampling where enabled.

Memory tools must be sampling/bounded by default. Full heap inspection can be shell/platform-specific.

### 6.14 Logs and diagnostics console

Fission already has structured diagnostics. The developer tools should make those events useful.

Capabilities:

- structured log viewer;
- filters by category, level, frame, route, node, action, reducer, resource, and shell;
- jump from diagnostic to source or inspector pane;
- export JSONL traces;
- ring-buffer display for in-app or terminal use;
- CI artifact viewer for failed tests.

This should unify `fission-diagnostics`, CLI logs, shell logs, device logs, browser console messages, and test harness output.

### 6.15 Test recorder and test inspector

The developer should be able to record an interaction once, turn it into a maintainable test, and inspect why it failed.

Capabilities:

- record clicks/touches, keyboard input, text input, scrolls, route changes, and assertions;
- prefer semantic selectors over coordinate selectors;
- show generated test code before writing it;
- record screenshots/goldens where the target supports visual output;
- compare screenshots with tolerances;
- inspect failed hit tests and missing selectors;
- replay tests against desktop, web, Android, iOS, terminal, and static site targets where applicable.

The recorder should use the same Fission test protocol. It should not automate apps through brittle screen scraping when Fission semantics are available.

### 6.16 Widget preview and property editor

Fission needs isolated preview support for components and screens.

Capabilities:

- preview a widget without launching the full app route graph;
- provide fixture state, environment, theme, locale, viewport, platform, and input mode;
- show multiple preview variants at once;
- edit simple properties through source-safe actions;
- jump between preview, inspector, and source;
- support screenshots and golden generation from previews.

The property editor should be conservative. It can edit constructor arguments, struct fields, DSP tokens, or `fission.toml` values when it can make a deterministic source edit. If it cannot safely preserve source structure, it should show a patch or explanation instead of rewriting code destructively.

### 6.17 Visual designer

The visual designer should help developers assemble and tune UI, but Rust remains the source of truth.

Capabilities:

- drag/select/reorder widgets when the source can be updated safely;
- create new screens/components from templates;
- inspect constraints, spacing, alignment, theme tokens, and accessibility labels;
- switch device sizes, platform targets, color schemes, locales, text scale, and input modes;
- generate Rust component structs implementing `Widget`, not ad-hoc functions returning `Node` unless the user explicitly chooses that style;
- update DSP design-system JSON for token edits;
- update Markdown/content/site metadata for static-site pages where applicable.

The designer should be built on the same preview and inspector protocol. It should not introduce a second "designer document" format that drifts away from the application code.

### 6.18 Static site tooling

The static site target needs developer tools that are not meaningful for an app window.

Capabilities:

- route graph viewer;
- content collection viewer;
- front matter validation;
- Markdown AST/HTML preview;
- generated HTML and CSS inspector;
- search index inspector;
- sitemap and robots validation;
- link checker;
- structured data validator;
- metadata/social-card preview;
- accessibility and heading-order checks;
- production asset budget and minification reports.

These should appear as shell-specific panes under the same DevTools UI.

### 6.19 Packaging and distribution readiness

Packaging and distribution are part of the developer story, not an afterthought.

Capabilities:

- read release metadata rooted in `fission.toml`;
- validate app identity, icons, signing, entitlements, privacy manifests, SDK requirements, screenshots, previews, release notes, store metadata, provider credentials, and target-specific package prerequisites;
- guide the developer through missing steps;
- produce CI-friendly JSON output;
- surface release blockers in IDE Problems panes.

This should connect to the post-build lifecycle design instead of creating a separate release tool.

### 6.20 Migration and upgrade assistant

As Fission evolves, users need safe upgrade help.

Capabilities:

- detect incompatible Fission versions, shell versions, DSP schema versions, and target templates;
- provide codemods where possible;
- provide explicit manual migration instructions where codemods are unsafe;
- update `fission.toml` target config idempotently;
- explain breaking changes in terms of the user's project.

This should be exposed through:

```text
fission upgrade check
fission upgrade apply --dry-run
fission fix
```

and through IDE code actions.

### 6.21 Fast edit-run loop

Developers expect the app to react quickly while they are working. Fission should provide the fastest loop it can without pretending Rust has the same runtime replacement model as an interpreted UI language.

Capabilities:

- file watching;
- automatic `cargo check` and target validation;
- automatic rebuild/relaunch for changed binaries;
- app restart with route, viewport, theme, locale, and selected device preserved where safe;
- WOF reload where a target uses compiled Fission app artifacts;
- static-site rebuild and browser live reload;
- widget preview refresh for isolated components;
- clear distinction between:
  - rebuild: source changed and compilation is required;
  - hot restart: the app process restarts but tooling/session context is preserved;
  - state-preserving reload: only allowed when the runtime can prove the state shape and external handles remain compatible.

The tooling should optimize for short feedback loops, but it must not hide unsafe state reuse. If a reducer, action, state type, capability, or resource signature changed, the devtools UI should explain why a full restart is required.

### 6.22 Build, bundle, and app-size analyzer

Fission should make build output and dependency weight visible.

Capabilities:

- show crate feature graph and target-specific dependency graph;
- identify platform dependencies pulled into the wrong target;
- show binary/package size by crate, asset type, font, image, chart/data file, generated code, and shell component;
- compare debug and release artifacts;
- compare app-size deltas between commits or builds;
- inspect generated static-site asset budgets;
- inspect generated mobile/desktop package contents;
- recommend feature-gating or asset changes when obvious.

This is especially important because Fission supports many targets. A web or static-site build should not accidentally carry desktop, Android, or terminal-only dependencies.

## 7. IDE integration plan

### 7.1 Tier 1: VS Code-family extension

Fission should build a VS Code extension first.

Reasons:

- it is widely used by Rust, web, and cross-platform developers;
- it has strong task/debug/status-bar integration;
- webviews can host the shared Fission DevTools UI;
- tree views can expose project targets, devices, routes, tests, and diagnostics;
- Code OSS-compatible editors can often consume the same extension packaging.

Required features:

- project detection from `fission.toml`;
- commands for init, add target, run, test, site serve, package readiness, and devtools attach;
- status-bar target/device selector;
- Problems integration from `fission check`, `fission site check`, diagnostics, and target readiness;
- DevTools webview;
- Widget Preview webview;
- generated test recorder integration;
- launch configurations for native Rust debugging plus Fission devtools attach;
- snippets for `Widget` structs, reducers, actions, routes, resources, commands, jobs, services, capabilities, design-system setup, and static site pages;
- syntax/schema support for `fission.toml`, DSP JSON, release metadata, content front matter, and trace files.

The extension should rely on `rust-analyzer` for Rust language intelligence. Fission-specific language services should add project semantics, not duplicate Rust parsing.

### 7.2 Tier 1: JetBrains platform plugin

Fission should build a JetBrains plugin for RustRover, IntelliJ IDEA, CLion, and Android Studio.

Reasons:

- JetBrains users expect integrated project, run, debug, and inspection tool windows;
- RustRover and CLion are important for Rust developers;
- Android Studio matters for Android target workflows;
- the same plugin architecture can cover multiple JetBrains IDEs with product-specific adaptation.

Required features:

- Fission tool window for targets, devices, runs, tests, traces, and package readiness;
- embedded DevTools UI or native panes where appropriate;
- run configurations for Fission targets;
- Problems/inspection integration;
- code actions for project config and migrations;
- preview/designer window;
- schema support for `fission.toml`, DSP JSON, release metadata, content front matter, and trace files.

The plugin should not try to replace JetBrains' Rust support. It should integrate with it.

### 7.3 Tier 2: Neovim, Helix, Emacs, and Zed

These editors should be supported through protocols and CLI rather than heavy bespoke UI at first.

Required features:

- `fission-lsp` for project files, DSP JSON, front matter, trace files, generated diagnostics, and command/code-action hooks;
- `fission-dap` for Fission-specific VM/action/reducer debugging where applicable;
- documented tasks/commands;
- machine-readable output from every CLI command that an editor needs;
- `fission devtools --open-browser` for full visual tooling outside the editor;
- stable file formats for traces and snapshots.

Zed can move to richer plugin support once its extension APIs cover the required UI surfaces. Until then, LSP/tasks/CLI are the correct baseline.

### 7.4 Tier 3: Xcode and Visual Studio

Fission should not start by building full plugins for Xcode or Visual Studio.

Required support instead:

- generate platform projects where required by iOS/macOS/Windows workflows;
- make those projects debuggable with native tools;
- integrate through CLI, logs, generated schemes/configurations, and documentation;
- provide source maps/provenance so native crashes can be correlated back to Fission code where possible.

Full IDE plugins can be reconsidered when Fission has enough platform-specific users to justify the maintenance cost.

### 7.5 Browser development integration

For the web shell, Fission should integrate with browser tooling rather than fighting it.

Required features:

- launch a controlled development browser from `fission run --target web --devtools`;
- collect browser console errors;
- collect network/performance/runtime information via browser devtools protocols where available;
- map browser-level diagnostics back to Fission route/widget/source provenance where possible;
- keep browser instrumentation optional and separate from the normal app runtime.

## 8. Fission language service

Fission needs a companion language service, but it should be scoped narrowly.

It should own:

- `fission.toml` schema validation and completion;
- target-specific configuration diagnostics;
- DSP JSON schema validation and completion;
- release metadata validation;
- static-site front matter validation;
- generated route/content diagnostics;
- Fission trace/snapshot file viewing support;
- commands/code actions that call the CLI;
- snippets and templates;
- links from diagnostics to docs.

It should not own:

- Rust name resolution;
- Rust type inference;
- Rust completion;
- Rust refactoring;
- Cargo workspace analysis already handled by Rust tooling.

`rust-analyzer` remains the Rust language intelligence provider. Fission tooling should cooperate with it through source spans, generated diagnostics, and editor commands.

## 9. Fission debug adapter

Fission should provide a debug adapter only for Fission-specific runtime concepts.

Good DAP use cases:

- attach to a running Fission development session;
- break on action dispatch;
- break on reducer entry/exit;
- break on route change;
- break on failed resource/job/service/capability result;
- inspect current action payload and reducer state snapshot where available;
- step through a recorded action timeline;
- inspect WOF/VM execution where Worka/Fission VM integration is active.

Native Rust source debugging should stay with existing native debuggers and editor integrations. Fission's DAP should complement native debugging by exposing UI/runtime semantics.

## 10. Command-line experience

The CLI remains the foundation. IDEs should be thin integrations over the CLI and protocol, not separate products.

Required commands:

```text
fission devtools
fission devtools attach
fission inspect
fission trace record
fission trace open
fission trace export
fission preview
fission designer
fission test record
fission test replay
fission fix
fission upgrade check
fission upgrade apply
fission package readiness
fission distribute readiness
```

All commands that can be used by IDEs or CI must support:

```text
--json
--project-dir <path>
--target <target>
--device <device-id>
--no-interactive
```

The terminal `fission ui` should expose the same capabilities for developers who prefer a guided interface, but it should call the same command implementations and protocol clients.

## 11. Trace files and CI artifacts

Developer tooling should produce durable artifacts for bug reports and CI.

Proposed artifacts:

```text
target/fission/traces/<timestamp>.fission-trace/
  manifest.json
  frames/
  snapshots/
  diagnostics.jsonl
  screenshots/
  profiles/
  network/
  accessibility/
  site/
```

Trace archives should:

- be redacted by default;
- include schema versions;
- include project and dependency versions;
- include shell/target metadata;
- include enough source provenance to explain failures;
- allow size limits and sampling;
- be viewable with `fission trace open`.

CI should upload these artifacts on test failure so developers can inspect the failure locally without rerunning the exact device environment.

## 12. Security and privacy

Developer tools can expose sensitive data. Fission should make safe defaults explicit.

Rules:

- no devtools server in release builds unless explicitly enabled;
- bind to localhost by default;
- require a random session token for every attach;
- require CLI-mediated tunnels for remote devices;
- redact authorization headers, cookies, known secret fields, credential paths, and configured patterns;
- cap body capture by size and content type;
- allow per-project redaction rules in `fission.toml`;
- never store secrets in trace files by default;
- make trace export show a redaction summary.

Example:

```toml
[devtools.redaction]
headers = ["authorization", "cookie", "x-api-key"]
fields = ["password", "token", "secret", "client_secret"]
max_body_bytes = 65536
```

## 13. Developer ecosystem beyond tools

The mature ecosystem needs more than inspectors.

Required ecosystem pieces:

- official templates for app, library, component package, DSP design system, static site, terminal app, chart-heavy app, and plugin/capability package;
- a documented package/plugin model for widgets, capabilities, resources, shells, renderers, chart extensions, and design systems;
- API reference generated from real Rust docs and linked from guides;
- a searchable example gallery with runnable desktop/web/mobile/static/terminal examples;
- a component gallery that shows every built-in widget with states, themes, accessibility notes, and source;
- a design-system gallery and token browser;
- migration guides and automated fixes;
- CI templates for GitHub Actions and other common systems;
- release lifecycle templates and readiness checks;
- crash/error reporting integration interfaces;
- observability hooks for logs, metrics, and traces without binding Fission to a single vendor;
- community extension publishing conventions;
- compatibility policy covering Fission versions, DSP schema versions, shell target support, and generated project templates.

## 14. Implementation order

The implementation should proceed in this order because each item unlocks the next without inventing parallel systems:

1. Define FDTP schemas and capability discovery.
2. Wire runtime/shell instrumentation to produce widget tree, Core IR, layout, display list, semantics, hit-test, action, reducer, diagnostics, and log streams.
3. Implement `fission devtools attach` and a minimal standalone UI that can inspect one running desktop app.
4. Add trace record/open/export support.
5. Add web, Android, iOS, terminal, and static-site attach support through shell-specific bridges.
6. Add layout/hit-test, semantics/accessibility, action/reducer, effects/resources/jobs/services/capabilities, and logs panes.
7. Add performance/frame profiler, memory/resource inspector, and app-size reports.
8. Add network inspector for Fission-owned network/resource APIs and web-shell browser integration.
9. Add test recorder and replay integration.
10. Add fast edit-run loop support: file watch, rebuild, relaunch, hot restart, static-site live reload, and safe state-preserving reload where provable.
11. Add widget preview and property editor.
12. Add design-system inspector and token editing.
13. Add build, bundle, dependency, and app-size analyzer reports.
14. Build the VS Code extension over the CLI and shared DevTools UI.
15. Build the JetBrains plugin over the CLI and shared DevTools UI.
16. Add `fission-lsp` and `fission-dap` surfaces for editor-neutral integrations.
17. Add visual designer workflows once preview, source provenance, and safe source edits are reliable.

## 15. Acceptance criteria

The developer story reaches the first mature milestone when:

- `fission run --devtools` starts an app and exposes an inspectable development session.
- `fission devtools` can attach to desktop, web, Android, iOS, terminal, and static site targets where applicable.
- Selecting a visible UI element shows its widget, Core IR node, layout rect, semantics node, paint/display operation, hit-test path, and source location.
- A layout or hit-test bug can be diagnosed from captured coordinates, constraints, scale factors, and shell output data.
- An action can be traced from input event to reducer to state update to rebuilt UI.
- Resource/job/service/capability work is visible and correlated to the action that triggered it.
- Network calls made through Fission-owned APIs are visible with redaction.
- Performance tooling identifies slow frames and the pipeline stage responsible.
- Accessibility tooling identifies missing labels, focus problems, contrast problems, and target-size problems.
- A failed UI test produces a trace artifact that can be opened locally.
- The edit-run loop automatically rebuilds and restarts or reloads with explicit safety diagnostics.
- The app-size analyzer explains which crates, features, shell pieces, and assets contribute to the final artifact.
- VS Code and JetBrains plugins can run the app, attach DevTools, show diagnostics, open traces, and jump from inspector nodes to source.
- Neovim, Helix, Emacs, Zed, and other editors can use CLI/LSP/DAP outputs without custom per-editor logic.
- Release/package readiness diagnostics can appear in CLI, CI, and IDE Problems panes.
- Production builds have no devtools listener and no instrumentation overhead unless explicitly enabled.

## 16. Open questions

- What encoding should FDTP use by default: JSON-RPC for easier tooling, a compact binary protocol for large traces, or both?
- Should trace archives use a directory format, a single compressed archive, or both?
- Which source-editing backend should the property editor use for Rust edits: rust-analyzer assists, a Fission-specific syntax edit layer, or generated patch files?
- How much native mobile performance data can be collected without requiring heavyweight platform-profiler dependencies?
- Should the visual designer ship as part of `fission devtools`, as a separate `fission designer`, or as an IDE-only tool window?
- What is the minimum safe state snapshot trait for action/reducer replay without forcing every app state to implement heavyweight serialization?

## 17. References

- [R1] Flutter and Dart DevTools, https://docs.flutter.dev/tools/devtools
- [R2] Flutter editor support, https://docs.flutter.dev/tools/editors
- [R3] Flutter Property Editor, https://docs.flutter.dev/tools/property-editor
- [R4] Flutter Widget Previewer, https://docs.flutter.dev/tools/widget-previewer
- [R5] Language Server Protocol, https://microsoft.github.io/language-server-protocol/
- [R6] Debug Adapter Protocol, https://microsoft.github.io/debug-adapter-protocol/
- [R7] VS Code Webviews UX guidance, https://code.visualstudio.com/api/ux-guidelines/webviews
- [R8] IntelliJ Platform Plugin SDK: Tool Window, https://plugins.jetbrains.com/docs/intellij/tool-window.html
- [R9] Chrome DevTools Protocol, https://chromedevtools.github.io/devtools-protocol/
- [R10] rust-analyzer manual, https://rust-analyzer.github.io/book/index.html
