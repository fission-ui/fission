# Chart Gallery

Chart Gallery is the main visual testbed for `fission-charts`. It presents a large catalogue of chart families, chart variants, chart data shapes, and chart styling options in one production-style app instead of isolated one-off demos.

Use this example when you want to see how chart series are described, how the gallery organizes a large chart catalogue, and how chart-specific state can drive chart configuration without hard-coding a separate app for every variant.

## Run it

```bash
cargo run -p chart-gallery
```

## What to look at

- [`src/app.rs`](src/app.rs) owns the top-level application shell and route-style composition.
- [`src/charts/catalog.rs`](src/charts/catalog.rs) defines the chart families and variants shown in the gallery.
- [`src/charts/gallery.rs`](src/charts/gallery.rs) renders the interactive gallery surface.
- [`src/charts/docs.rs`](src/charts/docs.rs) contains explanatory copy and chart-family descriptions used inside the app.
- [`src/data.rs`](src/data.rs) provides deterministic sample datasets for chart rendering.
- [`src/state.rs`](src/state.rs) contains the gallery state and reducer-facing model.
- [`src/style.rs`](src/style.rs) centralizes chart-gallery colors, spacing, and shared visual helpers.

## Features exercised

- `fission-charts` series, datasets, chart families, and variants.
- Large catalogue organization across Rust modules instead of a single oversized file.
- Gallery state, selected families, selected variants, and chart configuration controls.
- Desktop shell rendering with chart and 3D features enabled through [`Cargo.toml`](Cargo.toml).

## Learning path

Start in [`src/charts/catalog.rs`](src/charts/catalog.rs) to understand the data model, then open [`src/charts/gallery.rs`](src/charts/gallery.rs) to see how catalogue entries become UI. The separation is intentional: real apps should keep domain/catalogue data separate from rendering code once an example grows beyond a single screen.
