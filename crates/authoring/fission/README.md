# Fission

[![Crates.io](https://img.shields.io/crates/v/fission.svg)](https://crates.io/crates/fission)
[![Docs](https://img.shields.io/badge/docs-fission.rs-0f766e.svg)](https://fission.rs)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/fission-ui/fission/blob/main/LICENSE)

Fission is a production-focused Rust application framework for building GPU-accelerated apps across macOS, Windows, Linux, Web, Android, iOS, Terminal, Static site, and SSR targets.

This crate is the public facade. Application code should normally depend on `fission` and enable the target or feature it needs from here instead of depending on the internal crates directly.

**Documentation:** [fission.rs](https://fission.rs)
**Repository:** [github.com/fission-ui/fission](https://github.com/fission-ui/fission)

## Install

```toml
[dependencies]
fission = { version = "0.5.1", features = ["desktop"] }
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
| Application model | `GlobalState`, `Widget`, `BuildCtxHandle`, `ViewHandle`, typed actions, reducers, selectors, effects, jobs, services, and capabilities. |
| UI authoring | Core widgets, high-level widgets, icons, layout, portals, overlays, media/embed widgets, charts, 3D scenes, and design-system support. |
| Targets | macOS, Windows, Linux, Web, Android, iOS, Terminal, Static site, and SSR shells behind feature flags. |
| Platform integration | Notifications, deep links, NFC, biometrics, passkeys, barcode scanning, camera, clipboard, geolocation, haptics, microphone, Bluetooth, Wi-Fi, and volume control where the host supports them. |
| Tooling | The companion `fission` command handles setup, devices, run, test, package, static-site generation, release content, and distribution workflows. |

## Feature flags

Enable only what your app needs:

| Feature | Purpose |
| --- | --- |
| `desktop` | Desktop shell for macOS, Windows, and Linux. |
| `web` | Browser/WASM shell. |
| `android` / `ios` / `mobile` | Mobile shell exports for Android and iOS targets. |
| `site` | Static site shell. |
| `server` | SSR shell. |
| `terminal-shell` | Terminal shell. |
| `charts` | Fission Charts widgets and data-visualization primitives. |
| `three-d` | 3D scene and embed primitives. |
| `test-driver` | Live app testing client support. |

Portable widgets and core APIs are available from the facade without making application developers wire internal crates together manually.

## A small Fission app

```rust
use fission::prelude::*;

#[fission_component]
struct CounterApp {
    #[local_state(default = 0)]
    count: i32,
}

#[fission_reducer(Increment)]
fn increment(count: &mut i32) {
    *count += 1;
}

impl From<CounterApp> for Widget {
    fn from(counter: CounterApp) -> Widget {
        let (ctx, _) = fission::build::current::<()>();
        let count = counter.count();
        let increment = ctx.bind_local(Increment, count.clone(), reduce!(increment));

        Container::new(Column {
            gap: Some(20.0),
            children: widgets![
                Text::new("Counter").size(32.0),
                Text::new(format!("{}", count.get())).size(56.0),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(32.0)
        .into()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::<(), _>::new(CounterApp {}).run()
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
