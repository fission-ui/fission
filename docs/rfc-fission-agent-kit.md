# RFC: Fission Agent Kit and AI Tool Integrations

Status: proposal
Audience: Fission CLI, developer tooling, testing, documentation, editor integration, and MCP implementers
Scope: local-first AI tooling that makes Fission projects easier for coding agents to inspect, validate, and modify

## 1. Summary

Fission should provide an agent-facing toolkit that makes real Fission applications legible to AI coding tools without requiring developers to adopt a new editor, a new app model, or a hosted service. The toolkit should expose project context, documentation, route and widget structure, layout snapshots, screenshots, tests, action/reducer traces, and safe code-generation workflows through the Fission CLI and the Model Context Protocol.

The goal is adoption and correctness. Developers are already using AI assistants inside terminals, editors, chat interfaces, and CI systems. Fission should meet those tools where they are by making the framework easy to inspect and verify. The value is not a proprietary AI IDE. The value is that Fission projects can be understood, tested, debugged, and changed through stable, framework-aware tools instead of generic text search and guesswork.

This RFC defines the product and implementation shape for the Fission Agent Kit:

- a local MCP server;
- generated rules files for agentic coding tools;
- a deterministic project indexer;
- a version-aware documentation retriever;
- component and screen generators that produce normal Rust code;
- layout, screenshot, and visual comparison tools;
- action/reducer tracing and replay;
- test runners with machine-readable output;
- a safe permission model for read, write, execution, and network access.

These tools must remain aligned with Fission's architecture. They inspect and operate on real Fission projects. They must not create a parallel UI model, bypass `Widget::build`, bypass reducers, bypass the router, bypass the design-system pipeline, or generate code that depends on private framework internals.

## 2. Goals

- Make Fission applications easy for AI coding tools to understand without relying on broad repository scans.
- Provide a local MCP server that exposes Fission-aware resources, prompts, and tools.
- Generate concise agent rules for common coding environments while keeping the user's own instructions intact.
- Build a deterministic project index that records crates, modules, widgets, routes, actions, reducers, selectors, capabilities, design systems, tests, examples, and documentation.
- Provide a documentation retriever that is version-aware and grounded in the local Fission docs shipped with the project or installed toolchain.
- Provide code generators that create normal Fission Rust code using the public facade crate, prelude, widget structs, actions, reducers, selectors, and tests.
- Expose layout inspection and screenshot capture through the same runtime/devtools pipeline used by humans.
- Expose action/reducer traces that can be explained and replayed in tests where host effects are mocked or recorded.
- Make every agent-facing operation available through the CLI first, then expose the same functionality through MCP.
- Keep side effects explicit: reading is safe by default; writes, command execution, network access, and credential access require opt-in configuration.
- Produce stable machine-readable outputs so CI, editor plugins, MCP clients, and future tools can consume the same data.

## 3. Non-goals

- Do not build a new IDE.
- Do not replace `rust-analyzer`, Rust compiler diagnostics, or native debuggers.
- Do not require a hosted service for local development.
- Do not make AI tooling the primary commercial product surface for Fission.
- Do not make generated code a hidden source of truth.
- Do not infer application behavior from naming conventions when compiler/runtime data is available.
- Do not expose a generic shell execution tool through MCP by default.
- Do not let MCP clients read files outside the configured project roots.
- Do not send source code, screenshots, traces, credentials, or project indexes to a remote service unless the developer explicitly configures that service.
- Do not put agent/debug serialization on the production runtime hot path.

## 4. Positioning

Fission should be a strong framework for AI-assisted development because its architecture is explicit:

- UI is expressed as typed Rust widgets.
- Application changes flow through actions and reducers.
- Long-running work is represented through jobs and services.
- Host features are represented through capabilities.
- The shell pipeline can expose widget trees, layout, display output, semantics, hit testing, and screenshots.
- The CLI owns project setup, target management, test, build, package, and release workflows.

The Agent Kit should turn those properties into practical tooling. A coding agent should be able to answer questions such as:

- Which routes exist and what widgets build them?
- Which reducer handles this action?
- Why does this button not receive input at this viewport size?
- What changed visually after this patch?
- Which capability configuration is missing for Android or iOS?
- Which docs page explains this API?
- What tests should run before this change is accepted?

The answer should come from Fission's project index, runtime snapshots, traces, docs, and tests. If the tool cannot prove something from those sources, it should say so. It should not fill gaps with confident guesses.

## 5. Relationship to the Developer Tooling RFC

The Agent Kit depends on the developer tooling architecture described in `docs/rfc-developer-tooling.md`.

That RFC defines the Fission Developer Tooling Protocol, or FDTP, as the protocol exposed by development builds and consumed by devtools, IDE plugins, the terminal UI, CI trace viewers, and test recorders. The Agent Kit should consume FDTP instead of inventing a second inspection protocol.

The division of responsibility should be:

- FDTP exposes live runtime truth: widget tree, Core IR, layout, display list, semantics, hit testing, traces, logs, profiles, screenshots, and shell capability data.
- The project index exposes repository truth: crates, modules, source locations, route declarations, widget implementations, action/reducer definitions, design-system files, tests, examples, and docs.
- The MCP server exposes both through a standard AI-tool interface.
- The CLI owns local execution, artifact generation, security policy, and configuration.

When a live app is available, runtime data wins over static index data. When no app is running, the index can still help agents navigate the repository and propose changes, but it must mark static-only conclusions as advisory.

## 6. Command model

The CLI should expose the Agent Kit under `fission agent`.

```text
fission agent init --project-dir . --clients codex,claude,cursor --mcp
fission agent context --project-dir . --format markdown
fission agent context --project-dir . --format json
fission agent index --project-dir .
fission agent mcp --project-dir .
fission agent docs --query "reducers" --format markdown
fission agent inspect-layout --project-dir . --route / --viewport 1440x900
fission agent screenshot --project-dir . --route / --target desktop --theme light
fission agent compare-screenshot --project-dir . --route / --baseline main
fission agent replay-action --project-dir . --trace target/fission/traces/login.json
fission agent run-tests --project-dir . --profile smoke --format json
fission agent generate component --name EmptyState --route /inbox --with-test
fission agent propose-fix --project-dir . --diagnostic target/fission/diagnostics/layout.json
```

`fission agent init` must be idempotent. It may create managed files, but it must not overwrite user-managed instructions unless the file contains a Fission-managed marker. If a target file already exists without a marker, the command should print a patch or write a sibling file that the user can merge manually.

Generated files may include:

```text
AGENTS.md
.cursor/rules/fission.mdc
CLAUDE.md
.fission/agent/config.toml
.fission/agent/README.md
```

The exact client-specific filenames can evolve, but Fission-owned sections must be clearly marked and safely updateable.

## 7. Configuration

Agent configuration should live under `fission.toml` for project-level settings, with generated caches under `.fission/agent/`.

```toml
[agent]
enabled = true
index_dir = ".fission/agent/index"
docs = "bundled"
allow_write_tools = false
allow_command_tools = false
allow_network_tools = false
allow_screenshot_tools = true
include = ["src/**", "examples/**", "content/**", "documentation/**"]
exclude = ["target/**", ".git/**", "dist/**", ".fission/agent/index/**"]

[agent.mcp]
enabled = true
transport = "stdio"
host = "127.0.0.1"
port = 0

[agent.rules]
clients = ["codex", "claude", "cursor"]
managed = true

[agent.tests]
default_profile = "smoke"
```

`fission agent init` may populate this configuration, but developers must be able to edit it by hand. CI should be able to run every command non-interactively.

## 8. MCP server

The MCP server should be a local process started by an AI client, editor integration, or the Fission CLI.

```text
fission agent mcp --project-dir .
```

The server should support MCP resources, prompts, and tools:

- resources expose read-only context such as project summaries, docs, route manifests, design-system data, screenshots, traces, and diagnostics;
- prompts expose reusable workflows such as "build a component", "debug a layout problem", and "write a reducer test";
- tools perform bounded operations such as rendering a route, running tests, comparing screenshots, generating a patch, or explaining an action trace.

The server must respect the client's declared project roots and Fission's configured include/exclude rules. If the client does not provide roots, the CLI-supplied `--project-dir` becomes the only root.

### 8.1 MCP resources

Required resources:

```text
fission://project/context
fission://project/manifest
fission://project/index
fission://docs/index
fission://docs/page/<path>
fission://routes
fission://widgets
fission://actions
fission://reducers
fission://selectors
fission://capabilities
fission://design-system
fission://tests
fission://trace/<id>
fission://screenshot/<id>
fission://diagnostic/<id>
```

Resources should be stable and small enough for agents to use directly. Large resources should return summaries plus links to more specific resources.

### 8.2 MCP prompts

Required prompts:

```text
fission_build_component
fission_debug_layout
fission_write_reducer_test
fission_explain_reducer_path
fission_add_capability
fission_fix_visual_regression
fission_migrate_api
fission_prepare_release
```

Prompts should be workflow guides, not hidden policy. They should tell the agent which Fission tools to call, what evidence to collect, and what files are usually involved.

### 8.3 MCP tools

Required tools:

```text
read_fission_project
read_fission_tree
inspect_widget
inspect_layout
inspect_design_system
render_route
capture_screenshot
compare_screenshot
run_fission_tests
explain_reducer_path
trace_action
replay_action
generate_component
validate_project
propose_fix
```

Tool outputs should be JSON objects with stable schemas. Human-readable summaries may be included, but the machine-readable data is the contract.

Example `inspect_layout` output:

```json
{
  "route": "/",
  "target": "desktop",
  "viewport": { "width": 1440, "height": 900, "scale": 2.0 },
  "theme": "light",
  "nodes": [
    {
      "node_id": "node:42",
      "widget": "crate::components::HomePageHero",
      "source": "src/components/home_page_hero.rs:18:1",
      "bounds": { "x": 64, "y": 96, "width": 720, "height": 320 },
      "semantics": ["heading", "button"],
      "hit_testable": true
    }
  ],
  "diagnostics": []
}
```

### 8.4 Side-effect classes

Every MCP tool should declare one side-effect class:

```text
read_only
generates_artifact
writes_project_files
runs_project_code
uses_network
uses_credentials
```

The server should allow `read_only` tools by default. All other classes require project configuration or a CLI flag. Tools with multiple classes require every relevant permission.

`propose_fix` should return a patch by default. Applying that patch is a separate operation and should be disabled unless write tools are enabled.

## 9. Project indexer

The project indexer should build a deterministic, local index under `.fission/agent/index/`.

The indexer should use the strongest source of truth available:

1. `cargo metadata` for workspace, package, feature, target, dependency, and manifest information;
2. Rust compiler or rustdoc-derived data where available for resolved symbols and public API shape;
3. Fission-specific build metadata for routes, widgets, capabilities, design systems, and static site content;
4. source parsing as an advisory fallback for navigation hints only.

Source parsing must not become an authoritative substitute for compiler/runtime truth. If a result comes from parsing alone, the index should label it as `confidence = "advisory"`.

The index should capture:

- package and crate graph;
- enabled Fission targets;
- public app entry points;
- route declarations and source locations;
- structs that implement `Widget`;
- components organized by module path;
- actions from `#[fission_action]`, `#[fission_reducer]`, and manual `Action` implementations;
- reducers and their action/state types;
- selectors and source locations;
- capability usage and platform configuration;
- jobs, services, resources, and commands;
- design-system package files and generated Rust modules;
- static site content collections and generated routes;
- test targets, smoke tests, and visual baselines;
- README and documentation pages relevant to the project.

### 9.1 Cache invalidation

The index should be invalidated by:

- `Cargo.toml`, `Cargo.lock`, and `fission.toml` changes;
- changed feature sets;
- changed Rust source file hashes;
- changed design-system JSON files;
- changed content files;
- changed generated code version;
- changed Fission CLI version.

The index should be safe to delete. Rebuilding it must not change project behavior.

## 10. Documentation retriever

The docs retriever should give agents grounded documentation without forcing them to scrape the website or guess APIs.

Sources, in priority order:

1. docs bundled with the installed Fission CLI or facade crate;
2. docs checked into the current repository;
3. docs from `https://fission.rs` matching the configured Fission version;
4. user-provided documentation sources configured in `fission.toml`.

Every returned chunk should include:

- title;
- source path or URL;
- Fission version if known;
- target platform applicability;
- API names mentioned;
- short excerpt;
- next links.

The retriever should prefer guide material for "how do I" questions and reference material for "what does this type or method do" questions.

## 11. Rules files

`fission agent init` should generate concise rules for agentic coding tools. These rules should be practical and project-specific.

Rules should include:

- use the `fission` facade crate and prelude unless a lower-level crate is explicitly needed;
- organize reusable UI as structs implementing `Widget`, not ad hoc functions returning nodes;
- keep widget structs as configuration, not long-lived mutable state stores;
- model application changes as actions and reducers;
- use selectors for derived state and expensive reads;
- use capabilities for host-provided features;
- use the design-system APIs rather than hard-coded colors and spacing;
- run the relevant `fission` tests and screenshot checks before reporting UI changes complete;
- do not add platform-specific dependencies unless gated behind the correct feature;
- do not edit generated files unless the generator explicitly supports it.

Rules should also include project-specific commands:

```text
fission check --project-dir .
fission test --project-dir .
fission site check --project-dir . --release
fission agent inspect-layout --project-dir . --route /
```

The exact commands should come from `fission.toml` and the project index.

## 12. Editor and AI client integrations

The Agent Kit should integrate with the tools developers already use. The integration should be layered so each client consumes the same Fission functionality instead of receiving a separate implementation.

Required integration layers:

- CLI commands for every operation;
- MCP server for AI clients that support MCP;
- generated rules files for clients that read repository instructions;
- optional editor plugins for richer UI;
- CI output formats for non-interactive verification.

The first editor integrations should prioritize:

- VS Code-compatible editors, because they provide a common extension model and are widely used with AI coding tools;
- JetBrains IDEs, because they are common in Rust and professional application development workflows;
- terminal-first editors through CLI, MCP, LSP, DAP, and generated rules rather than bespoke UI.

Editor plugins should not reimplement Fission logic. They should call the CLI, connect to FDTP, or connect to the MCP server. Their job is presentation:

- show route and widget trees;
- open source locations from layout nodes;
- display screenshots and visual diffs;
- show action/reducer traces;
- run Fission test profiles;
- surface project readiness and capability diagnostics;
- launch the developer tooling UI.

The generated rules files are the minimum viable integration. A developer should still get useful AI behavior in clients that have no Fission-specific plugin.

## 13. Component and screen generation

The generator should create normal project files. It should never generate hidden runtime state or framework-private code.

Supported generation targets:

- component;
- screen;
- route;
- action;
- reducer;
- selector;
- capability setup;
- design-system usage example;
- widget reference example;
- static site page;
- reducer test;
- visual smoke test.

Example:

```text
fission agent generate component \
  --name EmptyInboxState \
  --module src/components/empty_inbox_state.rs \
  --with-test \
  --with-screenshot
```

Generated Rust should follow the same conventions documented for users:

```rust
use fission::prelude::*;

pub struct EmptyInboxState {
    pub title: String,
    pub message: String,
}

impl Widget for EmptyInboxState {
    fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> impl IntoWidget<AppState> {
        // real implementation generated from the selected template
        todo!()
    }
}
```

The generator may use templates, design-system tokens, route context, and existing project conventions. It should print a summary of created files and the tests it expects the developer or agent to run.

## 14. Layout inspector

The layout inspector should work in two modes:

1. attach to a running development app through FDTP;
2. launch a headless render session for a route, viewport, target, theme, and locale.

Required inputs:

```text
route
target
viewport width/height/scale
theme
locale
platform text scale
platform safe areas
```

Required output:

- widget tree;
- source locations;
- layout bounds;
- constraints;
- display list summary;
- semantics;
- hit-test regions;
- focus order;
- overflow diagnostics;
- accessibility diagnostics where available;
- screenshot artifact ID.

This is the primary tool agents should use before making layout changes. It gives the agent a framework-aware view instead of forcing it to infer layout from screenshots alone.

## 15. Screenshot oracle

The screenshot oracle should capture and compare route output.

Capabilities:

- capture a route at a target, viewport, theme, and locale;
- compare against a named baseline;
- produce pixel diff images;
- produce structural diff summaries from layout/display-list data;
- ignore configured unstable regions such as timestamps;
- output artifacts that CI can upload;
- return machine-readable pass/fail details.

Screenshot comparison should not be the only source of truth. It should be paired with layout and semantics data so agents can distinguish "looks different" from "tree structure changed", "hit testing changed", or "accessibility changed".

## 16. Test runner

The Agent Kit should wrap existing Fission and Cargo test flows and return structured results.

Required profiles:

```text
unit
integration
smoke
visual
site
platform
release-preflight
```

Example output:

```json
{
  "profile": "smoke",
  "status": "failed",
  "duration_ms": 18422,
  "failures": [
    {
      "kind": "layout",
      "route": "/settings",
      "message": "Primary button is outside the safe area",
      "artifact": "fission://diagnostic/layout/settings-safe-area"
    }
  ]
}
```

Agents should be able to run the smallest relevant profile first, then escalate to broader checks when a patch touches shared infrastructure.

## 17. Action and reducer tracing

The action/reducer tracer should explain how user input changes app state.

Trace entries should include:

- input event or synthetic action source;
- action type and payload summary;
- route and focused widget;
- reducer function and source location;
- previous state hash;
- next state hash;
- changed fields where safe to expose;
- selector invalidations;
- effects requested;
- jobs, services, commands, resources, and capabilities invoked;
- host responses where recorded or mocked;
- diagnostics and errors.

Replay should be deterministic when every host interaction is mocked or recorded. If a trace contains non-replayable host effects, the tool should mark them explicitly and stop before pretending the replay is complete.

## 18. `propose_fix`

`propose_fix` should be a patch generator, not an autonomous editor by default.

Inputs:

- diagnostic artifact;
- optional route;
- optional source span;
- optional expected outcome;
- optional test profile.

Outputs:

- unified diff patch;
- explanation of why the patch is proposed;
- files affected;
- commands to verify;
- risks and assumptions.

`propose_fix` may apply patches only when `allow_write_tools = true` and the caller uses an explicit apply mode.

## 19. Security model

Security must be designed into the Agent Kit from the start.

Required rules:

- read-only tools are enabled by default;
- write tools are disabled by default;
- command execution tools are disabled by default;
- network tools are disabled by default;
- credential access is disabled by default;
- project roots are enforced;
- include/exclude globs are enforced;
- `.gitignore` and Fission agent excludes are respected unless explicitly overridden;
- secrets are redacted in context summaries, traces, and screenshots where possible;
- generated rules must not include credentials or machine-specific secrets;
- MCP server sessions should be local by default through stdio or localhost;
- remote MCP access is out of scope for the first implementation.

The MCP server should produce an audit log for side-effecting operations:

```text
.fission/agent/audit.log
```

The audit log should include timestamp, tool name, side-effect class, files touched, commands run, artifacts produced, and caller identity when available.

## 20. Output schemas and artifacts

All machine-facing commands should support JSON output.

Artifacts should be addressable through stable IDs:

```text
fission://artifact/screenshot/<id>
fission://artifact/diff/<id>
fission://artifact/trace/<id>
fission://artifact/diagnostic/<id>
```

On disk, artifacts should live under:

```text
target/fission/agent/
```

The `.fission/agent/` directory is for local configuration and indexes. `target/fission/agent/` is for rebuildable artifacts.

## 21. Implementation plan

### 21.1 Pass 1: local context and rules

- Add `fission agent init`.
- Add idempotent generation for agent rules files.
- Add `fission agent context --format markdown|json`.
- Add bundled docs lookup for installed Fission docs.
- Add config parsing for `[agent]`.

Acceptance criteria:

- a new project can generate rules without losing user-authored content;
- an agent can read a concise project summary;
- no command runs arbitrary project code.

### 21.2 Pass 2: MCP read-only server

- Add `fission agent mcp`.
- Expose resources for project context, docs, routes, widgets, actions, reducers, selectors, capabilities, design system, tests, and diagnostics.
- Expose prompts for common Fission workflows.
- Expose read-only tools only.

Acceptance criteria:

- a compatible MCP client can discover Fission resources, prompts, and tools;
- the server cannot read outside the configured project root;
- tool/resource output is deterministic across repeated calls when the project is unchanged.

### 21.3 Pass 3: project indexer

- Add deterministic index generation.
- Use `cargo metadata` for workspace/package information.
- Add Fission-specific index extraction.
- Add cache invalidation.
- Add confidence labels for compiler-backed, runtime-backed, generated, and advisory findings.

Acceptance criteria:

- the index identifies route, widget, action, reducer, selector, capability, design-system, test, and documentation locations in real Fission apps;
- deleting the index and rebuilding produces equivalent output.

### 21.4 Pass 4: runtime inspection bridge

- Connect agent tools to FDTP.
- Add `inspect-layout`, `render-route`, and `screenshot`.
- Add headless route rendering where the target supports it.
- Return layout, semantics, hit-test, screenshot, and diagnostic artifacts.

Acceptance criteria:

- an agent can render a route and inspect layout without parsing screenshots;
- visual artifacts and structured layout data refer to the same frame.

### 21.5 Pass 5: tests and visual checks

- Add `run-tests`.
- Add `compare-screenshot`.
- Add artifact manifests.
- Add CI-friendly JSON output.

Acceptance criteria:

- CI can run the same checks exposed to MCP;
- an agent can identify which test failed and which artifact explains it.

### 21.6 Pass 6: action/reducer tracing

- Add action/reducer trace capture through FDTP.
- Add `explain-reducer-path`.
- Add `trace-action`.
- Add `replay-action` for recorded or mocked host interactions.

Acceptance criteria:

- an agent can explain how a user action changed state;
- replay refuses to claim success when a trace contains unmocked host effects.

### 21.7 Pass 7: safe generation and patch proposals

- Add component, screen, action, reducer, selector, capability, and test generators.
- Add `propose-fix` patch output.
- Add optional apply mode gated by write permissions.

Acceptance criteria:

- generated code uses public Fission APIs and project conventions;
- patch proposals include verification commands;
- no write occurs unless explicitly enabled.

## 22. Testing requirements

Unit tests:

- config parsing;
- include/exclude matching;
- managed rules-file section replacement;
- resource URI parsing;
- side-effect class enforcement;
- cache invalidation;
- JSON schema serialization;
- docs chunk ranking.

Integration tests:

- `fission agent init` is idempotent;
- MCP resources are discoverable;
- MCP read-only tools cannot escape the project root;
- project index survives deletion and rebuild;
- generated component compiles in a sample app;
- `inspect-layout` returns source-linked bounds for a sample route;
- screenshot capture produces an artifact manifest;
- `run-tests` maps failures into structured JSON;
- write tools are rejected by default.

Smoke tests:

- run the MCP server with a real Fission example;
- ask for project context;
- render one route;
- capture one screenshot;
- run the smoke test profile;
- generate one component patch without applying it.

## 23. Documentation requirements

Documentation should cover:

- why the Agent Kit exists;
- how to initialize agent support in a project;
- how to connect MCP clients;
- how to use the CLI commands directly;
- what data the tools can read;
- which operations can change files or run code;
- how to disable tools;
- how to write project-specific agent rules;
- how to add generated components safely;
- how to use screenshots and traces in code review;
- how to run the same checks in CI.

The docs should be written as practical guides. A developer should be able to follow a guide from an existing Fission app to a working MCP configuration, then verify it by asking the client to inspect a route and run a test.

## 24. Open questions

- Should the first MCP transport be stdio only, or should localhost HTTP/WebSocket be supported immediately for clients that prefer long-running servers?
- Should generated rules use one shared `AGENTS.md` plus client-specific pointers, or should each client receive fully expanded rules?
- Which compiler-backed data source should be the first implementation target for resolved Rust symbol information?
- How much of the screenshot oracle should live in the Agent Kit versus the lower-level developer tooling crates?
- Should CI visual regression storage be implemented as local artifacts only in the first version, or should provider uploads be included later through the post-build lifecycle tooling?

## 25. Consistency checks

The implementation is consistent with this RFC only if the following statements remain true:

- Fission keeps one app model: widgets, reducers, router, design systems, shells, and capabilities remain the source of truth.
- FDTP remains the live runtime inspection protocol.
- The MCP server is an adapter over Fission tooling, not a second runtime.
- Local development does not require a hosted service.
- Read-only agent operations work by default.
- Side-effecting operations require explicit configuration.
- Code generation produces normal Rust files that developers can review and edit.
- Runtime-backed evidence beats static index guesses.
- Static index findings are marked advisory when they are not compiler-backed, generated, or runtime-backed.
- The toolchain is useful from multiple AI clients and editors.

## 26. References

- Model Context Protocol specification: `https://modelcontextprotocol.io/specification/2025-06-18`
- MCP server concepts: `https://modelcontextprotocol.io/docs/learn/server-concepts`
- MCP tools: `https://modelcontextprotocol.io/specification/2025-06-18/server/tools`
- MCP prompts: `https://modelcontextprotocol.io/docs/concepts/prompts`
- Fission developer tooling RFC: `docs/rfc-developer-tooling.md`
