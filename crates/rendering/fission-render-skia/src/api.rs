use std::sync::Arc;

use fission_ir::op::ImageAlignment;
use fission_render::resource::ResourceId;
use fission_render::surface::{MemoryPressure, PhysicalSize};
use fission_render::ImageFit;
use fission_skia_sys::{DecodedImage, ParagraphDrawData, RecordedPicture, SvgDocument};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApiErrorKind {
    InvalidArgument,
    Unsupported,
    WrongThread,
    SurfaceLost,
    DeviceLost,
    OutOfMemory,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiError {
    pub kind: ApiErrorKind,
    pub code: String,
    pub operation: String,
    pub message: String,
}

impl ApiError {
    pub(crate) fn new(
        kind: ApiErrorKind,
        code: impl Into<String>,
        operation: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            operation: operation.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RasterColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RasterRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RasterPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RasterAffine {
    pub scale_x: f32,
    pub skew_x: f32,
    pub translate_x: f32,
    pub skew_y: f32,
    pub scale_y: f32,
    pub translate_y: f32,
}

impl RasterAffine {
    pub(crate) const IDENTITY: Self = Self {
        scale_x: 1.0,
        skew_x: 0.0,
        translate_x: 0.0,
        skew_y: 0.0,
        scale_y: 1.0,
        translate_y: 0.0,
    };

    pub(crate) const fn translation(x: f32, y: f32) -> Self {
        Self {
            translate_x: x,
            translate_y: y,
            ..Self::IDENTITY
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RasterGradientStop {
    pub offset: f32,
    pub color: RasterColor,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RasterPaint {
    Solid(RasterColor),
    LinearGradient {
        start: RasterPoint,
        end: RasterPoint,
        stops: Vec<RasterGradientStop>,
    },
    RadialGradient {
        center: RasterPoint,
        radius: f32,
        stops: Vec<RasterGradientStop>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RasterLineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RasterLineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RasterStroke {
    pub paint: RasterPaint,
    pub width: f32,
    pub dash_array: Option<Vec<f32>>,
    pub line_cap: RasterLineCap,
    pub line_join: RasterLineJoin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RasterBoxShadow {
    pub color: RasterColor,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub offset: RasterPoint,
    pub inset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RasterFillRule {
    NonZero,
    /// Used once SVG fill-rule lowering reaches the Skia compiler.
    #[allow(dead_code)]
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RasterPathCommand {
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
pub(crate) struct RasterPath {
    pub fill_rule: RasterFillRule,
    pub commands: Vec<RasterPathCommand>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RasterCommand {
    Clear(RasterColor),
    Save,
    Restore,
    /// Begins an isolated group. A later `Restore` composites the complete
    /// group once with `alpha` and restores the prior canvas state.
    OpacityLayer {
        bounds: RasterRect,
        alpha: f32,
    },
    /// Atomically filters content already painted behind `bounds`. Unlike an
    /// opacity layer, this command does not open a save scope.
    BackdropBlur {
        bounds: RasterRect,
        corner_radius: f32,
        sigma: f32,
    },
    ClipRect {
        rect: RasterRect,
    },
    ClipRoundedRect {
        rect: RasterRect,
        radius: f32,
    },
    ConcatAffine(RasterAffine),
    FillRect {
        rect: RasterRect,
        radius: f32,
        paint: RasterPaint,
    },
    StrokeRect {
        rect: RasterRect,
        radius: f32,
        stroke: RasterStroke,
    },
    FillPath {
        path: RasterPath,
        paint: RasterPaint,
    },
    StrokePath {
        path: RasterPath,
        stroke: RasterStroke,
    },
    BoxShadow {
        rect: RasterRect,
        radius: f32,
        shadow: RasterBoxShadow,
    },
    /// Paint data retained from the exact paragraph layout that supplied the
    /// frame's geometry. The resource is pinned through native execution.
    DrawParagraph {
        data: Arc<ParagraphDrawData>,
        origin: RasterPoint,
        scale_factor: f32,
    },
    /// Pins one immutable decoded image through native frame execution.
    DrawImage {
        image: DecodedImage,
        source: RasterRect,
        destination: RasterRect,
    },
    /// Defers image decode and intrinsic-size placement to the browser-owned
    /// CanvasKit resource table. The logical Fission resource identifier is
    /// resolved to a generational wire handle only after the matching resource
    /// plan has been prepared for this frame.
    DrawImageResource {
        resource_id: ResourceId,
        target: RasterRect,
        fit: ImageFit,
        alignment: ImageAlignment,
    },
    /// Pins one immutable parsed SVG document through native frame execution.
    /// Fission paint overrides are lowered to ordinary path commands instead.
    DrawSvg {
        document: SvgDocument,
        destination: RasterRect,
    },
    /// Replays one immutable picture through the live canvas state. The
    /// handle remains pinned through native frame execution.
    DrawPicture {
        picture: RecordedPicture,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RasterFrame {
    pub commands: Vec<RasterCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRegion {
    pub(crate) const fn full(size: PhysicalSize) -> Self {
        Self {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }
    }

    pub(crate) const fn size(self) -> PhysicalSize {
        PhysicalSize::new(self.width, self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiReadback {
    pub size: PhysicalSize,
    pub row_bytes: usize,
    pub pixels: Vec<u8>,
}

/// Injectable ownership boundary around the Fission-owned native ABI.
///
/// Production uses `NativeSkiaApi`; tests substitute inert handles so
/// lifecycle and error semantics do not depend on a linked Skia build. Handle
/// destruction remains ordinary Rust RAII in both cases.
pub(crate) trait SkiaApi {
    type Engine;
    type Context;
    type Surface;

    fn create_engine(&self) -> Result<Self::Engine, ApiError>;
    fn create_raster_context(&self, engine: &Self::Engine) -> Result<Self::Context, ApiError>;
    fn create_raster_surface(
        &self,
        context: &Self::Context,
        size: PhysicalSize,
    ) -> Result<Self::Surface, ApiError>;
    /// Records an immutable command fragment when this API supports native
    /// pictures. Test and alternative APIs may return `Ok(None)` to preserve
    /// the uncached command path exactly.
    fn record_picture(
        &self,
        _bounds: RasterRect,
        _frame: &RasterFrame,
    ) -> Result<Option<RecordedPicture>, ApiError> {
        Ok(None)
    }
    fn execute_frame(
        &self,
        context: &mut Self::Context,
        surface: &mut Self::Surface,
        frame: &RasterFrame,
    ) -> Result<(), ApiError>;
    fn read_pixels_rgba8888(
        &self,
        context: &mut Self::Context,
        surface: &mut Self::Surface,
        region: PixelRegion,
    ) -> Result<ApiReadback, ApiError>;
    fn trim_memory(
        &self,
        context: &mut Self::Context,
        pressure: MemoryPressure,
    ) -> Result<(), ApiError>;
}

/// Object-safe recording view used by the compiler without coupling it to an
/// API's engine/context/surface associated types.
pub(crate) trait SkiaPictureRecorder {
    fn record_picture(
        &self,
        bounds: RasterRect,
        frame: &RasterFrame,
    ) -> Result<Option<RecordedPicture>, ApiError>;
}

impl<T: SkiaApi> SkiaPictureRecorder for T {
    fn record_picture(
        &self,
        bounds: RasterRect,
        frame: &RasterFrame,
    ) -> Result<Option<RecordedPicture>, ApiError> {
        SkiaApi::record_picture(self, bounds, frame)
    }
}
