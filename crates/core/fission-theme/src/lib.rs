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

pub use fission_ir::op::{BoxShadow, Color, Fill, LineCap, LineJoin, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesignMode {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignSystemInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
}

pub trait DesignSystem {
    fn info() -> &'static DesignSystemInfo;
    fn tokens() -> &'static DesignTokenSet;
    fn components() -> &'static [DesignComponentSpec];
    fn patterns() -> &'static [DesignPatternSpec];
    fn assets() -> &'static DesignAssetManifest;
    fn theme_ref(mode: DesignMode) -> &'static Theme;

    fn theme(mode: DesignMode) -> Theme {
        Self::theme_ref(mode).clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResolvedDesignSystem {
    pub mode: DesignMode,
    pub info: DesignSystemInfo,
    pub tokens: DesignTokenSet,
    pub components: Vec<DesignComponentSpec>,
    pub patterns: Vec<DesignPatternSpec>,
    pub assets: DesignAssetManifest,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignTokenSet {
    pub tokens: Vec<DesignToken>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignToken {
    pub path: String,
    pub kind: String,
    pub value: DesignValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DesignValue {
    None,
    Bool(bool),
    Number(f32),
    Dimension(f32),
    DurationMs(u64),
    Text(String),
    Color(Color),
    Shadow(Vec<ShadowLayer>),
    Easing(EasingCurve),
    Object(Vec<DesignProperty>),
    List(Vec<DesignValue>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignProperty {
    pub name: String,
    pub value: DesignValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowLayer {
    pub color: Color,
    pub offset: (f32, f32),
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub inset: bool,
}

impl ShadowLayer {
    pub fn to_box_shadow(&self) -> BoxShadow {
        BoxShadow {
            color: self.color,
            offset: self.offset,
            blur_radius: self.blur_radius,
        }
    }
}

fn shadow_layer_from_box(shadow: BoxShadow) -> ShadowLayer {
    ShadowLayer {
        color: shadow.color,
        offset: shadow.offset,
        blur_radius: shadow.blur_radius,
        spread_radius: 0.0,
        inset: false,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EasingCurve {
    Linear,
    Ease,
    CubicBezier(f32, f32, f32, f32),
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignComponentSpec {
    pub name: String,
    pub description: String,
    pub anatomy: Vec<String>,
    pub properties: Vec<DesignProperty>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignPatternSpec {
    pub name: String,
    pub description: String,
    pub properties: Vec<DesignProperty>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignAssetManifest {
    pub logos: Vec<DesignAsset>,
    pub fonts: Vec<DesignAsset>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignAsset {
    pub id: String,
    pub path: String,
    pub format: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentSize {
    Sm,
    #[default]
    Md,
    Lg,
    Xl,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentState {
    #[default]
    Default,
    Hover,
    Active,
    Focus,
    Disabled,
    Error,
    Selected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonHierarchy {
    #[default]
    Primary,
    SecondaryColor,
    SecondaryGray,
    TertiaryColor,
    TertiaryGray,
    LinkColor,
    LinkGray,
    Destructive,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BadgeTone {
    #[default]
    Brand,
    Gray,
    Success,
    Warning,
    Error,
    Blue,
    Orange,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardPattern {
    Plain,
    #[default]
    Raised,
    Tinted,
    Elevated,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureIconTone {
    #[default]
    Brand,
    Gray,
    Blue,
    Orange,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentBorder {
    pub fill: Fill,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentMotion {
    pub duration_ms: u64,
    pub easing: EasingCurve,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResolvedComponentStyle {
    pub background: Option<Fill>,
    pub text_color: Option<Color>,
    pub border: Option<ComponentBorder>,
    pub radius: Option<f32>,
    pub height: Option<f32>,
    pub width: Option<f32>,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub padding: Option<[f32; 4]>,
    pub gap: Option<f32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub icon_size: Option<f32>,
    pub max_width: Option<f32>,
    pub shadows: Vec<ShadowLayer>,
    pub transition: Option<ComponentMotion>,
}

impl ResolvedComponentStyle {
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            background: overlay
                .background
                .clone()
                .or_else(|| self.background.clone()),
            text_color: overlay.text_color.or(self.text_color),
            border: overlay.border.clone().or_else(|| self.border.clone()),
            radius: overlay.radius.or(self.radius),
            height: overlay.height.or(self.height),
            width: overlay.width.or(self.width),
            padding_x: overlay.padding_x.or(self.padding_x),
            padding_y: overlay.padding_y.or(self.padding_y),
            padding: overlay.padding.or(self.padding),
            gap: overlay.gap.or(self.gap),
            font_size: overlay.font_size.or(self.font_size),
            font_weight: overlay.font_weight.or(self.font_weight),
            line_height: overlay.line_height.or(self.line_height),
            letter_spacing: overlay.letter_spacing.or(self.letter_spacing),
            icon_size: overlay.icon_size.or(self.icon_size),
            max_width: overlay.max_width.or(self.max_width),
            shadows: if overlay.shadows.is_empty() {
                self.shadows.clone()
            } else {
                overlay.shadows.clone()
            },
            transition: overlay
                .transition
                .clone()
                .or_else(|| self.transition.clone()),
        }
    }

    pub fn padding_box(&self, fallback_x: f32, fallback_y: f32) -> [f32; 4] {
        self.padding.unwrap_or([
            self.padding_x.unwrap_or(fallback_x),
            self.padding_x.unwrap_or(fallback_x),
            self.padding_y.unwrap_or(fallback_y),
            self.padding_y.unwrap_or(fallback_y),
        ])
    }

    pub fn outer_shadows(&self) -> Vec<BoxShadow> {
        self.shadows
            .iter()
            .filter(|layer| !layer.inset)
            .map(ShadowLayer::to_box_shadow)
            .collect()
    }

    pub fn inset_border(&self) -> Option<ComponentBorder> {
        self.shadows
            .iter()
            .find(|layer| layer.inset && layer.spread_radius > 0.0)
            .map(|layer| ComponentBorder {
                fill: Fill::Solid(layer.color),
                width: layer.spread_radius,
            })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComponentStateStyles {
    pub default: ResolvedComponentStyle,
    pub hover: Option<ResolvedComponentStyle>,
    pub active: Option<ResolvedComponentStyle>,
    pub focus: Option<ResolvedComponentStyle>,
    pub disabled: Option<ResolvedComponentStyle>,
    pub error: Option<ResolvedComponentStyle>,
    pub selected: Option<ResolvedComponentStyle>,
}

impl ComponentStateStyles {
    pub fn resolve(&self, state: ComponentState) -> ResolvedComponentStyle {
        let overlay = match state {
            ComponentState::Default => None,
            ComponentState::Hover => self.hover.as_ref(),
            ComponentState::Active => self.active.as_ref(),
            ComponentState::Focus => self.focus.as_ref(),
            ComponentState::Disabled => self.disabled.as_ref(),
            ComponentState::Error => self.error.as_ref(),
            ComponentState::Selected => self.selected.as_ref(),
        };
        overlay
            .map(|style| self.default.merge(style))
            .unwrap_or_else(|| self.default.clone())
    }
}

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
    pub primary_hover: Color,
    pub primary_subtle: Color,
    pub secondary: Color,
    pub on_secondary: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub surface_raised: Color,
    pub surface_sunken: Color,
    pub background: Color,
    pub on_background: Color,
    pub error: Color,
    pub on_error: Color,
    pub success: Color,
    pub warning: Color,
    pub info: Color,
    pub border: Color,
    pub border_strong: Color,
    pub divider: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_link: Color,
    pub heading: Color,
    pub focus_ring: Color,
}

impl Default for ColorTokens {
    fn default() -> Self {
        Self {
            primary: Color {
                r: 103,
                g: 85,
                b: 143,
                a: 255,
            }, // Purple 40
            on_primary: Color::WHITE,
            primary_hover: Color {
                r: 80,
                g: 63,
                b: 118,
                a: 255,
            },
            primary_subtle: Color {
                r: 244,
                g: 239,
                b: 255,
                a: 255,
            },
            secondary: Color {
                r: 98,
                g: 91,
                b: 113,
                a: 255,
            },
            on_secondary: Color::WHITE,
            surface: Color {
                r: 255,
                g: 251,
                b: 254,
                a: 255,
            },
            on_surface: Color {
                r: 28,
                g: 27,
                b: 31,
                a: 255,
            },
            surface_raised: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            surface_sunken: Color {
                r: 248,
                g: 248,
                b: 248,
                a: 255,
            },
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
            success: Color {
                r: 16,
                g: 185,
                b: 129,
                a: 255,
            },
            warning: Color {
                r: 245,
                g: 158,
                b: 11,
                a: 255,
            },
            info: Color {
                r: 14,
                g: 165,
                b: 233,
                a: 255,
            },
            border: Color {
                r: 188,
                g: 188,
                b: 188,
                a: 255,
            },
            border_strong: Color {
                r: 148,
                g: 148,
                b: 148,
                a: 255,
            },
            divider: Color {
                r: 188,
                g: 188,
                b: 188,
                a: 255,
            },
            text_primary: Color {
                r: 28,
                g: 27,
                b: 31,
                a: 255,
            },
            text_secondary: Color {
                r: 86,
                g: 86,
                b: 86,
                a: 255,
            },
            text_muted: Color {
                r: 120,
                g: 120,
                b: 120,
                a: 255,
            },
            text_link: Color {
                r: 103,
                g: 85,
                b: 143,
                a: 255,
            },
            heading: Color {
                r: 28,
                g: 27,
                b: 31,
                a: 255,
            },
            focus_ring: Color {
                r: 103,
                g: 85,
                b: 143,
                a: 255,
            },
        }
    }
}

impl ColorTokens {
    pub fn dark() -> Self {
        Self {
            primary: Color {
                r: 187,
                g: 134,
                b: 252,
                a: 255,
            },
            on_primary: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            primary_hover: Color {
                r: 210,
                g: 178,
                b: 255,
                a: 255,
            },
            primary_subtle: Color {
                r: 55,
                g: 36,
                b: 86,
                a: 255,
            },
            secondary: Color {
                r: 3,
                g: 218,
                b: 197,
                a: 255,
            },
            on_secondary: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            surface: Color {
                r: 30,
                g: 30,
                b: 30,
                a: 255,
            },
            on_surface: Color {
                r: 230,
                g: 230,
                b: 230,
                a: 255,
            },
            surface_raised: Color {
                r: 37,
                g: 37,
                b: 37,
                a: 255,
            },
            surface_sunken: Color {
                r: 12,
                g: 12,
                b: 12,
                a: 255,
            },
            background: Color {
                r: 18,
                g: 18,
                b: 18,
                a: 255,
            },
            on_background: Color {
                r: 230,
                g: 230,
                b: 230,
                a: 255,
            },
            error: Color {
                r: 207,
                g: 102,
                b: 121,
                a: 255,
            },
            on_error: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            success: Color {
                r: 16,
                g: 185,
                b: 129,
                a: 255,
            },
            warning: Color {
                r: 245,
                g: 158,
                b: 11,
                a: 255,
            },
            info: Color {
                r: 14,
                g: 165,
                b: 233,
                a: 255,
            },
            border: Color {
                r: 60,
                g: 60,
                b: 60,
                a: 255,
            },
            border_strong: Color {
                r: 96,
                g: 96,
                b: 96,
                a: 255,
            },
            divider: Color {
                r: 60,
                g: 60,
                b: 60,
                a: 255,
            },
            text_primary: Color {
                r: 230,
                g: 230,
                b: 230,
                a: 255,
            },
            text_secondary: Color {
                r: 160,
                g: 160,
                b: 160,
                a: 255,
            },
            text_muted: Color {
                r: 120,
                g: 120,
                b: 120,
                a: 255,
            },
            text_link: Color {
                r: 187,
                g: 134,
                b: 252,
                a: 255,
            },
            heading: Color {
                r: 230,
                g: 230,
                b: 230,
                a: 255,
            },
            focus_ring: Color {
                r: 187,
                g: 134,
                b: 252,
                a: 255,
            },
        }
    }
}

/// Standard spacing scale used for padding, margins, and gaps.
///
/// Values: `none` (0), `xs` (4), `s` (8), `m` (16), `l` (24), `xl` (32).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpacingTokens {
    pub none: f32,  // 0
    pub xs: f32,    // 4
    pub s: f32,     // 8
    pub m: f32,     // 16
    pub l: f32,     // 24
    pub xl: f32,    // 32
    pub xxl: f32,   // 48
    pub xxxl: f32,  // 64
    pub xxxxl: f32, // 96
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
            xxl: 48.0,
            xxxl: 64.0,
            xxxxl: 96.0,
        }
    }
}

/// Font size scale for text elements.
///
/// Sizes: `label_large_size` (15), `body_medium_size` (15), `body_large_size` (17),
/// `heading_size` (28).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyTokens {
    pub font_family_sans: String,
    pub font_family_serif: String,
    pub font_family_mono: String,
    pub font_weight_regular: u16,
    pub font_weight_medium: u16,
    pub font_weight_semibold: u16,
    pub font_weight_bold: u16,
    pub font_size_xs: f32,
    pub font_size_sm: f32,
    pub font_size_base: f32,
    pub label_large_size: f32,
    pub body_medium_size: f32,
    pub body_large_size: f32,
    pub font_size_lg: f32,
    pub font_size_xl: f32,
    pub heading_size: f32,
    pub heading2_size: f32,
    pub heading1_size: f32,
    pub display_sm_size: f32,
    pub display_md_size: f32,
    pub line_height_display: f32,
    pub line_height_heading: f32,
    pub line_height_snug: f32,
    pub line_height_normal: f32,
    pub line_height_relaxed: f32,
    pub letter_spacing_tight: f32,
    pub letter_spacing_normal: f32,
    pub letter_spacing_label: f32,
    pub letter_spacing_kicker: f32,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        Self {
            font_family_sans: "\"Inter\", \"Avenir Next\", \"Segoe UI\", Arial, sans-serif".into(),
            font_family_serif: "\"Iowan Old Style\", \"Palatino Linotype\", \"Book Antiqua\", Georgia, serif".into(),
            font_family_mono: "\"SFMono-Regular\", Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace".into(),
            font_weight_regular: 400,
            font_weight_medium: 500,
            font_weight_semibold: 600,
            font_weight_bold: 700,
            font_size_xs: 12.0,
            font_size_sm: 13.0,
            font_size_base: 14.0,
            label_large_size: 15.0,
            body_medium_size: 15.0,
            body_large_size: 17.0,
            font_size_lg: 20.0,
            font_size_xl: 24.0,
            heading_size: 28.0,
            heading2_size: 36.0,
            heading1_size: 48.0,
            display_sm_size: 60.0,
            display_md_size: 72.0,
            line_height_display: 0.98,
            line_height_heading: 1.05,
            line_height_snug: 1.4,
            line_height_normal: 1.6,
            line_height_relaxed: 1.68,
            letter_spacing_tight: -0.01,
            letter_spacing_normal: 0.0,
            letter_spacing_label: 0.1,
            letter_spacing_kicker: 0.14,
        }
    }
}

/// Corner radius scale for rounded containers.
///
/// Values: `small` (4), `medium` (8), `large` (12), `full` (9999 -- fully rounded pill).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadiusTokens {
    pub none: f32,
    pub small: f32,
    pub medium: f32,
    pub large: f32,
    pub xl: f32,
    pub xxl: f32,
    pub full: f32,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            none: 0.0,
            small: 4.0,
            medium: 8.0,
            large: 12.0,
            xl: 16.0,
            xxl: 24.0,
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
    pub focus: Option<BoxShadow>,
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
            focus: Some(BoxShadow {
                color: Color {
                    r: 20,
                    g: 184,
                    b: 166,
                    a: 82,
                },
                offset: (0.0, 0.0),
                blur_radius: 0.0,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionTokens {
    pub duration_instant_ms: u64,
    pub duration_micro_ms: u64,
    pub duration_fast_ms: u64,
    pub duration_normal_ms: u64,
    pub duration_slow_ms: u64,
    pub duration_deliberate_ms: u64,
    pub easing_linear: EasingCurve,
    pub easing_standard: EasingCurve,
    pub easing_in: EasingCurve,
    pub easing_out: EasingCurve,
    pub easing_ease: EasingCurve,
}

impl Default for MotionTokens {
    fn default() -> Self {
        Self {
            duration_instant_ms: 0,
            duration_micro_ms: 120,
            duration_fast_ms: 160,
            duration_normal_ms: 200,
            duration_slow_ms: 300,
            duration_deliberate_ms: 480,
            easing_linear: EasingCurve::Linear,
            easing_standard: EasingCurve::CubicBezier(0.16, 0.84, 0.32, 1.0),
            easing_in: EasingCurve::CubicBezier(0.4, 0.0, 1.0, 1.0),
            easing_out: EasingCurve::CubicBezier(0.0, 0.0, 0.2, 1.0),
            easing_ease: EasingCurve::Ease,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataVisualizationTokens {
    pub palette: Vec<Color>,
}

impl Default for DataVisualizationTokens {
    fn default() -> Self {
        Self {
            palette: vec![
                Color {
                    r: 20,
                    g: 184,
                    b: 166,
                    a: 255,
                },
                Color {
                    r: 77,
                    g: 166,
                    b: 224,
                    a: 255,
                },
                Color {
                    r: 245,
                    g: 158,
                    b: 11,
                    a: 255,
                },
                Color {
                    r: 244,
                    g: 63,
                    b: 94,
                    a: 255,
                },
                Color {
                    r: 132,
                    g: 204,
                    b: 22,
                    a: 255,
                },
                Color {
                    r: 14,
                    g: 165,
                    b: 233,
                    a: 255,
                },
                Color {
                    r: 168,
                    g: 85,
                    b: 247,
                    a: 255,
                },
                Color {
                    r: 249,
                    g: 115,
                    b: 22,
                    a: 255,
                },
            ],
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
    pub motion: MotionTokens,
    pub data_visualization: DataVisualizationTokens,
}

impl Tokens {
    pub fn dark() -> Self {
        Self {
            colors: ColorTokens::dark(),
            spacing: SpacingTokens::default(),
            typography: TypographyTokens::default(),
            radii: RadiusTokens::default(),
            elevations: ElevationTokens::default(),
            motion: MotionTokens::default(),
            data_visualization: DataVisualizationTokens::default(),
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
    pub icon_size: f32,
    pub font_weight: u16,
    pub line_height: f32,
    pub transition: Option<ComponentMotion>,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub hierarchies: Vec<(ButtonHierarchy, ComponentStateStyles)>,
}

impl ButtonTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let transition = Some(ComponentMotion {
            duration_ms: tokens.motion.duration_fast_ms,
            easing: tokens.motion.easing_standard.clone(),
        });
        let size_md = ResolvedComponentStyle {
            height: Some(40.0),
            padding_x: Some(14.0),
            padding_y: Some(tokens.spacing.s),
            gap: Some(4.0),
            font_size: Some(tokens.typography.label_large_size),
            font_weight: Some(tokens.typography.font_weight_semibold),
            line_height: Some(20.0),
            icon_size: Some(20.0),
            ..ResolvedComponentStyle::default()
        };
        let primary = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary)),
                text_color: Some(tokens.colors.on_primary),
                border: None,
                shadows: tokens
                    .elevations
                    .level1
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary_hover)),
                shadows: tokens
                    .elevations
                    .level2
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                shadows: tokens
                    .elevations
                    .level0
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(ResolvedComponentStyle {
                shadows: tokens
                    .elevations
                    .focus
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            }),
            disabled: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.border)),
                text_color: Some(tokens.colors.text_secondary),
                shadows: Vec::new(),
                ..ResolvedComponentStyle::default()
            }),
            ..ComponentStateStyles::default()
        };
        let secondary_gray = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                text_color: Some(tokens.colors.text_primary),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            disabled: Some(ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                ..ResolvedComponentStyle::default()
            }),
            ..ComponentStateStyles::default()
        };
        let tertiary_gray = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: None,
                text_color: Some(tokens.colors.primary),
                border: None,
                transition,
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            disabled: Some(ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                ..ResolvedComponentStyle::default()
            }),
            ..ComponentStateStyles::default()
        };
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
            icon_size: 20.0,
            font_weight: tokens.typography.font_weight_semibold,
            line_height: 20.0,
            transition: Some(ComponentMotion {
                duration_ms: tokens.motion.duration_fast_ms,
                easing: tokens.motion.easing_standard.clone(),
            }),
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(36.0),
                        padding_x: Some(12.0),
                        padding_y: Some(tokens.spacing.xs),
                        gap: Some(4.0),
                        font_size: Some(tokens.typography.font_size_sm),
                        line_height: Some(20.0),
                        icon_size: Some(18.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (ComponentSize::Md, size_md),
                (
                    ComponentSize::Lg,
                    ResolvedComponentStyle {
                        height: Some(44.0),
                        padding_x: Some(16.0),
                        padding_y: Some(tokens.spacing.s),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(24.0),
                        icon_size: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Xl,
                    ResolvedComponentStyle {
                        height: Some(48.0),
                        padding_x: Some(18.0),
                        padding_y: Some(tokens.spacing.s),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(24.0),
                        icon_size: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            hierarchies: vec![
                (ButtonHierarchy::Primary, primary.clone()),
                (ButtonHierarchy::SecondaryColor, secondary_gray.clone()),
                (ButtonHierarchy::SecondaryGray, secondary_gray),
                (ButtonHierarchy::TertiaryColor, tertiary_gray.clone()),
                (ButtonHierarchy::TertiaryGray, tertiary_gray.clone()),
                (ButtonHierarchy::LinkColor, tertiary_gray.clone()),
                (ButtonHierarchy::LinkGray, tertiary_gray.clone()),
                (
                    ButtonHierarchy::Destructive,
                    ComponentStateStyles {
                        default: ResolvedComponentStyle {
                            background: Some(Fill::Solid(tokens.colors.error)),
                            text_color: Some(tokens.colors.on_error),
                            ..primary.default.clone()
                        },
                        hover: Some(ResolvedComponentStyle {
                            background: Some(Fill::Solid(tokens.colors.error.with_alpha(230))),
                            ..ResolvedComponentStyle::default()
                        }),
                        ..primary
                    },
                ),
            ],
        }
    }

    pub fn size_style(&self, size: ComponentSize) -> ResolvedComponentStyle {
        self.sizes
            .iter()
            .find(|(candidate, _)| *candidate == size)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.sizes
                    .iter()
                    .find(|(candidate, _)| *candidate == ComponentSize::Md)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_else(|| ResolvedComponentStyle {
                height: Some(self.height),
                padding_x: Some(self.padding_horizontal),
                padding_y: Some(self.padding_vertical),
                radius: Some(self.radius),
                font_size: Some(self.text_size),
                font_weight: Some(self.font_weight),
                line_height: Some(self.line_height),
                icon_size: Some(self.icon_size),
                ..ResolvedComponentStyle::default()
            })
    }

    pub fn hierarchy_style(&self, hierarchy: ButtonHierarchy) -> ComponentStateStyles {
        self.hierarchies
            .iter()
            .find(|(candidate, _)| *candidate == hierarchy)
            .map(|(_, styles)| styles.clone())
            .or_else(|| {
                self.hierarchies
                    .iter()
                    .find(|(candidate, _)| *candidate == ButtonHierarchy::Primary)
                    .map(|(_, styles)| styles.clone())
            })
            .unwrap_or_default()
    }

    pub fn resolve(
        &self,
        hierarchy: ButtonHierarchy,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        let base = ResolvedComponentStyle {
            height: Some(self.height),
            padding_x: Some(self.padding_horizontal),
            padding_y: Some(self.padding_vertical),
            radius: Some(self.radius),
            font_size: Some(self.text_size),
            font_weight: Some(self.font_weight),
            line_height: Some(self.line_height),
            icon_size: Some(self.icon_size),
            transition: self.transition.clone(),
            ..ResolvedComponentStyle::default()
        };
        base.merge(&self.size_style(size))
            .merge(&self.hierarchy_style(hierarchy).resolve(state))
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
    pub line_height: f32,
    pub font_weight: u16,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub states: ComponentStateStyles,
    pub placeholder_style: ResolvedComponentStyle,
    pub label_style: ResolvedComponentStyle,
    pub helper_style: ResolvedComponentStyle,
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
            line_height: 24.0,
            font_weight: tokens.typography.font_weight_regular,
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(36.0),
                        padding_x: Some(12.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(40.0),
                        padding_x: Some(12.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.surface)),
                    text_color: Some(tokens.colors.text_primary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.border),
                        width: 1.0,
                    }),
                    shadows: tokens
                        .elevations
                        .level1
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    ..ResolvedComponentStyle::default()
                },
                focus: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.focus_ring),
                        width: 2.0,
                    }),
                    shadows: tokens
                        .elevations
                        .focus
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    padding_x: Some(11.0),
                    ..ResolvedComponentStyle::default()
                }),
                error: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.error),
                        width: 1.0,
                    }),
                    ..ResolvedComponentStyle::default()
                }),
                disabled: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                    text_color: Some(tokens.colors.text_secondary),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            placeholder_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_muted),
                ..ResolvedComponentStyle::default()
            },
            label_style: ResolvedComponentStyle {
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_medium),
                text_color: Some(tokens.colors.text_primary),
                ..ResolvedComponentStyle::default()
            },
            helper_style: ResolvedComponentStyle {
                font_size: Some(tokens.typography.font_size_base),
                text_color: Some(tokens.colors.text_muted),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    pub fn size_style(&self, size: ComponentSize) -> ResolvedComponentStyle {
        self.sizes
            .iter()
            .find(|(candidate, _)| *candidate == size)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.sizes
                    .iter()
                    .find(|(candidate, _)| *candidate == ComponentSize::Md)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_else(|| ResolvedComponentStyle {
                height: Some(self.height),
                padding_x: Some(self.padding_h),
                ..ResolvedComponentStyle::default()
            })
    }

    pub fn resolve(&self, size: ComponentSize, state: ComponentState) -> ResolvedComponentStyle {
        let base = ResolvedComponentStyle {
            height: Some(self.height),
            padding_x: Some(self.padding_h),
            radius: Some(self.radius),
            font_size: Some(self.font_size),
            line_height: Some(self.line_height),
            font_weight: Some(self.font_weight),
            text_color: Some(self.text_color),
            border: Some(ComponentBorder {
                fill: Fill::Solid(self.border_color),
                width: self.border_width,
            }),
            ..ResolvedComponentStyle::default()
        };
        base.merge(&self.size_style(size))
            .merge(&self.states.resolve(state))
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
            info_bg: Color {
                r: 230,
                g: 242,
                b: 255,
                a: 255,
            },
            warning_bg: Color {
                r: 255,
                g: 244,
                b: 229,
                a: 255,
            },
            error_bg: tokens.colors.error.with_alpha(30),
            success_bg: Color {
                r: 237,
                g: 247,
                b: 237,
                a: 255,
            },
            radius: tokens.radii.medium,
        }
    }
}

/// Visual parameters for the `Badge` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BadgeTheme {
    pub radius: f32,
    pub font_size: f32,
    pub font_weight: u16,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub tones: Vec<(BadgeTone, ResolvedComponentStyle)>,
}

impl BadgeTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            radius: tokens.radii.full,
            font_size: 10.0,
            font_weight: tokens.typography.font_weight_medium,
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(20.0),
                        padding_x: Some(8.0),
                        font_size: Some(tokens.typography.font_size_xs),
                        line_height: Some(18.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(24.0),
                        padding_x: Some(10.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            tones: vec![
                (
                    BadgeTone::Brand,
                    badge_tone(
                        tokens.colors.primary_subtle,
                        tokens.colors.primary,
                        tokens.colors.primary,
                    ),
                ),
                (
                    BadgeTone::Gray,
                    badge_tone(
                        tokens.colors.surface_sunken,
                        tokens.colors.border,
                        tokens.colors.text_primary,
                    ),
                ),
                (
                    BadgeTone::Success,
                    badge_tone(
                        tokens.colors.success.with_alpha(26),
                        tokens.colors.success.with_alpha(80),
                        tokens.colors.success,
                    ),
                ),
                (
                    BadgeTone::Warning,
                    badge_tone(
                        tokens.colors.warning.with_alpha(26),
                        tokens.colors.warning.with_alpha(80),
                        tokens.colors.warning,
                    ),
                ),
                (
                    BadgeTone::Error,
                    badge_tone(
                        tokens.colors.error.with_alpha(26),
                        tokens.colors.error.with_alpha(80),
                        tokens.colors.error,
                    ),
                ),
                (
                    BadgeTone::Blue,
                    badge_tone(
                        tokens.colors.info.with_alpha(26),
                        tokens.colors.info.with_alpha(80),
                        tokens.colors.info,
                    ),
                ),
                (
                    BadgeTone::Orange,
                    badge_tone(
                        tokens.colors.warning.with_alpha(26),
                        tokens.colors.warning.with_alpha(80),
                        tokens.colors.warning,
                    ),
                ),
            ],
        }
    }

    pub fn resolve(&self, tone: BadgeTone, size: ComponentSize) -> ResolvedComponentStyle {
        let base = ResolvedComponentStyle {
            radius: Some(self.radius),
            font_size: Some(self.font_size),
            font_weight: Some(self.font_weight),
            ..ResolvedComponentStyle::default()
        };
        let size_style = find_size_style(&self.sizes, size);
        let tone_style = self
            .tones
            .iter()
            .find(|(candidate, _)| *candidate == tone)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.tones
                    .iter()
                    .find(|(candidate, _)| *candidate == BadgeTone::Brand)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_default();
        base.merge(&size_style).merge(&tone_style)
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
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub states: ComponentStateStyles,
    pub track_style: ResolvedComponentStyle,
}

impl TabsTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            active_color: tokens.colors.primary,
            inactive_color: tokens.colors.text_secondary,
            indicator_height: 3.0,
            background: tokens.colors.background,
            divider_color: tokens.colors.border.with_alpha(120),
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        padding_y: Some(10.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        height: Some(40.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        padding_y: Some(12.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        height: Some(44.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    text_color: Some(tokens.colors.text_secondary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        }),
                        width: 2.0,
                    }),
                    ..ResolvedComponentStyle::default()
                },
                hover: Some(ResolvedComponentStyle {
                    text_color: Some(tokens.colors.text_primary),
                    ..ResolvedComponentStyle::default()
                }),
                active: Some(ResolvedComponentStyle {
                    text_color: Some(tokens.colors.primary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.primary),
                        width: 2.0,
                    }),
                    font_weight: Some(tokens.typography.font_weight_semibold),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            track_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.background)),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border.with_alpha(120)),
                    width: 1.0,
                }),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    pub fn resolve_tab(
        &self,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        find_size_style(&self.sizes, size).merge(&self.states.resolve(state))
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
    pub container_style: ResolvedComponentStyle,
    pub scrim_style: ResolvedComponentStyle,
    pub scrim_blur: f32,
}

impl ModalTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            radius: tokens.radii.large,
            shadow: tokens.elevations.level3,
            max_width: 600.0,
            container_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                radius: Some(tokens.radii.large),
                max_width: Some(600.0),
                shadows: tokens
                    .elevations
                    .level3
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            },
            scrim_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(Color {
                    r: 15,
                    g: 23,
                    b: 42,
                    a: 153,
                })),
                ..ResolvedComponentStyle::default()
            },
            scrim_blur: 4.0,
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
    pub radius: f32,
    pub track_style: ResolvedComponentStyle,
    pub fill_style: ResolvedComponentStyle,
}

impl ProgressTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            height: 8.0,
            track_color: tokens.colors.border,
            bar_color: tokens.colors.primary,
            radius: tokens.radii.full,
            track_style: ResolvedComponentStyle {
                height: Some(8.0),
                radius: Some(tokens.radii.full),
                background: Some(Fill::Solid(tokens.colors.border)),
                ..ResolvedComponentStyle::default()
            },
            fill_style: ResolvedComponentStyle {
                height: Some(8.0),
                radius: Some(tokens.radii.full),
                background: Some(Fill::Solid(tokens.colors.primary)),
                ..ResolvedComponentStyle::default()
            },
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
    pub padding_x: f32,
    pub padding_y: f32,
    pub max_width: f32,
    pub style: ResolvedComponentStyle,
}

impl TooltipTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: Color {
                r: 50,
                g: 50,
                b: 50,
                a: 255,
            },
            text_color: Color::WHITE,
            radius: tokens.radii.small,
            font_size: 12.0,
            padding_x: 10.0,
            padding_y: 8.0,
            max_width: 240.0,
            style: ResolvedComponentStyle {
                background: Some(Fill::Solid(Color {
                    r: 50,
                    g: 50,
                    b: 50,
                    a: 255,
                })),
                text_color: Some(Color::WHITE),
                radius: Some(tokens.radii.small),
                font_size: Some(12.0),
                padding_x: Some(10.0),
                padding_y: Some(8.0),
                max_width: Some(240.0),
                shadows: tokens
                    .elevations
                    .level2
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CardTheme {
    pub padding: f32,
    pub radius: f32,
    pub default_pattern: CardPattern,
    pub patterns: Vec<(CardPattern, ResolvedComponentStyle)>,
    pub hover_style: ResolvedComponentStyle,
}

impl CardTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let base_border = ComponentBorder {
            fill: Fill::Solid(tokens.colors.border),
            width: 1.0,
        };
        Self {
            padding: tokens.spacing.l,
            radius: tokens.radii.xl,
            default_pattern: CardPattern::Raised,
            patterns: vec![
                (
                    CardPattern::Plain,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        border: Some(base_border.clone()),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Raised,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        border: Some(base_border.clone()),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        shadows: tokens
                            .elevations
                            .level2
                            .map(shadow_layer_from_box)
                            .into_iter()
                            .collect(),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Tinted,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.primary_subtle)),
                        border: Some(ComponentBorder {
                            fill: Fill::Solid(tokens.colors.primary.with_alpha(80)),
                            width: 1.0,
                        }),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Elevated,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        border: Some(base_border),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        shadows: tokens
                            .elevations
                            .level1
                            .map(shadow_layer_from_box)
                            .into_iter()
                            .collect(),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            hover_style: ResolvedComponentStyle {
                shadows: tokens
                    .elevations
                    .level2
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    pub fn resolve(&self, pattern: CardPattern, hovered: bool) -> ResolvedComponentStyle {
        let base = self
            .patterns
            .iter()
            .find(|(candidate, _)| *candidate == pattern)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.patterns
                    .iter()
                    .find(|(candidate, _)| *candidate == self.default_pattern)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_default();
        if hovered {
            base.merge(&self.hover_style)
        } else {
            base
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatureIconTheme {
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub tones: Vec<(FeatureIconTone, ResolvedComponentStyle)>,
    pub shadow: Option<BoxShadow>,
}

impl FeatureIconTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            sizes: vec![
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        width: Some(40.0),
                        height: Some(40.0),
                        radius: Some(tokens.radii.medium),
                        icon_size: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Lg,
                    ResolvedComponentStyle {
                        width: Some(48.0),
                        height: Some(48.0),
                        radius: Some(10.0),
                        icon_size: Some(24.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Xl,
                    ResolvedComponentStyle {
                        width: Some(56.0),
                        height: Some(56.0),
                        radius: Some(12.0),
                        icon_size: Some(28.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            tones: vec![
                (
                    FeatureIconTone::Brand,
                    badge_tone(
                        tokens.colors.primary_subtle,
                        tokens.colors.primary.with_alpha(40),
                        tokens.colors.primary,
                    ),
                ),
                (
                    FeatureIconTone::Gray,
                    badge_tone(
                        tokens.colors.surface_sunken,
                        tokens.colors.border,
                        tokens.colors.text_primary,
                    ),
                ),
                (
                    FeatureIconTone::Blue,
                    badge_tone(
                        tokens.colors.info.with_alpha(26),
                        tokens.colors.info.with_alpha(80),
                        tokens.colors.info,
                    ),
                ),
                (
                    FeatureIconTone::Orange,
                    badge_tone(
                        tokens.colors.warning.with_alpha(26),
                        tokens.colors.warning.with_alpha(80),
                        tokens.colors.warning,
                    ),
                ),
            ],
            shadow: tokens.elevations.level1,
        }
    }
}

fn badge_tone(background: Color, border: Color, text_color: Color) -> ResolvedComponentStyle {
    ResolvedComponentStyle {
        background: Some(Fill::Solid(background)),
        text_color: Some(text_color),
        border: Some(ComponentBorder {
            fill: Fill::Solid(border),
            width: 1.0,
        }),
        ..ResolvedComponentStyle::default()
    }
}

fn find_size_style(
    styles: &[(ComponentSize, ResolvedComponentStyle)],
    size: ComponentSize,
) -> ResolvedComponentStyle {
    styles
        .iter()
        .find(|(candidate, _)| *candidate == size)
        .map(|(_, style)| style.clone())
        .or_else(|| {
            styles
                .iter()
                .find(|(candidate, _)| *candidate == ComponentSize::Md)
                .map(|(_, style)| style.clone())
        })
        .unwrap_or_default()
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
    pub card: CardTheme,
    pub feature_icon: FeatureIconTheme,
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
            card: CardTheme::from_tokens(tokens),
            feature_icon: FeatureIconTheme::from_tokens(tokens),
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
    #[serde(default)]
    pub design_system: ResolvedDesignSystem,
}

impl Default for Theme {
    fn default() -> Self {
        FissionDefaultDesignSystem::theme(DesignMode::Light)
    }
}

impl Theme {
    pub fn dark() -> Self {
        FissionDefaultDesignSystem::theme(DesignMode::Dark)
    }

    pub fn from_tokens(tokens: Tokens, mode: DesignMode) -> Self {
        let components = ComponentTheme::from_tokens(&tokens);
        Self {
            tokens,
            components,
            design_system: ResolvedDesignSystem {
                mode,
                ..ResolvedDesignSystem::default()
            },
        }
    }
}

include!(concat!(
    env!("OUT_DIR"),
    "/generated_default_design_system.rs"
));

pub mod presets {
    pub mod material3 {
        include!(concat!(
            env!("OUT_DIR"),
            "/generated_material3_design_system.rs"
        ));
    }

    pub mod fluent2 {
        include!(concat!(
            env!("OUT_DIR"),
            "/generated_fluent2_design_system.rs"
        ));
    }

    pub mod liquid_glass {
        include!(concat!(
            env!("OUT_DIR"),
            "/generated_liquid_glass_design_system.rs"
        ));
    }

    pub mod cupertino {
        include!(concat!(
            env!("OUT_DIR"),
            "/generated_cupertino_design_system.rs"
        ));
    }
}

pub use presets::cupertino::FissionCupertinoDesignSystem;
pub use presets::fluent2::FissionFluent2DesignSystem;
pub use presets::liquid_glass::FissionLiquidGlassDesignSystem;
pub use presets::material3::FissionMaterialDesign3DesignSystem;

/// Bundled font files embedded at compile time.
///
/// Provides Noto Sans Regular (the default) and Inter 24pt Regular.
pub mod fonts {
    pub const NOTO_SANS_REGULAR_TTF: &[u8] =
        include_bytes!("../fonts/Noto_Sans/static/NotoSans-Regular.ttf");
    pub const INTER_24PT_REGULAR_TTF: &[u8] =
        include_bytes!("../fonts/Inter/static/Inter_24pt-Regular.ttf");
    #[inline]
    pub fn default_font_bytes() -> &'static [u8] {
        NOTO_SANS_REGULAR_TTF
    }
}
