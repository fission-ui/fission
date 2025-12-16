# Theming System (v1)

This document defines the **v1 theming system** for the framework, with explicit goals:
- widgets inherit sensible defaults and override only what they need,
- the same widget code can render with **Material** defaults on Android and **Cupertino** defaults on iOS,
- theming remains **deterministic**, testable, and compatible with headless execution.

The theming system is intentionally split into **Tokens** (design atoms) and **Component Defaults** (per-widget styles).
Platform “look-and-feel” is provided via **Theme Packs**.

---

## 1. Design Requirements

### 1.1 Inheritance and Overrides
- Widgets should be able to omit most styling fields.
- Styling should be computed by merging:
  1) widget overrides,
  2) component defaults from the active theme,
  3) fallback tokens.

### 1.2 Cross-Platform Look Without Rewriting Widgets
- The same widgets must be able to adopt different platform defaults.
- The platform shell should be able to provide a default theme pack:
  - Android → Material pack
  - iOS → Cupertino pack
  - Desktop/Web → default pack (configurable)

### 1.3 Determinism and Testability
- The resolved style must be a **pure function** of:
  - widget properties,
  - theme data,
  - environment (DPI, color scheme, etc.), all explicit.
- No implicit global theme state.
- Theme selection and overrides are observable in snapshots.

---

## 2. Core Concepts

### 2.1 Tokens (Design Atoms)
**Tokens** are primitive design values used across the system:
- colors
- typography scale
- spacing scale
- radii
- elevation/shadows (as data parameters)

Tokens are pure data and small enough to be stable.

Example (illustrative):

```rust
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub color: ColorTokens,
    pub typography: TypographyTokens,
    pub spacing: SpacingTokens,
    pub radius: RadiusTokens,
    pub elevation: ElevationTokens,
}
```

### 2.2 Component Defaults
Component defaults define how a specific widget looks by default. Examples:
- `ButtonTheme`
- `TextTheme`
- `SurfaceTheme`
- `InputTheme`

Component defaults are derived from tokens but stored explicitly for fast resolution and clarity.

```rust
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentTheme {
    pub button: ButtonTheme,
    pub text: TextTheme,
    pub surface: SurfaceTheme,
    // ...
}
```

### 2.3 Theme
A **Theme** is the combination of tokens and component defaults.

```rust
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub tokens: Tokens,
    pub components: ComponentTheme,
}
```

Themes are immutable values (copied by sharing) and can be snapshotted.

---

## 3. Theme Packs (Material/Cupertino)

A **Theme Pack** is a versioned bundle that provides a complete `Theme`:
- tokens tuned to the design language,
- component defaults aligned to that design language,
- optional minor policy knobs (strictly limited; see below).

Theme packs are chosen at the platform boundary, but may be overridden by the app.

### 3.1 Theme Pack Selection
Default selection (recommended):
- Android shell: `MaterialThemePack::default()`
- iOS shell: `CupertinoThemePack::default()`
- Desktop/Web shell: `DefaultThemePack::default()` (or configurable)

The selection is explicit input to the Core Runtime (part of the environment).

### 3.2 What Theme Packs May and May Not Change
Theme packs **may** change:
- colors, typography, radii, spacing, elevations
- per-component padding, border widths, corner radii
- default icon sizes and color usage
- default pressed/hover/disabled visual treatments (as style parameters)

Theme packs **must not** change (without an explicit opt-in runtime setting):
- semantics roles and actions,
- layout algorithm semantics (flex/scroll rules),
- input behavior,
- time and animation semantics.

If a platform needs behavioral differences (e.g. scroll physics), it must be configured via dedicated runtime services, not theme.

---

## 4. Theme Inheritance and Scoping

### 4.1 ThemeScope Node
Theme inheritance is explicit via a `ThemeScope` wrapper node:

```rust
pub struct ThemeScope {
    pub theme: ThemeOverride,
    pub child: Box<Node>,
}
```

A `ThemeScope` merges its override with the inherited theme and applies the result to its subtree.

### 4.2 ThemeOverride
Overrides are partial and field-wise:

```rust
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeOverride {
    pub tokens: Option<TokensOverride>,
    pub components: Option<ComponentThemeOverride>,
}
```

Where override types use `Option<T>` for each field.

This supports:
- “override only what you want”
- small targeted adjustments
- predictable merge behavior

---

## 5. Widget-Level Styling Model

### 5.1 Override Types
Widgets have override fields rather than full resolved styles.

Example:

```rust
#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Button {
    pub style: Option<ButtonStyleOverride>,
    // ...
}
```

`ButtonStyleOverride` might include:
- background color override
- text style override
- padding override
- border override
- corner radius override
- state variants override (hover/pressed/disabled)

### 5.2 Deterministic Merge Order
The resolved style is computed as:

1. start from `theme.components.button.base` (resolved default),
2. apply widget overrides field-wise,
3. apply fallback to tokens for any unspecified derived values (only when required).

This yields a single resolved `ButtonStyle` used for layout and paint.

### 5.3 State Variants
State variants are style parameters, not behavior.

Example states:
- hovered
- pressed
- focused
- disabled

Widget state and input state determine which variant applies.
The variant selection rule is deterministic.

---

## 6. Theme as Explicit Runtime Input

The Core Runtime receives:
- the active theme (or theme pack id + resolved theme),
- environment (DPI, color scheme, accessibility settings) explicitly.

The theme is included in snapshots:
- for deterministic reproduction of layout/paint
- to make test failures explainable

---

## 7. Testing Theming

The test harness can:
- set theme pack explicitly
- override theme via `ThemeScope`
- assert resolved styles (optional instrumentation)

Suggested test APIs (illustrative):
- `harness.set_theme(MaterialThemePack::default())`
- `find("increment_button").style().background_color()`
- snapshot diffs that include theme identity/version

Pixel tests should pin:
- theme pack version
- fonts and locale
- viewport and DPR

---

## 8. Versioning and Compatibility

### 8.1 Theme Pack Versioning
Theme packs are versioned as data:
- `MaterialPack@v1`
- `CupertinoPack@v1`

Changes to defaults that affect appearance should:
- bump the pack version, or
- be opt-in via app configuration.

### 8.2 Theme Data Compatibility
`Theme`, `Tokens`, and component theme structs are part of the compatibility surface:
- avoid breaking field renames without migrations,
- prefer additive changes,
- maintain deterministic merge semantics.

---

## 9. Summary

v1 theming is built on a strict separation:

- **Tokens**: primitives (atoms), stable and small
- **Component Defaults**: per-widget defaults derived from tokens
- **Theme Packs**: platform-aligned bundles (Material/Cupertino)
- **ThemeScope + Overrides**: explicit inheritance and local customization

This design achieves:
- ergonomic “override only what you need,”
- platform look-and-feel without rewriting widgets,
- deterministic, testable styling across platforms and CI.
