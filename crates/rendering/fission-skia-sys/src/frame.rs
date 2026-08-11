use crate::paragraph::ParagraphDrawData;
use crate::{ffi, DecodedImage, Error, ErrorKind, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);

    pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub scale_x: f32,
    pub skew_x: f32,
    pub translate_x: f32,
    pub skew_y: f32,
    pub scale_y: f32,
    pub translate_y: f32,
}

impl Affine {
    pub const IDENTITY: Self = Self {
        scale_x: 1.0,
        skew_x: 0.0,
        translate_x: 0.0,
        skew_y: 0.0,
        scale_y: 1.0,
        translate_y: 0.0,
    };

    pub const fn translation(x: f32, y: f32) -> Self {
        Self {
            translate_x: x,
            translate_y: y,
            ..Self::IDENTITY
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCommand {
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    QuadTo {
        cx: f32,
        cy: f32,
        x: f32,
        y: f32,
    },
    CubicTo {
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    },
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub fill_rule: FillRule,
    pub commands: Vec<PathCommand>,
}

impl Path {
    pub fn new(fill_rule: FillRule, commands: impl Into<Vec<PathCommand>>) -> Self {
        Self {
            fill_rule,
            commands: commands.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

impl GradientStop {
    pub const fn new(offset: f32, color: Color) -> Self {
        Self { offset, color }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    Solid(Color),
    LinearGradient {
        start: Point,
        end: Point,
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        center: Point,
        radius: f32,
        stops: Vec<GradientStop>,
    },
}

impl Paint {
    pub const fn solid(color: Color) -> Self {
        Self::Solid(color)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub paint: Paint,
    pub width: f32,
    pub dash_array: Option<Vec<f32>>,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    pub color: Color,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub offset: Point,
    pub inset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSampling {
    /// Select the nearest source texel without mipmapping.
    Nearest,
    /// Bilinearly filter adjacent source texels without mipmapping.
    Linear,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameOp {
    Clear(Color),
    Save,
    Restore,
    /// Begins an isolated layer clipped to `bounds`. A matching [`FrameOp::Restore`]
    /// composites the complete group once using `alpha` in the inclusive range
    /// `0.0..=1.0`.
    OpacityLayer {
        bounds: Rect,
        alpha: f32,
    },
    ClipRect {
        rect: Rect,
    },
    ClipRoundedRect {
        rect: Rect,
        radius: f32,
    },
    ConcatAffine(Affine),
    FillRect {
        rect: Rect,
        radius: f32,
        paint: Paint,
    },
    StrokeRect {
        rect: Rect,
        radius: f32,
        stroke: Stroke,
    },
    FillPath {
        path: Path,
        paint: Paint,
    },
    StrokePath {
        path: Path,
        stroke: Stroke,
    },
    BoxShadow {
        rect: Rect,
        radius: f32,
        shadow: BoxShadow,
    },
    /// Paints immutable data from the exact paragraph layout that produced its
    /// geometry. `origin` is physical and `scale_factor` converts the retained
    /// logical picture into physical pixels without reshaping it.
    DrawParagraph {
        data: ParagraphDrawData,
        origin: Point,
        scale_factor: f32,
    },
    /// Draws an immutable decoded image from an explicit pixel-space source
    /// rectangle into a destination rectangle. Sampling is strictly confined
    /// to `source` and never uses mipmaps.
    DrawImage {
        image: DecodedImage,
        source: Rect,
        destination: Rect,
        sampling: ImageSampling,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    pub operations: Vec<FrameOp>,
}

impl Frame {
    pub fn new(operations: impl Into<Vec<FrameOp>>) -> Self {
        Self {
            operations: operations.into(),
        }
    }

    pub(crate) fn encode(&self) -> Result<EncodedFrame> {
        let mut encoded = EncodedFrame::with_capacity(self.operations.len());
        let mut save_depth = 0usize;

        for operation in &self.operations {
            let mut raw = zero_operation();
            match operation {
                FrameOp::Clear(color) => {
                    raw.kind = ffi::FRAME_CLEAR;
                    raw.paint = raw_paint(&Paint::Solid(*color), &mut encoded.gradient_stops)?;
                }
                FrameOp::Save => {
                    raw.kind = ffi::FRAME_SAVE;
                    save_depth = save_depth.saturating_add(1);
                }
                FrameOp::Restore => {
                    raw.kind = ffi::FRAME_RESTORE;
                    save_depth = save_depth
                        .checked_sub(1)
                        .ok_or_else(|| invalid("restore has no matching save or opacity layer"))?;
                }
                FrameOp::OpacityLayer { bounds, alpha } => {
                    raw.kind = ffi::FRAME_OPACITY_LAYER;
                    raw.rect = raw_rect(*bounds)?;
                    raw.opacity = unit_interval(*alpha, "opacity layer alpha")?;
                    save_depth = save_depth.saturating_add(1);
                }
                FrameOp::ClipRect { rect } => {
                    raw.kind = ffi::FRAME_CLIP_RECT;
                    raw.rect = raw_rect(*rect)?;
                }
                FrameOp::ClipRoundedRect { rect, radius } => {
                    raw.kind = ffi::FRAME_CLIP_ROUNDED_RECT;
                    raw.rect = raw_rect(*rect)?;
                    raw.radius = non_negative(*radius, "rounded clip radius")?;
                }
                FrameOp::ConcatAffine(affine) => {
                    raw.kind = ffi::FRAME_CONCAT_AFFINE;
                    raw.affine = raw_affine(*affine)?;
                }
                FrameOp::FillRect {
                    rect,
                    radius,
                    paint,
                } => {
                    raw.kind = ffi::FRAME_FILL_RECT;
                    raw.rect = raw_rect(*rect)?;
                    raw.radius = non_negative(*radius, "rectangle corner radius")?;
                    raw.paint = raw_paint(paint, &mut encoded.gradient_stops)?;
                }
                FrameOp::StrokeRect {
                    rect,
                    radius,
                    stroke,
                } => {
                    raw.kind = ffi::FRAME_STROKE_RECT;
                    raw.rect = raw_rect(*rect)?;
                    raw.radius = non_negative(*radius, "rectangle corner radius")?;
                    let (paint, stroke) = raw_stroke(
                        stroke,
                        &mut encoded.gradient_stops,
                        &mut encoded.dash_intervals,
                    )?;
                    raw.paint = paint;
                    raw.stroke = stroke;
                }
                FrameOp::FillPath { path, paint } => {
                    raw.kind = ffi::FRAME_FILL_PATH;
                    let encoded_path = encode_path(path, &mut encoded.path_commands)?;
                    raw.path_offset = encoded_path.offset;
                    raw.path_count = encoded_path.count;
                    raw.fill_rule = encoded_path.fill_rule;
                    raw.paint = raw_paint(paint, &mut encoded.gradient_stops)?;
                }
                FrameOp::StrokePath { path, stroke } => {
                    raw.kind = ffi::FRAME_STROKE_PATH;
                    let encoded_path = encode_path(path, &mut encoded.path_commands)?;
                    raw.path_offset = encoded_path.offset;
                    raw.path_count = encoded_path.count;
                    raw.fill_rule = encoded_path.fill_rule;
                    let (paint, stroke) = raw_stroke(
                        stroke,
                        &mut encoded.gradient_stops,
                        &mut encoded.dash_intervals,
                    )?;
                    raw.paint = paint;
                    raw.stroke = stroke;
                }
                FrameOp::BoxShadow {
                    rect,
                    radius,
                    shadow,
                } => {
                    raw.kind = ffi::FRAME_BOX_SHADOW;
                    raw.rect = raw_rect(*rect)?;
                    raw.radius = non_negative(*radius, "shadow corner radius")?;
                    raw.shadow = raw_shadow(*shadow)?;
                }
                FrameOp::DrawParagraph {
                    data,
                    origin,
                    scale_factor,
                } => {
                    raw.kind = ffi::FRAME_DRAW_PARAGRAPH;
                    raw.rect.x = finite(origin.x, "paragraph origin x")?;
                    raw.rect.y = finite(origin.y, "paragraph origin y")?;
                    raw.radius = positive(*scale_factor, "paragraph scale factor")?;
                    let handle = data.raw_handle();
                    if handle == 0 {
                        return Err(invalid("paragraph draw handle must not be null"));
                    }
                    raw.path_offset = handle as u32;
                    raw.path_count = (handle >> 32) as u32;
                    encoded.paragraph_draw_data.push(data.clone());
                }
                FrameOp::DrawImage {
                    image,
                    source,
                    destination,
                    sampling,
                } => {
                    raw.kind = ffi::FRAME_DRAW_IMAGE;
                    let source = raw_non_empty_rect(*source, "image source")?;
                    let destination = raw_non_empty_rect(*destination, "image destination")?;
                    let source_right = f64::from(source.x) + f64::from(source.width);
                    let source_bottom = f64::from(source.y) + f64::from(source.height);
                    if source.x < 0.0
                        || source.y < 0.0
                        || source_right > f64::from(image.width())
                        || source_bottom > f64::from(image.height())
                    {
                        return Err(invalid(
                            "image source rectangle must lie inside the decoded image",
                        ));
                    }
                    let handle = image.raw_handle();
                    if handle == 0 {
                        return Err(invalid("decoded image handle must not be null"));
                    }
                    raw.image = ffi::ImageDraw {
                        struct_size: std::mem::size_of::<ffi::ImageDraw>() as u32,
                        sampling: match sampling {
                            ImageSampling::Nearest => ffi::IMAGE_SAMPLING_NEAREST,
                            ImageSampling::Linear => ffi::IMAGE_SAMPLING_LINEAR,
                        },
                        image: handle,
                        source,
                        destination,
                    };
                    encoded.images.push(image.clone());
                }
            }
            encoded.operations.push(raw);
        }

        if save_depth != 0 {
            return Err(invalid(format!(
                "frame leaves {save_depth} save or opacity-layer operation(s) unrestored"
            )));
        }
        Ok(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

pub(crate) struct EncodedFrame {
    operations: Vec<ffi::FrameOp>,
    path_commands: Vec<ffi::PathCommand>,
    gradient_stops: Vec<ffi::GradientStop>,
    dash_intervals: Vec<f32>,
    // Keeps every packed native paragraph handle alive through execution even
    // if another internal caller encodes from a temporary Frame.
    paragraph_draw_data: Vec<ParagraphDrawData>,
    // Keeps every packed native image handle alive through execution even if
    // another internal caller encodes from a temporary Frame.
    images: Vec<DecodedImage>,
}

impl EncodedFrame {
    fn with_capacity(operation_count: usize) -> Self {
        Self {
            operations: Vec::with_capacity(operation_count),
            path_commands: Vec::new(),
            gradient_stops: Vec::new(),
            dash_intervals: Vec::new(),
            paragraph_draw_data: Vec::new(),
            images: Vec::new(),
        }
    }

    pub(crate) fn as_raw(&self) -> ffi::Frame {
        ffi::Frame {
            struct_size: std::mem::size_of::<ffi::Frame>() as u32,
            reserved: 0,
            operations: self.operations.as_ptr(),
            operation_count: self.operations.len(),
            path_commands: self.path_commands.as_ptr(),
            path_command_count: self.path_commands.len(),
            gradient_stops: self.gradient_stops.as_ptr(),
            gradient_stop_count: self.gradient_stops.len(),
            dash_intervals: self.dash_intervals.as_ptr(),
            dash_interval_count: self.dash_intervals.len(),
        }
    }
}

struct EncodedPath {
    offset: u32,
    count: u32,
    fill_rule: u32,
}

fn encode_path(path: &Path, output: &mut Vec<ffi::PathCommand>) -> Result<EncodedPath> {
    if path.commands.is_empty() {
        return Err(invalid("path must contain at least one command"));
    }
    let offset = u32::try_from(output.len())
        .map_err(|_| invalid("path command offset exceeds the ABI limit"))?;
    let mut has_current_point = false;
    for command in &path.commands {
        if matches!(command, PathCommand::MoveTo { .. }) {
            has_current_point = true;
        } else if !has_current_point {
            return Err(invalid("each path contour must begin with move-to"));
        }
        output.push(raw_path_command(*command)?);
    }
    let count = u32::try_from(path.commands.len())
        .map_err(|_| invalid("path command count exceeds the ABI limit"))?;
    let fill_rule = match path.fill_rule {
        FillRule::NonZero => ffi::FILL_NON_ZERO,
        FillRule::EvenOdd => ffi::FILL_EVEN_ODD,
    };
    Ok(EncodedPath {
        offset,
        count,
        fill_rule,
    })
}

fn raw_paint(paint: &Paint, output: &mut Vec<ffi::GradientStop>) -> Result<ffi::Paint> {
    let mut raw = zero_paint();
    raw.struct_size = std::mem::size_of::<ffi::Paint>() as u32;
    match paint {
        Paint::Solid(color) => {
            raw.kind = ffi::PAINT_SOLID;
            raw.color = raw_color(*color)?;
        }
        Paint::LinearGradient { start, end, stops } => {
            raw.kind = ffi::PAINT_LINEAR_GRADIENT;
            raw.start = raw_point(*start, "linear gradient start")?;
            raw.end = raw_point(*end, "linear gradient end")?;
            let (offset, count) = append_stops(stops, output)?;
            raw.stop_offset = offset;
            raw.stop_count = count;
        }
        Paint::RadialGradient {
            center,
            radius,
            stops,
        } => {
            raw.kind = ffi::PAINT_RADIAL_GRADIENT;
            raw.start = raw_point(*center, "radial gradient center")?;
            raw.radius = non_negative(*radius, "radial gradient radius")?;
            let (offset, count) = append_stops(stops, output)?;
            raw.stop_offset = offset;
            raw.stop_count = count;
        }
    }
    Ok(raw)
}

fn append_stops(stops: &[GradientStop], output: &mut Vec<ffi::GradientStop>) -> Result<(u32, u32)> {
    let offset = u32::try_from(output.len())
        .map_err(|_| invalid("gradient stop offset exceeds the ABI limit"))?;
    let mut stops = stops.to_vec();
    for stop in &stops {
        if !stop.offset.is_finite() || !(0.0..=1.0).contains(&stop.offset) {
            return Err(invalid("gradient stop offsets must be finite and in 0..=1"));
        }
        raw_color(stop.color)?;
    }
    stops.sort_by(|left, right| left.offset.total_cmp(&right.offset));
    make_offsets_strict(&mut stops)?;
    let count = u32::try_from(stops.len())
        .map_err(|_| invalid("gradient stop count exceeds the ABI limit"))?;
    for stop in stops {
        output.push(ffi::GradientStop {
            offset: stop.offset,
            color: raw_color(stop.color)?,
        });
    }
    Ok((offset, count))
}

fn make_offsets_strict(stops: &mut [GradientStop]) -> Result<()> {
    if stops.len() < 2 {
        return Ok(());
    }
    for index in 1..stops.len() {
        if stops[index].offset <= stops[index - 1].offset {
            stops[index].offset = next_float_up(stops[index - 1].offset);
        }
    }
    if stops.last().is_some_and(|stop| stop.offset > 1.0) {
        stops.last_mut().unwrap().offset = 1.0;
        for index in (0..stops.len() - 1).rev() {
            if stops[index].offset >= stops[index + 1].offset {
                stops[index].offset = next_float_down(stops[index + 1].offset);
            }
        }
    }
    if stops.first().is_some_and(|stop| stop.offset < 0.0) {
        return Err(invalid(
            "gradient has more coincident stops than f32 can represent distinctly",
        ));
    }
    Ok(())
}

fn next_float_up(value: f32) -> f32 {
    if value == -0.0 {
        return f32::from_bits(1);
    }
    let bits = value.to_bits();
    f32::from_bits(if value >= 0.0 { bits + 1 } else { bits - 1 })
}

fn next_float_down(value: f32) -> f32 {
    if value == 0.0 {
        return -f32::from_bits(1);
    }
    let bits = value.to_bits();
    f32::from_bits(if value > 0.0 { bits - 1 } else { bits + 1 })
}

fn raw_stroke(
    stroke: &Stroke,
    gradients: &mut Vec<ffi::GradientStop>,
    dashes: &mut Vec<f32>,
) -> Result<(ffi::Paint, ffi::Stroke)> {
    let paint = raw_paint(&stroke.paint, gradients)?;
    let width = non_negative(stroke.width, "stroke width")?;
    let line_cap = match stroke.line_cap {
        LineCap::Butt => ffi::LINE_CAP_BUTT,
        LineCap::Round => ffi::LINE_CAP_ROUND,
        LineCap::Square => ffi::LINE_CAP_SQUARE,
    };
    let line_join = match stroke.line_join {
        LineJoin::Miter => ffi::LINE_JOIN_MITER,
        LineJoin::Round => ffi::LINE_JOIN_ROUND,
        LineJoin::Bevel => ffi::LINE_JOIN_BEVEL,
    };
    let dash_offset = u32::try_from(dashes.len())
        .map_err(|_| invalid("dash interval offset exceeds the ABI limit"))?;
    let mut normalized = stroke.dash_array.clone().unwrap_or_default();
    if normalized
        .iter()
        .any(|interval| !interval.is_finite() || *interval < 0.0)
    {
        return Err(invalid(
            "stroke dash intervals must be finite and non-negative",
        ));
    }
    if normalized.iter().all(|interval| *interval == 0.0) {
        normalized.clear();
    } else if normalized.len() % 2 == 1 {
        let repeated = normalized.clone();
        normalized.extend(repeated);
    }
    let dash_count = u32::try_from(normalized.len())
        .map_err(|_| invalid("dash interval count exceeds the ABI limit"))?;
    dashes.extend(normalized);
    Ok((
        paint,
        ffi::Stroke {
            struct_size: std::mem::size_of::<ffi::Stroke>() as u32,
            width,
            line_cap,
            line_join,
            dash_offset,
            dash_count,
        },
    ))
}

fn raw_shadow(shadow: BoxShadow) -> Result<ffi::BoxShadow> {
    Ok(ffi::BoxShadow {
        struct_size: std::mem::size_of::<ffi::BoxShadow>() as u32,
        inset: if shadow.inset { 1 } else { 0 },
        color: raw_color(shadow.color)?,
        blur_radius: non_negative(shadow.blur_radius, "shadow blur radius")?,
        spread_radius: finite(shadow.spread_radius, "shadow spread radius")?,
        offset_x: finite(shadow.offset.x, "shadow x offset")?,
        offset_y: finite(shadow.offset.y, "shadow y offset")?,
    })
}

fn raw_color(color: Color) -> Result<ffi::Color> {
    let values = [color.red, color.green, color.blue, color.alpha];
    if values
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(invalid(
            "colors must contain finite, unpremultiplied sRGB components in 0..=1",
        ));
    }
    Ok(ffi::Color {
        red: color.red,
        green: color.green,
        blue: color.blue,
        alpha: color.alpha,
    })
}

fn raw_point(point: Point, label: &str) -> Result<ffi::Point> {
    Ok(ffi::Point {
        x: finite(point.x, label)?,
        y: finite(point.y, label)?,
    })
}

fn raw_rect(rect: Rect) -> Result<ffi::Rect> {
    if ![rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|value| value.is_finite())
        || rect.width < 0.0
        || rect.height < 0.0
    {
        return Err(invalid(
            "rect coordinates must be finite and dimensions must be non-negative",
        ));
    }
    Ok(ffi::Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    })
}

fn raw_non_empty_rect(rect: Rect, label: &str) -> Result<ffi::Rect> {
    let rect = raw_rect(rect)?;
    if rect.width == 0.0 || rect.height == 0.0 {
        return Err(invalid(format!("{label} rectangle must be non-empty")));
    }
    Ok(rect)
}

fn raw_affine(affine: Affine) -> Result<ffi::Affine> {
    let values = [
        affine.scale_x,
        affine.skew_x,
        affine.translate_x,
        affine.skew_y,
        affine.scale_y,
        affine.translate_y,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid("affine transform values must be finite"));
    }
    Ok(ffi::Affine {
        scale_x: affine.scale_x,
        skew_x: affine.skew_x,
        translate_x: affine.translate_x,
        skew_y: affine.skew_y,
        scale_y: affine.scale_y,
        translate_y: affine.translate_y,
    })
}

fn raw_path_command(command: PathCommand) -> Result<ffi::PathCommand> {
    let (verb, coordinates) = match command {
        PathCommand::MoveTo { x, y } => (ffi::PATH_MOVE, [x, y, 0.0, 0.0, 0.0, 0.0]),
        PathCommand::LineTo { x, y } => (ffi::PATH_LINE, [x, y, 0.0, 0.0, 0.0, 0.0]),
        PathCommand::QuadTo { cx, cy, x, y } => (ffi::PATH_QUAD, [cx, cy, x, y, 0.0, 0.0]),
        PathCommand::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => (ffi::PATH_CUBIC, [c1x, c1y, c2x, c2y, x, y]),
        PathCommand::Close => (ffi::PATH_CLOSE, [0.0; 6]),
    };
    if coordinates.iter().any(|value| !value.is_finite()) {
        return Err(invalid("path coordinates must be finite"));
    }
    Ok(ffi::PathCommand {
        struct_size: std::mem::size_of::<ffi::PathCommand>() as u32,
        verb,
        x1: coordinates[0],
        y1: coordinates[1],
        x2: coordinates[2],
        y2: coordinates[3],
        x3: coordinates[4],
        y3: coordinates[5],
    })
}

fn zero_operation() -> ffi::FrameOp {
    ffi::FrameOp {
        struct_size: std::mem::size_of::<ffi::FrameOp>() as u32,
        kind: 0,
        paint: zero_paint(),
        stroke: zero_stroke(),
        shadow: zero_shadow(),
        rect: zero_rect(),
        affine: zero_affine(),
        radius: 0.0,
        path_offset: 0,
        path_count: 0,
        fill_rule: 0,
        opacity: 0.0,
        image: zero_image_draw(),
    }
}

fn zero_image_draw() -> ffi::ImageDraw {
    ffi::ImageDraw {
        struct_size: 0,
        sampling: 0,
        image: 0,
        source: zero_rect(),
        destination: zero_rect(),
    }
}

fn zero_paint() -> ffi::Paint {
    ffi::Paint {
        struct_size: 0,
        kind: 0,
        color: zero_color(),
        start: zero_point(),
        end: zero_point(),
        radius: 0.0,
        stop_offset: 0,
        stop_count: 0,
    }
}

fn zero_stroke() -> ffi::Stroke {
    ffi::Stroke {
        struct_size: 0,
        width: 0.0,
        line_cap: 0,
        line_join: 0,
        dash_offset: 0,
        dash_count: 0,
    }
}

fn zero_shadow() -> ffi::BoxShadow {
    ffi::BoxShadow {
        struct_size: 0,
        inset: 0,
        color: zero_color(),
        blur_radius: 0.0,
        spread_radius: 0.0,
        offset_x: 0.0,
        offset_y: 0.0,
    }
}

fn zero_color() -> ffi::Color {
    ffi::Color {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    }
}

fn zero_point() -> ffi::Point {
    ffi::Point { x: 0.0, y: 0.0 }
}

fn zero_rect() -> ffi::Rect {
    ffi::Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }
}

fn zero_affine() -> ffi::Affine {
    ffi::Affine {
        scale_x: 0.0,
        skew_x: 0.0,
        translate_x: 0.0,
        skew_y: 0.0,
        scale_y: 0.0,
        translate_y: 0.0,
    }
}

fn finite(value: f32, label: &str) -> Result<f32> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid(format!("{label} must be finite")))
    }
}

fn non_negative(value: f32, label: &str) -> Result<f32> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(invalid(format!("{label} must be finite and non-negative")))
    }
}

fn positive(value: f32, label: &str) -> Result<f32> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid(format!("{label} must be finite and positive")))
    }
}

fn unit_interval(value: f32, label: &str) -> Result<f32> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(invalid(format!("{label} must be finite and in 0..=1")))
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::local(ErrorKind::InvalidArgument, "Frame::encode", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_dash_arrays_are_repeated_and_gradient_stops_are_sorted() {
        let frame = Frame::new([FrameOp::StrokeRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            radius: 2.0,
            stroke: Stroke {
                paint: Paint::LinearGradient {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(10.0, 0.0),
                    stops: vec![
                        GradientStop::new(1.0, Color::rgba(1.0, 0.0, 0.0, 1.0)),
                        GradientStop::new(1.0, Color::rgba(0.0, 1.0, 0.0, 1.0)),
                        GradientStop::new(0.0, Color::rgba(0.0, 0.0, 1.0, 1.0)),
                    ],
                },
                width: 1.0,
                dash_array: Some(vec![1.0, 2.0, 3.0]),
                line_cap: LineCap::Round,
                line_join: LineJoin::Bevel,
            },
        }]);

        let encoded = frame.encode().unwrap();

        assert_eq!(encoded.gradient_stops[0].offset, 0.0);
        assert!(encoded.gradient_stops[1].offset < encoded.gradient_stops[2].offset);
        assert_eq!(encoded.gradient_stops[2].offset, 1.0);
        assert_eq!(encoded.dash_intervals, [1.0, 2.0, 3.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn state_stack_and_finite_values_fail_closed() {
        assert!(Frame::new([FrameOp::Restore]).encode().is_err());
        assert!(Frame::new([FrameOp::Save]).encode().is_err());
        assert!(Frame::new([FrameOp::OpacityLayer {
            bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            alpha: 0.5,
        }])
        .encode()
        .is_err());
        assert!(Frame::new([FrameOp::ConcatAffine(Affine {
            scale_x: f32::NAN,
            ..Affine::IDENTITY
        })])
        .encode()
        .is_err());
    }

    #[test]
    fn opacity_layers_encode_bounded_group_alpha_and_balance_with_restore() {
        let frame = Frame::new([
            FrameOp::OpacityLayer {
                bounds: Rect::new(1.0, 2.0, 3.0, 4.0),
                alpha: 0.25,
            },
            FrameOp::Restore,
        ]);

        let encoded = frame.encode().unwrap();

        assert_eq!(encoded.operations[0].kind, ffi::FRAME_OPACITY_LAYER);
        assert_eq!(encoded.operations[0].rect.x, 1.0);
        assert_eq!(encoded.operations[0].rect.y, 2.0);
        assert_eq!(encoded.operations[0].rect.width, 3.0);
        assert_eq!(encoded.operations[0].rect.height, 4.0);
        assert_eq!(encoded.operations[0].opacity, 0.25);
        assert_eq!(encoded.operations[1].kind, ffi::FRAME_RESTORE);
    }

    #[test]
    fn opacity_layer_rejects_invalid_bounds_and_alpha() {
        for alpha in [f32::NAN, f32::NEG_INFINITY, -0.01, 1.01] {
            assert!(Frame::new([
                FrameOp::OpacityLayer {
                    bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
                    alpha,
                },
                FrameOp::Restore
            ])
            .encode()
            .is_err());
        }
        assert!(Frame::new([
            FrameOp::OpacityLayer {
                bounds: Rect::new(0.0, 0.0, -1.0, 1.0),
                alpha: 0.5,
            },
            FrameOp::Restore,
        ])
        .encode()
        .is_err());
    }

    #[test]
    fn gradient_edge_cases_remain_explicit_in_the_encoded_frame() {
        let frame = Frame::new([
            FrameOp::FillRect {
                rect: Rect::new(0.0, 0.0, 4.0, 4.0),
                radius: 0.0,
                paint: Paint::LinearGradient {
                    start: Point::new(1.0, 1.0),
                    end: Point::new(1.0, 1.0),
                    stops: Vec::new(),
                },
            },
            FrameOp::FillRect {
                rect: Rect::new(0.0, 0.0, 4.0, 4.0),
                radius: 0.0,
                paint: Paint::RadialGradient {
                    center: Point::new(2.0, 2.0),
                    radius: 0.0,
                    stops: vec![GradientStop::new(0.5, Color::rgba(1.0, 0.0, 0.0, 1.0))],
                },
            },
        ]);

        let encoded = frame.encode().unwrap();

        assert_eq!(encoded.operations[0].paint.stop_count, 0);
        assert_eq!(encoded.operations[1].paint.stop_count, 1);
        assert_eq!(encoded.operations[1].paint.radius, 0.0);
    }
}
