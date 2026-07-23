use super::semantics::{ActionEntry, Semantics};
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
    Group { stable_hash: u64 },
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash, Default)]
pub struct CompositeStyle {
    pub opacity: Option<CompositeScalar>,
    pub translate_x: Option<CompositeScalar>,
    pub translate_y: Option<CompositeScalar>,
    pub scale: Option<CompositeScalar>,
    pub rotation: Option<CompositeScalar>,
    pub clip_to_bounds: bool,
    pub repaint_boundary: bool,
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
    pub padding: Option<[Length; 4]>,
    /// Outer spacing in `[left, right, top, bottom]` order.
    pub margin: Option<[Length; 4]>,
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
const TEXT_PARAGRAPH_MAX_ENCODED_LINES: usize = ((1 << 24) - 1) >> TEXT_PARAGRAPH_MAX_LINES_SHIFT;

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
    Align,
    Flyout {
        anchor: WidgetId,
        content: WidgetId,
    },
    Transform {
        transform: [f32; 16],
    },
    Clip {
        path: Option<String>,
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
            Self::ZStack => {
                8.hash(state);
            }
            Self::Align => {
                9.hash(state);
            }
            Self::Flyout { anchor, content } => {
                10.hash(state);
                anchor.hash(state);
                content.hash(state);
            }
            Self::Transform { transform } => {
                11.hash(state);
                for v in transform {
                    v.to_bits().hash(state);
                }
            }
            Self::Clip { path } => {
                12.hash(state);
                path.hash(state);
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
    LinearGradient {
        start: (f32, f32),
        end: (f32, f32),
        stops: Vec<(f32, Color)>,
    },
    RadialGradient {
        center: (f32, f32),
        radius: f32,
        stops: Vec<(f32, Color)>,
    },
}

impl std::hash::Hash for Fill {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Solid(c) => {
                0.hash(state);
                c.hash(state);
            }
            Self::LinearGradient { start, end, stops } => {
                1.hash(state);
                start.0.to_bits().hash(state);
                start.1.to_bits().hash(state);
                end.0.to_bits().hash(state);
                end.1.to_bits().hash(state);
                for (off, c) in stops {
                    off.to_bits().hash(state);
                    c.hash(state);
                }
            }
            Self::RadialGradient {
                center,
                radius,
                stops,
            } => {
                2.hash(state);
                center.0.to_bits().hash(state);
                center.1.to_bits().hash(state);
                radius.to_bits().hash(state);
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_size: LayoutUnit,
    pub color: Color,
    pub underline: bool,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default = "text_weight_default")]
    pub font_weight: u16,
    #[serde(default)]
    pub font_style: FontStyle,
    #[serde(default)]
    pub line_height: Option<LayoutUnit>,
    #[serde(default)]
    pub letter_spacing: LayoutUnit,
    /// Optional background highlight color for this run (find matches, error squiggles, etc.).
    pub background_color: Option<Color>,
}

impl std::hash::Hash for TextStyle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.font_size.to_bits().hash(state);
        self.color.hash(state);
        self.underline.hash(state);
        self.font_family.hash(state);
        self.locale.hash(state);
        self.font_weight.hash(state);
        self.font_style.hash(state);
        self.line_height.map(f32::to_bits).hash(state);
        self.letter_spacing.to_bits().hash(state);
        self.background_color.hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

const fn text_weight_default() -> u16 {
    400
}

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

/// A filter applied to content already painted behind a widget.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BackdropFilter {
    /// Applies a Gaussian blur using the supplied standard deviation.
    Blur(LayoutUnit),
}

impl std::hash::Hash for BackdropFilter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Blur(sigma) => {
                0_u8.hash(state);
                sigma.to_bits().hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaintOp {
    BackdropFilter {
        filter: BackdropFilter,
        corner_radius: LayoutUnit,
    },
    DrawRect {
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        corner_radius: LayoutUnit,
        shadow: Option<BoxShadow>,
    },
    DrawText {
        text: String,
        size: LayoutUnit,
        color: Color,
        underline: bool,
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

impl std::hash::Hash for PaintOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::BackdropFilter {
                filter,
                corner_radius,
            } => {
                0_u8.hash(state);
                filter.hash(state);
                corner_radius.to_bits().hash(state);
            }
            Self::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
            } => {
                1_u8.hash(state);
                fill.hash(state);
                stroke.hash(state);
                corner_radius.to_bits().hash(state);
                shadow.hash(state);
            }
            Self::DrawText {
                text,
                size,
                color,
                underline,
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

#[cfg(test)]
mod tests {
    use super::{
        decode_inline_widget_marker, decode_text_paragraph_style, encode_inline_widget_marker,
        encode_text_paragraph_style, HttpHeader, ImageCachePolicy, ImageRequest, ImageSource,
        InlineWidgetMarker, TextAlign, TextDirection, TextHeightBehavior, TextOverflow,
        TextParagraphStyle, TextWidthBasis, TEXT_PARAGRAPH_MAX_ENCODED_LINES,
    };

    #[test]
    fn paragraph_style_round_trips_alignment_overflow_and_line_cap() {
        let style = TextParagraphStyle {
            text_align: TextAlign::Justify,
            max_lines: Some(3),
            overflow: TextOverflow::Fade,
            text_direction: TextDirection::Auto,
            text_width_basis: TextWidthBasis::Parent,
            strut_line_height: None,
            text_height_behavior: TextHeightBehavior::default(),
        };

        let encoded = encode_text_paragraph_style(style);
        assert_eq!(decode_text_paragraph_style(encoded), Some(style));
    }

    #[test]
    fn paragraph_style_clamps_line_count_to_precise_encoding_budget() {
        let encoded = encode_text_paragraph_style(TextParagraphStyle {
            text_align: TextAlign::End,
            max_lines: Some(TEXT_PARAGRAPH_MAX_ENCODED_LINES + 99),
            overflow: TextOverflow::Ellipsis,
            text_direction: TextDirection::Auto,
            text_width_basis: TextWidthBasis::Parent,
            strut_line_height: None,
            text_height_behavior: TextHeightBehavior::default(),
        });

        assert_eq!(
            decode_text_paragraph_style(encoded),
            Some(TextParagraphStyle {
                text_align: TextAlign::End,
                max_lines: Some(TEXT_PARAGRAPH_MAX_ENCODED_LINES),
                overflow: TextOverflow::Ellipsis,
                text_direction: TextDirection::Auto,
                text_width_basis: TextWidthBasis::Parent,
                strut_line_height: None,
                text_height_behavior: TextHeightBehavior::default(),
            })
        );
    }

    #[test]
    fn image_request_cache_key_is_stable_and_dimension_sensitive() {
        let request = ImageRequest {
            source: ImageSource::Network {
                url: "https://cdn.example.com/image.webp".into(),
                headers: vec![HttpHeader {
                    name: "Accept".into(),
                    value: "image/webp".into(),
                }],
                cache_policy: ImageCachePolicy::Default,
            },
            cache_width: Some(320),
            cache_height: Some(180),
            ..Default::default()
        };

        let same = request.clone();
        let mut resized = request.clone();
        resized.cache_width = Some(640);

        assert_eq!(request.stable_cache_key(), same.stable_cache_key());
        assert_ne!(request.stable_cache_key(), resized.stable_cache_key());
    }

    #[test]
    fn image_source_helpers_report_path_and_network_sources() {
        assert_eq!(
            ImageSource::Asset {
                path: "assets/logo.png".into()
            }
            .local_path(),
            Some("assets/logo.png")
        );
        assert_eq!(
            ImageSource::Network {
                url: "https://example.com/logo.png".into(),
                headers: Vec::new(),
                cache_policy: ImageCachePolicy::Default,
            }
            .network_url(),
            Some("https://example.com/logo.png")
        );
    }

    #[test]
    fn paragraph_style_compact_encoding_rejects_extended_fields() {
        assert_eq!(
            encode_text_paragraph_style(TextParagraphStyle {
                text_align: TextAlign::Start,
                max_lines: Some(2),
                overflow: TextOverflow::Visible,
                text_direction: TextDirection::Rtl,
                text_width_basis: TextWidthBasis::LongestLine,
                strut_line_height: Some(24.0),
                text_height_behavior: TextHeightBehavior {
                    apply_height_to_first_ascent: false,
                    apply_height_to_last_descent: true,
                },
            }),
            None
        );
    }

    #[test]
    fn inline_widget_marker_round_trips() {
        let encoded = encode_inline_widget_marker(7, 24.5, 12.0);
        assert_eq!(
            decode_inline_widget_marker(Some(encoded.as_str())),
            Some(InlineWidgetMarker {
                id: 7,
                width: 24.5,
                height: 12.0,
            })
        );
    }
}
