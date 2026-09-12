use super::semantics::{ActionEntry, Semantics};
pub use crate::layout_policy::{
    FlyoutAlignment, FlyoutOptions, FlyoutPlacement, FlyoutWidth, LayoutDirection,
};
pub use crate::viewport::{
    ViewportBoundary, ViewportClip, ViewportMargin, ViewportPanAxis, ViewportTransform,
    ViewportZoomPolicy,
};
use crate::WidgetId;
use serde::{Deserialize, Serialize};

// The fundamental operations that can be performed in the Core IR.
// These are low-level, platform-agnostic, and deterministic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    Structural(StructuralOp),
    Layout(LayoutOp),
    Paint(PaintOp),
    Semantics(Semantics),
}

impl Op {
    /// Returns the text this node renders, if it renders any.
    ///
    /// See [`PaintOp::text`] for why callers should prefer this over matching
    /// a single text paint op.
    pub fn text(&self) -> Option<std::borrow::Cow<'_, str>> {
        match self {
            Self::Paint(paint) => paint.text(),
            _ => None,
        }
    }
}

impl std::hash::Hash for Op {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Structural(s) => {
                0.hash(state);
                s.hash(state);
            }
            Self::Layout(l) => {
                1.hash(state);
                l.hash(state);
            }
            Self::Paint(p) => {
                2.hash(state);
                p.hash(state);
            }
            Self::Semantics(s) => {
                3.hash(state);
                s.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash)]
pub enum StructuralOp {
    Group {
        stable_hash: u64,
    },
    /// Retains and paints a subtree while excluding it from pointer hit testing.
    PointerTransparent {
        stable_hash: u64,
    },
    /// Retains and paints a subtree while excluding it from all interaction
    /// and assistive semantic traversal.
    ///
    /// This is used for visual exit lifecycles: content may remain painted for
    /// a short animation after it is no longer logically present, but it must
    /// not receive pointer or keyboard input, constrain focus, or be announced.
    InteractionInert {
        stable_hash: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CompositeScalar {
    pub base: f32,
    pub motion_target: Option<WidgetId>,
}

impl CompositeScalar {
    pub fn new(base: f32) -> Self {
        Self {
            base,
            motion_target: None,
        }
    }

    pub fn motion(mut self, target: WidgetId) -> Self {
        self.motion_target = Some(target);
        self
    }
}

impl std::hash::Hash for CompositeScalar {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.base.to_bits().hash(state);
        self.motion_target.hash(state);
    }
}

/// How a composited layer's pixels combine with what is already painted beneath.
///
/// This is deliberately the CSS `mix-blend-mode` set, which is also exactly
/// Skia's separable and non-separable mix modes, plus [`Plus`](Self::Plus) for
/// additive glow. Authoring against a fixed vocabulary keeps the IR closed: a
/// new backend implements sixteen known modes rather than an open-ended filter
/// language it has to interpret.
///
/// Backends that cannot express a mode must fall back to
/// [`Normal`](Self::Normal) rather than dropping the layer. A blend mode is a
/// refinement of how content appears, never a condition for it appearing, so
/// losing it degrades fidelity and never correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum BlendMode {
    /// Source-over. The default, and the only mode every backend supports.
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
    /// Additive (Porter-Duff `plus`/`lighter`). Not part of `mix-blend-mode`,
    /// but needed for glows and light accumulation, and supported by Skia,
    /// Core Graphics and Direct2D alike.
    Plus,
}

impl BlendMode {
    /// Whether this mode composites as plain source-over.
    ///
    /// Backends use this to skip allocating a blend layer for the common case.
    pub fn is_normal(self) -> bool {
        matches!(self, Self::Normal)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash, Default)]
pub struct CompositeStyle {
    pub opacity: Option<CompositeScalar>,
    pub translate_x: Option<CompositeScalar>,
    pub translate_y: Option<CompositeScalar>,
    pub scale: Option<CompositeScalar>,
    pub rotation: Option<CompositeScalar>,
    pub clip_to_bounds: bool,
    pub repaint_boundary: bool,
    /// How this layer combines with the content already painted beneath it.
    ///
    /// A non-[`Normal`](BlendMode::Normal) mode makes the subtree a composited
    /// layer whether or not `repaint_boundary` is set, because the blend needs
    /// the subtree's pixels isolated before they are mixed down.
    #[serde(default)]
    pub blend_mode: BlendMode,
}

pub type LayoutUnit = f32;

/// A declarative layout length resolved by the constraint engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Length {
    /// Fixed logical points.
    Points(LayoutUnit),
    /// Percentage of the containing axis. `50.0` means 50%, not `0.5`.
    Percent(f32),
    /// Percentage of the active viewport width. `100.0` means full viewport width.
    ViewportWidth(f32),
    /// Percentage of the active viewport height. `100.0` means full viewport height.
    ViewportHeight(f32),
    /// Sum of two length expressions.
    Add(Box<Length>, Box<Length>),
    /// Difference between two length expressions.
    Subtract(Box<Length>, Box<Length>),
    /// Smallest value from a list of fully resolvable length expressions.
    Min(Vec<Length>),
    /// Largest value from a list of fully resolvable length expressions.
    Max(Vec<Length>),
    /// Preferred value clamped between lower and upper bounds.
    Clamp {
        /// Lower bound.
        min: Box<Length>,
        /// Preferred value before clamping.
        preferred: Box<Length>,
        /// Upper bound.
        max: Box<Length>,
    },
    /// Size to intrinsic content, optionally capped by a limit.
    FitContent(Option<Box<Length>>),
    /// Minimum intrinsic size required by the content.
    MinContent,
    /// Preferred intrinsic size of the content without wrapping.
    MaxContent,
    /// Let the active layout algorithm choose the size.
    Auto,
}

impl Length {
    /// Creates a fixed logical-point length.
    pub fn points(value: LayoutUnit) -> Self {
        Self::Points(value)
    }

    /// Creates a percentage of the containing axis.
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Creates a percentage of the viewport width.
    pub fn vw(value: f32) -> Self {
        Self::ViewportWidth(value)
    }

    /// Creates a percentage of the viewport height.
    pub fn vh(value: f32) -> Self {
        Self::ViewportHeight(value)
    }

    /// Clamps a preferred length between lower and upper bounds.
    pub fn clamp(min: Length, preferred: Length, max: Length) -> Self {
        Self::Clamp {
            min: Box::new(min),
            preferred: Box::new(preferred),
            max: Box::new(max),
        }
    }

    /// Selects the smallest fully resolved length.
    pub fn min(values: impl Into<Vec<Length>>) -> Self {
        Self::Min(values.into())
    }

    /// Selects the largest fully resolved length.
    pub fn max(values: impl Into<Vec<Length>>) -> Self {
        Self::Max(values.into())
    }

    /// Sizes to content, optionally capped by a resolved limit.
    pub fn fit_content(limit: impl Into<Option<Length>>) -> Self {
        Self::FitContent(limit.into().map(Box::new))
    }

    /// Creates `[left, right, top, bottom]` edges with one shared value.
    pub fn all(value: Length) -> [Length; 4] {
        std::array::from_fn(|_| value.clone())
    }

    /// Creates `[left, right, top, bottom]` edges from axis values.
    pub fn symmetric(horizontal: Length, vertical: Length) -> [Length; 4] {
        [horizontal.clone(), horizontal, vertical.clone(), vertical]
    }

    /// Resolves a numeric length against one axis and the active viewport.
    ///
    /// Intrinsic and automatic lengths return `None` because they require a
    /// layout measurement rather than arithmetic resolution.
    pub fn resolve(
        &self,
        reference: LayoutUnit,
        viewport_width: LayoutUnit,
        viewport_height: LayoutUnit,
    ) -> Option<LayoutUnit> {
        let resolved = match self {
            Self::Points(value) => *value,
            Self::Percent(value) => reference.is_finite().then_some(reference * value / 100.0)?,
            Self::ViewportWidth(value) => viewport_width * value / 100.0,
            Self::ViewportHeight(value) => viewport_height * value / 100.0,
            Self::Add(left, right) => {
                left.resolve(reference, viewport_width, viewport_height)?
                    + right.resolve(reference, viewport_width, viewport_height)?
            }
            Self::Subtract(left, right) => {
                left.resolve(reference, viewport_width, viewport_height)?
                    - right.resolve(reference, viewport_width, viewport_height)?
            }
            Self::Min(values) => resolve_length_list(
                values,
                reference,
                viewport_width,
                viewport_height,
                LayoutUnit::min,
            )?,
            Self::Max(values) => resolve_length_list(
                values,
                reference,
                viewport_width,
                viewport_height,
                LayoutUnit::max,
            )?,
            Self::Clamp {
                min,
                preferred,
                max,
            } => {
                let minimum = min.resolve(reference, viewport_width, viewport_height)?;
                let maximum = max.resolve(reference, viewport_width, viewport_height)?;
                preferred
                    .resolve(reference, viewport_width, viewport_height)?
                    .clamp(minimum.min(maximum), minimum.max(maximum))
            }
            Self::FitContent(_) | Self::MinContent | Self::MaxContent | Self::Auto => return None,
        };
        resolved.is_finite().then_some(resolved)
    }
}

fn resolve_length_list(
    values: &[Length],
    reference: LayoutUnit,
    viewport_width: LayoutUnit,
    viewport_height: LayoutUnit,
    combine: impl Fn(LayoutUnit, LayoutUnit) -> LayoutUnit,
) -> Option<LayoutUnit> {
    let mut values = values.iter();
    let mut resolved = values
        .next()?
        .resolve(reference, viewport_width, viewport_height)?;
    for value in values {
        resolved = combine(
            resolved,
            value.resolve(reference, viewport_width, viewport_height)?,
        );
    }
    Some(resolved)
}

impl From<LayoutUnit> for Length {
    fn from(value: LayoutUnit) -> Self {
        Self::Points(value)
    }
}

impl std::ops::Add for Length {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Add(Box::new(self), Box::new(rhs))
    }
}

impl std::ops::Sub for Length {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Subtract(Box::new(self), Box::new(rhs))
    }
}

impl std::hash::Hash for Length {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Points(value)
            | Self::Percent(value)
            | Self::ViewportWidth(value)
            | Self::ViewportHeight(value) => value.to_bits().hash(state),
            Self::Add(left, right) | Self::Subtract(left, right) => {
                left.hash(state);
                right.hash(state);
            }
            Self::Min(values) | Self::Max(values) => values.hash(state),
            Self::Clamp {
                min,
                preferred,
                max,
            } => {
                min.hash(state);
                preferred.hash(state);
                max.hash(state);
            }
            Self::FitContent(limit) => limit.hash(state),
            Self::MinContent | Self::MaxContent | Self::Auto => {}
        }
    }
}

/// Overflow behavior for a common box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Overflow {
    /// Let content paint outside the box's assigned rectangle.
    #[default]
    Visible,
    /// Clip content to the box's assigned rectangle.
    Clip,
}

/// Alignment of a box's child within its content rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum BoxAlignment {
    /// Place the child at the start of both axes.
    #[default]
    Start,
    /// Center the child on both axes.
    Center,
    /// Place the child at the end of both axes.
    End,
    /// Stretch the child to the content rectangle where the child has no explicit size.
    Stretch,
}

/// Absolute positioning values for a common box.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Hash)]
pub struct BoxPosition {
    /// Distance from the parent's left edge.
    pub left: Option<Length>,
    /// Distance from the parent's top edge.
    pub top: Option<Length>,
    /// Distance from the parent's right edge.
    pub right: Option<Length>,
    /// Distance from the parent's bottom edge.
    pub bottom: Option<Length>,
}

/// Grid placement values for a common box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct BoxGridPlacement {
    /// Starting row line or automatic placement.
    pub row_start: GridPlacement,
    /// Ending row line, span, or automatic placement.
    pub row_end: GridPlacement,
    /// Starting column line or automatic placement.
    pub col_start: GridPlacement,
    /// Ending column line, span, or automatic placement.
    pub col_end: GridPlacement,
}

/// Typed sizing and overflow shared by common box-like widgets.
///
/// `BoxStyle` lets widgets expose CSS-like layout capabilities without
/// embedding CSS or shell-specific behavior in application code.
///
/// # Example
///
/// ```rust
/// use fission_ir::op::{BoxAlignment, BoxStyle, Length, Overflow};
///
/// let style = BoxStyle::default()
///     .width(Length::clamp(
///         Length::points(280.0),
///         Length::percent(50.0),
///         Length::points(720.0),
///     ))
///     .padding_symmetric(Length::points(24.0), Length::points(16.0))
///     .align(BoxAlignment::Center)
///     .overflow(Overflow::Clip);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Hash)]
pub struct BoxStyle {
    /// Preferred width.
    pub width: Option<Length>,
    /// Preferred height.
    pub height: Option<Length>,
    /// Minimum width constraint.
    pub min_width: Option<Length>,
    /// Maximum width constraint.
    pub max_width: Option<Length>,
    /// Minimum height constraint.
    pub min_height: Option<Length>,
    /// Maximum height constraint.
    pub max_height: Option<Length>,
    /// Inner spacing in `[left, right, top, bottom]` order.
    ///
    /// Physical edges. Prefer [`padding_directional`](Self::padding_directional)
    /// for anything that should follow reading order.
    pub padding: Option<[Length; 4]>,
    /// Outer spacing in `[left, right, top, bottom]` order.
    ///
    /// Physical edges. Prefer [`margin_directional`](Self::margin_directional)
    /// for anything that should follow reading order.
    pub margin: Option<[Length; 4]>,
    /// Inner spacing in `[start, end, top, bottom]` order.
    ///
    /// The inline edges follow the layout direction: `start` is the left edge
    /// in a left-to-right layout and the right edge in a right-to-left one.
    /// This is the directional counterpart of [`padding`](Self::padding), the
    /// way Flutter pairs `EdgeInsetsDirectional` with `EdgeInsets`.
    ///
    /// When present this replaces `padding` rather than merging with it, so a
    /// box has exactly one source of inner spacing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_directional: Option<[Length; 4]>,
    /// Outer spacing in `[start, end, top, bottom]` order.
    ///
    /// The directional counterpart of [`margin`](Self::margin), with the same
    /// replacement rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_directional: Option<[Length; 4]>,
    /// Width-to-height ratio.
    pub aspect_ratio: Option<OrderedLayoutUnit>,
    /// Whether content can paint outside this box.
    pub overflow: Overflow,
    /// Child alignment inside the content rectangle.
    pub alignment: BoxAlignment,
    /// Optional absolute positioning offsets.
    pub position: Option<BoxPosition>,
    /// Optional parent-grid placement.
    pub grid: Option<BoxGridPlacement>,
    /// Flex grow participation for box-like widgets.
    pub flex_grow: Option<OrderedLayoutUnit>,
    /// Flex shrink participation for box-like widgets.
    pub flex_shrink: Option<OrderedLayoutUnit>,
}

impl BoxStyle {
    /// Sets the preferred width.
    pub fn width(mut self, value: Length) -> Self {
        self.width = Some(value);
        self
    }

    /// Sets the preferred height.
    pub fn height(mut self, value: Length) -> Self {
        self.height = Some(value);
        self
    }

    /// Sets the minimum width.
    pub fn min_width(mut self, value: Length) -> Self {
        self.min_width = Some(value);
        self
    }

    /// Sets the maximum width.
    pub fn max_width(mut self, value: Length) -> Self {
        self.max_width = Some(value);
        self
    }

    /// Sets the minimum height.
    pub fn min_height(mut self, value: Length) -> Self {
        self.min_height = Some(value);
        self
    }

    /// Sets the maximum height.
    pub fn max_height(mut self, value: Length) -> Self {
        self.max_height = Some(value);
        self
    }

    /// Sets `[left, right, top, bottom]` inner spacing.
    pub fn padding(mut self, edges: [Length; 4]) -> Self {
        self.padding = Some(edges);
        self
    }

    /// Sets equal inner spacing on every edge.
    pub fn padding_all(self, value: Length) -> Self {
        self.padding(Length::all(value))
    }

    /// Sets horizontal and vertical inner spacing.
    pub fn padding_symmetric(self, horizontal: Length, vertical: Length) -> Self {
        self.padding(Length::symmetric(horizontal, vertical))
    }

    /// Sets `[left, right, top, bottom]` outer spacing.
    pub fn margin(mut self, edges: [Length; 4]) -> Self {
        self.margin = Some(edges);
        self
    }

    /// Sets equal outer spacing on every edge.
    pub fn margin_all(self, value: Length) -> Self {
        self.margin(Length::all(value))
    }

    /// Sets horizontal and vertical outer spacing.
    pub fn margin_symmetric(self, horizontal: Length, vertical: Length) -> Self {
        self.margin(Length::symmetric(horizontal, vertical))
    }

    /// Sets overflow visibility or clipping.
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Aligns the child within the box's content rectangle.
    pub fn align(mut self, alignment: BoxAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets a non-negative width-to-height ratio.
    pub fn aspect_ratio(mut self, ratio: LayoutUnit) -> Self {
        self.aspect_ratio = Some(OrderedLayoutUnit(ratio.max(0.0)));
        self
    }

    /// Absolutely positions the box within its positioned parent.
    pub fn positioned(mut self, position: BoxPosition) -> Self {
        self.position = Some(position);
        self
    }

    /// Places the box in a parent grid.
    pub fn grid(mut self, placement: BoxGridPlacement) -> Self {
        self.grid = Some(placement);
        self
    }

    /// Sets flex grow and shrink participation.
    pub fn flex(mut self, grow: LayoutUnit, shrink: LayoutUnit) -> Self {
        self.flex_grow = Some(OrderedLayoutUnit(grow));
        self.flex_shrink = Some(OrderedLayoutUnit(shrink));
        self
    }
}

/// Hashable/serializable wrapper for floating-point layout values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrderedLayoutUnit(
    /// Wrapped finite layout value.
    pub LayoutUnit,
);

impl std::hash::Hash for OrderedLayoutUnit {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
    #[default]
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
    Fade,
    #[default]
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum TextWidthBasis {
    #[default]
    Parent,
    LongestLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum MouseCursor {
    #[default]
    Basic,
    Pointer,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TextHeightBehavior {
    pub apply_height_to_first_ascent: bool,
    pub apply_height_to_last_descent: bool,
}

impl Default for TextHeightBehavior {
    fn default() -> Self {
        Self {
            apply_height_to_first_ascent: true,
            apply_height_to_last_descent: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct TextParagraphStyle {
    pub text_align: TextAlign,
    pub max_lines: Option<usize>,
    pub overflow: TextOverflow,
    #[serde(default)]
    pub text_direction: TextDirection,
    #[serde(default)]
    pub text_width_basis: TextWidthBasis,
    #[serde(default)]
    pub strut_line_height: Option<LayoutUnit>,
    #[serde(default)]
    pub text_height_behavior: TextHeightBehavior,
}

impl PartialEq for TextParagraphStyle {
    fn eq(&self, other: &Self) -> bool {
        self.text_align == other.text_align
            && self.max_lines == other.max_lines
            && self.overflow == other.overflow
            && self.text_direction == other.text_direction
            && self.text_width_basis == other.text_width_basis
            && self.strut_line_height.map(f32::to_bits) == other.strut_line_height.map(f32::to_bits)
            && self.text_height_behavior == other.text_height_behavior
    }
}

impl Eq for TextParagraphStyle {}

impl std::hash::Hash for TextParagraphStyle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text_align.hash(state);
        self.max_lines.hash(state);
        self.overflow.hash(state);
        self.text_direction.hash(state);
        self.text_width_basis.hash(state);
        self.strut_line_height.map(f32::to_bits).hash(state);
        self.text_height_behavior.hash(state);
    }
}

const TEXT_PARAGRAPH_ALIGN_BITS: u32 = 0b111;
const TEXT_PARAGRAPH_OVERFLOW_BITS: u32 = 0b111 << 3;
const TEXT_PARAGRAPH_MAX_LINES_SHIFT: u32 = 6;
const TEXT_PARAGRAPH_SENTINEL: u32 = 1;
pub(super) const TEXT_PARAGRAPH_MAX_ENCODED_LINES: usize =
    ((1 << 24) - 1) >> TEXT_PARAGRAPH_MAX_LINES_SHIFT;

const fn text_align_code(align: TextAlign) -> u32 {
    match align {
        TextAlign::Start => 0,
        TextAlign::Left => 1,
        TextAlign::Center => 2,
        TextAlign::Right => 3,
        TextAlign::End => 4,
        TextAlign::Justify => 5,
    }
}

const fn text_overflow_code(overflow: TextOverflow) -> u32 {
    match overflow {
        TextOverflow::Visible => 0,
        TextOverflow::Clip => 1,
        TextOverflow::Ellipsis => 2,
        TextOverflow::Fade => 3,
    }
}

const fn decode_text_align(code: u32) -> TextAlign {
    match code {
        1 => TextAlign::Left,
        2 => TextAlign::Center,
        3 => TextAlign::Right,
        4 => TextAlign::End,
        5 => TextAlign::Justify,
        _ => TextAlign::Start,
    }
}

const fn decode_text_overflow(code: u32) -> TextOverflow {
    match code {
        1 => TextOverflow::Clip,
        2 => TextOverflow::Ellipsis,
        3 => TextOverflow::Fade,
        _ => TextOverflow::Visible,
    }
}

pub fn encode_text_paragraph_style(style: TextParagraphStyle) -> Option<LayoutUnit> {
    if style == TextParagraphStyle::default() {
        return None;
    }
    if style.text_direction != TextDirection::Auto
        || style.text_width_basis != TextWidthBasis::Parent
        || style.strut_line_height.is_some()
        || style.text_height_behavior != TextHeightBehavior::default()
    {
        return None;
    }

    let max_lines = style
        .max_lines
        .unwrap_or(0)
        .min(TEXT_PARAGRAPH_MAX_ENCODED_LINES) as u32;
    let encoded = TEXT_PARAGRAPH_SENTINEL
        + text_align_code(style.text_align)
        + (text_overflow_code(style.overflow) << 3)
        + (max_lines << TEXT_PARAGRAPH_MAX_LINES_SHIFT);

    Some(-(encoded as LayoutUnit))
}

pub fn decode_text_paragraph_style(
    encoded_width: Option<LayoutUnit>,
) -> Option<TextParagraphStyle> {
    let encoded_width = encoded_width?;
    if !encoded_width.is_finite() || encoded_width >= 0.0 {
        return None;
    }

    let raw = (-encoded_width).round();
    if raw < TEXT_PARAGRAPH_SENTINEL as f32 {
        return None;
    }

    let bits = raw as u32 - TEXT_PARAGRAPH_SENTINEL;
    let text_align = decode_text_align(bits & TEXT_PARAGRAPH_ALIGN_BITS);
    let overflow = decode_text_overflow((bits & TEXT_PARAGRAPH_OVERFLOW_BITS) >> 3);
    let max_lines = match bits >> TEXT_PARAGRAPH_MAX_LINES_SHIFT {
        0 => None,
        lines => Some(lines as usize),
    };

    Some(TextParagraphStyle {
        text_align,
        max_lines,
        overflow,
        text_direction: TextDirection::Auto,
        text_width_basis: TextWidthBasis::Parent,
        strut_line_height: None,
        text_height_behavior: TextHeightBehavior::default(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub enum FlexDirection {
    Row,
    Column,
}

impl Default for FlexDirection {
    fn default() -> Self {
        FlexDirection::Row
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash)]
pub enum EmbedKind {
    Video,
    Web,
    Custom(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridTrack {
    /// Fixed track size in logical points.
    Points(LayoutUnit),
    /// Percentage of the available grid axis. `50.0` means 50%.
    Percent(f32),
    /// Fraction of remaining free space after fixed and intrinsic tracks.
    Fr(f32),
    /// Track sized by the largest participating item's intrinsic size.
    Auto,
    /// Track sized by the participating items' minimum intrinsic size.
    MinContent,
    /// Track sized by the participating items' preferred intrinsic size.
    MaxContent,
    /// Track with independent minimum and maximum sizing functions.
    MinMax(Box<GridTrack>, Box<GridTrack>),
    /// Repeats an ordered track list a fixed number of times.
    Repeat { count: u16, tracks: Vec<GridTrack> },
    /// Repeats a track to fit available space, dropping empty trailing tracks.
    AutoFit(Box<GridTrack>),
    /// Repeats a track to fill available space, retaining empty tracks.
    AutoFill(Box<GridTrack>),
}

/// The width source used to evaluate a responsive layout branch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ResponsiveQuery {
    /// Compare breakpoints against the application viewport.
    #[default]
    Viewport,
    /// Compare breakpoints against the constraints supplied by the parent.
    Container,
}

/// An inclusive lower and exclusive upper width bound for a responsive branch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ResponsiveCondition {
    /// Inclusive lower width bound.
    pub min_width: Option<LayoutUnit>,
    /// Exclusive upper width bound.
    pub max_width: Option<LayoutUnit>,
}

impl ResponsiveCondition {
    pub fn matches(self, width: LayoutUnit) -> bool {
        self.min_width.is_none_or(|minimum| width >= minimum)
            && self.max_width.is_none_or(|maximum| width < maximum)
    }
}

impl std::hash::Hash for ResponsiveCondition {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.min_width.map(f32::to_bits).hash(state);
        self.max_width.map(f32::to_bits).hash(state);
    }
}

impl GridTrack {
    /// Creates a `minmax(min, max)` grid track.
    ///
    /// # Example
    ///
    /// ```rust
    /// use fission_ir::op::GridTrack;
    ///
    /// let track = GridTrack::minmax(GridTrack::Points(180.0), GridTrack::Fr(1.0));
    /// ```
    pub fn minmax(min: GridTrack, max: GridTrack) -> Self {
        Self::MinMax(Box::new(min), Box::new(max))
    }

    /// Repeats `tracks` `count` times.
    pub fn repeat(count: u16, tracks: impl Into<Vec<GridTrack>>) -> Self {
        Self::Repeat {
            count,
            tracks: tracks.into(),
        }
    }

    /// Repeats `track` up to the available space and collapses empty tracks.
    pub fn auto_fit(track: GridTrack) -> Self {
        Self::AutoFit(Box::new(track))
    }

    /// Repeats `track` up to the available space and keeps empty tracks.
    pub fn auto_fill(track: GridTrack) -> Self {
        Self::AutoFill(Box::new(track))
    }
}

impl std::hash::Hash for GridTrack {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Points(u) => {
                0.hash(state);
                u.to_bits().hash(state);
            }
            Self::Percent(f) => {
                1.hash(state);
                f.to_bits().hash(state);
            }
            Self::Fr(f) => {
                2.hash(state);
                f.to_bits().hash(state);
            }
            Self::Auto => {
                3.hash(state);
            }
            Self::MinContent => {
                4.hash(state);
            }
            Self::MaxContent => {
                5.hash(state);
            }
            Self::MinMax(min, max) => {
                6.hash(state);
                min.hash(state);
                max.hash(state);
            }
            Self::Repeat { count, tracks } => {
                7.hash(state);
                count.hash(state);
                tracks.hash(state);
            }
            Self::AutoFit(track) => {
                8.hash(state);
                track.hash(state);
            }
            Self::AutoFill(track) => {
                9.hash(state);
                track.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum GridPlacement {
    /// Let the grid auto-placement algorithm choose the line.
    Auto,
    /// A one-based grid line number. Negative values count back from the end.
    Line(i16),
    /// Span this many tracks from the resolved start line.
    Span(u16),
}

impl Default for GridPlacement {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

impl Default for FlexWrap {
    fn default() -> Self {
        FlexWrap::NoWrap
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub enum AlignItems {
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

impl Default for AlignItems {
    fn default() -> Self {
        AlignItems::Stretch
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub enum JustifyContent {
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl Default for JustifyContent {
    fn default() -> Self {
        JustifyContent::Start
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutOp {
    Box {
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
        min_width: Option<LayoutUnit>,
        max_width: Option<LayoutUnit>,
        min_height: Option<LayoutUnit>,
        max_height: Option<LayoutUnit>,
        padding: [LayoutUnit; 4],
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
        aspect_ratio: Option<f32>,
    },
    /// A common box using declarative length expressions.
    StyledBox {
        style: BoxStyle,
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
    },
    Flex {
        direction: FlexDirection,
        wrap: FlexWrap,
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
        padding: [LayoutUnit; 4],
        gap: Option<LayoutUnit>,
        /// Cross-axis spacing between wrapped lines. `gap` is used when absent.
        #[serde(default)]
        line_gap: Option<LayoutUnit>,
        align_items: AlignItems,
        justify_content: JustifyContent,
    },
    Grid {
        columns: Vec<GridTrack>,
        rows: Vec<GridTrack>,
        column_gap: Option<LayoutUnit>,
        row_gap: Option<LayoutUnit>,
        padding: [LayoutUnit; 4],
    },
    GridItem {
        row_start: GridPlacement,
        row_end: GridPlacement,
        col_start: GridPlacement,
        col_end: GridPlacement,
    },
    /// Selects one case child or the final fallback child from local constraints.
    Responsive {
        query: ResponsiveQuery,
        cases: Vec<ResponsiveCondition>,
    },
    Scroll {
        direction: FlexDirection,
        show_scrollbar: bool,
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
        min_width: Option<LayoutUnit>,
        max_width: Option<LayoutUnit>,
        min_height: Option<LayoutUnit>,
        max_height: Option<LayoutUnit>,
        padding: [LayoutUnit; 4],
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
    },
    Embed {
        kind: EmbedKind,
        widget_id: WidgetId,
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
    },
    AbsoluteFill,
    Positioned {
        left: Option<LayoutUnit>,
        top: Option<LayoutUnit>,
        right: Option<LayoutUnit>,
        bottom: Option<LayoutUnit>,
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
    },
    /// Absolutely positions a child using typed lengths resolved by layout.
    PositionedLengths {
        left: Option<Length>,
        top: Option<Length>,
        right: Option<Length>,
        bottom: Option<Length>,
        width: Option<Length>,
        height: Option<Length>,
    },
    ZStack,
    /// Legacy two-axis centered alignment retained for serialized IR compatibility.
    Align,
    /// Aligns one child independently on the horizontal and vertical axes.
    Aligned {
        horizontal: BoxAlignment,
        vertical: BoxAlignment,
    },
    Flyout {
        anchor: WidgetId,
        content: WidgetId,
        #[serde(default)]
        options: FlyoutOptions,
    },
    /// Lays out five overlay children around an external anchor.
    ///
    /// Children are ordered as top, bottom, left, right, and focus ring. The
    /// four surrounding regions leave the padded anchor rectangle uncovered.
    Spotlight {
        anchor: WidgetId,
        padding: LayoutUnit,
    },
    Transform {
        transform: [f32; 16],
    },
    InteractiveViewport {
        initial_transform: ViewportTransform,
        controlled_transform: Option<ViewportTransform>,
        pan_axis: ViewportPanAxis,
        boundary: ViewportBoundary,
        clip: ViewportClip,
        zoom_policy: ViewportZoomPolicy,
        min_scale: f32,
        max_scale: f32,
        friction: f32,
        on_interaction_start: Option<ActionEntry>,
        on_interaction_update: Option<ActionEntry>,
        on_interaction_end: Option<ActionEntry>,
    },
    Clip {
        path: Option<String>,
    },
    /// Positions an intrinsically sized child around a point in its parent.
    AnchoredPositioned {
        x: LayoutUnit,
        y: LayoutUnit,
        /// Horizontal fraction of the child's width placed before `x`.
        anchor_x: LayoutUnit,
        /// Vertical fraction of the child's height placed before `y`.
        anchor_y: LayoutUnit,
    },
}

impl std::hash::Hash for LayoutOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let hash_unit = |u: LayoutUnit, h: &mut H| u.to_bits().hash(h);
        let hash_opt_unit = |u: Option<LayoutUnit>, h: &mut H| u.map(|v| v.to_bits()).hash(h);
        let hash_units = |us: [LayoutUnit; 4], h: &mut H| {
            for u in us {
                u.to_bits().hash(h);
            }
        };

        match self {
            Self::Box {
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                flex_grow,
                flex_shrink,
                aspect_ratio,
            } => {
                0.hash(state);
                hash_opt_unit(*width, state);
                hash_opt_unit(*height, state);
                hash_opt_unit(*min_width, state);
                hash_opt_unit(*max_width, state);
                hash_opt_unit(*min_height, state);
                hash_opt_unit(*max_height, state);
                hash_units(*padding, state);
                hash_unit(*flex_grow, state);
                hash_unit(*flex_shrink, state);
                aspect_ratio.map(|f| f.to_bits()).hash(state);
            }
            Self::StyledBox {
                style,
                flex_grow,
                flex_shrink,
            } => {
                13.hash(state);
                style.hash(state);
                hash_unit(*flex_grow, state);
                hash_unit(*flex_shrink, state);
            }
            Self::Flex {
                direction,
                wrap,
                flex_grow,
                flex_shrink,
                padding,
                gap,
                line_gap,
                align_items,
                justify_content,
            } => {
                1.hash(state);
                direction.hash(state);
                wrap.hash(state);
                hash_unit(*flex_grow, state);
                hash_unit(*flex_shrink, state);
                hash_units(*padding, state);
                hash_opt_unit(*gap, state);
                hash_opt_unit(*line_gap, state);
                align_items.hash(state);
                justify_content.hash(state);
            }
            Self::Grid {
                columns,
                rows,
                column_gap,
                row_gap,
                padding,
            } => {
                2.hash(state);
                columns.hash(state);
                rows.hash(state);
                hash_opt_unit(*column_gap, state);
                hash_opt_unit(*row_gap, state);
                hash_units(*padding, state);
            }
            Self::GridItem {
                row_start,
                row_end,
                col_start,
                col_end,
            } => {
                3.hash(state);
                row_start.hash(state);
                row_end.hash(state);
                col_start.hash(state);
                col_end.hash(state);
            }
            Self::Responsive { query, cases } => {
                14.hash(state);
                query.hash(state);
                cases.hash(state);
            }
            Self::Scroll {
                direction,
                show_scrollbar,
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                flex_grow,
                flex_shrink,
            } => {
                4.hash(state);
                direction.hash(state);
                show_scrollbar.hash(state);
                hash_opt_unit(*width, state);
                hash_opt_unit(*height, state);
                hash_opt_unit(*min_width, state);
                hash_opt_unit(*max_width, state);
                hash_opt_unit(*min_height, state);
                hash_opt_unit(*max_height, state);
                hash_units(*padding, state);
                hash_unit(*flex_grow, state);
                hash_unit(*flex_shrink, state);
            }
            Self::Embed {
                kind,
                widget_id,
                width,
                height,
            } => {
                5.hash(state);
                kind.hash(state);
                widget_id.hash(state);
                hash_opt_unit(*width, state);
                hash_opt_unit(*height, state);
            }
            Self::AbsoluteFill => {
                6.hash(state);
            }
            Self::Positioned {
                left,
                top,
                right,
                bottom,
                width,
                height,
            } => {
                7.hash(state);
                hash_opt_unit(*left, state);
                hash_opt_unit(*top, state);
                hash_opt_unit(*right, state);
                hash_opt_unit(*bottom, state);
                hash_opt_unit(*width, state);
                hash_opt_unit(*height, state);
            }
            Self::PositionedLengths {
                left,
                top,
                right,
                bottom,
                width,
                height,
            } => {
                15.hash(state);
                left.hash(state);
                top.hash(state);
                right.hash(state);
                bottom.hash(state);
                width.hash(state);
                height.hash(state);
            }
            Self::AnchoredPositioned {
                x,
                y,
                anchor_x,
                anchor_y,
            } => {
                18.hash(state);
                hash_unit(*x, state);
                hash_unit(*y, state);
                hash_unit(*anchor_x, state);
                hash_unit(*anchor_y, state);
            }
            Self::ZStack => {
                8.hash(state);
            }
            Self::Align => {
                9.hash(state);
            }
            Self::Aligned {
                horizontal,
                vertical,
            } => {
                19.hash(state);
                horizontal.hash(state);
                vertical.hash(state);
            }
            Self::Flyout {
                anchor,
                content,
                options,
            } => {
                10.hash(state);
                anchor.hash(state);
                content.hash(state);
                options.alignment.hash(state);
                options.placement.hash(state);
                options.width.hash(state);
                hash_unit(options.gap, state);
                options.alignment_target.hash(state);
            }
            Self::Transform { transform } => {
                11.hash(state);
                for v in transform {
                    v.to_bits().hash(state);
                }
            }
            Self::InteractiveViewport {
                initial_transform,
                controlled_transform,
                pan_axis,
                boundary,
                clip,
                zoom_policy,
                min_scale,
                max_scale,
                friction,
                on_interaction_start,
                on_interaction_update,
                on_interaction_end,
            } => {
                17.hash(state);
                initial_transform.hash(state);
                controlled_transform.hash(state);
                pan_axis.hash(state);
                boundary.hash(state);
                clip.hash(state);
                zoom_policy.hash(state);
                min_scale.to_bits().hash(state);
                max_scale.to_bits().hash(state);
                friction.to_bits().hash(state);
                on_interaction_start.hash(state);
                on_interaction_update.hash(state);
                on_interaction_end.hash(state);
            }
            Self::Clip { path } => {
                12.hash(state);
                path.hash(state);
            }
            Self::Spotlight { anchor, padding } => {
                16.hash(state);
                anchor.hash(state);
                hash_unit(*padding, state);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const RED: Self = Self {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const GREEN: Self = Self {
        r: 0,
        g: 255,
        b: 0,
        a: 255,
    };
    pub const BLUE: Self = Self {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };

    pub fn with_alpha(mut self, a: u8) -> Self {
        self.a = a;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    Solid(Color),
    /// A gradient whose start and end points are normalized to the painted
    /// bounds, where `(0.0, 0.0)` is the top-left and `(1.0, 1.0)` is the
    /// bottom-right.
    LinearGradient {
        start: (f32, f32),
        end: (f32, f32),
        stops: Vec<(f32, Color)>,
        /// How colour continues outside the stop range.
        #[serde(default)]
        extend: GradientExtend,
    },
    /// A gradient whose center and radius are normalized to the painted bounds.
    RadialGradient {
        center: (f32, f32),
        radius: f32,
        stops: Vec<(f32, Color)>,
        /// How colour continues outside the stop range.
        #[serde(default)]
        extend: GradientExtend,
    },
    /// A gradient sweeping around a center point, normalized to the painted
    /// bounds.
    ///
    /// Angles are in radians, measured clockwise from the positive x axis. This
    /// is CSS's `conic-gradient` and Skia's sweep gradient; it is what a
    /// circular progress track, a colour wheel and a rotating shimmer all need,
    /// and none of them can be built from linear and radial alone.
    SweepGradient {
        center: (f32, f32),
        start_angle: f32,
        end_angle: f32,
        stops: Vec<(f32, Color)>,
        /// How colour continues outside the swept range.
        #[serde(default)]
        extend: GradientExtend,
    },
}

/// How a gradient continues beyond its first and last stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum GradientExtend {
    /// Holds the end colours. The default, and CSS's behaviour.
    #[default]
    Pad,
    /// Tiles the stop range, as CSS `repeating-*-gradient` does.
    Repeat,
    /// Tiles the stop range, mirroring every other repetition.
    Reflect,
}

impl Fill {
    /// How this fill continues beyond its stops.
    ///
    /// A solid fill has no stops to extend, and reports [`GradientExtend::Pad`]
    /// so callers need no special case for it.
    pub fn extend(&self) -> GradientExtend {
        match self {
            Self::Solid(_) => GradientExtend::Pad,
            Self::LinearGradient { extend, .. }
            | Self::RadialGradient { extend, .. }
            | Self::SweepGradient { extend, .. } => *extend,
        }
    }
}

impl std::hash::Hash for Fill {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Solid(c) => {
                0.hash(state);
                c.hash(state);
            }
            Self::LinearGradient {
                start,
                end,
                stops,
                extend,
            } => {
                1.hash(state);
                start.0.to_bits().hash(state);
                start.1.to_bits().hash(state);
                end.0.to_bits().hash(state);
                end.1.to_bits().hash(state);
                for (off, c) in stops {
                    off.to_bits().hash(state);
                    c.hash(state);
                }
                extend.hash(state);
            }
            Self::RadialGradient {
                center,
                radius,
                stops,
                extend,
            } => {
                2.hash(state);
                center.0.to_bits().hash(state);
                center.1.to_bits().hash(state);
                radius.to_bits().hash(state);
                extend.hash(state);
                for (off, c) in stops {
                    off.to_bits().hash(state);
                    c.hash(state);
                }
            }
            Self::SweepGradient {
                center,
                start_angle,
                end_angle,
                stops,
                extend,
            } => {
                3.hash(state);
                center.0.to_bits().hash(state);
                center.1.to_bits().hash(state);
                start_angle.to_bits().hash(state);
                end_angle.to_bits().hash(state);
                extend.hash(state);
                for (off, c) in stops {
                    off.to_bits().hash(state);
                    c.hash(state);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub fill: Fill,
    pub width: LayoutUnit,
    pub dash_array: Option<Vec<f32>>,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
}

impl std::hash::Hash for Stroke {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fill.hash(state);
        self.width.to_bits().hash(state);
        if let Some(da) = &self.dash_array {
            1.hash(state);
            for d in da {
                d.to_bits().hash(state);
            }
        } else {
            0.hash(state);
        }
        self.line_cap.hash(state);
        self.line_join.hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxShadow {
    pub color: Color,
    pub blur_radius: LayoutUnit,
    /// Positive values expand the shadow shape; negative values contract it.
    pub spread_radius: LayoutUnit,
    pub offset: (LayoutUnit, LayoutUnit),
    /// Draws the shadow inside the shape instead of behind it.
    pub inset: bool,
}

impl std::hash::Hash for BoxShadow {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.color.hash(state);
        self.blur_radius.to_bits().hash(state);
        self.spread_radius.to_bits().hash(state);
        self.offset.0.to_bits().hash(state);
        self.offset.1.to_bits().hash(state);
        self.inset.hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub enum ImageFit {
    Contain,
    Cover,
    Fill,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum ImageAlignment {
    TopStart,
    TopCenter,
    TopEnd,
    CenterStart,
    #[default]
    Center,
    CenterEnd,
    BottomStart,
    BottomCenter,
    BottomEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum ImageCachePolicy {
    #[default]
    Default,
    Reload,
    MemoryOnly,
    Disk,
    NoStore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ImageSource {
    Asset {
        path: String,
    },
    File {
        path: String,
    },
    Network {
        url: String,
        #[serde(default)]
        headers: Vec<HttpHeader>,
        #[serde(default)]
        cache_policy: ImageCachePolicy,
    },
    Memory {
        bytes: Vec<u8>,
        #[serde(default)]
        mime_type: Option<String>,
    },
    SvgText {
        content: String,
    },
}

impl Default for ImageSource {
    fn default() -> Self {
        Self::Asset {
            path: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum ImageLoadingBehavior {
    #[default]
    Empty,
    ThemePlaceholder,
    BlurHash(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum ImageErrorBehavior {
    #[default]
    Empty,
    ThemeError,
    AltText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub struct ImageRequest {
    pub source: ImageSource,
    #[serde(default)]
    pub cache_width: Option<u32>,
    #[serde(default)]
    pub cache_height: Option<u32>,
    #[serde(default)]
    pub semantic_label: Option<String>,
    #[serde(default)]
    pub loading: ImageLoadingBehavior,
    #[serde(default)]
    pub error: ImageErrorBehavior,
}

impl ImageSource {
    pub fn stable_identity(&self) -> String {
        match self {
            Self::Asset { path } => format!("asset:{path}"),
            Self::File { path } => format!("file:{path}"),
            Self::Network {
                url,
                headers,
                cache_policy,
            } => {
                let mut identity = format!("network:{cache_policy:?}:{url}");
                for header in headers {
                    identity.push('|');
                    identity.push_str(&header.name.to_ascii_lowercase());
                    identity.push('=');
                    identity.push_str(&header.value);
                }
                identity
            }
            Self::Memory { bytes, mime_type } => {
                let digest = blake3::hash(bytes);
                format!("memory:{}:{digest}", mime_type.as_deref().unwrap_or(""))
            }
            Self::SvgText { content } => {
                let digest = blake3::hash(content.as_bytes());
                format!("svg:{digest}")
            }
        }
    }

    pub fn local_path(&self) -> Option<&str> {
        match self {
            Self::Asset { path } | Self::File { path } => Some(path),
            _ => None,
        }
    }

    pub fn network_url(&self) -> Option<&str> {
        match self {
            Self::Network { url, .. } => Some(url),
            _ => None,
        }
    }
}

impl ImageRequest {
    pub fn stable_cache_key(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.source.stable_identity().as_bytes());
        hasher.update(&self.cache_width.unwrap_or_default().to_le_bytes());
        hasher.update(&self.cache_height.unwrap_or_default().to_le_bytes());
        hasher.finalize().to_hex().to_string()
    }
}

pub use crate::text_style::{
    FontFeature, FontStyle, FontVariation, TextBaseline, TextDecoration, TextDecorationLines,
    TextDecorationStyle, TextHyphenation, TextLeadingDistribution, TextLineBreakPolicy, TextShadow,
    TextStyle, TextTypography,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash)]
pub struct TextRun {
    pub text: String,
    pub style: TextStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct RichTextAnnotation {
    pub range: std::ops::Range<usize>,
    #[serde(default)]
    pub semantics_label: Option<String>,
    #[serde(default)]
    pub semantics_identifier: Option<String>,
    #[serde(default)]
    pub spell_out: Option<bool>,
    #[serde(default)]
    pub mouse_cursor: Option<MouseCursor>,
    #[serde(default)]
    pub actions: Vec<ActionEntry>,
}

pub const INLINE_WIDGET_MARKER_PREFIX: &str = "__fission_inline_widget__:";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InlineWidgetMarker {
    pub id: u64,
    pub width: LayoutUnit,
    pub height: LayoutUnit,
}

pub fn encode_inline_widget_marker(id: u64, width: LayoutUnit, height: LayoutUnit) -> String {
    format!("{INLINE_WIDGET_MARKER_PREFIX}{id}:{width}:{height}")
}

pub fn decode_inline_widget_marker(family: Option<&str>) -> Option<InlineWidgetMarker> {
    let family = family?;
    let encoded = family.strip_prefix(INLINE_WIDGET_MARKER_PREFIX)?;
    let mut parts = encoded.split(':');
    let id = parts.next()?.parse().ok()?;
    let width = parts.next()?.parse().ok()?;
    let height = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(InlineWidgetMarker { id, width, height })
}

const fn text_wrap_default() -> bool {
    true
}

/// Per-corner radii in `[top_left, top_right, bottom_right, bottom_left]` order.
///
/// The scalar `corner_radius` on a paint op covers the uniform case, which is
/// most of them. This covers the shapes it cannot express at all: a sheet or
/// drawer rounded only along its leading edge, a tab, the first and last rows
/// of a grouped list, the end caps of a segmented control, a bubble with one
/// square corner.
///
/// Read it through [`PaintOp::corner_radii`] rather than directly, so a backend
/// handles the uniform and per-corner cases through one path.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CornerRadii {
    pub top_left: LayoutUnit,
    pub top_right: LayoutUnit,
    pub bottom_right: LayoutUnit,
    pub bottom_left: LayoutUnit,
}

impl CornerRadii {
    /// The same radius on every corner.
    pub const fn uniform(radius: LayoutUnit) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// Rounds the two top corners and leaves the bottom square.
    pub const fn top(radius: LayoutUnit) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: 0.0,
            bottom_left: 0.0,
        }
    }

    /// Rounds the two bottom corners and leaves the top square.
    pub const fn bottom(radius: LayoutUnit) -> Self {
        Self {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// Rounds the two left corners and leaves the right square.
    pub const fn left(radius: LayoutUnit) -> Self {
        Self {
            top_left: radius,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: radius,
        }
    }

    /// Rounds the two right corners and leaves the left square.
    pub const fn right(radius: LayoutUnit) -> Self {
        Self {
            top_left: 0.0,
            top_right: radius,
            bottom_right: radius,
            bottom_left: 0.0,
        }
    }

    /// Mirrors the left and right corners, for a right-to-left layout.
    pub const fn mirrored(self) -> Self {
        Self {
            top_left: self.top_right,
            top_right: self.top_left,
            bottom_right: self.bottom_left,
            bottom_left: self.bottom_right,
        }
    }

    /// Whether every corner shares one radius.
    ///
    /// Backends that can only draw a uniform rounded rectangle use this to tell
    /// the cases apart rather than guessing from the values.
    pub fn is_uniform(self) -> bool {
        self.top_left == self.top_right
            && self.top_right == self.bottom_right
            && self.bottom_right == self.bottom_left
    }

    /// Whether every corner is square.
    pub fn is_square(self) -> bool {
        self.top_left == 0.0
            && self.top_right == 0.0
            && self.bottom_right == 0.0
            && self.bottom_left == 0.0
    }

    /// The corners in `[top_left, top_right, bottom_right, bottom_left]` order.
    pub fn to_array(self) -> [LayoutUnit; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

impl From<LayoutUnit> for CornerRadii {
    fn from(radius: LayoutUnit) -> Self {
        Self::uniform(radius)
    }
}

impl std::hash::Hash for CornerRadii {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for corner in self.to_array() {
            corner.to_bits().hash(state);
        }
    }
}

/// Strokes for the four edges of a box, in physical order.
///
/// The uniform `stroke` on a paint op cannot express an edge on its own, which
/// rules out a filled text field's underline, a table's cell grid, a card with
/// a coloured leading accent, and a tab indicator drawn as a bottom edge.
///
/// Sides are physical rather than logical because they are resolved at paint
/// time, after layout has already resolved reading order. Widgets that want
/// leading and trailing edges mirror them while lowering, the same way
/// directional padding is resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Hash)]
pub struct BorderSides {
    pub top: Option<Stroke>,
    pub right: Option<Stroke>,
    pub bottom: Option<Stroke>,
    pub left: Option<Stroke>,
}

impl BorderSides {
    /// The same stroke on every edge.
    pub fn uniform(stroke: Stroke) -> Self {
        Self {
            top: Some(stroke.clone()),
            right: Some(stroke.clone()),
            bottom: Some(stroke.clone()),
            left: Some(stroke),
        }
    }

    /// A stroke on the bottom edge only, as a filled text field uses.
    pub fn bottom_only(stroke: Stroke) -> Self {
        Self {
            bottom: Some(stroke),
            ..Default::default()
        }
    }

    /// Mirrors the left and right edges, for a right-to-left layout.
    pub fn mirrored(self) -> Self {
        Self {
            top: self.top,
            right: self.left,
            bottom: self.bottom,
            left: self.right,
        }
    }

    /// Whether no edge carries a stroke.
    pub fn is_empty(&self) -> bool {
        self.top.is_none() && self.right.is_none() && self.bottom.is_none() && self.left.is_none()
    }

    /// The single stroke shared by all four edges, if there is one.
    ///
    /// Lets a backend fall back to its uniform stroke path when the sides
    /// happen to agree, rather than always drawing four separate edges.
    pub fn as_uniform(&self) -> Option<&Stroke> {
        let top = self.top.as_ref()?;
        (self.right.as_ref() == Some(top)
            && self.bottom.as_ref() == Some(top)
            && self.left.as_ref() == Some(top))
        .then_some(top)
    }
}

/// A filter applied to content already painted behind a widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BackdropFilter {
    /// Applies a Gaussian blur using the supplied standard deviation.
    Blur(LayoutUnit),
    /// Scales backdrop saturation. `1.0` leaves it unchanged, `0.0` is greyscale,
    /// values above `1.0` oversaturate.
    ///
    /// Frosted-glass surfaces need this as much as they need blur: blurring
    /// alone washes a backdrop out, and real glass materials on every platform
    /// pair a blur with a saturation boost.
    Saturate(f32),
    /// Scales backdrop luminance. `1.0` leaves it unchanged.
    Brightness(f32),
    /// Applies the listed filters in order.
    ///
    /// The common case — blur plus saturation — is a chain, so the IR has to be
    /// able to express one rather than forcing a widget to stack nested
    /// backdrop layers and pay for a full backdrop read per filter.
    Chain(Vec<BackdropFilter>),
}

impl BackdropFilter {
    /// Appends this filter's leaves to `out` in application order.
    ///
    /// Backends match on leaves rather than on [`Chain`](Self::Chain), so adding
    /// a filter to the vocabulary does not force every backend to re-handle
    /// nesting.
    pub fn flatten_into<'a>(&'a self, out: &mut Vec<&'a BackdropFilter>) {
        match self {
            Self::Chain(filters) => {
                for filter in filters {
                    filter.flatten_into(out);
                }
            }
            leaf => out.push(leaf),
        }
    }

    /// This filter's leaves in application order.
    pub fn flatten(&self) -> Vec<&BackdropFilter> {
        let mut out = Vec::new();
        self.flatten_into(&mut out);
        out
    }

    /// The total Gaussian blur this filter applies.
    ///
    /// Backends that can blur the backdrop but cannot adjust its colour use
    /// this to render the part they support instead of dropping the surface.
    pub fn blur_sigma(&self) -> LayoutUnit {
        self.flatten()
            .into_iter()
            .filter_map(|filter| match filter {
                Self::Blur(sigma) => Some(*sigma),
                _ => None,
            })
            .fold(0.0, f32::max)
    }
}

impl std::hash::Hash for BackdropFilter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Blur(sigma) => {
                0_u8.hash(state);
                sigma.to_bits().hash(state);
            }
            Self::Saturate(amount) => {
                1_u8.hash(state);
                amount.to_bits().hash(state);
            }
            Self::Brightness(amount) => {
                2_u8.hash(state);
                amount.to_bits().hash(state);
            }
            Self::Chain(filters) => {
                3_u8.hash(state);
                filters.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaintOp {
    BackdropFilter {
        filter: BackdropFilter,
        corner_radius: LayoutUnit,
        /// Per-corner override. Replaces `corner_radius` when present.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        corner_radii: Option<CornerRadii>,
    },
    DrawRect {
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        corner_radius: LayoutUnit,
        shadow: Option<BoxShadow>,
        /// Per-corner override. Replaces `corner_radius` when present.
        ///
        /// Read through [`PaintOp::corner_radii`] so the uniform and per-corner
        /// cases reach a backend by one path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        corner_radii: Option<CornerRadii>,
        /// Per-edge strokes. Replaces `stroke` when present.
        ///
        /// Read through [`PaintOp::border_sides`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        border_sides: Option<BorderSides>,
    },
    DrawText {
        text: String,
        size: LayoutUnit,
        color: Color,
        underline: bool,
        /// BCP 47 language tag used for shaping and semantic HTML lowering.
        #[serde(default)]
        locale: Option<String>,
        #[serde(default = "text_wrap_default")]
        wrap: bool,
        caret_index: Option<usize>,
        #[serde(default)]
        caret_color: Option<Color>,
        #[serde(default)]
        caret_width: Option<LayoutUnit>,
        #[serde(default)]
        caret_height: Option<LayoutUnit>,
        #[serde(default)]
        caret_radius: Option<LayoutUnit>,
        #[serde(default)]
        paragraph_style: Option<TextParagraphStyle>,
    },
    DrawRichText {
        runs: Vec<TextRun>,
        #[serde(default = "text_wrap_default")]
        wrap: bool,
        caret_index: Option<usize>,
        #[serde(default)]
        caret_color: Option<Color>,
        #[serde(default)]
        caret_width: Option<LayoutUnit>,
        #[serde(default)]
        caret_height: Option<LayoutUnit>,
        #[serde(default)]
        caret_radius: Option<LayoutUnit>,
        #[serde(default)]
        paragraph_style: Option<TextParagraphStyle>,
    },
    DrawImage {
        request: ImageRequest,
        fit: ImageFit,
        alignment: ImageAlignment,
    },
    DrawPath {
        path: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
    DrawSvg {
        content: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
}

impl PaintOp {
    /// The corner radii this op paints with, uniform or per-corner.
    ///
    /// Backends must go through this rather than reading `corner_radius`, or a
    /// per-corner shape silently renders as a uniform one.
    pub fn corner_radii(&self) -> Option<CornerRadii> {
        match self {
            Self::DrawRect {
                corner_radius,
                corner_radii,
                ..
            }
            | Self::BackdropFilter {
                corner_radius,
                corner_radii,
                ..
            } => Some(corner_radii.unwrap_or(CornerRadii::uniform(*corner_radius))),
            _ => None,
        }
    }

    /// The per-edge strokes this op paints with.
    ///
    /// Returns the uniform `stroke` expanded to four edges when no per-edge
    /// override is set, so a backend has one path for both.
    pub fn border_sides(&self) -> Option<BorderSides> {
        match self {
            Self::DrawRect {
                stroke,
                border_sides,
                ..
            } => match border_sides {
                Some(sides) => Some(sides.clone()),
                None => stroke.clone().map(BorderSides::uniform),
            },
            _ => None,
        }
    }

    /// Returns the text this op renders, if it renders any.
    ///
    /// Fission has two text paint ops: [`DrawText`](Self::DrawText) carries one
    /// unstyled string, [`DrawRichText`](Self::DrawRichText) carries styled
    /// runs. Which one a widget emits is an implementation detail of that
    /// widget, so anything asking "what does the user read here" — semantic
    /// tooling, devtools, search indexing, tests — should go through this
    /// rather than matching a single variant and silently missing the other.
    ///
    /// Rich text is joined across runs without separators, because runs split
    /// on styling rather than on word boundaries.
    pub fn text(&self) -> Option<std::borrow::Cow<'_, str>> {
        match self {
            Self::DrawText { text, .. } => Some(std::borrow::Cow::Borrowed(text.as_str())),
            Self::DrawRichText { runs, .. } => match runs.as_slice() {
                [] => None,
                [run] => Some(std::borrow::Cow::Borrowed(run.text.as_str())),
                runs => Some(std::borrow::Cow::Owned(
                    runs.iter().map(|run| run.text.as_str()).collect(),
                )),
            },
            _ => None,
        }
    }

    /// Returns the paragraph controls this text op carries, if it is a text op.
    ///
    /// Alignment, line limits, overflow, direction, width basis, strut and
    /// height behaviour are identical on both text paint ops, so callers should
    /// not have to know which one a widget emitted.
    pub fn paragraph_style(&self) -> Option<&TextParagraphStyle> {
        match self {
            Self::DrawText {
                paragraph_style, ..
            }
            | Self::DrawRichText {
                paragraph_style, ..
            } => paragraph_style.as_ref(),
            _ => None,
        }
    }
}

impl std::hash::Hash for PaintOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::BackdropFilter {
                filter,
                corner_radius,
                corner_radii,
            } => {
                0_u8.hash(state);
                filter.hash(state);
                corner_radius.to_bits().hash(state);
                corner_radii.hash(state);
            }
            Self::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
                corner_radii,
                border_sides,
            } => {
                1_u8.hash(state);
                fill.hash(state);
                stroke.hash(state);
                corner_radius.to_bits().hash(state);
                shadow.hash(state);
                corner_radii.hash(state);
                border_sides.hash(state);
            }
            Self::DrawText {
                text,
                size,
                color,
                underline,
                locale,
                wrap,
                caret_index,
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
                paragraph_style,
            } => {
                2_u8.hash(state);
                text.hash(state);
                size.to_bits().hash(state);
                color.hash(state);
                underline.hash(state);
                locale.hash(state);
                wrap.hash(state);
                caret_index.hash(state);
                caret_color.hash(state);
                caret_width.map(|w| w.to_bits()).hash(state);
                caret_height.map(|h| h.to_bits()).hash(state);
                caret_radius.map(|r| r.to_bits()).hash(state);
                paragraph_style.hash(state);
            }
            Self::DrawRichText {
                runs,
                wrap,
                caret_index,
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
                paragraph_style,
            } => {
                3_u8.hash(state);
                runs.hash(state);
                wrap.hash(state);
                caret_index.hash(state);
                caret_color.hash(state);
                caret_width.map(|w| w.to_bits()).hash(state);
                caret_height.map(|h| h.to_bits()).hash(state);
                caret_radius.map(|r| r.to_bits()).hash(state);
                paragraph_style.hash(state);
            }
            Self::DrawImage {
                request,
                fit,
                alignment,
            } => {
                4_u8.hash(state);
                request.hash(state);
                fit.hash(state);
                alignment.hash(state);
            }
            Self::DrawPath { path, fill, stroke } => {
                5_u8.hash(state);
                path.hash(state);
                fill.hash(state);
                stroke.hash(state);
            }
            Self::DrawSvg {
                content,
                fill,
                stroke,
            } => {
                6_u8.hash(state);
                content.hash(state);
                fill.hash(state);
                stroke.hash(state);
            }
        }
    }
}
