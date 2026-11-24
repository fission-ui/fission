use base64::Engine;
use fission_layout::LayoutSize;

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
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

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

    rgba_screenshot_response(
        rgba,
        texture_width,
        texture_height,
        output_width,
        output_height,
        path,
    )
}

/// Answers a screenshot from a software rendering of the retained scene when the presented surface
/// cannot be read back reliably.
pub(super) fn covered_window_screenshot<E: std::fmt::Display>(
    rendered: Option<Result<Vec<u8>, E>>,
    render_size: (u32, u32),
    output_size: (u32, u32),
    path: Option<&str>,
) -> fission_test_driver::TestResponse {
    match rendered {
        None => fission_test_driver::TestResponse::Error {
            message: "nothing has been rendered yet".into(),
        },
        Some(Err(error)) => fission_test_driver::TestResponse::Error {
            message: format!("software screenshot failed: {error}"),
        },
        Some(Ok(_)) if output_size.0 == 0 || output_size.1 == 0 => {
            fission_test_driver::TestResponse::Error {
                message: "zero-size viewport".into(),
            }
        }
        Some(Ok(rgba)) => rgba_screenshot_response(
            rgba,
            render_size.0,
            render_size.1,
            output_size.0,
            output_size.1,
            path,
        ),
    }
}

/// Scales captured RGBA pixels to the output size, then writes them to `path` as a PNG or returns
/// them in the response.
pub(super) fn rgba_screenshot_response(
    rgba: Vec<u8>,
    texture_width: u32,
    texture_height: u32,
    output_width: u32,
    output_height: u32,
    path: Option<&str>,
) -> fission_test_driver::TestResponse {
    let (rgba, width, height) = if texture_width == output_width && texture_height == output_height
    {
        (rgba, texture_width, texture_height)
    } else if let Some(resized) = downscale_rgba_box(
        &rgba,
        texture_width,
        texture_height,
        output_width,
        output_height,
    ) {
        (resized, output_width, output_height)
    } else {
        let Some(image) = image::RgbaImage::from_raw(texture_width, texture_height, rgba) else {
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
