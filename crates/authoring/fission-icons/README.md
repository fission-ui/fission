# fission-icons

Compile-time Material Design icon access for the Fission UI framework.

This crate provides the complete Material Design icon set as inline SVG strings, generated at build time from the icon source files. Icons are organized by category and name, with multiple style variants per icon.

## Architecture

The crate uses a `build.rs` script that scans the Material Design icon directory tree and generates a Rust module hierarchy. Each icon becomes a `const fn` that returns `&'static str` containing the SVG markup (the 24px variant).

The generated code is included via `include!(concat!(env!("OUT_DIR"), "/material_icons.rs"))` inside the `material` module.

## Icon sources

The build script searches for icons in this priority order:

1. `FISSION_MATERIAL_ICONS_DIR` environment variable (override path).
2. `material-icons-vendor/index.json` -- a pre-built JSON index of icon paths.
3. `material-icons-vendor/src/` -- vendored icon SVG files.
4. `material-design-icons/src/` -- the full Material Design Icons git submodule.

## Module structure

Icons are organized as `material::<category>::<icon_name>::<variant>()`:

```
material::
  action::
    home::
      regular()      -> &'static str  (SVG)
      outlined()     -> &'static str
      round()        -> &'static str
      sharp()        -> &'static str
      two_tone()     -> &'static str
    delete::
      regular()
      outlined()
      ...
  navigation::
    close::
      regular()
      ...
    expand_more::
      regular()
      ...
  alert::
    error::
      regular()
      ...
  ...
```

## Variants

Each icon may have up to five style variants, mapped from the Material Design directory names:

| Variant function | Source directory |
|-----------------|-----------------|
| `regular()` | `materialicons/` |
| `outlined()` | `materialiconsoutlined/` |
| `round()` | `materialiconsround/` |
| `sharp()` | `materialiconssharp/` |
| `two_tone()` | `materialiconstwotone/` |

## Usage

```rust
use fission_icons::material;
use fission_widgets::Icon;

// Get the SVG string for the "close" icon
let svg: &str = material::navigation::close::regular();

// Use with the Icon widget
let icon = Icon::svg(material::action::home::regular())
    .size(24.0)
    .color(theme.tokens.colors.on_surface)
    .into();

// Outlined variant
let icon = Icon::svg(material::action::delete::outlined())
    .size(20.0)
    .into();
```

## Reflection

When the `reflection` feature is enabled, the crate generates an `all_icons()` function that returns a `Vec` of `(category, icon_name, variant, fn() -> &'static str)` tuples. This is useful for icon browsers and development tools.

```rust
#[cfg(feature = "reflection")]
let icons = fission_icons::material::all_icons();
for (category, name, variant, get_svg) in &icons {
    println!("{}/{}/{}", category, name, variant);
    let _svg: &str = get_svg();
}
```

## Build configuration

| Environment variable | Purpose |
|---------------------|---------|
| `FISSION_MATERIAL_ICONS_DIR` | Override path to the icon source directory. |

The build script emits `cargo:rerun-if-changed` directives so icons are only regenerated when the source files change.
