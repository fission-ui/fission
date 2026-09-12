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
use fission_ir::LayoutDirection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    Outline,
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

/// Visual presentation selected for a tab list.
///
/// `Default` preserves the active design system's ordinary tabs recipe.
/// `Underline` requests its underline variant and falls back to that ordinary
/// recipe when an older design system does not provide the variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabPresentation {
    #[default]
    Default,
    Underline,
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
    /// Font-family stack used by component-owned text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    pub font_weight: Option<u16>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub icon_size: Option<f32>,
    pub max_width: Option<f32>,
    pub shadows: Vec<ShadowLayer>,
    pub transition: Option<ComponentMotion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_bottom: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inset_top: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inset_end: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translate_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_dash: Option<Vec<f32>>,
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
            min_height: overlay.min_height.or(self.min_height),
            width: overlay.width.or(self.width),
            padding_x: overlay.padding_x.or(self.padding_x),
            padding_y: overlay.padding_y.or(self.padding_y),
            padding: overlay.padding.or(self.padding),
            margin: overlay.margin.or(self.margin),
            gap: overlay.gap.or(self.gap),
            font_size: overlay.font_size.or(self.font_size),
            font_family: overlay
                .font_family
                .clone()
                .or_else(|| self.font_family.clone()),
            font_weight: overlay.font_weight.or(self.font_weight),
            line_height: overlay.line_height.or(self.line_height),
            letter_spacing: overlay.letter_spacing.or(self.letter_spacing),
            icon_size: overlay.icon_size.or(self.icon_size),
            max_width: overlay.max_width.or(self.max_width),
            margin_bottom: overlay.margin_bottom.or(self.margin_bottom),
            inset_top: overlay.inset_top.or(self.inset_top),
            inset_end: overlay.inset_end.or(self.inset_end),
            opacity: overlay.opacity.or(self.opacity),
            translate_y: overlay.translate_y.or(self.translate_y),
            border_dash: overlay
                .border_dash
                .clone()
                .or_else(|| self.border_dash.clone()),
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

    /// Merges an overlay while preserving both sets of shadow layers.
    ///
    /// Normal [`Self::merge`] semantics intentionally replace a non-empty base
    /// shadow list. Use this method for orthogonal states such as a selected tab
    /// receiving a focus ring, where elevation and focus must both remain.
    pub fn merge_composing_shadows(&self, overlay: &Self) -> Self {
        let mut merged = self.merge(overlay);
        if !self.shadows.is_empty() && !overlay.shadows.is_empty() {
            merged.shadows = self
                .shadows
                .iter()
                .chain(&overlay.shadows)
                .cloned()
                .collect();
        }
        merged
    }

    /// Returns `[start, end, top, bottom]` padding in reading order.
    ///
    /// Component recipes describe spacing logically: a select trigger reserves
    /// room at the end for its chevron, not "on the right". Resolve to physical
    /// edges with [`padding_box_for`](Self::padding_box_for) at the point the
    /// value reaches layout.
    pub fn padding_box(&self, fallback_x: f32, fallback_y: f32) -> [f32; 4] {
        self.padding.unwrap_or([
            self.padding_x.unwrap_or(fallback_x),
            self.padding_x.unwrap_or(fallback_x),
            self.padding_y.unwrap_or(fallback_y),
            self.padding_y.unwrap_or(fallback_y),
        ])
    }

    /// Returns `[left, right, top, bottom]` padding for a reading order.
    ///
    /// Mirrors the inline edges of [`padding_box`](Self::padding_box) under a
    /// right-to-left layout. Controls should call this once, where the recipe
    /// meets layout, instead of every widget branching on direction itself.
    pub fn padding_box_for(
        &self,
        direction: LayoutDirection,
        fallback_x: f32,
        fallback_y: f32,
    ) -> [f32; 4] {
        let [start, end, top, bottom] = self.padding_box(fallback_x, fallback_y);
        match direction {
            LayoutDirection::LeftToRight => [start, end, top, bottom],
            LayoutDirection::RightToLeft => [end, start, top, bottom],
        }
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

/// A component's complete design-system recipe, resolved from its DSP entry.
///
/// Every `/components/<name>` block in a design system lowers into one of
/// these, generically. A widget reads its recipe by name and asks for the parts
/// it needs, so adding design authority to a widget is two steps — write the
/// recipe in each design system, then read it — rather than also hand-writing a
/// theme struct, a token fallback, a field on [`ComponentTheme`], and a bespoke
/// emitter in the codegen.
///
/// Sub-objects in the recipe become [`parts`](Self::parts) keyed by their JSON
/// name, `sizes` becomes density variants, `states` becomes interaction states,
/// and plain numbers become [`scalars`](Self::scalars).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComponentRecipe {
    /// Style declared directly on the recipe, outside any sub-object.
    #[serde(default)]
    pub base: ResolvedComponentStyle,
    /// Named anatomy, keyed as the design system names it: `item`, `surface`,
    /// `indicator`, `title`, and so on.
    #[serde(default)]
    pub parts: BTreeMap<String, ResolvedComponentStyle>,
    /// Density variants from the recipe's `sizes` block.
    #[serde(default)]
    pub sizes: BTreeMap<ComponentSize, ResolvedComponentStyle>,
    /// Interaction states from the recipe's `states` block.
    #[serde(default)]
    pub states: ComponentStateStyles,
    /// Plain numeric values that are not styles, such as `max_visible`,
    /// `overlap` or `narrow_breakpoint`.
    #[serde(default)]
    pub scalars: BTreeMap<String, f32>,
}

impl ComponentRecipe {
    /// Returns a named part, or an empty style when the design system does not
    /// declare one.
    ///
    /// Returning a default rather than `None` keeps widget code linear: ask for
    /// the part, then fall back per field with `unwrap_or`, so a design system
    /// can declare as much or as little as it wants.
    pub fn part(&self, name: &str) -> ResolvedComponentStyle {
        self.parts.get(name).cloned().unwrap_or_default()
    }

    /// Returns a named part only when the design system declares it.
    pub fn try_part(&self, name: &str) -> Option<&ResolvedComponentStyle> {
        self.parts.get(name)
    }

    /// Returns the density variant for `size`, falling back to the base style.
    pub fn size(&self, size: ComponentSize) -> ResolvedComponentStyle {
        self.sizes
            .get(&size)
            .map(|style| self.base.merge(style))
            .unwrap_or_else(|| self.base.clone())
    }

    /// Returns the base style with `state`'s overlay applied.
    pub fn state(&self, state: ComponentState) -> ResolvedComponentStyle {
        self.base.merge(&self.states.resolve(state))
    }

    /// Returns a scalar declared on the recipe.
    pub fn scalar(&self, name: &str) -> Option<f32> {
        self.scalars.get(name).copied()
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
/// uses the light-mode values generated from the bundled default DSP. Use
/// [`Tokens::dark()`] for its generated dark-mode values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub colors: ColorTokens,
    pub spacing: SpacingTokens,
    pub typography: TypographyTokens,
    pub radii: RadiusTokens,
    pub elevations: ElevationTokens,
    pub motion: MotionTokens,
    pub data_visualization: DataVisualizationTokens,
}

impl Default for Tokens {
    fn default() -> Self {
        FissionDefaultDesignSystem::theme_ref(DesignMode::Light)
            .tokens
            .clone()
    }
}

impl Tokens {
    pub fn dark() -> Self {
        FissionDefaultDesignSystem::theme_ref(DesignMode::Dark)
            .tokens
            .clone()
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
        let transparent = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        let transparent_border = ComponentBorder {
            fill: Fill::Solid(transparent),
            width: 1.0,
        };
        let focus_style = ResolvedComponentStyle {
            border: Some(ComponentBorder {
                fill: Fill::Solid(tokens.colors.focus_ring),
                width: 1.0,
            }),
            shadows: tokens
                .elevations
                .focus
                .map(shadow_layer_from_box)
                .into_iter()
                .collect(),
            ..ResolvedComponentStyle::default()
        };
        let disabled_style = ResolvedComponentStyle {
            opacity: Some(0.5),
            ..ResolvedComponentStyle::default()
        };
        let size_md = ResolvedComponentStyle {
            height: Some(32.0),
            padding_x: Some(10.0),
            padding_y: Some(0.0),
            gap: Some(6.0),
            font_size: Some(tokens.typography.font_size_base),
            font_weight: Some(tokens.typography.font_weight_medium),
            line_height: Some(20.0),
            icon_size: Some(16.0),
            ..ResolvedComponentStyle::default()
        };
        let primary = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary)),
                text_color: Some(tokens.colors.on_primary),
                border: Some(transparent_border.clone()),
                shadows: Vec::new(),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary.with_alpha(204))),
                shadows: Vec::new(),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary.with_alpha(204))),
                translate_y: Some(1.0),
                shadows: Vec::new(),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(focus_style.clone()),
            disabled: Some(disabled_style.clone()),
            ..ComponentStateStyles::default()
        };
        let secondary_color = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                text_color: Some(tokens.colors.primary),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.primary_subtle),
                    width: 1.0,
                }),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary_subtle)),
                text_color: Some(tokens.colors.primary_hover),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary_subtle)),
                translate_y: Some(1.0),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(focus_style.clone()),
            disabled: Some(disabled_style.clone()),
            ..ComponentStateStyles::default()
        };
        let secondary_gray = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                text_color: Some(tokens.colors.text_primary),
                border: Some(transparent_border.clone()),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                translate_y: Some(1.0),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(focus_style.clone()),
            disabled: Some(disabled_style.clone()),
            ..ComponentStateStyles::default()
        };
        let outline = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(transparent)),
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
            active: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                translate_y: Some(1.0),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(focus_style.clone()),
            disabled: Some(disabled_style.clone()),
            ..ComponentStateStyles::default()
        };
        let tertiary_color = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(transparent)),
                text_color: Some(tokens.colors.primary),
                border: Some(transparent_border.clone()),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary_subtle)),
                text_color: Some(tokens.colors.primary_hover),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary_subtle)),
                translate_y: Some(1.0),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(focus_style.clone()),
            disabled: Some(disabled_style.clone()),
            ..ComponentStateStyles::default()
        };
        let tertiary_gray = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(transparent)),
                text_color: Some(tokens.colors.text_primary),
                border: Some(transparent_border.clone()),
                transition,
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                translate_y: Some(1.0),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(focus_style.clone()),
            disabled: Some(disabled_style.clone()),
            ..ComponentStateStyles::default()
        };
        Self {
            height: 32.0,
            padding_horizontal: 10.0,
            padding_vertical: 0.0,
            radius: tokens.radii.medium,
            text_size: tokens.typography.font_size_base,
            elevation_rest: tokens.elevations.level0,
            elevation_hover: tokens.elevations.level0,
            elevation_pressed: tokens.elevations.level0,
            focus_stroke: Some(Stroke {
                fill: fission_ir::op::Fill::Solid(tokens.colors.focus_ring),
                width: 1.0,
                dash_array: None,
                line_cap: fission_ir::op::LineCap::Round,
                line_join: fission_ir::op::LineJoin::Round,
            }),
            icon_size: 16.0,
            font_weight: tokens.typography.font_weight_medium,
            line_height: 20.0,
            transition: Some(ComponentMotion {
                duration_ms: tokens.motion.duration_fast_ms,
                easing: tokens.motion.easing_standard.clone(),
            }),
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(28.0),
                        padding_x: Some(10.0),
                        padding_y: Some(0.0),
                        gap: Some(4.0),
                        radius: Some(tokens.radii.small),
                        font_size: Some(12.8),
                        line_height: Some(19.2),
                        icon_size: Some(14.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (ComponentSize::Md, size_md),
                (
                    ComponentSize::Lg,
                    ResolvedComponentStyle {
                        height: Some(36.0),
                        padding_x: Some(10.0),
                        padding_y: Some(0.0),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        icon_size: Some(16.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Xl,
                    ResolvedComponentStyle {
                        height: Some(40.0),
                        padding_x: Some(12.0),
                        padding_y: Some(0.0),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        icon_size: Some(16.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            hierarchies: vec![
                (ButtonHierarchy::Primary, primary.clone()),
                (ButtonHierarchy::SecondaryColor, secondary_color),
                (ButtonHierarchy::SecondaryGray, secondary_gray),
                (ButtonHierarchy::Outline, outline),
                (ButtonHierarchy::TertiaryColor, tertiary_color.clone()),
                (ButtonHierarchy::TertiaryGray, tertiary_gray.clone()),
                (ButtonHierarchy::LinkColor, tertiary_color),
                (ButtonHierarchy::LinkGray, tertiary_gray.clone()),
                (
                    ButtonHierarchy::Destructive,
                    ComponentStateStyles {
                        default: ResolvedComponentStyle {
                            background: Some(Fill::Solid(tokens.colors.error.with_alpha(26))),
                            text_color: Some(tokens.colors.error),
                            ..primary.default.clone()
                        },
                        hover: Some(ResolvedComponentStyle {
                            background: Some(Fill::Solid(tokens.colors.error.with_alpha(51))),
                            ..ResolvedComponentStyle::default()
                        }),
                        active: Some(ResolvedComponentStyle {
                            background: Some(Fill::Solid(tokens.colors.error.with_alpha(51))),
                            translate_y: Some(1.0),
                            ..ResolvedComponentStyle::default()
                        }),
                        focus: Some(ResolvedComponentStyle {
                            border: Some(ComponentBorder {
                                fill: Fill::Solid(tokens.colors.error.with_alpha(102)),
                                width: 1.0,
                            }),
                            shadows: vec![ShadowLayer {
                                color: tokens.colors.error.with_alpha(51),
                                offset: (0.0, 0.0),
                                blur_radius: 0.0,
                                spread_radius: 3.0,
                                inset: false,
                            }],
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
            height: 32.0,
            padding_h: 10.0,
            radius: tokens.radii.medium,
            font_size: tokens.typography.font_size_base,
            border_color: tokens.colors.border,
            border_width: 1.0,
            focus_color: tokens.colors.primary,
            text_color: tokens.colors.text_primary,
            placeholder_color: tokens.colors.text_secondary,
            line_height: 20.0,
            font_weight: tokens.typography.font_weight_regular,
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(28.0),
                        padding_x: Some(10.0),
                        padding_y: Some(3.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(32.0),
                        padding_x: Some(10.0),
                        padding_y: Some(4.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    background: Some(Fill::Solid(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    })),
                    text_color: Some(tokens.colors.text_primary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.border),
                        width: 1.0,
                    }),
                    shadows: Vec::new(),
                    ..ResolvedComponentStyle::default()
                },
                focus: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.focus_ring),
                        width: 1.0,
                    }),
                    shadows: tokens
                        .elevations
                        .focus
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    ..ResolvedComponentStyle::default()
                }),
                error: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.error),
                        width: 1.0,
                    }),
                    shadows: vec![ShadowLayer {
                        color: tokens.colors.error.with_alpha(51),
                        offset: (0.0, 0.0),
                        blur_radius: 0.0,
                        spread_radius: 3.0,
                        inset: false,
                    }],
                    ..ResolvedComponentStyle::default()
                }),
                disabled: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.border.with_alpha(128))),
                    opacity: Some(0.5),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            placeholder_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                ..ResolvedComponentStyle::default()
            },
            label_style: ResolvedComponentStyle {
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_medium),
                line_height: Some(19.25),
                text_color: Some(tokens.colors.text_primary),
                ..ResolvedComponentStyle::default()
            },
            helper_style: ResolvedComponentStyle {
                font_size: Some(tokens.typography.font_size_base),
                text_color: Some(tokens.colors.text_secondary),
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

/// Visual recipes for the retained trigger of a single-value selection control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelectTheme {
    /// Trigger density recipes.
    #[serde(default)]
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    /// Trigger paint and interaction recipes.
    #[serde(default)]
    pub trigger_states: ComponentStateStyles,
    /// Presentation for an unresolved or placeholder value.
    #[serde(default)]
    pub placeholder_style: ResolvedComponentStyle,
    /// Geometry and paint for the trailing disclosure icon.
    #[serde(default)]
    pub indicator_style: ResolvedComponentStyle,
}

impl SelectTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let transparent = Fill::Solid(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        Self {
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(28.0),
                        radius: Some(tokens.radii.small),
                        padding: Some([10.0, 8.0, 3.0, 3.0]),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        font_weight: Some(tokens.typography.font_weight_regular),
                        line_height: Some(20.0),
                        icon_size: Some(16.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(32.0),
                        radius: Some(tokens.radii.medium),
                        padding: Some([10.0, 8.0, 5.0, 5.0]),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        font_weight: Some(tokens.typography.font_weight_regular),
                        line_height: Some(20.0),
                        icon_size: Some(16.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            trigger_states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    background: Some(transparent),
                    text_color: Some(tokens.colors.text_primary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.border),
                        width: 1.0,
                    }),
                    transition: Some(ComponentMotion {
                        duration_ms: tokens.motion.duration_fast_ms,
                        easing: tokens.motion.easing_standard.clone(),
                    }),
                    ..ResolvedComponentStyle::default()
                },
                hover: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                    ..ResolvedComponentStyle::default()
                }),
                focus: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.focus_ring),
                        width: 1.0,
                    }),
                    shadows: tokens
                        .elevations
                        .focus
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    ..ResolvedComponentStyle::default()
                }),
                disabled: Some(ResolvedComponentStyle {
                    opacity: Some(0.5),
                    ..ResolvedComponentStyle::default()
                }),
                error: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.error),
                        width: 1.0,
                    }),
                    shadows: vec![ShadowLayer {
                        color: tokens.colors.error.with_alpha(51),
                        offset: (0.0, 0.0),
                        blur_radius: 0.0,
                        spread_radius: 3.0,
                        inset: false,
                    }],
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            placeholder_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                ..ResolvedComponentStyle::default()
            },
            indicator_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                icon_size: Some(16.0),
                inset_end: Some(8.0),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    /// Resolves trigger density first and then its current interaction state.
    pub fn resolve_trigger(
        &self,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        find_size_style(&self.sizes, size).merge(&self.trigger_states.resolve(state))
    }
}

impl Default for SelectTheme {
    fn default() -> Self {
        Self::from_tokens(&Tokens::default())
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
    /// Shared geometry and typography for previous, page, and next controls.
    #[serde(default)]
    pub item_style: ResolvedComponentStyle,
    /// Static overlay for the current page.
    #[serde(default)]
    pub selected_style: ResolvedComponentStyle,
    /// Presentation of the noninteractive page-range ellipsis.
    #[serde(default)]
    pub ellipsis_style: ResolvedComponentStyle,
    pub spacing: f32,
    pub active_bg: Color,
    pub active_text: Color,
}

impl PaginationTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            item_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_primary),
                radius: Some(tokens.radii.medium),
                height: Some(tokens.spacing.xl),
                width: Some(tokens.spacing.xl),
                padding: Some([tokens.spacing.none; 4]),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(tokens.spacing.m + tokens.spacing.xs),
                icon_size: Some(tokens.spacing.m),
                ..ResolvedComponentStyle::default()
            },
            selected_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                font_weight: Some(tokens.typography.font_weight_medium),
                ..ResolvedComponentStyle::default()
            },
            ellipsis_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_muted),
                height: Some(tokens.spacing.xl),
                width: Some(tokens.spacing.xl),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(tokens.spacing.m + tokens.spacing.xs),
                ..ResolvedComponentStyle::default()
            },
            spacing: tokens.spacing.xs,
            active_bg: tokens.colors.surface,
            active_text: tokens.colors.text_primary,
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
    /// Shared surface geometry and paint before the semantic tone is applied.
    #[serde(default)]
    pub surface_style: ResolvedComponentStyle,
    /// Leading icon geometry.
    #[serde(default)]
    pub icon_style: ResolvedComponentStyle,
    /// Vertical layout for the title and supporting description.
    #[serde(default)]
    pub content_style: ResolvedComponentStyle,
    /// Primary message typography.
    #[serde(default)]
    pub title_style: ResolvedComponentStyle,
    /// Supporting message typography.
    #[serde(default)]
    pub description_style: ResolvedComponentStyle,
    /// Optional trailing action region.
    #[serde(default)]
    pub action_style: ResolvedComponentStyle,
    /// Information-tone surface and foreground overlay.
    #[serde(default)]
    pub info_style: ResolvedComponentStyle,
    /// Warning-tone surface and foreground overlay.
    #[serde(default)]
    pub warning_style: ResolvedComponentStyle,
    /// Error-tone surface and foreground overlay.
    #[serde(default)]
    pub error_style: ResolvedComponentStyle,
    /// Success-tone surface and foreground overlay.
    #[serde(default)]
    pub success_style: ResolvedComponentStyle,
    /// Compatibility color used by older direct theme construction.
    pub info_bg: Color,
    /// Compatibility color used by older direct theme construction.
    pub warning_bg: Color,
    /// Compatibility color used by older direct theme construction.
    pub error_bg: Color,
    /// Compatibility color used by older direct theme construction.
    pub success_bg: Color,
    /// Compatibility radius used when a partial theme omits `surface_style`.
    pub radius: f32,
}

impl AlertTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let surface_style = ResolvedComponentStyle {
            background: Some(Fill::Solid(tokens.colors.surface)),
            border: Some(ComponentBorder {
                fill: Fill::Solid(tokens.colors.border),
                width: 1.0,
            }),
            radius: Some(tokens.radii.medium),
            min_height: Some(60.0),
            padding: Some([
                tokens.spacing.s + tokens.spacing.xs / 2.0,
                tokens.spacing.s + tokens.spacing.xs / 2.0,
                tokens.spacing.s,
                tokens.spacing.s,
            ]),
            gap: Some(tokens.spacing.s),
            ..ResolvedComponentStyle::default()
        };
        let tone_style = |color| ResolvedComponentStyle {
            background: Some(Fill::Solid(tokens.colors.surface)),
            text_color: Some(color),
            ..ResolvedComponentStyle::default()
        };
        Self {
            surface_style,
            icon_style: ResolvedComponentStyle {
                icon_size: Some(tokens.spacing.m),
                inset_top: Some(2.0),
                translate_y: Some(2.0),
                ..ResolvedComponentStyle::default()
            },
            content_style: ResolvedComponentStyle {
                gap: Some(tokens.spacing.xs / 2.0),
                ..ResolvedComponentStyle::default()
            },
            title_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_primary),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_medium),
                line_height: Some(tokens.spacing.m + tokens.spacing.xs),
                ..ResolvedComponentStyle::default()
            },
            description_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(tokens.spacing.m + tokens.spacing.xs),
                ..ResolvedComponentStyle::default()
            },
            action_style: ResolvedComponentStyle {
                width: Some(64.0),
                inset_top: Some(tokens.spacing.s),
                inset_end: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
            info_style: tone_style(tokens.colors.info),
            warning_style: tone_style(tokens.colors.warning),
            error_style: tone_style(tokens.colors.error),
            success_style: tone_style(tokens.colors.success),
            info_bg: Color {
                r: tokens.colors.surface.r,
                g: tokens.colors.surface.g,
                b: tokens.colors.surface.b,
                a: 255,
            },
            warning_bg: Color {
                r: tokens.colors.surface.r,
                g: tokens.colors.surface.g,
                b: tokens.colors.surface.b,
                a: 255,
            },
            error_bg: tokens.colors.surface,
            success_bg: Color {
                r: tokens.colors.surface.r,
                g: tokens.colors.surface.g,
                b: tokens.colors.surface.b,
                a: 255,
            },
            radius: tokens.radii.medium,
        }
    }
}

/// Visual parameters for the `Avatar` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvatarTheme {
    /// Background and foreground used when no image is available.
    pub fallback_style: ResolvedComponentStyle,
}

impl AvatarTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            fallback_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary)),
                text_color: Some(tokens.colors.on_primary),
                ..ResolvedComponentStyle::default()
            },
        }
    }
}

impl Default for AvatarTheme {
    fn default() -> Self {
        Self::from_tokens(&Tokens::default())
    }
}

/// Visual parameters for the `AvatarGroup` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvatarGroupTheme {
    /// Shared avatar diameter, separating ring, and radius.
    pub avatar_style: ResolvedComponentStyle,
    /// Overflow-count surface and typography.
    pub overflow_style: ResolvedComponentStyle,
    /// Logical pixels by which adjacent avatar surfaces overlap.
    pub overlap: f32,
    /// Default number of people shown directly before a `+N` indicator.
    pub max_visible: usize,
}

impl AvatarGroupTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let ring = ComponentBorder {
            fill: Fill::Solid(tokens.colors.surface),
            width: 2.0,
        };
        Self {
            avatar_style: ResolvedComponentStyle {
                width: Some(32.0),
                height: Some(32.0),
                radius: Some(tokens.radii.full),
                border: Some(ring.clone()),
                ..ResolvedComponentStyle::default()
            },
            overflow_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                text_color: Some(tokens.colors.text_secondary),
                border: Some(ring),
                width: Some(32.0),
                height: Some(32.0),
                radius: Some(tokens.radii.full),
                font_size: Some(tokens.typography.font_size_xs),
                font_weight: Some(tokens.typography.font_weight_medium),
                line_height: Some(16.0),
                ..ResolvedComponentStyle::default()
            },
            overlap: 10.0,
            max_visible: 4,
        }
    }
}

impl Default for AvatarGroupTheme {
    fn default() -> Self {
        Self::from_tokens(&Tokens::default())
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

/// Visual recipes shared by menu and list-box popup anatomy.
///
/// The popup surface, actionable rows, labels, descriptions, trailing metadata,
/// selection indicator, group labels, and separators all resolve through this
/// one theme so convenience widgets and composed menu content stay visually
/// consistent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuTheme {
    /// Popup surface paint and layout recipe.
    #[serde(default)]
    pub surface_style: ResolvedComponentStyle,
    /// Interaction states for normal actionable rows.
    #[serde(default)]
    pub item_states: ComponentStateStyles,
    /// Tone overlay and interaction states for destructive rows.
    #[serde(default)]
    pub destructive_item_states: ComponentStateStyles,
    /// Secondary line shown beneath an item label.
    #[serde(default)]
    pub description_style: ResolvedComponentStyle,
    /// Keyboard shortcut presentation at the trailing edge.
    #[serde(default)]
    pub shortcut_style: ResolvedComponentStyle,
    /// Non-shortcut trailing metadata presentation.
    #[serde(default)]
    pub metadata_style: ResolvedComponentStyle,
    /// Heading shown above a related group of items.
    #[serde(default)]
    pub group_label_style: ResolvedComponentStyle,
    /// Boundary between related menu regions.
    #[serde(default)]
    pub separator_style: ResolvedComponentStyle,
    /// Selected-item indicator icon recipe.
    #[serde(default)]
    pub indicator_style: ResolvedComponentStyle,
    /// Density recipes for the control which opens the popup.
    #[serde(default)]
    pub trigger_sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    /// Paint and interaction recipes for the popup trigger.
    #[serde(default)]
    pub trigger_states: ComponentStateStyles,
}

impl MenuTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let transparent = Fill::Solid(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        let mut trigger_states =
            ButtonTheme::from_tokens(tokens).hierarchy_style(ButtonHierarchy::Outline);
        trigger_states.selected = trigger_states.hover.clone();
        let normal_states = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(transparent.clone()),
                text_color: Some(tokens.colors.text_primary),
                radius: Some(tokens.radii.small),
                height: Some(28.0),
                padding_x: Some(6.0),
                padding_y: Some(tokens.spacing.xs),
                gap: Some(6.0),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(20.0),
                icon_size: Some(16.0),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            disabled: Some(ResolvedComponentStyle {
                background: Some(transparent.clone()),
                opacity: Some(0.5),
                ..ResolvedComponentStyle::default()
            }),
            selected: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            ..ComponentStateStyles::default()
        };
        Self {
            trigger_sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(28.0),
                        radius: Some(tokens.radii.small),
                        padding: Some([10.0, 6.0, 0.0, 0.0]),
                        gap: Some(4.0),
                        font_size: Some(12.8),
                        font_weight: Some(tokens.typography.font_weight_medium),
                        line_height: Some(19.2),
                        icon_size: Some(14.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(32.0),
                        radius: Some(tokens.radii.medium),
                        padding: Some([10.0, 8.0, 0.0, 0.0]),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        font_weight: Some(tokens.typography.font_weight_medium),
                        line_height: Some(20.0),
                        icon_size: Some(16.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            trigger_states,
            surface_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                radius: Some(tokens.radii.medium),
                width: Some(208.0),
                padding: Some([tokens.spacing.xs; 4]),
                gap: Some(0.0),
                shadows: std::iter::once(ShadowLayer {
                    color: tokens.colors.text_primary.with_alpha(26),
                    offset: (0.0, 0.0),
                    blur_radius: 0.0,
                    spread_radius: 1.0,
                    inset: false,
                })
                .chain(tokens.elevations.level2.map(shadow_layer_from_box))
                .collect(),
                ..ResolvedComponentStyle::default()
            },
            item_states: normal_states,
            destructive_item_states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    text_color: Some(tokens.colors.error),
                    ..ResolvedComponentStyle::default()
                },
                hover: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.error.with_alpha(26))),
                    ..ResolvedComponentStyle::default()
                }),
                active: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.error.with_alpha(51))),
                    ..ResolvedComponentStyle::default()
                }),
                focus: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.error.with_alpha(26))),
                    ..ResolvedComponentStyle::default()
                }),
                disabled: Some(ResolvedComponentStyle {
                    background: Some(transparent),
                    opacity: Some(0.5),
                    ..ResolvedComponentStyle::default()
                }),
                selected: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.error.with_alpha(26))),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            description_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                font_size: Some(tokens.typography.font_size_sm),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(18.0),
                gap: Some(2.0),
                ..ResolvedComponentStyle::default()
            },
            shortcut_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                font_size: Some(tokens.typography.font_size_xs),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(16.0),
                letter_spacing: Some(0.4),
                ..ResolvedComponentStyle::default()
            },
            metadata_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                font_size: Some(tokens.typography.font_size_sm),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(18.0),
                ..ResolvedComponentStyle::default()
            },
            group_label_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                height: Some(24.0),
                padding_x: Some(6.0),
                padding_y: Some(tokens.spacing.xs),
                font_size: Some(tokens.typography.font_size_xs),
                font_weight: Some(tokens.typography.font_weight_medium),
                line_height: Some(16.0),
                ..ResolvedComponentStyle::default()
            },
            separator_style: ResolvedComponentStyle {
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.divider),
                    width: 1.0,
                }),
                height: Some(9.0),
                padding_y: Some(tokens.spacing.xs),
                margin: Some([-4.0, -4.0, 0.0, 0.0]),
                ..ResolvedComponentStyle::default()
            },
            indicator_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_primary),
                icon_size: Some(16.0),
                inset_end: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    /// Resolves an actionable row for its tone and current interaction state.
    pub fn resolve_item(&self, destructive: bool, state: ComponentState) -> ResolvedComponentStyle {
        let style = self.item_states.resolve(state);
        if destructive {
            style.merge(&self.destructive_item_states.resolve(state))
        } else {
            style
        }
    }

    /// Resolves popup-trigger density first and then its interaction state.
    pub fn resolve_trigger(
        &self,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        find_size_style(&self.trigger_sizes, size).merge(&self.trigger_states.resolve(state))
    }
}

impl Default for MenuTheme {
    fn default() -> Self {
        Self::from_tokens(&Tokens::default())
    }
}

/// One named visual treatment for the `Tabs` widget.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TabPresentationTheme {
    pub indicator_height: f32,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub states: ComponentStateStyles,
    pub track_style: ResolvedComponentStyle,
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
    /// Optional named treatments in addition to the design system's default.
    ///
    /// This defaults empty so themes serialized before named presentations
    /// were introduced continue to decode and use their existing treatment.
    #[serde(default)]
    pub presentations: Vec<(TabPresentation, TabPresentationTheme)>,
}

impl TabsTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            active_color: tokens.colors.text_primary,
            inactive_color: tokens.colors.text_secondary,
            indicator_height: 0.0,
            background: tokens.colors.surface_sunken,
            divider_color: tokens.colors.border,
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(24.0),
                        padding_x: Some(6.0),
                        padding_y: Some(2.0),
                        gap: Some(4.0),
                        font_size: Some(tokens.typography.font_size_sm),
                        line_height: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(25.0),
                        padding_x: Some(6.0),
                        padding_y: Some(2.0),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    background: Some(Fill::Solid(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    })),
                    text_color: Some(tokens.colors.text_secondary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        }),
                        width: 1.0,
                    }),
                    radius: Some(tokens.radii.small),
                    ..ResolvedComponentStyle::default()
                },
                hover: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.surface)),
                    text_color: Some(tokens.colors.text_primary),
                    ..ResolvedComponentStyle::default()
                }),
                active: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.surface)),
                    text_color: Some(tokens.colors.text_primary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        }),
                        width: 1.0,
                    }),
                    radius: Some(tokens.radii.small),
                    font_weight: Some(tokens.typography.font_weight_medium),
                    shadows: tokens
                        .elevations
                        .level1
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    ..ResolvedComponentStyle::default()
                }),
                focus: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.focus_ring),
                        width: 1.0,
                    }),
                    shadows: tokens
                        .elevations
                        .focus
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    ..ResolvedComponentStyle::default()
                }),
                disabled: Some(ResolvedComponentStyle {
                    opacity: Some(0.5),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            track_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                border: None,
                radius: Some(tokens.radii.medium),
                height: Some(32.0),
                padding: Some([3.0; 4]),
                gap: Some(0.0),
                ..ResolvedComponentStyle::default()
            },
            presentations: vec![(
                TabPresentation::Underline,
                TabPresentationTheme {
                    indicator_height: 2.0,
                    sizes: vec![
                        (
                            ComponentSize::Sm,
                            ResolvedComponentStyle {
                                height: Some(40.0),
                                padding_x: Some(8.0),
                                gap: Some(4.0),
                                font_size: Some(tokens.typography.font_size_sm),
                                line_height: Some(20.0),
                                ..ResolvedComponentStyle::default()
                            },
                        ),
                        (
                            ComponentSize::Md,
                            ResolvedComponentStyle {
                                height: Some(44.0),
                                padding_x: Some(12.0),
                                gap: Some(6.0),
                                font_size: Some(tokens.typography.font_size_base),
                                line_height: Some(20.0),
                                ..ResolvedComponentStyle::default()
                            },
                        ),
                    ],
                    states: ComponentStateStyles {
                        default: ResolvedComponentStyle {
                            background: Some(Fill::Solid(Color::TRANSPARENT)),
                            text_color: Some(tokens.colors.text_secondary),
                            ..ResolvedComponentStyle::default()
                        },
                        hover: Some(ResolvedComponentStyle {
                            text_color: Some(tokens.colors.text_primary),
                            ..ResolvedComponentStyle::default()
                        }),
                        active: Some(ResolvedComponentStyle {
                            text_color: Some(tokens.colors.primary),
                            font_weight: Some(tokens.typography.font_weight_medium),
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
                            opacity: Some(0.5),
                            ..ResolvedComponentStyle::default()
                        }),
                        ..ComponentStateStyles::default()
                    },
                    track_style: ResolvedComponentStyle {
                        background: Some(Fill::Solid(Color::TRANSPARENT)),
                        padding: Some([0.0; 4]),
                        gap: Some(0.0),
                        ..ResolvedComponentStyle::default()
                    },
                },
            )],
        }
    }

    pub fn resolve_tab(
        &self,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        self.resolve_tab_presentation(TabPresentation::Default, size, state)
    }

    pub fn resolve_tab_presentation(
        &self,
        presentation: TabPresentation,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        let (sizes, states) = self
            .presentation(presentation)
            .map(|variant| (&variant.sizes, &variant.states))
            .unwrap_or((&self.sizes, &self.states));
        find_size_style(sizes, size).merge(&states.resolve(state))
    }

    pub fn track_style_for(&self, presentation: TabPresentation) -> &ResolvedComponentStyle {
        self.presentation(presentation)
            .map(|variant| &variant.track_style)
            .unwrap_or(&self.track_style)
    }

    pub fn indicator_height_for(&self, presentation: TabPresentation) -> f32 {
        self.presentation(presentation)
            .map(|variant| variant.indicator_height)
            .unwrap_or(self.indicator_height)
    }

    fn presentation(&self, presentation: TabPresentation) -> Option<&TabPresentationTheme> {
        if presentation == TabPresentation::Default {
            return None;
        }
        self.presentations
            .iter()
            .find(|(candidate, _)| *candidate == presentation)
            .map(|(_, theme)| theme)
    }
}

/// Visual parameters for the `Modal` widget.
///
/// The container and named anatomy styles are the single visual authority used
/// by both the compact `Modal` API and retained `ModalLayout` composition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModalTheme {
    pub bg_color: Color,
    pub radius: f32,
    pub shadow: Option<BoxShadow>,
    pub max_width: f32,
    pub container_style: ResolvedComponentStyle,
    pub scrim_style: ResolvedComponentStyle,
    pub scrim_blur: f32,
    /// Layout and paint recipe for the title/description/close region.
    #[serde(default)]
    pub header_style: ResolvedComponentStyle,
    /// Typography recipe for the dialog title.
    #[serde(default)]
    pub title_style: ResolvedComponentStyle,
    /// Typography recipe for supporting description text.
    #[serde(default)]
    pub description_style: ResolvedComponentStyle,
    /// Layout and paint recipe for the scrolling body.
    #[serde(default)]
    pub content_style: ResolvedComponentStyle,
    /// Layout and paint recipe for the action region.
    #[serde(default)]
    pub footer_style: ResolvedComponentStyle,
    /// Geometry recipe for the generated icon-only close control.
    #[serde(default)]
    pub close_button_style: ResolvedComponentStyle,
    /// Minimum distance between the dialog surface and viewport edges.
    #[serde(default = "default_modal_viewport_margin")]
    pub viewport_margin: f32,
    /// Width below which footer actions use a vertical layout.
    #[serde(default = "default_modal_action_stack_breakpoint")]
    pub action_stack_breakpoint: f32,
    /// Duration used by the built-in modal enter and exit motion.
    #[serde(default = "default_modal_motion_duration_ms")]
    pub motion_duration_ms: u64,
    /// Scale used at the start of the built-in modal enter motion.
    #[serde(default = "default_modal_motion_initial_scale")]
    pub motion_initial_scale: f32,
}

const fn default_modal_viewport_margin() -> f32 {
    16.0
}

const fn default_modal_action_stack_breakpoint() -> f32 {
    640.0
}

const fn default_modal_motion_duration_ms() -> u64 {
    100
}

fn default_modal_motion_initial_scale() -> f32 {
    0.95
}

impl ModalTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            radius: tokens.radii.large,
            shadow: Some(BoxShadow {
                color: tokens.colors.text_primary.with_alpha(26),
                offset: (0.0, 0.0),
                blur_radius: 0.0,
                spread_radius: 1.0,
                inset: false,
            }),
            max_width: 384.0,
            container_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                radius: Some(tokens.radii.large),
                padding: Some([tokens.spacing.m; 4]),
                gap: Some(tokens.spacing.m),
                max_width: Some(384.0),
                shadows: vec![ShadowLayer {
                    color: tokens.colors.text_primary.with_alpha(26),
                    offset: (0.0, 0.0),
                    blur_radius: 0.0,
                    spread_radius: 1.0,
                    inset: false,
                }],
                ..ResolvedComponentStyle::default()
            },
            scrim_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 26,
                })),
                ..ResolvedComponentStyle::default()
            },
            scrim_blur: 4.0,
            header_style: ResolvedComponentStyle {
                gap: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
            title_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_primary),
                font_size: Some(16.0),
                font_weight: Some(tokens.typography.font_weight_medium),
                line_height: Some(16.0),
                ..ResolvedComponentStyle::default()
            },
            description_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(20.0),
                ..ResolvedComponentStyle::default()
            },
            content_style: ResolvedComponentStyle::default(),
            footer_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken.with_alpha(128))),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                padding: Some([tokens.spacing.m; 4]),
                margin: Some([-tokens.spacing.m, -tokens.spacing.m, 0.0, -tokens.spacing.m]),
                gap: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
            close_button_style: ResolvedComponentStyle {
                width: Some(28.0),
                height: Some(28.0),
                padding: Some([0.0; 4]),
                icon_size: Some(16.0),
                inset_top: Some(tokens.spacing.s),
                inset_end: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
            viewport_margin: default_modal_viewport_margin(),
            action_stack_breakpoint: default_modal_action_stack_breakpoint(),
            motion_duration_ms: default_modal_motion_duration_ms(),
            motion_initial_scale: default_modal_motion_initial_scale(),
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
    /// Orthogonal treatment applied while a card is selected.
    #[serde(default)]
    pub selected_style: ResolvedComponentStyle,
    /// Optional logical-leading accent painted while a card is selected.
    ///
    /// `width` controls its thickness, `background` its fill, and vertical
    /// values from `margin` inset it from the top and bottom edges. A missing
    /// or non-positive width disables the accent without affecting the
    /// selected surface, border, tint, or shadow recipe.
    #[serde(default)]
    pub selected_indicator_style: ResolvedComponentStyle,
    /// Density recipes shared by every named card region.
    #[serde(default)]
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    /// Layout and paint recipe for the heading region.
    #[serde(default)]
    pub header_style: ResolvedComponentStyle,
    /// Layout and paint recipe for the primary content region.
    #[serde(default)]
    pub content_style: ResolvedComponentStyle,
    /// Layout and paint recipe for the action region.
    #[serde(default)]
    pub footer_style: ResolvedComponentStyle,
    /// Typography recipe for the standard card title.
    #[serde(default)]
    pub title_style: ResolvedComponentStyle,
    /// Typography recipe for the standard card description.
    #[serde(default)]
    pub description_style: ResolvedComponentStyle,
    /// Paint and margin recipe for boundaries between card regions.
    #[serde(default)]
    pub separator_style: ResolvedComponentStyle,
}

impl CardTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let surface_ring = ShadowLayer {
            color: tokens.colors.text_primary.with_alpha(26),
            offset: (0.0, 0.0),
            blur_radius: 0.0,
            spread_radius: 1.0,
            inset: false,
        };
        Self {
            padding: tokens.spacing.m,
            radius: tokens.radii.large,
            default_pattern: CardPattern::Raised,
            patterns: vec![
                (
                    CardPattern::Plain,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        radius: Some(tokens.radii.large),
                        padding_x: Some(tokens.spacing.m),
                        padding_y: Some(tokens.spacing.m),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Raised,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        radius: Some(tokens.radii.large),
                        padding_x: Some(tokens.spacing.m),
                        padding_y: Some(tokens.spacing.m),
                        shadows: vec![surface_ring.clone()],
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Tinted,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.primary_subtle)),
                        radius: Some(tokens.radii.large),
                        padding_x: Some(tokens.spacing.m),
                        padding_y: Some(tokens.spacing.m),
                        shadows: vec![surface_ring.clone()],
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Elevated,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        radius: Some(tokens.radii.large),
                        padding_x: Some(tokens.spacing.m),
                        padding_y: Some(tokens.spacing.m),
                        shadows: std::iter::once(surface_ring.clone())
                            .chain(tokens.elevations.level1.map(shadow_layer_from_box))
                            .collect(),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            hover_style: ResolvedComponentStyle {
                shadows: std::iter::once(surface_ring)
                    .chain(tokens.elevations.level1.map(shadow_layer_from_box))
                    .collect(),
                ..ResolvedComponentStyle::default()
            },
            selected_style: ResolvedComponentStyle::default(),
            selected_indicator_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary)),
                width: Some(4.0),
                ..ResolvedComponentStyle::default()
            },
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        padding: Some([12.0; 4]),
                        gap: Some(12.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        padding: Some([tokens.spacing.m; 4]),
                        gap: Some(tokens.spacing.m),
                        font_size: Some(16.0),
                        line_height: Some(24.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            header_style: ResolvedComponentStyle {
                gap: Some(tokens.spacing.xs),
                ..ResolvedComponentStyle::default()
            },
            content_style: ResolvedComponentStyle::default(),
            footer_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken.with_alpha(128))),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                gap: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
            title_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_primary),
                font_size: Some(16.0),
                font_weight: Some(tokens.typography.font_weight_medium),
                line_height: Some(24.0),
                ..ResolvedComponentStyle::default()
            },
            description_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(20.0),
                ..ResolvedComponentStyle::default()
            },
            separator_style: ResolvedComponentStyle {
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    pub fn resolve(&self, pattern: CardPattern, hovered: bool) -> ResolvedComponentStyle {
        self.resolve_state(pattern, hovered, false)
    }

    /// Resolves the pattern and orthogonal hover/selection presentation.
    pub fn resolve_state(
        &self,
        pattern: CardPattern,
        hovered: bool,
        selected: bool,
    ) -> ResolvedComponentStyle {
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
        let hovered = if hovered {
            base.merge(&self.hover_style)
        } else {
            base
        };
        if selected {
            hovered.merge_composing_shadows(&self.selected_style)
        } else {
            hovered
        }
    }

    /// Resolves card density, falling back to the medium recipe.
    pub fn resolve_size(&self, size: ComponentSize) -> ResolvedComponentStyle {
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
                padding: Some([self.padding; 4]),
                gap: Some(self.padding),
                ..ResolvedComponentStyle::default()
            })
    }

    /// Resolves standard title typography for a card density.
    pub fn resolve_title(&self, size: ComponentSize) -> ResolvedComponentStyle {
        self.title_style.merge(&self.resolve_size(size))
    }

    /// Resolves standard description typography for a card density.
    pub fn resolve_description(&self, size: ComponentSize) -> ResolvedComponentStyle {
        self.resolve_size(size).merge(&self.description_style)
    }
}

/// Visual recipes for an empty-content explanation and its next action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmptyStateTheme {
    /// Outer empty-state surface.
    #[serde(default)]
    pub surface_style: ResolvedComponentStyle,
    /// Surface overrides used below [`Self::narrow_breakpoint`].
    #[serde(default)]
    pub narrow_surface_style: ResolvedComponentStyle,
    /// Viewport width below which [`Self::narrow_surface_style`] is composed.
    #[serde(default = "default_empty_state_narrow_breakpoint")]
    pub narrow_breakpoint: f32,
    /// Width and spacing for icon, title, and description.
    #[serde(default)]
    pub header_style: ResolvedComponentStyle,
    /// Optional icon-media surface.
    #[serde(default)]
    pub icon_style: ResolvedComponentStyle,
    /// Primary message typography.
    #[serde(default)]
    pub title_style: ResolvedComponentStyle,
    /// Supporting message typography.
    #[serde(default)]
    pub description_style: ResolvedComponentStyle,
    /// Width and spacing for the next-action region.
    #[serde(default)]
    pub action_style: ResolvedComponentStyle,
}

impl EmptyStateTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            surface_style: ResolvedComponentStyle {
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                border_dash: Some(vec![4.0, 4.0]),
                radius: Some(tokens.radii.large),
                min_height: Some(160.0),
                padding: Some([tokens.spacing.l; 4]),
                gap: Some(tokens.spacing.m),
                ..ResolvedComponentStyle::default()
            },
            narrow_surface_style: ResolvedComponentStyle {
                min_height: Some(128.0),
                ..ResolvedComponentStyle::default()
            },
            narrow_breakpoint: default_empty_state_narrow_breakpoint(),
            header_style: ResolvedComponentStyle {
                max_width: Some(384.0),
                gap: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
            icon_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                radius: Some(tokens.radii.medium),
                width: Some(32.0),
                height: Some(32.0),
                icon_size: Some(16.0),
                margin_bottom: Some(tokens.spacing.s),
                ..ResolvedComponentStyle::default()
            },
            title_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_primary),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_medium),
                line_height: Some(
                    tokens.typography.font_size_base * tokens.typography.line_height_snug,
                ),
                ..ResolvedComponentStyle::default()
            },
            description_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_regular),
                line_height: Some(
                    tokens.typography.font_size_base * tokens.typography.line_height_relaxed,
                ),
                ..ResolvedComponentStyle::default()
            },
            action_style: ResolvedComponentStyle {
                max_width: Some(384.0),
                gap: Some(10.0),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    /// Resolves the outer surface without replacing unspecified base fields.
    pub fn resolve_surface(&self, viewport_width: f32) -> ResolvedComponentStyle {
        if viewport_width.is_finite()
            && viewport_width > 0.0
            && viewport_width < self.narrow_breakpoint
        {
            self.surface_style.merge(&self.narrow_surface_style)
        } else {
            self.surface_style.clone()
        }
    }
}

const fn default_empty_state_narrow_breakpoint() -> f32 {
    640.0
}

impl Default for EmptyStateTheme {
    fn default() -> Self {
        Self::from_tokens(&Tokens::default())
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

/// Design-system recipe for inline source-code presentation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeTheme {
    /// Surface and typography applied to inline code.
    pub style: ResolvedComponentStyle,
}

impl CodeTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                text_color: Some(tokens.colors.text_primary),
                radius: Some(tokens.radii.small),
                padding_x: Some(tokens.spacing.xs),
                padding_y: Some(tokens.spacing.xs / 2.0),
                font_size: Some(tokens.typography.font_size_xs),
                font_family: Some(tokens.typography.font_family_mono.clone()),
                ..ResolvedComponentStyle::default()
            },
        }
    }
}

impl Default for CodeTheme {
    fn default() -> Self {
        Self::from_tokens(&Tokens::default())
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
    #[serde(default)]
    pub menu: MenuTheme,
    pub calendar: CalendarTheme,
    pub pagination: PaginationTheme,
    pub timeline: TimelineTheme,
    pub segmented_control: SegmentedControlTheme,
    pub alert: AlertTheme,
    #[serde(default)]
    pub avatar: AvatarTheme,
    #[serde(default)]
    pub avatar_group: AvatarGroupTheme,
    pub badge: BadgeTheme,
    pub tabs: TabsTheme,
    pub modal: ModalTheme,
    pub tree_view: TreeViewTheme,
    pub progress: ProgressTheme,
    pub tooltip: TooltipTheme,
    pub card: CardTheme,
    #[serde(default)]
    pub code: CodeTheme,
    pub feature_icon: FeatureIconTheme,
    #[serde(default)]
    pub select: SelectTheme,
    #[serde(default)]
    pub empty_state: EmptyStateTheme,
    /// Every component recipe the active design system declares, by name.
    ///
    /// This is the generic path. A widget with no hand-written theme struct
    /// still gets full design authority by reading its recipe from here, which
    /// is why adding authority to a widget no longer requires touching this
    /// crate or the codegen at all.
    ///
    /// The named fields above predate it and stay for the components that were
    /// already migrated; new components should use this.
    #[serde(default)]
    pub recipes: BTreeMap<String, ComponentRecipe>,
}

impl ComponentTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            button: ButtonTheme::from_tokens(tokens),
            text_input: TextInputTheme::from_tokens(tokens),
            select: SelectTheme::from_tokens(tokens),
            menu: MenuTheme::from_tokens(tokens),
            calendar: CalendarTheme::from_tokens(tokens),
            pagination: PaginationTheme::from_tokens(tokens),
            timeline: TimelineTheme::from_tokens(tokens),
            segmented_control: SegmentedControlTheme::from_tokens(tokens),
            alert: AlertTheme::from_tokens(tokens),
            avatar: AvatarTheme::from_tokens(tokens),
            avatar_group: AvatarGroupTheme::from_tokens(tokens),
            badge: BadgeTheme::from_tokens(tokens),
            tabs: TabsTheme::from_tokens(tokens),
            modal: ModalTheme::from_tokens(tokens),
            tree_view: TreeViewTheme::from_tokens(tokens),
            progress: ProgressTheme::from_tokens(tokens),
            tooltip: TooltipTheme::from_tokens(tokens),
            card: CardTheme::from_tokens(tokens),
            recipes: BTreeMap::new(),
            code: CodeTheme::from_tokens(tokens),
            empty_state: EmptyStateTheme::from_tokens(tokens),
            feature_icon: FeatureIconTheme::from_tokens(tokens),
        }
    }
}

/// The top-level theme combining primitive [`Tokens`] and derived [`ComponentTheme`].
///
/// Use [`Theme::default()`] for light mode and [`Theme::dark()`] for dark mode.
/// [`Theme::from_tokens()`] preserves the generated component recipes when
/// passed the bundled default light or dark tokens. Other token sets use the
/// compatible component recipe provided by [`ComponentTheme::from_tokens()`].
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
    /// Returns the design system's recipe for `name`.
    ///
    /// Returns an empty recipe when the design system does not declare one, so
    /// a widget can read parts and fall back per field rather than branching on
    /// whether a recipe exists.
    ///
    /// A build-time check requires every supplied design system to declare the
    /// recipes widgets read, so an empty result here means either an
    /// application design system that chose not to override this component, or
    /// a widget reading a name nobody declares.
    pub fn recipe(&self, name: &str) -> ComponentRecipe {
        self.components
            .recipes
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns the recipe for `name` only when the design system declares it.
    pub fn try_recipe(&self, name: &str) -> Option<&ComponentRecipe> {
        self.components.recipes.get(name)
    }

    pub fn dark() -> Self {
        FissionDefaultDesignSystem::theme(DesignMode::Dark)
    }

    pub fn from_tokens(tokens: Tokens, mode: DesignMode) -> Self {
        let generated = FissionDefaultDesignSystem::theme_ref(mode);
        if tokens == generated.tokens {
            return generated.clone();
        }

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
/// Provides Noto Sans Regular and Inter 24pt Regular (the default).
pub mod fonts {
    pub const NOTO_SANS_REGULAR_TTF: &[u8] =
        include_bytes!("../fonts/Noto_Sans/static/NotoSans-Regular.ttf");
    pub const INTER_24PT_REGULAR_TTF: &[u8] =
        include_bytes!("../fonts/Inter/static/Inter_24pt-Regular.ttf");
    #[inline]
    pub fn default_font_bytes() -> &'static [u8] {
        INTER_24PT_REGULAR_TTF
    }
}
