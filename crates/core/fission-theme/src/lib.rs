//! Design token system and component themes for the Fission UI framework.
//!
//! This crate defines the complete visual language: colors, spacing, typography,
//! corner radii, elevations (box shadows), and per-component theme overrides.
//! It follows the Material Design 3 token architecture.
//!
//! # Usage
//!
//! ```rust,ignore
//! use fission_theme::Theme;
//!
//! let light = Theme::default();
//! let dark = Theme::dark();
//! ```

use fission_ir::op::{BoxShadow, Color, Stroke};
use serde::{Deserialize, Serialize};

/// Semantic color palette for the application.
///
/// Provides primary, secondary, surface, background, error, border, and text
/// colors. Each color has an `on_*` counterpart for content displayed on that
/// surface (e.g., `on_primary` is the text/icon color used on `primary` backgrounds).
///
/// The [`Default`] implementation provides a light theme. Use [`ColorTokens::dark()`]
/// for dark mode colors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorTokens {
    pub primary: Color,
    pub on_primary: Color,
    pub secondary: Color,
    pub on_secondary: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub background: Color,
    pub on_background: Color,
    pub error: Color,
    pub on_error: Color,
    pub border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
}

impl Default for ColorTokens {
    fn default() -> Self {
        Self {
            primary: Color { r: 103, g: 85, b: 143, a: 255 }, // Purple 40
            on_primary: Color::WHITE,
            secondary: Color { r: 98, g: 91, b: 113, a: 255 },
            on_secondary: Color::WHITE,
            surface: Color { r: 255, g: 251, b: 254, a: 255 },
            on_surface: Color { r: 28, g: 27, b: 31, a: 255 },
            background: Color { r: 255, g: 251, b: 254, a: 255 },
            on_background: Color { r: 28, g: 27, b: 31, a: 255 },
            error: Color { r: 179, g: 38, b: 30, a: 255 },
            on_error: Color::WHITE,
            border: Color { r: 188, g: 188, b: 188, a: 255 },
            text_primary: Color { r: 28, g: 27, b: 31, a: 255 },
            text_secondary: Color { r: 86, g: 86, b: 86, a: 255 },
        }
    }
}

impl ColorTokens {
    pub fn dark() -> Self {
        Self {
            primary: Color { r: 187, g: 134, b: 252, a: 255 },
            on_primary: Color { r: 0, g: 0, b: 0, a: 255 },
            secondary: Color { r: 3, g: 218, b: 197, a: 255 },
            on_secondary: Color { r: 0, g: 0, b: 0, a: 255 },
            surface: Color { r: 30, g: 30, b: 30, a: 255 },
            on_surface: Color { r: 230, g: 230, b: 230, a: 255 },
            background: Color { r: 18, g: 18, b: 18, a: 255 },
            on_background: Color { r: 230, g: 230, b: 230, a: 255 },
            error: Color { r: 207, g: 102, b: 121, a: 255 },
            on_error: Color { r: 0, g: 0, b: 0, a: 255 },
            border: Color { r: 60, g: 60, b: 60, a: 255 },
            text_primary: Color { r: 230, g: 230, b: 230, a: 255 },
            text_secondary: Color { r: 160, g: 160, b: 160, a: 255 },
        }
    }
}

/// Standard spacing scale used for padding, margins, and gaps.
///
/// Values: `none` (0), `xs` (4), `s` (8), `m` (16), `l` (24), `xl` (32).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpacingTokens {
    pub none: f32, // 0
    pub xs: f32,   // 4
    pub s: f32,    // 8
    pub m: f32,    // 16
    pub l: f32,    // 24
    pub xl: f32,   // 32
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            none: 0.0,
            xs: 4.0,
            s: 8.0,
            m: 16.0,
            l: 24.0,
            xl: 32.0,
        }
    }
}

/// Font size scale for text elements.
///
/// Sizes: `label_large_size` (15), `body_medium_size` (15), `body_large_size` (17),
/// `heading_size` (28).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyTokens {
    pub label_large_size: f32,
    pub body_medium_size: f32,
    pub body_large_size: f32,
    pub heading_size: f32,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        Self {
            label_large_size: 15.0,
            body_medium_size: 15.0,
            body_large_size: 17.0,
            heading_size: 28.0,
        }
    }
}

/// Corner radius scale for rounded containers.
///
/// Values: `small` (4), `medium` (8), `large` (12), `full` (9999 -- fully rounded pill).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadiusTokens {
    pub small: f32,
    pub medium: f32,
    pub large: f32,
    pub full: f32,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            small: 4.0,
            medium: 8.0,
            large: 12.0,
            full: 9999.0,
        }
    }
}

/// Box shadow levels for surface elevation.
///
/// Six levels (0-5). Levels 0, 4, and 5 default to `None`. Levels 1-3 provide
/// progressively stronger shadows with increasing blur radius and y-offset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElevationTokens {
    pub level0: Option<BoxShadow>,
    pub level1: Option<BoxShadow>,
    pub level2: Option<BoxShadow>,
    pub level3: Option<BoxShadow>,
    pub level4: Option<BoxShadow>,
    pub level5: Option<BoxShadow>,
}

impl Default for ElevationTokens {
    fn default() -> Self {
        let black_alpha = |a| Color { r: 0, g: 0, b: 0, a };
        Self {
            level0: None,
            level1: Some(BoxShadow { color: black_alpha(40), offset: (0.0, 1.0), blur_radius: 2.0 }),
            level2: Some(BoxShadow { color: black_alpha(60), offset: (0.0, 2.0), blur_radius: 4.0 }),
            level3: Some(BoxShadow { color: black_alpha(60), offset: (0.0, 4.0), blur_radius: 8.0 }),
            level4: None,
            level5: None,
        }
    }
}

/// The complete set of primitive design tokens.
///
/// Combines [`ColorTokens`], [`SpacingTokens`], [`TypographyTokens`],
/// [`RadiusTokens`], and [`ElevationTokens`]. The [`Default`] implementation
/// provides light-mode values. Use [`Tokens::dark()`] for dark mode.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Tokens {
    pub colors: ColorTokens,
    pub spacing: SpacingTokens,
    pub typography: TypographyTokens,
    pub radii: RadiusTokens,
    pub elevations: ElevationTokens,
}

impl Tokens {
    pub fn dark() -> Self {
        Self {
            colors: ColorTokens::dark(),
            spacing: SpacingTokens::default(),
            typography: TypographyTokens::default(),
            radii: RadiusTokens::default(),
            elevations: ElevationTokens::default(),
        }
    }
}

// --- Component Themes ---

/// Visual parameters for the `Button` widget.
///
/// Includes dimensions, padding, corner radius, text size, elevation for
/// rest/hover/pressed states, and an optional focus stroke.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ButtonTheme {
    pub height: f32,
    pub padding_horizontal: f32,
    pub padding_vertical: f32,
    pub radius: f32,
    pub text_size: f32,
    pub elevation_rest: Option<BoxShadow>,
    pub elevation_hover: Option<BoxShadow>,
    pub elevation_pressed: Option<BoxShadow>,
    pub focus_stroke: Option<Stroke>,
}

impl ButtonTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            height: 42.0,
            padding_horizontal: tokens.spacing.m,
            padding_vertical: tokens.spacing.s,
            radius: tokens.radii.full,
            text_size: tokens.typography.label_large_size,
            elevation_rest: tokens.elevations.level1,
            elevation_hover: tokens.elevations.level2,
            elevation_pressed: tokens.elevations.level0,
            focus_stroke: Some(Stroke {
                fill: fission_ir::op::Fill::Solid(tokens.colors.on_background),
                width: 1.0,
                dash_array: None,
                line_cap: fission_ir::op::LineCap::Butt,
                line_join: fission_ir::op::LineJoin::Miter,
            }),
        }
    }
}

/// Visual parameters for the `TextInput` widget.
///
/// Controls height, horizontal padding, corner radius, font size, and colors
/// for border, focus ring, text, and placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextInputTheme {
    pub height: f32,
    pub padding_h: f32,
    pub radius: f32,
    pub font_size: f32,
    pub border_color: Color,
    pub border_width: f32,
    pub focus_color: Color,
    pub text_color: Color,
    pub placeholder_color: Color,
}

impl TextInputTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            height: 40.0,
            padding_h: tokens.spacing.m,
            radius: tokens.radii.small,
            font_size: tokens.typography.body_large_size,
            border_color: tokens.colors.border,
            border_width: 1.0,
            focus_color: tokens.colors.primary,
            text_color: tokens.colors.text_primary,
            placeholder_color: tokens.colors.text_secondary,
        }
    }
}

/// Visual parameters for the `Calendar` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalendarTheme {
    pub bg_color: Color,
    pub border_color: Color,
    pub radius: f32,
    pub selected_bg: Color,
    pub selected_text: Color,
    pub today_outline: Color,
}

impl CalendarTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            border_color: tokens.colors.border,
            radius: tokens.radii.medium,
            selected_bg: tokens.colors.primary,
            selected_text: tokens.colors.on_primary,
            today_outline: tokens.colors.secondary,
        }
    }
}

/// Visual parameters for the `Pagination` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaginationTheme {
    pub spacing: f32,
    pub active_bg: Color,
    pub active_text: Color,
}

impl PaginationTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            spacing: tokens.spacing.s,
            active_bg: tokens.colors.primary,
            active_text: tokens.colors.on_primary,
        }
    }
}

/// Visual parameters for the `Timeline` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineTheme {
    pub dot_size: f32,
    pub line_width: f32,
    pub dot_color: Color,
    pub line_color: Color,
}

impl TimelineTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            dot_size: 12.0,
            line_width: 2.0,
            dot_color: tokens.colors.primary,
            line_color: tokens.colors.border,
        }
    }
}

/// Visual parameters for the `SegmentedControl` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentedControlTheme {
    pub bg_color: Color,
    pub border_color: Color,
    pub radius: f32,
    pub active_bg: Color,
    pub active_text: Color,
}

impl SegmentedControlTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            border_color: tokens.colors.border,
            radius: tokens.radii.full,
            active_bg: tokens.colors.primary,
            active_text: tokens.colors.on_primary,
        }
    }
}

/// Visual parameters for the `Alert` widget, with per-severity background colors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlertTheme {
    pub info_bg: Color,
    pub warning_bg: Color,
    pub error_bg: Color,
    pub success_bg: Color,
    pub radius: f32,
}

impl AlertTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            info_bg: Color { r: 230, g: 242, b: 255, a: 255 },
            warning_bg: Color { r: 255, g: 244, b: 229, a: 255 },
            error_bg: tokens.colors.error.with_alpha(30),
            success_bg: Color { r: 237, g: 247, b: 237, a: 255 },
            radius: tokens.radii.medium,
        }
    }
}

/// Visual parameters for the `Badge` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BadgeTheme {
    pub radius: f32,
    pub font_size: f32,
}

impl BadgeTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            radius: tokens.radii.full,
            font_size: 10.0,
        }
    }
}

/// Visual parameters for the `Tabs` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabsTheme {
    pub active_color: Color,
    pub inactive_color: Color,
    pub indicator_height: f32,
    pub background: Color,
    pub divider_color: Color,
}

impl TabsTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            active_color: tokens.colors.primary,
            inactive_color: tokens.colors.text_secondary,
            indicator_height: 3.0,
            background: tokens.colors.background,
            divider_color: tokens.colors.border.with_alpha(120),
        }
    }
}

/// Visual parameters for the `Modal` widget.
///
/// Controls the dialog background color, corner radius, shadow, and maximum width.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModalTheme {
    pub bg_color: Color,
    pub radius: f32,
    pub shadow: Option<BoxShadow>,
    pub max_width: f32,
}

impl ModalTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            radius: tokens.radii.large,
            shadow: tokens.elevations.level3,
            max_width: 600.0,
        }
    }
}

/// Visual parameters for the `TreeView` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeViewTheme {
    pub indent: f32,
    pub selected_bg: Color,
    pub hover_bg: Color,
}

impl TreeViewTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            indent: 16.0,
            selected_bg: tokens.colors.primary.with_alpha(52),
            hover_bg: tokens.colors.surface,
        }
    }
}

/// Visual parameters for the `ProgressBar` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgressTheme {
    pub height: f32,
    pub track_color: Color,
    pub bar_color: Color,
}

impl ProgressTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            height: 8.0,
            track_color: tokens.colors.border,
            bar_color: tokens.colors.primary,
        }
    }
}

/// Visual parameters for the `Tooltip` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TooltipTheme {
    pub bg_color: Color,
    pub text_color: Color,
    pub radius: f32,
    pub font_size: f32,
}

impl TooltipTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: Color { r: 50, g: 50, b: 50, a: 255 },
            text_color: Color::WHITE,
            radius: tokens.radii.small,
            font_size: 12.0,
        }
    }
}

/// Aggregates all per-component visual themes.
///
/// Each field holds the theme for a specific widget type. Construct via
/// [`ComponentTheme::from_tokens()`] to derive all values from the primitive tokens.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentTheme {
    pub button: ButtonTheme,
    pub text_input: TextInputTheme,
    pub calendar: CalendarTheme,
    pub pagination: PaginationTheme,
    pub timeline: TimelineTheme,
    pub segmented_control: SegmentedControlTheme,
    pub alert: AlertTheme,
    pub badge: BadgeTheme,
    pub tabs: TabsTheme,
    pub modal: ModalTheme,
    pub tree_view: TreeViewTheme,
    pub progress: ProgressTheme,
    pub tooltip: TooltipTheme,
}

impl ComponentTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            button: ButtonTheme::from_tokens(tokens),
            text_input: TextInputTheme::from_tokens(tokens),
            calendar: CalendarTheme::from_tokens(tokens),
            pagination: PaginationTheme::from_tokens(tokens),
            timeline: TimelineTheme::from_tokens(tokens),
            segmented_control: SegmentedControlTheme::from_tokens(tokens),
            alert: AlertTheme::from_tokens(tokens),
            badge: BadgeTheme::from_tokens(tokens),
            tabs: TabsTheme::from_tokens(tokens),
            modal: ModalTheme::from_tokens(tokens),
            tree_view: TreeViewTheme::from_tokens(tokens),
            progress: ProgressTheme::from_tokens(tokens),
            tooltip: TooltipTheme::from_tokens(tokens),
        }
    }
}

/// The top-level theme combining primitive [`Tokens`] and derived [`ComponentTheme`].
///
/// Use [`Theme::default()`] for light mode and [`Theme::dark()`] for dark mode.
/// For custom themes, construct [`Tokens`] manually and derive components via
/// [`ComponentTheme::from_tokens()`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub tokens: Tokens,
    pub components: ComponentTheme,
}

impl Default for Theme {
    fn default() -> Self {
        let tokens = Tokens::default();
        let components = ComponentTheme::from_tokens(&tokens);
        Self { tokens, components }
    }
}

impl Theme {
    pub fn dark() -> Self {
        let tokens = Tokens::dark();
        let components = ComponentTheme::from_tokens(&tokens);
        Self { tokens, components }
    }
}

/// Bundled font files embedded at compile time.
///
/// Provides Noto Sans Regular (the default) and Inter 24pt Regular.
pub mod fonts {
    pub const NOTO_SANS_REGULAR_TTF: &[u8] = include_bytes!("../fonts/Noto_Sans/static/NotoSans-Regular.ttf");
    pub const INTER_24PT_REGULAR_TTF: &[u8] = include_bytes!("../fonts/Inter/static/Inter_24pt-Regular.ttf");
    #[inline]
    pub fn default_font_bytes() -> &'static [u8] { NOTO_SANS_REGULAR_TTF }
}
