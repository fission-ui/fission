# Product Browser

Product Browser is a small commerce-style app that demonstrates async loading, refresh behavior, remote images, search, category filters, grid/list layout, and detail selection against real API data.

Use this example when you want to learn how Fission apps fetch data, represent loading/error/success states, and keep a responsive UI around asynchronous jobs.

## Run it

```bash
cargo run -p product-browser
```

## What to look at

- [`src/main.rs`](src/main.rs) registers the async product and category jobs on the desktop shell.
- [`src/api.rs`](src/api.rs) contains the DummyJSON request and response types.
- [`src/model.rs`](src/model.rs) defines the app state, product/category models, async result reducers, search, filters, and selection.
- [`src/components/browser.rs`](src/components/browser.rs) composes the main application shell.
- [`src/components/categories.rs`](src/components/categories.rs) renders the category rail.
- [`src/components/product_results.rs`](src/components/product_results.rs) owns the result region and refresh behavior.
- [`src/components/product_card.rs`](src/components/product_card.rs) shows each product tile, including remote image use.
- [`src/components/product_detail.rs`](src/components/product_detail.rs) renders the selected product details.

## Features exercised

- `FutureBuilder` for typed async jobs.
- `RefreshIndicator` for user-triggered reloads.
- Remote `Image` sources and image loading state.
- `Grid` layout for product tiles.
- Search, category filtering, and selected-item detail state.
- Live screenshot/e2e testing through `fission-test-driver`.

## Tests

```bash
cargo test -p product-browser
```

The live test is ignored by default. Run it explicitly when you want screenshot verification:

```bash
cargo test -p product-browser --test live_e2e -- --ignored --nocapture
```

## Learning path

Start in [`src/main.rs`](src/main.rs) to see the async jobs being registered. Then follow `PRODUCTS_JOB` into [`src/api.rs`](src/api.rs) and [`src/model.rs`](src/model.rs). Finally, inspect [`src/components/product_results.rs`](src/components/product_results.rs) to see how loading, refresh, and loaded data become a user-facing grid.
