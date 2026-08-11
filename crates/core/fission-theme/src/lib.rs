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
    /// Font faces packaged with this design system.
    ///
    /// Hosts register these faces with their text measurer and renderer before
    /// the first frame so declared weight, style, and variation axes are used
    /// consistently instead of synthesized fallbacks.
    fn font_faces() -> &'static [PackagedFont] {
        &[]
    }
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
            spread_radius: self.spread_radius,
            inset: self.inset,
        }
    }
}

fn shadow_layer_from_box(shadow: BoxShadow) -> ShadowLayer {
    ShadowLayer {
        color: shadow.color,
        offset: shadow.offset,
        blur_radius: shadow.blur_radius,
        spread_radius: shadow.spread_radius,
        inset: shadow.inset,
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
    /// Logo and image assets declared by the design system.
    pub logos: Vec<DesignAsset>,
    /// Font assets declared by the design system.
    pub fonts: Vec<DesignFontAsset>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignAsset {
    /// Stable asset identifier from the DSP package.
    pub id: String,
    /// Path to the asset relative to the DSP file.
    pub path: String,
    /// File format such as `svg`, `png`, or `webp`.
    pub format: String,
}

/// Metadata for a font face declared by a Design System Package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignFontAsset {
    /// CSS/font family name exposed to app code.
    pub family: String,
    /// OpenType font weight, normally in the `100..=900` range.
    pub weight: u16,
    /// Font slope style.
    pub style: PackagedFontStyle,
    /// Path to the font file relative to the DSP file.
    pub path: String,
    /// Font format such as `truetype`, `opentype`, `woff`, or `woff2`.
    pub format: String,
    /// Optional variation-axis defaults.
    pub axes: Vec<FontVariationAxis>,
}

/// A variation-axis default applied when a packaged font is registered.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontVariationAxis {
    /// Four-byte OpenType variation tag, for example `wght`.
    pub tag: [u8; 4],
    /// Axis value used when the font face is registered.
    pub value: f32,
}

/// Font slope metadata used by packaged design-system fonts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackagedFontStyle {
    /// Upright roman glyphs.
    #[default]
    Normal,
    /// Italic glyphs.
    Italic,
    /// Oblique glyphs.
    Oblique,
}

/// A font face embedded in an application binary by design-system codegen.
#[derive(Clone, Copy, Debug)]
pub struct PackagedFont {
    /// CSS/font family name exposed to app code.
    pub family: &'static str,
    /// OpenType font weight.
    pub weight: u16,
    /// Font slope style.
    pub style: PackagedFontStyle,
    /// Font format such as `truetype`, `opentype`, `woff`, or `woff2`.
    pub format: &'static str,
    /// Embedded font bytes.
    pub data: &'static [u8],
    /// Optional variation-axis defaults.
    pub axes: &'static [FontVariationAxis],
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
                spread_radius: 0.0,
                inset: false,
                color: black_alpha(40),
                offset: (0.0, 1.0),
                blur_radius: 2.0,
            }),
            level2: Some(BoxShadow {
                spread_radius: 0.0,
                inset: false,
                color: black_alpha(60),
                offset: (0.0, 2.0),
                blur_radius: 4.0,
            }),
            level3: Some(BoxShadow {
                spread_radius: 0.0,
                inset: false,
                color: black_alpha(60),
                offset: (0.0, 4.0),
                blur_radius: 8.0,
            }),
            level4: None,
            level5: None,
            focus: Some(BoxShadow {
                spread_radius: 0.0,
                inset: false,
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

mod component_styles;
pub use component_styles::*;

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
