# Fission website and tutorials

`docs2` is the Compose-first Fission website. The marketing pages, documentation
hub, tutorial index, and tutorial screens use `fission_component!`; reference
pages and guides are Markdown content rendered by the Fission site template.

## Run the site locally

From the Fission workspace root:

```console
fission site serve --project-dir docs2
```

The command first extracts each tutorial's complete source files and builds the
Web previews as WASM. A tutorial with invalid Rust fails the build instead of
showing a preview that does not match its code. Install `wasm-pack` to build the
previews.

Unique tutorial previews are collected into generated Cargo workspaces and
compiled into a shared target cache. The web and charts previews use separate
workspaces so their Cargo features stay isolated. Cargo compiles up to ten
preview apps at once by default, then the site packages and optimizes their
WASM previews with the same number of workers. Set
`FISSION_TUTORIAL_BUILD_PARALLELISM` to a value from `1` to `64` to tune CPU and
memory use. The site serving startup deadline defaults to 900 seconds; set
`FISSION_SERVING_STARTUP_TIMEOUT_SECONDS` to a value from `1` to `3600` when a
complete site build needs more time. The same timeout can be set with
`--startup-timeout-seconds` on the CLI.

For example:

```sh
FISSION_TUTORIAL_BUILD_PARALLELISM=10 \
FISSION_SERVING_STARTUP_TIMEOUT_SECONDS=3600 \
fission site serve --project-dir docs2
```

## Tutorial authoring

Goal tutorials are Markdown files under `content/goals`. Their frontmatter
defines SEO metadata, a canonical route, prerequisites, outcomes, and ordering.
Use the content markers to define the runnable application:

- `fission:step` creates a cumulative tutorial step.
- `fission:file` marks a complete source file extracted for that step.
- `fission:preview` places the compiled Web app beside the step's explanation.
- `fission:checkpoint` gives the learner an observable result to verify.

Each later step inherits files from earlier steps and replaces files it declares
again. Author app UI with `fission_component!`; put durable product data in
`GlobalState`, local interaction state in the owning component, and asynchronous
work in an effect, job, service, capability, or resource.

## Site structure

- `src/app.rs` configures routes, SEO metadata, structured data, theme, and locale.
- `src/screens` composes the homepage, documentation hub, tutorials, and goal pages.
- `src/widgets` contains reusable site components.
- `src/content.rs` parses tutorial frontmatter and step markers.
- `src/tutorials.rs` extracts tutorial source and builds its Web previews.
- `content/legacy/docs` preserves the previous documentation URLs.
- `content/reference` contains the reference section under the new menu structure.
- `content/compose` explains the recommended authoring API.
- `design` and `i18n` contain checked-in design-system and translation sources.

Visual review references are in `design/reference`: the SEO homepage, the
documentation hub, and the tutorial page. `examples/visual_review.rs` provides a
LiveTest browser controller for inspecting real rendered routes at desktop and
mobile sizes.
