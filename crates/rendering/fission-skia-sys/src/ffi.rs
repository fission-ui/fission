//! Raw declarations for the versioned Fission Skia C ABI.

use std::ffi::{c_char, c_uchar};

pub type Status = u32;
pub type EngineHandle = u64;
pub type ContextHandle = u64;
pub type SurfaceHandle = u64;

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
pub const FEATURE_TEST_SHIM: u64 = 1 << 63;

pub const PATH_MOVE: u32 = 1;
pub const PATH_LINE: u32 = 2;
pub const PATH_QUAD: u32 = 3;
pub const PATH_CUBIC: u32 = 4;
pub const PATH_CLOSE: u32 = 5;

pub const FILL_NON_ZERO: u32 = 1;
pub const FILL_EVEN_ODD: u32 = 2;

pub const FRAME_CLEAR: u32 = 1;
pub const FRAME_FILL_RECT: u32 = 2;
pub const FRAME_FILL_PATH: u32 = 3;

pub const MEMORY_PRESSURE_MODERATE: u32 = 1;
pub const MEMORY_PRESSURE_CRITICAL: u32 = 2;

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
pub struct FrameOp {
    pub struct_size: u32,
    pub kind: u32,
    pub color: Color,
    pub rect: Rect,
    pub path_offset: u32,
    pub path_count: u32,
    pub fill_rule: u32,
    pub reserved: u32,
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
    pub fn fission_skia_context_trim_memory(
        context: ContextHandle,
        pressure: u32,
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
    pub fn fission_skia_surface_destroy(surface: SurfaceHandle, out_error: *mut Error) -> Status;

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
}
