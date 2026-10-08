<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Setup and browser targets

`fission init my-site` creates a Rust counter app and the host desktop target.
Read the instruction paths printed by init immediately. For a local checkout use
`fission init my-site --local-path /path/to/fission`; this does not prove the
checkout matches the bundled API revision.

| Requirement | Commands from the app directory |
| --- | --- |
| Build-time widget/content pages without runtime state | `fission add-target static-site`, `fission site check`, `fission site build`, `fission site serve --no-open` |
| Stateful browser UI | `fission add-target web`, `fission build --target web`, `fission run --target web --no-open`, `fission test --target web` |

All shown commands accept `--project-dir PATH`. Bare build/run default to the
host desktop target. If a particular browser/device is requested, inspect
`fission devices --project-dir . --json` and pass its actual id through
`fission run --target web --device ID`. Check Web tools with
`fission doctor web --project-dir . --strict`.

Adding `static-site` creates content/platform support and enables the
`fission/site` dependency feature; it does not convert the counter into a
custom website entrypoint. Markdown/MDX in `content/` renders through Fission
widgets without a custom binary. For authored Rust pages, add `src/bin/site.rs`
and invoke the real site builder directly:
`cargo run --bin site -- build --project-dir .` (or `check`, `routes`, `serve`).
This keeps generated `fission.toml` intact. Minimal custom page:

```rust
use fission::prelude::*;
use fission::site::{build_from_cli, FissionSite};
#[derive(Clone)]
struct Home;
impl From<Home> for Widget {
    fn from(_: Home) -> Widget {
        Column { children: widgets![Text::new("Welcome")], ..Default::default() }.into()
    }
}
fn main() -> anyhow::Result<()> {
    build_from_cli(FissionSite::new().route_widget::<(), _>("/", "Home", None, Home))
}
```

In this base, `[site].entry` only switches the CLI to a normal `cargo run`
builder path; the CLI does not select a named binary or add a `static-site`
Cargo feature. Existing apps whose normal entrypoint is a site builder can use
that path. For the separate binary above, keep using the explicit Cargo command.
Do not assume a pending website-template scaffold is present.

Static rendering rejects unsupported interactive widgets; select Web for
reducers/local state and dynamic browser interaction. The Web scaffold exposes
a WASM library entrypoint, uses `wasm-pack`, and provides a browser shell.
Keep its bootstrap/shell functions and author UI in Rust.

SSR is a separate target outside this guide. Static-site and Web do not imply
a backend. Hosting/GitHub Pages is a separate task. Read generated
`platforms/site/README.md` or `platforms/web/README.md` for scaffold paths.
