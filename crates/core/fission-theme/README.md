# fission-theme

Design token system and component themes for the Fission UI framework.

This crate defines the complete visual language for Fission applications: colors, spacing, typography, corner radii, elevations (box shadows), and per-component theme overrides. It follows the Material Design 3 token architecture, where a small set of primitive tokens cascade into component-specific values.

## Architecture

```
Tokens (primitive design tokens)
  +-- ColorTokens       (primary, surface, error, text, border)
  +-- SpacingTokens      (none, xs, s, m, l, xl)
  +-- TypographyTokens   (label, body, heading sizes)
  +-- RadiusTokens       (small, medium, large, full)
  +-- ElevationTokens    (level0..level5 box shadows)
       |
       v
ComponentTheme (derived per-widget themes)
  +-- ButtonTheme
  +-- TextInputTheme
  +-- ModalTheme
  +-- TabsTheme
  +-- TooltipTheme
  +-- ... (13 component themes total)
       |
       v
Theme (top-level struct combining tokens + components)
```

Every component theme provides a `from_tokens()` constructor that derives all values from the primitive tokens. This means switching from light to dark mode is a single call to `Theme::dark()` -- all component values update automatically.

## Design tokens

### `ColorTokens`

| Token | Light default | Dark default | Purpose |
|-------|--------------|-------------|---------|
| `primary` | Purple 40 | Purple 80 | Primary interactive color |
| `on_primary` | White | Black | Text/icon on primary surfaces |
| `secondary` | Gray-purple | Teal | Secondary interactive color |
| `surface` | Near-white | Dark gray (30) | Card and container backgrounds |
| `background` | Near-white | Near-black (18) | Page background |
| `error` | Red | Pink-red | Error states |
| `border` | Light gray | Dark gray (60) | Borders and dividers |
| `text_primary` | Near-black | Light gray | Primary text color |
| `text_secondary` | Medium gray | Medium gray | Secondary/helper text |

### `SpacingTokens`

Standard spacing scale: `none` (0), `xs` (4), `s` (8), `m` (16), `l` (24), `xl` (32).

### `TypographyTokens`

Font size scale: `label_large_size` (15), `body_medium_size` (15), `body_large_size` (17), `heading_size` (28).

### `RadiusTokens`

Corner radius scale: `small` (4), `medium` (8), `large` (12), `full` (9999 -- fully rounded).

### `ElevationTokens`

Six elevation levels (0-5). Levels 0, 4, and 5 are `None` (no shadow). Levels 1-3 provide progressively stronger `BoxShadow` values with increasing blur and offset.

## Component themes

Each component theme struct holds the visual parameters that a specific widget reads during rendering. All are constructed from `Tokens` via `from_tokens()`.

| Theme struct | Used by | Key fields |
|-------------|---------|------------|
| `ButtonTheme` | `Button` | height, padding, radius, text size, elevation states, focus stroke |
| `TextInputTheme` | `TextInput` | height, padding, radius, font size, border/focus/text/placeholder colors |
| `ModalTheme` | `Modal` | background color, radius, shadow, max width |
| `TabsTheme` | `Tabs` | active/inactive colors, indicator height, background, divider |
| `BadgeTheme` | `Badge` | radius, font size |
| `AlertTheme` | `Alert` | per-severity background colors, radius |
| `ProgressTheme` | `ProgressBar` | height, track color, bar color |
| `TooltipTheme` | `Tooltip` | background, text color, radius, font size |
| `CalendarTheme` | `Calendar` | background, border, radius, selected/today colors |
| `PaginationTheme` | `Pagination` | spacing, active background/text |
| `TimelineTheme` | `Timeline` | dot size, line width, dot/line colors |
| `SegmentedControlTheme` | `SegmentedControl` | background, border, radius, active colors |
| `TreeViewTheme` | `TreeView` | indent, selected/hover backgrounds |

## Bundled fonts

The `fonts` module embeds Noto Sans Regular and the default Inter family:

- `NOTO_SANS_REGULAR_TTF` -- Noto Sans Regular
- `INTER_24PT_REGULAR_TTF` -- Inter 24pt Regular (400)
- `INTER_24PT_MEDIUM_TTF` -- Inter 24pt Medium (500)
- `INTER_24PT_SEMIBOLD_TTF` -- Inter 24pt SemiBold (600)
- `INTER_24PT_BOLD_TTF` -- Inter 24pt Bold (700)

`default_font_faces()` returns the weighted faces registered by interactive and
test shells. `default_font_bytes()` remains available for single-font consumers
and returns the regular Inter face.

## Usage

```rust
use fission_theme::{Theme, Tokens};

// Light theme (default)
let light = Theme::default();

// Dark theme
let dark = Theme::dark();

// Custom theme from modified tokens
let mut tokens = Tokens::default();
tokens.colors.primary = Color { r: 0, g: 120, b: 212, a: 255 }; // Blue
let custom = Theme {
    tokens: tokens.clone(),
    components: ComponentTheme::from_tokens(&tokens),
};
```

## Serialization

All types implement `Serialize` and `Deserialize` (via serde), so themes can be stored in JSON or any other serde-compatible format.
