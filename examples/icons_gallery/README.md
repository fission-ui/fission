# Icons Gallery

Icons Gallery browses the Material icon set exposed through the Fission facade. The set holds over ten thousand variants, so the app is a worked example of making a very long list findable: a search field narrows it by icon name, a row of category chips narrows it by group, a count says how much is left, and an empty state explains a query that matches nothing. The surviving rows render through `LazyColumn` so the list stays smooth at any length.

Use this example when you want to browse icon names, or to see how search, filtering, and a large list fit together.

## Run it

```bash
cargo run -p icons_gallery
```

## What to look at

- [`src/lib.rs`](src/lib.rs) assembles the screen: title, controls, list.
- [`src/model.rs`](src/model.rs) holds the `State` (query and category), the `SearchChanged` and `CategorySelected` reducers, and `filter_icons`, which walks `fission::icons::material::all_icons()` once to collect the categories, the surviving rows, and the total.
- [`src/icon_gallery_controls.rs`](src/icon_gallery_controls.rs) shows `TextInput` bound to a reducer, `Tag` chips as a single-select filter, and the result count. The chips wrap on a wide window and scroll sideways on a narrow one.
- [`src/icon_gallery_list.rs`](src/icon_gallery_list.rs) chooses between `LazyColumn` and `EmptyState`.
- [`src/icon_gallery_row.rs`](src/icon_gallery_row.rs) shows `Icon::svg(...)` with a fixed row height.

## Features exercised

- Material icon reflection through the `icons-reflection` feature in [`Cargo.toml`](Cargo.toml).
- `Icon::svg(...)` rendering.
- `LazyColumn` for large fixed-height lists, and `EmptyState` when the filters leave nothing.
- `#[fission_reducer]` with `with_reducer!` for the query and the category.
- Simple desktop app startup through `DesktopApp`.

## Learning path

Start with `filter_icons` in `src/model.rs`: it is the whole search feature, and everything on screen is derived from what it returns. Then read `icon_gallery_controls.rs` to see how a text field and a chip row dispatch actions into that state. If you only want to use icons in your own app, the key line is `Icon::svg(func()).size(24.0).into()` in `icon_gallery_row.rs`.
