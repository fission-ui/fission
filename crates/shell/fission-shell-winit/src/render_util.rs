use super::*;

pub(super) fn build_font_context() -> FontContext {
    let use_system_fonts = std::env::var("FISSION_USE_SYSTEM_FONTS")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let options = CollectionOptions {
        shared: false,
        system_fonts: use_system_fonts,
    };
    FontContext {
        collection: Collection::new(options),
        source_cache: SourceCache::default(),
    }
}

pub(super) fn register_packaged_fonts(
    font_cx: &Arc<Mutex<FontContext>>,
    fonts: &'static [fission_theme::PackagedFont],
) {
    let mut font_cx = font_cx.lock().unwrap();
    for font in fonts {
        let axes = font
            .axes
            .iter()
            .map(|axis| (Tag::new(&axis.tag), axis.value))
            .collect::<Vec<_>>();
        let style = match font.style {
            fission_theme::PackagedFontStyle::Normal => FontiqueStyle::Normal,
            fission_theme::PackagedFontStyle::Italic => FontiqueStyle::Italic,
            fission_theme::PackagedFontStyle::Oblique => FontiqueStyle::Oblique(None),
        };
        let info_override = FontInfoOverride {
            family_name: Some(font.family),
            style: Some(style),
            weight: Some(FontWeight::new(f32::from(font.weight))),
            axes: (!axes.is_empty()).then_some(axes.as_slice()),
            ..Default::default()
        };
        font_cx
            .collection
            .register_fonts(Blob::new(Arc::new(font.data)), Some(info_override));
    }
}

// Helpers...
pub(super) fn map_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Other(id) => Some(PointerButton::Other(id as u8)),
        _ => None,
    }
}

pub(super) fn clamp_copy_extent_to_texture(
    requested_width: u32,
    requested_height: u32,
    actual_width: u32,
    actual_height: u32,
) -> (u32, u32) {
    (
        requested_width.min(actual_width).max(1),
        requested_height.min(actual_height).max(1),
    )
}

pub(super) fn gpu_screenshot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    texture_width: u32,
    texture_height: u32,
    output_width: u32,
    output_height: u32,
    path: Option<&str>,
) -> fission_test_driver::TestResponse {
    let actual_texture_width = texture.width();
    let actual_texture_height = texture.height();
    let (texture_width, texture_height) = clamp_copy_extent_to_texture(
        texture_width,
        texture_height,
        actual_texture_width,
        actual_texture_height,
    );
    if output_width == 0 || output_height == 0 {
        return fission_test_driver::TestResponse::Error {
            message: "zero-size viewport".into(),
        };
    }

    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = texture_width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
    let buffer_size = (padded_bytes_per_row * texture_height) as u64;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot staging"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("screenshot copy"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(texture_height),
            },
        },
        wgpu::Extent3d {
            width: texture_width,
            height: texture_height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let (tx, rx) = std::sync::mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    let _ = device.poll(wgpu::PollType::Wait);

    match rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return fission_test_driver::TestResponse::Error {
                message: format!("buffer map failed: {:?}", e),
            };
        }
        Err(e) => {
            return fission_test_driver::TestResponse::Error {
                message: format!("buffer map channel error: {}", e),
            };
        }
    }

    let data = staging.slice(..).get_mapped_range();

    // Remove row padding (texture is Rgba8Unorm, no swizzle needed)
    let mut rgba = Vec::with_capacity((texture_width * texture_height * 4) as usize);
    for row in 0..texture_height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + (texture_width * bytes_per_pixel) as usize;
        rgba.extend_from_slice(&data[start..end]);
    }

    drop(data);
    staging.unmap();

    rgba_screenshot(
        rgba,
        texture_width,
        texture_height,
        output_width,
        output_height,
        path,
    )
}

/// Encodes a tightly packed RGBA8 frame using the same output sizing and
/// response contract as the wgpu screenshot path.
pub(super) fn rgba_screenshot(
    rgba: Vec<u8>,
    input_width: u32,
    input_height: u32,
    output_width: u32,
    output_height: u32,
    path: Option<&str>,
) -> fission_test_driver::TestResponse {
    if input_width == 0 || input_height == 0 || output_width == 0 || output_height == 0 {
        return fission_test_driver::TestResponse::Error {
            message: "zero-size viewport".into(),
        };
    }
    let expected_len = usize::try_from(input_width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .and_then(|row_bytes| {
            usize::try_from(input_height)
                .ok()
                .and_then(|height| row_bytes.checked_mul(height))
        });
    if expected_len != Some(rgba.len()) {
        return fission_test_driver::TestResponse::Error {
            message: format!(
                "invalid screenshot RGBA buffer: {} bytes for {}x{}",
                rgba.len(),
                input_width,
                input_height
            ),
        };
    }

    let (rgba, width, height) = if input_width == output_width && input_height == output_height {
        (rgba, input_width, input_height)
    } else if let Some(resized) = downscale_rgba_box(
        &rgba,
        input_width,
        input_height,
        output_width,
        output_height,
    ) {
        (resized, output_width, output_height)
    } else {
        let Some(image) = image::RgbaImage::from_raw(input_width, input_height, rgba) else {
            return fission_test_driver::TestResponse::Error {
                message: "failed to decode screenshot RGBA buffer".into(),
            };
        };
        let resized = image::imageops::resize(
            &image,
            output_width,
            output_height,
            image::imageops::FilterType::Triangle,
        );
        (resized.into_raw(), output_width, output_height)
    };

    let mut png = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        if let Err(e) = encoder.write_image(&rgba, width, height, image::ExtendedColorType::Rgba8) {
            return fission_test_driver::TestResponse::Error {
                message: format!("PNG encode failed: {}", e),
            };
        }
    }

    if let Some(path) = path {
        match std::fs::write(path, &png) {
            Ok(()) => fission_test_driver::TestResponse::Ok {},
            Err(e) => fission_test_driver::TestResponse::Error {
                message: format!("PNG save failed: {}", e),
            },
        }
    } else {
        fission_test_driver::TestResponse::Screenshot {
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            width,
            height,
        }
    }
}

pub(super) fn downscale_rgba_box(
    rgba: &[u8],
    input_width: u32,
    input_height: u32,
    output_width: u32,
    output_height: u32,
) -> Option<Vec<u8>> {
    if output_width == 0
        || output_height == 0
        || input_width % output_width != 0
        || input_height % output_height != 0
    {
        return None;
    }

    let scale_x = input_width / output_width;
    let scale_y = input_height / output_height;
    if scale_x <= 1 && scale_y <= 1 {
        return None;
    }

    let samples_per_pixel = scale_x.checked_mul(scale_y)?;
    let mut out = vec![0u8; (output_width * output_height * 4) as usize];

    for out_y in 0..output_height {
        let src_y0 = out_y * scale_y;
        for out_x in 0..output_width {
            let src_x0 = out_x * scale_x;
            let mut sum = [0u32; 4];
            for dy in 0..scale_y {
                let src_y = src_y0 + dy;
                let row_offset = ((src_y * input_width) * 4) as usize;
                for dx in 0..scale_x {
                    let src_x = src_x0 + dx;
                    let src_index = row_offset + (src_x * 4) as usize;
                    sum[0] += rgba[src_index] as u32;
                    sum[1] += rgba[src_index + 1] as u32;
                    sum[2] += rgba[src_index + 2] as u32;
                    sum[3] += rgba[src_index + 3] as u32;
                }
            }

            let dst_index = (((out_y * output_width) + out_x) * 4) as usize;
            out[dst_index] = (sum[0] / samples_per_pixel) as u8;
            out[dst_index + 1] = (sum[1] / samples_per_pixel) as u8;
            out[dst_index + 2] = (sum[2] / samples_per_pixel) as u8;
            out[dst_index + 3] = (sum[3] / samples_per_pixel) as u8;
        }
    }

    Some(out)
}

pub(super) fn layout_size_to_image_dimensions(size: LayoutSize) -> (u32, u32) {
    let width = size.width.max(1.0).round() as u32;
    let height = size.height.max(1.0).round() as u32;
    (width.max(1), height.max(1))
}

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
    let scale_factor = normalize_scale_factor(window.device_pixel_ratio());
    Some(WindowViewportState {
        physical_size: logical_viewport_to_physical_size(
            LayoutSize::new(width, height),
            scale_factor,
        ),
        scale_factor,
    })
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
