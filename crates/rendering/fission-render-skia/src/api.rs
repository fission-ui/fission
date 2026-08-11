use fission_render::surface::{MemoryPressure, PhysicalSize};

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

impl RasterColor {
    pub const TRANSPARENT: Self = Self {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RasterRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RasterCommand {
    Clear(RasterColor),
    FillRect {
        rect: RasterRect,
        color: RasterColor,
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
