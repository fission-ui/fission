//! Raw declarations for the versioned Fission Skia C ABI.

use std::ffi::{c_char, c_uchar};

mod paragraph;
pub use paragraph::*;

pub type Status = u32;
pub type EngineHandle = u64;
pub type ContextHandle = u64;
pub type SurfaceHandle = u64;
pub type ImageHandle = u64;
pub type SvgDocumentHandle = u64;
pub type PictureHandle = u64;

pub const MAX_SVG_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

pub const STATUS_OK: Status = 0;
pub const STATUS_INVALID_ARGUMENT: Status = 1;
pub const STATUS_INVALID_HANDLE: Status = 2;
pub const STATUS_INVALID_STATE: Status = 3;
pub const STATUS_UNSUPPORTED: Status = 4;
pub const STATUS_WRONG_THREAD: Status = 5;
pub const STATUS_SURFACE_LOST: Status = 6;
pub const STATUS_CONTEXT_LOST: Status = 7;
pub const STATUS_DEVICE_LOST: Status = 8;
pub const STATUS_OUT_OF_MEMORY: Status = 9;
pub const STATUS_ABI_MISMATCH: Status = 10;
pub const STATUS_INTERNAL: Status = 11;

pub const FEATURE_RASTER_SURFACE: u64 = 1 << 0;
pub const FEATURE_BASIC_FRAME: u64 = 1 << 1;
pub const FEATURE_RGBA_READBACK: u64 = 1 << 2;
pub const FEATURE_STRUCTURED_ERRORS: u64 = 1 << 3;
pub const FEATURE_THREAD_AFFINITY: u64 = 1 << 4;
pub const FEATURE_MEMORY_PRESSURE: u64 = 1 << 5;
pub const FEATURE_PAINT_STATE: u64 = 1 << 6;
pub const FEATURE_PARAGRAPH: u64 = 1 << 7;
pub const FEATURE_OPACITY_LAYER: u64 = 1 << 8;
pub const FEATURE_IMAGE_DECODE: u64 = 1 << 9;
pub const FEATURE_BACKDROP_BLUR: u64 = 1 << 10;
pub const FEATURE_SVG_DOCUMENT: u64 = 1 << 11;
pub const FEATURE_RETAINED_PICTURE: u64 = 1 << 12;
pub const FEATURE_GANESH: u64 = 1 << 13;
pub const FEATURE_VULKAN: u64 = 1 << 14;
pub const FEATURE_NATIVE_PRESENTATION: u64 = 1 << 15;
pub const FEATURE_METAL: u64 = 1 << 16;
pub const FEATURE_TEST_SHIM: u64 = 1 << 63;

pub const PATH_MOVE: u32 = 1;
pub const PATH_LINE: u32 = 2;
pub const PATH_QUAD: u32 = 3;
pub const PATH_CUBIC: u32 = 4;
pub const PATH_CLOSE: u32 = 5;

pub const FILL_NON_ZERO: u32 = 1;
pub const FILL_EVEN_ODD: u32 = 2;

pub const PAINT_SOLID: u32 = 1;
pub const PAINT_LINEAR_GRADIENT: u32 = 2;
pub const PAINT_RADIAL_GRADIENT: u32 = 3;

pub const LINE_CAP_BUTT: u32 = 1;
pub const LINE_CAP_ROUND: u32 = 2;
pub const LINE_CAP_SQUARE: u32 = 3;

pub const LINE_JOIN_MITER: u32 = 1;
pub const LINE_JOIN_ROUND: u32 = 2;
pub const LINE_JOIN_BEVEL: u32 = 3;

pub const FRAME_CLEAR: u32 = 1;
pub const FRAME_SAVE: u32 = 2;
pub const FRAME_RESTORE: u32 = 3;
pub const FRAME_CLIP_RECT: u32 = 4;
pub const FRAME_CLIP_ROUNDED_RECT: u32 = 5;
pub const FRAME_CONCAT_AFFINE: u32 = 6;
pub const FRAME_FILL_RECT: u32 = 7;
pub const FRAME_STROKE_RECT: u32 = 8;
pub const FRAME_FILL_PATH: u32 = 9;
pub const FRAME_STROKE_PATH: u32 = 10;
pub const FRAME_BOX_SHADOW: u32 = 11;
pub const FRAME_DRAW_PARAGRAPH: u32 = 12;
pub const FRAME_OPACITY_LAYER: u32 = 13;
pub const FRAME_DRAW_IMAGE: u32 = 14;
pub const FRAME_BACKDROP_BLUR: u32 = 15;
pub const FRAME_DRAW_SVG: u32 = 16;
pub const FRAME_DRAW_PICTURE: u32 = 17;

pub const IMAGE_SAMPLING_NEAREST: u32 = 1;
pub const IMAGE_SAMPLING_LINEAR: u32 = 2;

pub const MEMORY_PRESSURE_MODERATE: u32 = 1;
pub const MEMORY_PRESSURE_CRITICAL: u32 = 2;

pub const NATIVE_WINDOW_WAYLAND: u32 = 1;
pub const NATIVE_WINDOW_XLIB: u32 = 2;
pub const NATIVE_WINDOW_XCB: u32 = 3;
pub const NATIVE_WINDOW_APPKIT: u32 = 4;
pub const NATIVE_WINDOW_UIKIT: u32 = 5;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AbiInfo {
    pub struct_size: u32,
    pub abi_version: u32,
    pub feature_bits: u64,
    pub skia_revision: [c_char; 41],
    pub build_profile: [c_char; 32],
}

impl Default for AbiInfo {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: 0,
            feature_bits: 0,
            skia_revision: [0; 41],
            build_profile: [0; 32],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Error {
    pub struct_size: u32,
    pub code: u32,
    pub sequence: u64,
    pub operation: [c_char; 64],
    pub message: [c_char; 512],
}

impl Default for Error {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            code: STATUS_OK,
            sequence: 0,
            operation: [0; 64],
            message: [0; 512],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EngineConfig {
    pub struct_size: u32,
    pub expected_abi_version: u32,
    pub required_feature_bits: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeWindow {
    pub struct_size: u32,
    pub kind: u32,
    pub display: u64,
    pub window: u64,
    pub visual_id: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuCacheUsage {
    pub struct_size: u32,
    pub reserved: u32,
    pub resource_count: u64,
    pub resource_bytes: u64,
}

impl Default for GpuCacheUsage {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            reserved: 0,
            resource_count: 0,
            resource_bytes: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Affine {
    pub scale_x: f32,
    pub skew_x: f32,
    pub translate_x: f32,
    pub skew_y: f32,
    pub scale_y: f32,
    pub translate_y: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PathCommand {
    pub struct_size: u32,
    pub verb: u32,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub x3: f32,
    pub y3: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Paint {
    pub struct_size: u32,
    pub kind: u32,
    pub color: Color,
    pub start: Point,
    pub end: Point,
    pub radius: f32,
    pub stop_offset: u32,
    pub stop_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stroke {
    pub struct_size: u32,
    pub width: f32,
    pub line_cap: u32,
    pub line_join: u32,
    pub dash_offset: u32,
    pub dash_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BoxShadow {
    pub struct_size: u32,
    pub inset: u32,
    pub color: Color,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ImageDraw {
    pub struct_size: u32,
    pub sampling: u32,
    pub image: ImageHandle,
    pub source: Rect,
    pub destination: Rect,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ImageInfo {
    pub struct_size: u32,
    pub width: u32,
    pub height: u32,
    pub reserved: u32,
    pub approximate_decoded_bytes: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SvgDraw {
    pub struct_size: u32,
    pub reserved: u32,
    pub document: SvgDocumentHandle,
    pub destination: Rect,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PictureDraw {
    pub struct_size: u32,
    pub reserved: u32,
    pub picture: PictureHandle,
}

impl Default for ImageInfo {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            width: 0,
            height: 0,
            reserved: 0,
            approximate_decoded_bytes: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FrameOp {
    pub struct_size: u32,
    pub kind: u32,
    pub paint: Paint,
    pub stroke: Stroke,
    pub shadow: BoxShadow,
    pub rect: Rect,
    pub affine: Affine,
    pub radius: f32,
    pub path_offset: u32,
    pub path_count: u32,
    pub fill_rule: u32,
    pub opacity: f32,
    pub sigma: f32,
    pub image: ImageDraw,
    pub svg: SvgDraw,
    pub picture: PictureDraw,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub struct_size: u32,
    pub reserved: u32,
    pub operations: *const FrameOp,
    pub operation_count: usize,
    pub path_commands: *const PathCommand,
    pub path_command_count: usize,
    pub gradient_stops: *const GradientStop,
    pub gradient_stop_count: usize,
    pub dash_intervals: *const f32,
    pub dash_interval_count: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

extern "C" {
    pub fn fission_skia_get_abi_info(out_info: *mut AbiInfo, out_error: *mut Error) -> Status;
    pub fn fission_skia_engine_create(
        config: *const EngineConfig,
        out_engine: *mut EngineHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_engine_destroy(engine: EngineHandle, out_error: *mut Error) -> Status;
    pub fn fission_skia_context_create_raster(
        engine: EngineHandle,
        out_context: *mut ContextHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_context_create_ganesh(
        engine: EngineHandle,
        compatible_window: *const NativeWindow,
        out_context: *mut ContextHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_context_trim_memory(
        context: ContextHandle,
        pressure: u32,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_context_set_resource_cache_limit(
        context: ContextHandle,
        limit_bytes: u64,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_context_get_resource_cache_usage(
        context: ContextHandle,
        out_usage: *mut GpuCacheUsage,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_context_destroy(context: ContextHandle, out_error: *mut Error) -> Status;
    pub fn fission_skia_surface_create_raster(
        context: ContextHandle,
        width: u32,
        height: u32,
        out_surface: *mut SurfaceHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_surface_create_ganesh(
        context: ContextHandle,
        window: *const NativeWindow,
        width: u32,
        height: u32,
        out_surface: *mut SurfaceHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_surface_resize_ganesh(
        surface: SurfaceHandle,
        window: *const NativeWindow,
        width: u32,
        height: u32,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_surface_execute_frame(
        surface: SurfaceHandle,
        frame: *const Frame,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_surface_read_pixels_rgba8888(
        surface: SurfaceHandle,
        source_rect: *const PixelRect,
        destination: *mut c_uchar,
        destination_length: usize,
        destination_row_bytes: usize,
        out_required_length: *mut usize,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_surface_present(surface: SurfaceHandle, out_error: *mut Error) -> Status;
    pub fn fission_skia_surface_destroy(surface: SurfaceHandle, out_error: *mut Error) -> Status;
    pub fn fission_skia_image_decode_encoded(
        encoded: *const c_uchar,
        encoded_length: usize,
        max_decoded_bytes: usize,
        out_image: *mut ImageHandle,
        out_info: *mut ImageInfo,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_image_get_info(
        image: ImageHandle,
        out_info: *mut ImageInfo,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_image_destroy(image: ImageHandle, out_error: *mut Error) -> Status;
    pub fn fission_skia_svg_document_parse(
        svg: *const c_uchar,
        svg_length: usize,
        out_document: *mut SvgDocumentHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_svg_document_destroy(
        document: SvgDocumentHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_picture_record(
        cull_bounds: *const Rect,
        frame: *const Frame,
        out_picture: *mut PictureHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_picture_destroy(picture: PictureHandle, out_error: *mut Error) -> Status;

    #[cfg(feature = "test-shim")]
    pub fn fission_skia_test_live_counts(
        out_counts: *mut TestCounts,
        out_error: *mut Error,
    ) -> Status;
}

#[cfg(feature = "test-shim")]
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TestCounts {
    pub engines: u64,
    pub contexts: u64,
    pub surfaces: u64,
    pub images: u64,
    pub svg_documents: u64,
    pub pictures: u64,
}
