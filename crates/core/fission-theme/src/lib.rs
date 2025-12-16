use fission_ir::op::{BoxShadow, Color, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorTokens {
    pub primary: Color,
    pub on_primary: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub background: Color,
    pub on_background: Color,
    pub error: Color,
    pub on_error: Color,
}

impl Default for ColorTokens {
    fn default() -> Self {
        // Material 3-ish Baseline defaults (simplified)
        Self {
            primary: Color {
                r: 103,
                g: 85,
                b: 143,
                a: 255,
            }, // Purple 40
            on_primary: Color::WHITE,
            surface: Color {
                r: 255,
                g: 251,
                b: 254,
                a: 255,
            }, // Purple 99
            on_surface: Color {
                r: 28,
                g: 27,
                b: 31,
                a: 255,
            }, // Purple 10
            background: Color {
                r: 255,
                g: 251,
                b: 254,
                a: 255,
            },
            on_background: Color {
                r: 28,
                g: 27,
                b: 31,
                a: 255,
            },
            error: Color {
                r: 179,
                g: 38,
                b: 30,
                a: 255,
            },
            on_error: Color::WHITE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyTokens {
    // Simplified for MVP. Stores font size.
    pub label_large_size: f32,
    pub body_medium_size: f32,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        Self {
            label_large_size: 14.0,
            body_medium_size: 16.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadiusTokens {
    pub small: f32,
    pub medium: f32,
    pub large: f32,
    pub full: f32, // For pill shapes
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            small: 4.0,
            medium: 12.0,
            large: 16.0,
            full: 9999.0,
        }
    }
}

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
        let black_alpha = |a| Color {
            r: 0,
            g: 0,
            b: 0,
            a,
        };

        Self {
            level0: None,
            level1: Some(BoxShadow {
                color: black_alpha(40),
                offset: (0.0, 1.0),
                blur_radius: 2.0,
            }),
            level2: Some(BoxShadow {
                color: black_alpha(60),
                offset: (0.0, 2.0),
                blur_radius: 4.0,
            }),
            level3: Some(BoxShadow {
                color: black_alpha(60),
                offset: (0.0, 4.0),
                blur_radius: 8.0,
            }),
            level4: None,
            level5: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Tokens {
    pub colors: ColorTokens,
    pub typography: TypographyTokens,
    pub radii: RadiusTokens,
    pub elevations: ElevationTokens,
}

// --- Component Themes ---

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ButtonTheme {
    pub height: f32,
    pub padding_horizontal: f32,
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
            height: 40.0,
            padding_horizontal: 24.0,
            radius: tokens.radii.full,
            text_size: tokens.typography.label_large_size,
            elevation_rest: tokens.elevations.level1,
            elevation_hover: tokens.elevations.level2,
            elevation_pressed: tokens.elevations.level0,
            focus_stroke: Some(Stroke {
                color: tokens.colors.on_background,
                width: 2.0,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentTheme {
    pub button: ButtonTheme,
}

impl ComponentTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            button: ButtonTheme::from_tokens(tokens),
        }
    }
}

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

// --- Fonts ---
// Expose pinned, embedded font bytes for deterministic text measurement/rendering across platforms.
// We start with Noto Sans Regular as the default UI font.

#[allow(dead_code)]
pub mod fonts {
    /// Noto Sans Regular (static) bundled as part of the theme for deterministic metrics.
    ///
    /// Source: crates/core/fission-theme/fonts/Noto_Sans/static/NotoSans-Regular.ttf
    pub const NOTO_SANS_REGULAR_TTF: &[u8] =
        include_bytes!("../fonts/Noto_Sans/static/NotoSans-Regular.ttf");

    /// Inter 24pt Regular (static). Useful for headings or future variations.
    ///
    /// Source: crates/core/fission-theme/fonts/Inter/static/Inter_24pt-Regular.ttf
    pub const INTER_24PT_REGULAR_TTF: &[u8] =
        include_bytes!("../fonts/Inter/static/Inter_24pt-Regular.ttf");

    /// Returns the default UI font bytes to be used by measurement and rendering backends.
    #[inline]
    pub fn default_font_bytes() -> &'static [u8] {
        NOTO_SANS_REGULAR_TTF
    }
}
