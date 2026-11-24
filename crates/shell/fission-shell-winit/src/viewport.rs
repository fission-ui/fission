use super::{RenderContext, RenderSurface, WindowViewportState};
use fission_layout::LayoutSize;
use winit::dpi::PhysicalSize;

pub(super) fn normalize_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

#[cfg(target_os = "ios")]
pub(super) fn ios_effective_scale_factor(reported_scale_factor: f64) -> f64 {
    std::env::var("FISSION_IOS_SCALE_FACTOR")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or_else(|| {
            if reported_scale_factor >= 2.0 {
                reported_scale_factor
            } else {
                3.0
            }
        })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn web_browser_viewport_state() -> Option<WindowViewportState> {
    let window = web_sys::window()?;
    let width = window.inner_width().ok()?.as_f64()? as f32;
    let height = window.inner_height().ok()?.as_f64()? as f32;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let scale_factor =
        web_effective_render_scale(LayoutSize::new(width, height), window.device_pixel_ratio());
    Some(WindowViewportState {
        physical_size: logical_viewport_to_physical_size(
            LayoutSize::new(width, height),
            scale_factor,
        ),
        scale_factor,
    })
}

#[cfg(any(test, target_arch = "wasm32"))]
pub(super) fn web_effective_render_scale(size: LayoutSize, device_pixel_ratio: f64) -> f64 {
    // Preserve CSS-pixel layout while bounding the WebGPU backing store. Large
    // high-DPI displays can otherwise request textures and Vello buffers far
    // beyond practical browser GPU budgets (for example 6880x2880 for a
    // 3440x1440 viewport at DPR 2), leaving the canvas blank after allocation.
    const MAX_WEB_RENDER_PIXELS: f64 = 8_388_608.0;

    let device_pixel_ratio = normalize_scale_factor(device_pixel_ratio);
    let logical_pixels = (size.width.max(1.0) as f64) * (size.height.max(1.0) as f64);
    let budgeted_scale = (MAX_WEB_RENDER_PIXELS / logical_pixels).sqrt();
    device_pixel_ratio.min(budgeted_scale.max(1.0))
}

pub(super) fn physical_size_to_layout_size(
    size: PhysicalSize<u32>,
    scale_factor: f64,
) -> LayoutSize {
    let scale_factor = normalize_scale_factor(scale_factor);
    LayoutSize {
        width: (size.width as f64 / scale_factor) as f32,
        height: (size.height as f64 / scale_factor) as f32,
    }
}

pub(super) fn logical_viewport_to_render_target_size(
    size: LayoutSize,
    scale_factor: f64,
) -> (u32, u32) {
    let scale_factor = normalize_scale_factor(scale_factor);
    let width = (size.width.max(1.0) as f64 * scale_factor).ceil() as u32;
    let height = (size.height.max(1.0) as f64 * scale_factor).ceil() as u32;
    (width.max(1), height.max(1))
}

pub(super) fn logical_viewport_to_physical_size(
    size: LayoutSize,
    scale_factor: f64,
) -> PhysicalSize<u32> {
    let (width, height) = logical_viewport_to_render_target_size(size, scale_factor);
    PhysicalSize::new(width, height)
}

pub(super) fn recreate_target_texture(
    surface: &mut RenderSurface,
    render_cx: &RenderContext,
    width: u32,
    height: u32,
) {
    let device = &render_cx.devices[surface.dev_id].device;
    let size = wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    };
    let new_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fission_target_with_copy"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, // Must match Vello's internal format
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let new_view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
    surface.target_texture = new_texture;
    surface.target_view = new_view;
    surface.blitter.update_source(device, &surface.target_view);
}

pub(super) fn sync_tracked_target_texture_size_to_surface(
    target_texture_size: &mut (u32, u32),
    surface_size: PhysicalSize<u32>,
) {
    *target_texture_size = (surface_size.width.max(1), surface_size.height.max(1));
}

#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
pub(super) fn native_window_size_for_logical_viewport(
    size: LayoutSize,
) -> winit::dpi::LogicalSize<f64> {
    winit::dpi::LogicalSize::new(size.width as f64, size.height as f64)
}
