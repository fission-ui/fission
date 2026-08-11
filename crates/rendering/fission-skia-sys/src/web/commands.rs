//! Canonical paint-command stream carried inside a CanvasKit frame packet.
//!
//! The command stream contains only values and generational resource handles.
//! It never contains Rust, JavaScript, Skia, or WebAssembly pointers. Every
//! command is length-delimited so malformed input is rejected before a browser
//! host mutates CanvasKit state.

use std::fmt;

use crate::{
    Affine, BoxShadow, Color, FillRule, GradientStop, ImageSampling, LineCap, LineJoin, Paint,
    Path, PathCommand, Point, Rect, Stroke,
};

use super::ResourceHandle;

pub const COMMAND_MAGIC: [u8; 4] = *b"FSCM";
pub const COMMAND_VERSION: u16 = 1;
pub const COMMAND_HEADER_LEN: usize = 16;
pub const MAX_COMMAND_STREAM_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_COMMANDS: usize = 262_144;
pub const MAX_PATH_COMMANDS: usize = 1_048_576;
pub const MAX_GRADIENT_STOPS: usize = 65_536;
pub const MAX_DASH_INTERVALS: usize = 65_536;

const ENTRY_HEADER_LEN: usize = 8;
const PAINT_HEADER_LEN: usize = 44;
const PATH_HEADER_LEN: usize = 8;
const PATH_ENTRY_LEN: usize = 28;
const STROKE_HEADER_LEN: usize = 12;

#[derive(Debug, Clone, PartialEq)]
pub enum WebCommand {
    Clear(Color),
    Save,
    Restore,
    OpacityLayer {
        bounds: Rect,
        alpha: f32,
    },
    ClipRect(Rect),
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
    DrawParagraph {
        paragraph: ResourceHandle,
        origin: Point,
        scale_factor: f32,
    },
    DrawImage {
        image: ResourceHandle,
        source: Rect,
        destination: Rect,
        sampling: ImageSampling,
    },
    BackdropBlur {
        bounds: Rect,
        corner_radius: f32,
        sigma: f32,
    },
    DrawSvg {
        document: ResourceHandle,
        destination: Rect,
    },
    DrawPicture {
        picture: ResourceHandle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStreamError {
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u16),
    NonZeroReserved,
    LengthMismatch,
    UnknownCommand(u16),
    InvalidValue(&'static str),
    LimitExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    UnbalancedRestore,
    UnclosedSaveDepth(usize),
}

impl fmt::Display for CommandStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CommandStreamError {}

#[repr(u16)]
enum CommandKind {
    Clear = 1,
    Save = 2,
    Restore = 3,
    OpacityLayer = 4,
    ClipRect = 5,
    ClipRoundedRect = 6,
    ConcatAffine = 7,
    FillRect = 8,
    StrokeRect = 9,
    FillPath = 10,
    StrokePath = 11,
    BoxShadow = 12,
    DrawParagraph = 13,
    DrawImage = 14,
    BackdropBlur = 15,
    DrawSvg = 16,
    DrawPicture = 17,
}

pub fn encode_commands(commands: &[WebCommand]) -> Result<Vec<u8>, CommandStreamError> {
    require_limit("commands", commands.len(), MAX_COMMANDS)?;
    let mut body = Vec::new();
    let mut save_depth = 0usize;
    for command in commands {
        let (kind, payload) = encode_command(command, &mut save_depth)?;
        let entry_len = ENTRY_HEADER_LEN
            .checked_add(payload.len())
            .ok_or(CommandStreamError::LengthMismatch)?;
        let entry_len = u32::try_from(entry_len).map_err(|_| CommandStreamError::LengthMismatch)?;
        put_u16(&mut body, kind as u16);
        put_u16(&mut body, 0);
        put_u32(&mut body, entry_len);
        body.extend_from_slice(&payload);
        require_limit(
            "command stream bytes",
            COMMAND_HEADER_LEN.saturating_add(body.len()),
            MAX_COMMAND_STREAM_BYTES,
        )?;
    }
    if save_depth != 0 {
        return Err(CommandStreamError::UnclosedSaveDepth(save_depth));
    }

    let total_len = COMMAND_HEADER_LEN
        .checked_add(body.len())
        .ok_or(CommandStreamError::LengthMismatch)?;
    let total_len = u32::try_from(total_len).map_err(|_| CommandStreamError::LengthMismatch)?;
    let command_count =
        u32::try_from(commands.len()).map_err(|_| CommandStreamError::LengthMismatch)?;
    let mut bytes = Vec::with_capacity(total_len as usize);
    bytes.extend_from_slice(&COMMAND_MAGIC);
    put_u16(&mut bytes, COMMAND_VERSION);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, total_len);
    put_u32(&mut bytes, command_count);
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

pub fn decode_commands(bytes: &[u8]) -> Result<Vec<WebCommand>, CommandStreamError> {
    require_limit(
        "command stream bytes",
        bytes.len(),
        MAX_COMMAND_STREAM_BYTES,
    )?;
    if bytes.len() < COMMAND_HEADER_LEN {
        return Err(CommandStreamError::Truncated);
    }
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != COMMAND_MAGIC {
        return Err(CommandStreamError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != COMMAND_VERSION {
        return Err(CommandStreamError::UnsupportedVersion(version));
    }
    if reader.u16()? != 0 {
        return Err(CommandStreamError::NonZeroReserved);
    }
    if reader.u32()? as usize != bytes.len() {
        return Err(CommandStreamError::LengthMismatch);
    }
    let command_count = reader.u32()? as usize;
    require_limit("commands", command_count, MAX_COMMANDS)?;
    if command_count > reader.remaining() / ENTRY_HEADER_LEN {
        return Err(CommandStreamError::Truncated);
    }

    let mut commands = Vec::with_capacity(command_count);
    let mut save_depth = 0usize;
    for _ in 0..command_count {
        let kind = reader.u16()?;
        if reader.u16()? != 0 {
            return Err(CommandStreamError::NonZeroReserved);
        }
        let entry_len = reader.u32()? as usize;
        if entry_len < ENTRY_HEADER_LEN {
            return Err(CommandStreamError::LengthMismatch);
        }
        let mut payload = Reader::new(reader.take(entry_len - ENTRY_HEADER_LEN)?);
        let command = decode_command(kind, &mut payload, &mut save_depth)?;
        payload.finish()?;
        commands.push(command);
    }
    reader.finish()?;
    if save_depth != 0 {
        return Err(CommandStreamError::UnclosedSaveDepth(save_depth));
    }
    Ok(commands)
}

fn encode_command(
    command: &WebCommand,
    save_depth: &mut usize,
) -> Result<(CommandKind, Vec<u8>), CommandStreamError> {
    let mut bytes = Vec::new();
    let kind = match command {
        WebCommand::Clear(color) => {
            encode_color(&mut bytes, *color)?;
            CommandKind::Clear
        }
        WebCommand::Save => {
            *save_depth = save_depth.saturating_add(1);
            CommandKind::Save
        }
        WebCommand::Restore => {
            *save_depth = save_depth
                .checked_sub(1)
                .ok_or(CommandStreamError::UnbalancedRestore)?;
            CommandKind::Restore
        }
        WebCommand::OpacityLayer { bounds, alpha } => {
            encode_rect(&mut bytes, *bounds)?;
            put_f32(&mut bytes, unit(*alpha, "opacity alpha")?);
            *save_depth = save_depth.saturating_add(1);
            CommandKind::OpacityLayer
        }
        WebCommand::ClipRect(rect) => {
            encode_rect(&mut bytes, *rect)?;
            CommandKind::ClipRect
        }
        WebCommand::ClipRoundedRect { rect, radius } => {
            encode_rect(&mut bytes, *rect)?;
            put_f32(&mut bytes, non_negative(*radius, "clip radius")?);
            CommandKind::ClipRoundedRect
        }
        WebCommand::ConcatAffine(affine) => {
            for value in [
                affine.scale_x,
                affine.skew_x,
                affine.translate_x,
                affine.skew_y,
                affine.scale_y,
                affine.translate_y,
            ] {
                put_f32(&mut bytes, finite(value, "affine")?);
            }
            CommandKind::ConcatAffine
        }
        WebCommand::FillRect {
            rect,
            radius,
            paint,
        } => {
            encode_rect(&mut bytes, *rect)?;
            put_f32(&mut bytes, non_negative(*radius, "rectangle radius")?);
            encode_paint(&mut bytes, paint)?;
            CommandKind::FillRect
        }
        WebCommand::StrokeRect {
            rect,
            radius,
            stroke,
        } => {
            encode_rect(&mut bytes, *rect)?;
            put_f32(&mut bytes, non_negative(*radius, "rectangle radius")?);
            encode_stroke(&mut bytes, stroke)?;
            CommandKind::StrokeRect
        }
        WebCommand::FillPath { path, paint } => {
            encode_path(&mut bytes, path)?;
            encode_paint(&mut bytes, paint)?;
            CommandKind::FillPath
        }
        WebCommand::StrokePath { path, stroke } => {
            encode_path(&mut bytes, path)?;
            encode_stroke(&mut bytes, stroke)?;
            CommandKind::StrokePath
        }
        WebCommand::BoxShadow {
            rect,
            radius,
            shadow,
        } => {
            encode_rect(&mut bytes, *rect)?;
            put_f32(&mut bytes, non_negative(*radius, "shadow radius")?);
            encode_color(&mut bytes, shadow.color)?;
            put_f32(&mut bytes, non_negative(shadow.blur_radius, "shadow blur")?);
            put_f32(&mut bytes, finite(shadow.spread_radius, "shadow spread")?);
            put_f32(&mut bytes, finite(shadow.offset.x, "shadow offset")?);
            put_f32(&mut bytes, finite(shadow.offset.y, "shadow offset")?);
            bytes.push(u8::from(shadow.inset));
            bytes.extend_from_slice(&[0; 3]);
            CommandKind::BoxShadow
        }
        WebCommand::DrawParagraph {
            paragraph,
            origin,
            scale_factor,
        } => {
            encode_handle(&mut bytes, *paragraph)?;
            encode_point(&mut bytes, *origin)?;
            put_f32(
                &mut bytes,
                positive(*scale_factor, "paragraph scale factor")?,
            );
            CommandKind::DrawParagraph
        }
        WebCommand::DrawImage {
            image,
            source,
            destination,
            sampling,
        } => {
            encode_handle(&mut bytes, *image)?;
            encode_non_empty_rect(&mut bytes, *source, "image source")?;
            encode_non_empty_rect(&mut bytes, *destination, "image destination")?;
            bytes.push(match sampling {
                ImageSampling::Nearest => 1,
                ImageSampling::Linear => 2,
            });
            bytes.extend_from_slice(&[0; 3]);
            CommandKind::DrawImage
        }
        WebCommand::BackdropBlur {
            bounds,
            corner_radius,
            sigma,
        } => {
            encode_rect(&mut bytes, *bounds)?;
            put_f32(&mut bytes, non_negative(*corner_radius, "backdrop radius")?);
            put_f32(&mut bytes, non_negative(*sigma, "backdrop sigma")?);
            CommandKind::BackdropBlur
        }
        WebCommand::DrawSvg {
            document,
            destination,
        } => {
            encode_handle(&mut bytes, *document)?;
            encode_non_empty_rect(&mut bytes, *destination, "SVG destination")?;
            CommandKind::DrawSvg
        }
        WebCommand::DrawPicture { picture } => {
            encode_handle(&mut bytes, *picture)?;
            CommandKind::DrawPicture
        }
    };
    Ok((kind, bytes))
}

fn decode_command(
    kind: u16,
    bytes: &mut Reader<'_>,
    save_depth: &mut usize,
) -> Result<WebCommand, CommandStreamError> {
    Ok(match kind {
        1 => WebCommand::Clear(decode_color(bytes)?),
        2 => {
            *save_depth = save_depth.saturating_add(1);
            WebCommand::Save
        }
        3 => {
            *save_depth = save_depth
                .checked_sub(1)
                .ok_or(CommandStreamError::UnbalancedRestore)?;
            WebCommand::Restore
        }
        4 => {
            let command = WebCommand::OpacityLayer {
                bounds: decode_rect(bytes)?,
                alpha: unit(bytes.f32()?, "opacity alpha")?,
            };
            *save_depth = save_depth.saturating_add(1);
            command
        }
        5 => WebCommand::ClipRect(decode_rect(bytes)?),
        6 => WebCommand::ClipRoundedRect {
            rect: decode_rect(bytes)?,
            radius: non_negative(bytes.f32()?, "clip radius")?,
        },
        7 => WebCommand::ConcatAffine(Affine {
            scale_x: finite(bytes.f32()?, "affine")?,
            skew_x: finite(bytes.f32()?, "affine")?,
            translate_x: finite(bytes.f32()?, "affine")?,
            skew_y: finite(bytes.f32()?, "affine")?,
            scale_y: finite(bytes.f32()?, "affine")?,
            translate_y: finite(bytes.f32()?, "affine")?,
        }),
        8 => WebCommand::FillRect {
            rect: decode_rect(bytes)?,
            radius: non_negative(bytes.f32()?, "rectangle radius")?,
            paint: decode_paint(bytes)?,
        },
        9 => WebCommand::StrokeRect {
            rect: decode_rect(bytes)?,
            radius: non_negative(bytes.f32()?, "rectangle radius")?,
            stroke: decode_stroke(bytes)?,
        },
        10 => WebCommand::FillPath {
            path: decode_path(bytes)?,
            paint: decode_paint(bytes)?,
        },
        11 => WebCommand::StrokePath {
            path: decode_path(bytes)?,
            stroke: decode_stroke(bytes)?,
        },
        12 => {
            let rect = decode_rect(bytes)?;
            let radius = non_negative(bytes.f32()?, "shadow radius")?;
            let color = decode_color(bytes)?;
            let blur_radius = non_negative(bytes.f32()?, "shadow blur")?;
            let spread_radius = finite(bytes.f32()?, "shadow spread")?;
            let offset = decode_point(bytes)?;
            let inset = match bytes.u8()? {
                0 => false,
                1 => true,
                _ => return Err(CommandStreamError::InvalidValue("shadow inset")),
            };
            if bytes.take(3)? != [0; 3] {
                return Err(CommandStreamError::NonZeroReserved);
            }
            WebCommand::BoxShadow {
                rect,
                radius,
                shadow: BoxShadow {
                    color,
                    blur_radius,
                    spread_radius,
                    offset,
                    inset,
                },
            }
        }
        13 => WebCommand::DrawParagraph {
            paragraph: decode_handle(bytes)?,
            origin: decode_point(bytes)?,
            scale_factor: positive(bytes.f32()?, "paragraph scale factor")?,
        },
        14 => {
            let image = decode_handle(bytes)?;
            let source = decode_non_empty_rect(bytes, "image source")?;
            let destination = decode_non_empty_rect(bytes, "image destination")?;
            let sampling = match bytes.u8()? {
                1 => ImageSampling::Nearest,
                2 => ImageSampling::Linear,
                _ => return Err(CommandStreamError::InvalidValue("image sampling")),
            };
            if bytes.take(3)? != [0; 3] {
                return Err(CommandStreamError::NonZeroReserved);
            }
            WebCommand::DrawImage {
                image,
                source,
                destination,
                sampling,
            }
        }
        15 => WebCommand::BackdropBlur {
            bounds: decode_rect(bytes)?,
            corner_radius: non_negative(bytes.f32()?, "backdrop radius")?,
            sigma: non_negative(bytes.f32()?, "backdrop sigma")?,
        },
        16 => WebCommand::DrawSvg {
            document: decode_handle(bytes)?,
            destination: decode_non_empty_rect(bytes, "SVG destination")?,
        },
        17 => WebCommand::DrawPicture {
            picture: decode_handle(bytes)?,
        },
        other => return Err(CommandStreamError::UnknownCommand(other)),
    })
}

fn encode_paint(bytes: &mut Vec<u8>, paint: &Paint) -> Result<(), CommandStreamError> {
    let (kind, solid, start, end, radius, stops) = match paint {
        Paint::Solid(color) => (
            1,
            *color,
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            0.0,
            vec![],
        ),
        Paint::LinearGradient { start, end, stops } => {
            (2, Color::TRANSPARENT, *start, *end, 0.0, {
                require_limit("gradient stops", stops.len(), MAX_GRADIENT_STOPS)?;
                normalize_stops(stops)?
            })
        }
        Paint::RadialGradient {
            center,
            radius,
            stops,
        } => (
            3,
            Color::TRANSPARENT,
            *center,
            Point::new(0.0, 0.0),
            non_negative(*radius, "gradient radius")?,
            {
                require_limit("gradient stops", stops.len(), MAX_GRADIENT_STOPS)?;
                normalize_stops(stops)?
            },
        ),
    };
    require_limit("gradient stops", stops.len(), MAX_GRADIENT_STOPS)?;
    bytes.push(kind);
    bytes.extend_from_slice(&[0; 3]);
    encode_color(bytes, solid)?;
    encode_point(bytes, start)?;
    encode_point(bytes, end)?;
    put_f32(bytes, radius);
    put_u32(bytes, stops.len() as u32);
    for stop in stops {
        put_f32(bytes, stop.offset);
        encode_color(bytes, stop.color)?;
    }
    Ok(())
}

fn decode_paint(bytes: &mut Reader<'_>) -> Result<Paint, CommandStreamError> {
    bytes.require(PAINT_HEADER_LEN)?;
    let kind = bytes.u8()?;
    if bytes.take(3)? != [0; 3] {
        return Err(CommandStreamError::NonZeroReserved);
    }
    let solid = decode_color(bytes)?;
    let start = decode_point(bytes)?;
    let end = decode_point(bytes)?;
    let radius = non_negative(bytes.f32()?, "gradient radius")?;
    let stop_count = bytes.u32()? as usize;
    require_limit("gradient stops", stop_count, MAX_GRADIENT_STOPS)?;
    if stop_count > bytes.remaining() / 20 {
        return Err(CommandStreamError::Truncated);
    }
    let mut stops = Vec::with_capacity(stop_count);
    for _ in 0..stop_count {
        stops.push(GradientStop::new(
            unit(bytes.f32()?, "gradient stop")?,
            decode_color(bytes)?,
        ));
    }
    if stops
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(CommandStreamError::InvalidValue("gradient stop ordering"));
    }
    let zero = Point::new(0.0, 0.0);
    match kind {
        1 if stop_count == 0 && start == zero && end == zero && radius == 0.0 => {
            Ok(Paint::Solid(solid))
        }
        2 if solid == Color::TRANSPARENT && radius == 0.0 => {
            Ok(Paint::LinearGradient { start, end, stops })
        }
        3 if solid == Color::TRANSPARENT && end == zero => Ok(Paint::RadialGradient {
            center: start,
            radius,
            stops,
        }),
        _ => Err(CommandStreamError::InvalidValue("paint kind or payload")),
    }
}

fn encode_stroke(bytes: &mut Vec<u8>, stroke: &Stroke) -> Result<(), CommandStreamError> {
    put_f32(bytes, non_negative(stroke.width, "stroke width")?);
    bytes.push(match stroke.line_cap {
        LineCap::Butt => 1,
        LineCap::Round => 2,
        LineCap::Square => 3,
    });
    bytes.push(match stroke.line_join {
        LineJoin::Miter => 1,
        LineJoin::Round => 2,
        LineJoin::Bevel => 3,
    });
    put_u16(bytes, 0);
    let source_dashes = stroke.dash_array.as_deref().unwrap_or_default();
    require_limit("dash intervals", source_dashes.len(), MAX_DASH_INTERVALS)?;
    let dashes = normalize_dashes(source_dashes)?;
    require_limit("dash intervals", dashes.len(), MAX_DASH_INTERVALS)?;
    put_u32(bytes, dashes.len() as u32);
    encode_paint(bytes, &stroke.paint)?;
    for dash in dashes {
        put_f32(bytes, dash);
    }
    Ok(())
}

fn decode_stroke(bytes: &mut Reader<'_>) -> Result<Stroke, CommandStreamError> {
    bytes.require(STROKE_HEADER_LEN)?;
    let width = non_negative(bytes.f32()?, "stroke width")?;
    let line_cap = match bytes.u8()? {
        1 => LineCap::Butt,
        2 => LineCap::Round,
        3 => LineCap::Square,
        _ => return Err(CommandStreamError::InvalidValue("line cap")),
    };
    let line_join = match bytes.u8()? {
        1 => LineJoin::Miter,
        2 => LineJoin::Round,
        3 => LineJoin::Bevel,
        _ => return Err(CommandStreamError::InvalidValue("line join")),
    };
    if bytes.u16()? != 0 {
        return Err(CommandStreamError::NonZeroReserved);
    }
    let dash_count = bytes.u32()? as usize;
    require_limit("dash intervals", dash_count, MAX_DASH_INTERVALS)?;
    let paint = decode_paint(bytes)?;
    if dash_count > bytes.remaining() / 4 {
        return Err(CommandStreamError::Truncated);
    }
    let mut dashes = Vec::with_capacity(dash_count);
    for _ in 0..dash_count {
        dashes.push(non_negative(bytes.f32()?, "dash interval")?);
    }
    if dash_count % 2 != 0 || (!dashes.is_empty() && dashes.iter().all(|dash| *dash == 0.0)) {
        return Err(CommandStreamError::InvalidValue("dash intervals"));
    }
    Ok(Stroke {
        paint,
        width,
        dash_array: (!dashes.is_empty()).then_some(dashes),
        line_cap,
        line_join,
    })
}

fn encode_path(bytes: &mut Vec<u8>, path: &Path) -> Result<(), CommandStreamError> {
    if path.commands.is_empty() {
        return Err(CommandStreamError::InvalidValue("empty path"));
    }
    require_limit("path commands", path.commands.len(), MAX_PATH_COMMANDS)?;
    bytes.push(match path.fill_rule {
        FillRule::NonZero => 1,
        FillRule::EvenOdd => 2,
    });
    bytes.extend_from_slice(&[0; 3]);
    put_u32(bytes, path.commands.len() as u32);
    let mut current = false;
    for command in &path.commands {
        let (kind, values) = match command {
            PathCommand::MoveTo { x, y } => {
                current = true;
                (1, [*x, *y, 0.0, 0.0, 0.0, 0.0])
            }
            PathCommand::LineTo { x, y } if current => (2, [*x, *y, 0.0, 0.0, 0.0, 0.0]),
            PathCommand::QuadTo { cx, cy, x, y } if current => (3, [*cx, *cy, *x, *y, 0.0, 0.0]),
            PathCommand::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } if current => (4, [*c1x, *c1y, *c2x, *c2y, *x, *y]),
            PathCommand::Close if current => (5, [0.0; 6]),
            _ => return Err(CommandStreamError::InvalidValue("path contour")),
        };
        bytes.push(kind);
        bytes.extend_from_slice(&[0; 3]);
        for value in values {
            put_f32(bytes, finite(value, "path coordinate")?);
        }
    }
    Ok(())
}

fn decode_path(bytes: &mut Reader<'_>) -> Result<Path, CommandStreamError> {
    bytes.require(PATH_HEADER_LEN)?;
    let fill_rule = match bytes.u8()? {
        1 => FillRule::NonZero,
        2 => FillRule::EvenOdd,
        _ => return Err(CommandStreamError::InvalidValue("fill rule")),
    };
    if bytes.take(3)? != [0; 3] {
        return Err(CommandStreamError::NonZeroReserved);
    }
    let count = bytes.u32()? as usize;
    if count == 0 {
        return Err(CommandStreamError::InvalidValue("empty path"));
    }
    require_limit("path commands", count, MAX_PATH_COMMANDS)?;
    if count > bytes.remaining() / PATH_ENTRY_LEN {
        return Err(CommandStreamError::Truncated);
    }
    let mut commands = Vec::with_capacity(count);
    let mut current = false;
    for _ in 0..count {
        let kind = bytes.u8()?;
        if bytes.take(3)? != [0; 3] {
            return Err(CommandStreamError::NonZeroReserved);
        }
        let mut values = [0.0; 6];
        for value in &mut values {
            *value = finite(bytes.f32()?, "path coordinate")?;
        }
        let command = match kind {
            1 if values[2..].iter().all(|value| *value == 0.0) => {
                current = true;
                PathCommand::MoveTo {
                    x: values[0],
                    y: values[1],
                }
            }
            2 if current && values[2..].iter().all(|value| *value == 0.0) => PathCommand::LineTo {
                x: values[0],
                y: values[1],
            },
            3 if current && values[4..].iter().all(|value| *value == 0.0) => PathCommand::QuadTo {
                cx: values[0],
                cy: values[1],
                x: values[2],
                y: values[3],
            },
            4 if current => PathCommand::CubicTo {
                c1x: values[0],
                c1y: values[1],
                c2x: values[2],
                c2y: values[3],
                x: values[4],
                y: values[5],
            },
            5 if current && values.iter().all(|value| *value == 0.0) => PathCommand::Close,
            _ => return Err(CommandStreamError::InvalidValue("path command")),
        };
        commands.push(command);
    }
    Ok(Path {
        fill_rule,
        commands,
    })
}

fn normalize_stops(stops: &[GradientStop]) -> Result<Vec<GradientStop>, CommandStreamError> {
    let mut stops = stops.to_vec();
    for stop in &stops {
        unit(stop.offset, "gradient stop")?;
        validate_color(stop.color)?;
    }
    stops.sort_by(|left, right| left.offset.total_cmp(&right.offset));
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
        return Err(CommandStreamError::InvalidValue(
            "coincident gradient stops",
        ));
    }
    Ok(stops)
}

fn normalize_dashes(dashes: &[f32]) -> Result<Vec<f32>, CommandStreamError> {
    let mut result = Vec::with_capacity(dashes.len().saturating_mul(2));
    for dash in dashes {
        result.push(non_negative(*dash, "dash interval")?);
    }
    if result.iter().all(|interval| *interval == 0.0) {
        result.clear();
    } else if result.len() % 2 == 1 {
        result.extend_from_slice(dashes);
    }
    Ok(result)
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

fn encode_color(bytes: &mut Vec<u8>, color: Color) -> Result<(), CommandStreamError> {
    validate_color(color)?;
    for value in [color.red, color.green, color.blue, color.alpha] {
        put_f32(bytes, value);
    }
    Ok(())
}

fn decode_color(bytes: &mut Reader<'_>) -> Result<Color, CommandStreamError> {
    let color = Color::rgba(bytes.f32()?, bytes.f32()?, bytes.f32()?, bytes.f32()?);
    validate_color(color)?;
    Ok(color)
}

fn validate_color(color: Color) -> Result<(), CommandStreamError> {
    for value in [color.red, color.green, color.blue, color.alpha] {
        unit(value, "color component")?;
    }
    Ok(())
}

fn encode_point(bytes: &mut Vec<u8>, point: Point) -> Result<(), CommandStreamError> {
    put_f32(bytes, finite(point.x, "point")?);
    put_f32(bytes, finite(point.y, "point")?);
    Ok(())
}

fn decode_point(bytes: &mut Reader<'_>) -> Result<Point, CommandStreamError> {
    Ok(Point::new(
        finite(bytes.f32()?, "point")?,
        finite(bytes.f32()?, "point")?,
    ))
}

fn encode_rect(bytes: &mut Vec<u8>, rect: Rect) -> Result<(), CommandStreamError> {
    validate_rect(rect)?;
    for value in [rect.x, rect.y, rect.width, rect.height] {
        put_f32(bytes, value);
    }
    Ok(())
}

fn encode_non_empty_rect(
    bytes: &mut Vec<u8>,
    rect: Rect,
    label: &'static str,
) -> Result<(), CommandStreamError> {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Err(CommandStreamError::InvalidValue(label));
    }
    encode_rect(bytes, rect)
}

fn decode_rect(bytes: &mut Reader<'_>) -> Result<Rect, CommandStreamError> {
    let rect = Rect::new(bytes.f32()?, bytes.f32()?, bytes.f32()?, bytes.f32()?);
    validate_rect(rect)?;
    Ok(rect)
}

fn decode_non_empty_rect(
    bytes: &mut Reader<'_>,
    label: &'static str,
) -> Result<Rect, CommandStreamError> {
    let rect = decode_rect(bytes)?;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Err(CommandStreamError::InvalidValue(label));
    }
    Ok(rect)
}

fn validate_rect(rect: Rect) -> Result<(), CommandStreamError> {
    if ![rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|value| value.is_finite())
        || rect.width < 0.0
        || rect.height < 0.0
        || !(rect.x + rect.width).is_finite()
        || !(rect.y + rect.height).is_finite()
    {
        return Err(CommandStreamError::InvalidValue("rectangle"));
    }
    Ok(())
}

fn encode_handle(bytes: &mut Vec<u8>, handle: ResourceHandle) -> Result<(), CommandStreamError> {
    if handle.slot == 0 || handle.generation == 0 {
        return Err(CommandStreamError::InvalidValue("resource handle"));
    }
    put_u32(bytes, handle.slot);
    put_u32(bytes, handle.generation);
    Ok(())
}

fn decode_handle(bytes: &mut Reader<'_>) -> Result<ResourceHandle, CommandStreamError> {
    let handle = ResourceHandle {
        slot: bytes.u32()?,
        generation: bytes.u32()?,
    };
    if handle.slot == 0 || handle.generation == 0 {
        return Err(CommandStreamError::InvalidValue("resource handle"));
    }
    Ok(handle)
}

fn finite(value: f32, label: &'static str) -> Result<f32, CommandStreamError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(CommandStreamError::InvalidValue(label))
}

fn non_negative(value: f32, label: &'static str) -> Result<f32, CommandStreamError> {
    let value = finite(value, label)?;
    (value >= 0.0)
        .then_some(value)
        .ok_or(CommandStreamError::InvalidValue(label))
}

fn positive(value: f32, label: &'static str) -> Result<f32, CommandStreamError> {
    let value = finite(value, label)?;
    (value > 0.0)
        .then_some(value)
        .ok_or(CommandStreamError::InvalidValue(label))
}

fn unit(value: f32, label: &'static str) -> Result<f32, CommandStreamError> {
    let value = finite(value, label)?;
    (0.0..=1.0)
        .contains(&value)
        .then_some(value)
        .ok_or(CommandStreamError::InvalidValue(label))
}

fn require_limit(
    field: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), CommandStreamError> {
    if actual > maximum {
        Err(CommandStreamError::LimitExceeded {
            field,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn require(&self, count: usize) -> Result<(), CommandStreamError> {
        if count <= self.remaining() {
            Ok(())
        } else {
            Err(CommandStreamError::Truncated)
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CommandStreamError> {
        self.require(count)?;
        let start = self.offset;
        self.offset += count;
        Ok(&self.bytes[start..self.offset])
    }

    fn u8(&mut self) -> Result<u8, CommandStreamError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, CommandStreamError> {
        let mut bytes = [0; 2];
        bytes.copy_from_slice(self.take(2)?);
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, CommandStreamError> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn f32(&mut self) -> Result<f32, CommandStreamError> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(f32::from_le_bytes(bytes))
    }

    fn finish(&self) -> Result<(), CommandStreamError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(CommandStreamError::LengthMismatch)
        }
    }
}
