use crate::{ffi, Error, ErrorKind, Result};

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

#[derive(Debug, Clone, PartialEq)]
pub enum FrameOp {
    Clear(Color),
    FillRect { rect: Rect, color: Color },
    FillPath { path: Path, color: Color },
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
        let mut operations = Vec::with_capacity(self.operations.len());
        let mut path_commands = Vec::new();

        for operation in &self.operations {
            let (kind, color, rect, path_offset, path_count, fill_rule) = match operation {
                FrameOp::Clear(color) => {
                    (ffi::FRAME_CLEAR, raw_color(*color)?, zero_rect(), 0, 0, 0)
                }
                FrameOp::FillRect { rect, color } => (
                    ffi::FRAME_FILL_RECT,
                    raw_color(*color)?,
                    raw_rect(*rect)?,
                    0,
                    0,
                    0,
                ),
                FrameOp::FillPath { path, color } => {
                    if path.commands.is_empty() {
                        return Err(invalid("fill path must contain at least one command"));
                    }
                    let offset = u32::try_from(path_commands.len())
                        .map_err(|_| invalid("path command offset exceeds the ABI limit"))?;
                    for command in &path.commands {
                        path_commands.push(raw_path_command(*command)?);
                    }
                    let count = u32::try_from(path.commands.len())
                        .map_err(|_| invalid("path command count exceeds the ABI limit"))?;
                    let fill_rule = match path.fill_rule {
                        FillRule::NonZero => ffi::FILL_NON_ZERO,
                        FillRule::EvenOdd => ffi::FILL_EVEN_ODD,
                    };
                    (
                        ffi::FRAME_FILL_PATH,
                        raw_color(*color)?,
                        zero_rect(),
                        offset,
                        count,
                        fill_rule,
                    )
                }
            };

            operations.push(ffi::FrameOp {
                struct_size: std::mem::size_of::<ffi::FrameOp>() as u32,
                kind,
                color,
                rect,
                path_offset,
                path_count,
                fill_rule,
                reserved: 0,
            });
        }

        Ok(EncodedFrame {
            operations,
            path_commands,
        })
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
}

impl EncodedFrame {
    pub(crate) fn as_raw(&self) -> ffi::Frame {
        ffi::Frame {
            struct_size: std::mem::size_of::<ffi::Frame>() as u32,
            reserved: 0,
            operations: self.operations.as_ptr(),
            operation_count: self.operations.len(),
            path_commands: self.path_commands.as_ptr(),
            path_command_count: self.path_commands.len(),
        }
    }
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

fn zero_rect() -> ffi::Rect {
    ffi::Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }
}

fn invalid(message: &str) -> Error {
    Error::local(ErrorKind::InvalidArgument, "Frame::encode", message)
}
