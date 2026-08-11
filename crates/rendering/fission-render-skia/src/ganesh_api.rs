use fission_render::surface::{MemoryPressure, PhysicalSize};
use fission_skia_sys::{NativeWindow, RecordedPicture};

use crate::api::{ApiError, RasterFrame, RasterRect, SkiaPictureRecorder};

/// Injectable boundary around Skia's native Ganesh presentation handles.
///
/// Production uses [`crate::ganesh_native::NativeGaneshApi`]. Tests substitute
/// inert handles so target lowering and lifecycle ordering can be exercised
/// without constructing a Vulkan device or native swapchain.
pub(crate) trait GaneshApi {
    type Engine;
    type Context;
    type Surface;

    fn create_engine(&self) -> Result<Self::Engine, ApiError>;
    fn create_context(
        &self,
        engine: &Self::Engine,
        compatible_window: NativeWindow,
    ) -> Result<Self::Context, ApiError>;
    fn create_surface(
        &self,
        context: &Self::Context,
        window: NativeWindow,
        size: PhysicalSize,
    ) -> Result<Self::Surface, ApiError>;
    fn resize_surface(
        &self,
        surface: &mut Self::Surface,
        window: NativeWindow,
        size: PhysicalSize,
    ) -> Result<(), ApiError>;
    fn record_picture(
        &self,
        _bounds: RasterRect,
        _frame: &RasterFrame,
    ) -> Result<Option<RecordedPicture>, ApiError> {
        Ok(None)
    }
    fn execute_frame(
        &self,
        surface: &mut Self::Surface,
        frame: &RasterFrame,
    ) -> Result<(), ApiError>;
    fn present(&self, surface: &mut Self::Surface) -> Result<(), ApiError>;
    fn trim_memory(
        &self,
        context: &Self::Context,
        pressure: MemoryPressure,
    ) -> Result<(), ApiError>;
}

pub(crate) struct GaneshPictureRecorder<'api, A>(&'api A);

impl<'api, A> GaneshPictureRecorder<'api, A> {
    pub(crate) const fn new(api: &'api A) -> Self {
        Self(api)
    }
}

impl<A: GaneshApi> SkiaPictureRecorder for GaneshPictureRecorder<'_, A> {
    fn record_picture(
        &self,
        bounds: RasterRect,
        frame: &RasterFrame,
    ) -> Result<Option<RecordedPicture>, ApiError> {
        self.0.record_picture(bounds, frame)
    }
}
