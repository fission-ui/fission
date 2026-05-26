# Fission

[![Crates.io](https://img.shields.io/crates/v/fission.svg)](https://crates.io/crates/fission)
[![Docs](https://img.shields.io/badge/docs-fission.rs-0f766e.svg)](https://fission.rs)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/worka-ai/fission/blob/main/LICENSE)

Fission is a production-focused Rust application framework for building GPU-accelerated apps across desktop, web, Android, iOS, terminal interfaces, and static HTML sites.

This crate is the public facade. Application code should normally depend on `fission` and enable the target or feature it needs from here instead of depending on the internal crates directly.

**Documentation:** [fission.rs](https://fission.rs)
**Repository:** [github.com/worka-ai/fission](https://github.com/worka-ai/fission)

## Install

```toml
[dependencies]
fission = { version = "0.2.0", features = ["desktop"] }
```

For the full developer workflow, install the Fission command:

```sh
cargo install cargo-fission
fission init my-app
cd my-app
fission run
```

## What the facade gives you

| Area | What is exposed |
| --- | --- |
| Application model | `AppState`, `Widget`, `BuildCtx`, `View`, typed actions, reducers, selectors, effects, jobs, services, and capabilities. |
| UI authoring | Core widgets, high-level widgets, icons, layout, portals, overlays, media/embed widgets, charts, 3D scenes, and design-system support. |
| Targets | Desktop, web/WASM, Android, iOS, terminal UI, and static site shells behind feature flags. |
| Platform integration | Notifications, deep links, NFC, biometrics, passkeys, barcode scanning, camera, clipboard, geolocation, haptics, microphone, Bluetooth, Wi-Fi, and volume control where the host supports them. |
| Tooling | The companion `fission` command handles setup, devices, run, test, package, static-site generation, release content, and distribution workflows. |

## Feature flags

Enable only what your app needs:

| Feature | Purpose |
| --- | --- |
| `desktop` | Desktop shell for Windows, macOS, and Linux. |
| `web` | Browser/WASM shell. |
| `android` / `ios` / `mobile` | Mobile shell exports for Android and iOS targets. |
| `site` | Static HTML site shell. |
| `terminal-shell` | Terminal UI shell. |
| `charts` | Fission Charts widgets and data-visualization primitives. |
| `three-d` | 3D scene and embed primitives. |
| `test-driver` | Live app testing client support. |

Portable widgets and core APIs are available from the facade without making application developers wire internal crates together manually.

## A small Fission app

```rust
use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CounterState {
    count: i32,
}

impl AppState for CounterState {}

#[fission_reducer(Increment)]
fn increment(state: &mut CounterState) {
    state.count += 1;
}

struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let increment = with_reducer!(ctx, Increment, increment);

        Container::new(
            Column {
                gap: Some(20.0),
                children: vec![
                    Text::new("Counter").size(32.0).into_node(),
                    Text::new(format!("{}", view.state.count)).size(56.0).into_node(),
                    Button {
                        on_press: Some(increment),
                        child: Some(Box::new(Text::new("Increment").into_node())),
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(32.0)
        .into_node()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::new(CounterApp).run()
}
```

Use `#[fission_reducer]` for compact local actions, or `#[fission_action]` when you want a named action type that is shared across modules or documented as part of your app API.

## Lifecycle workflow

The facade gives application code one dependency. The `fission` command gives developers one workflow:

```sh
fission init my-app
fission add-target web android ios
fission devices
fission run --target web
fission test --target web
fission site build --project-dir documentation --release
fission package --project-dir . --target windows --format msix --release
fission distribute --project-dir . --provider github-releases --artifact target/fission/release/windows/msix/artifact-manifest.json
```

## Documentation

Start at [fission.rs](https://fission.rs):

- [Quickstart](https://fission.rs/docs/learn/quickstart/)
- [App structure](https://fission.rs/docs/guides/app-structure/)
- [Widgets and layout](https://fission.rs/docs/guides/layout-and-widgets/)
- [Design systems](https://fission.rs/docs/guides/design-system/)
- [Charts](https://fission.rs/docs/charts/overview/)
- [Platform capabilities](https://fission.rs/docs/guides/platform-capabilities/)
- [Static sites](https://fission.rs/docs/guides/static-sites/)
- [Terminal user interfaces](https://fission.rs/docs/guides/terminal-user-interfaces/)
- [Build and package](https://fission.rs/docs/build-and-package/overview/)
- [Release and distribute](https://fission.rs/docs/release-and-distribute/overview/)

## License

Fission is available under the MIT license.
