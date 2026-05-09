use super::semantics::{ActionEntry, Semantics};
use super::widget_id::WidgetNodeId;
use crate::NodeId;
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
    pub animation_target: Option<WidgetNodeId>,
}

impl CompositeScalar {
    pub fn new(base: f32) -> Self {
        Self {
            base,
            animation_target: None,
        }
    }

    pub fn animated(mut self, target: WidgetNodeId) -> Self {
        self.animation_target = Some(target);
        self
    }
}

impl std::hash::Hash for CompositeScalar {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.base.to_bits().hash(state);
        self.animation_target.hash(state);
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GridTrack {
    Points(LayoutUnit),
    Percent(f32),
    Fr(f32),
    Auto,
    MinContent,
    MaxContent,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub enum GridPlacement {
    Auto,
    Line(i16),
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
        widget_id: WidgetNodeId,
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
    ZStack,
    Align,
    Flyout {
        anchor: NodeId,
        content: NodeId,
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
    pub offset: (LayoutUnit, LayoutUnit),
}

impl std::hash::Hash for BoxShadow {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.color.hash(state);
        self.blur_radius.to_bits().hash(state);
        self.offset.0.to_bits().hash(state);
        self.offset.1.to_bits().hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub enum ImageFit {
    Contain,
    Cover,
    Fill,
    None,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaintOp {
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
        source: String,
        fit: ImageFit,
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
            Self::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
            } => {
                0.hash(state);
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
                1.hash(state);
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
                2.hash(state);
                runs.hash(state);
                wrap.hash(state);
                caret_index.hash(state);
                caret_color.hash(state);
                caret_width.map(|w| w.to_bits()).hash(state);
                caret_height.map(|h| h.to_bits()).hash(state);
                caret_radius.map(|r| r.to_bits()).hash(state);
                paragraph_style.hash(state);
            }
            Self::DrawImage { source, fit } => {
                3.hash(state);
                source.hash(state);
                fit.hash(state);
            }
            Self::DrawPath { path, fill, stroke } => {
                4.hash(state);
                path.hash(state);
                fill.hash(state);
                stroke.hash(state);
            }
            Self::DrawSvg {
                content,
                fill,
                stroke,
            } => {
                5.hash(state);
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
        encode_text_paragraph_style, InlineWidgetMarker, TextAlign, TextDirection,
        TextHeightBehavior, TextOverflow, TextParagraphStyle, TextWidthBasis,
        TEXT_PARAGRAPH_MAX_ENCODED_LINES,
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
