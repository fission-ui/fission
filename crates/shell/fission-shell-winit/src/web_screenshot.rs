use super::capture::{clamp_copy_extent_to_texture, rgba_screenshot_response};
use super::record_web_test_diagnostic;
use std::cell::Cell;
use std::sync::mpsc;

const READBACK_TIMEOUT_MS: i32 = 5_000;

pub(super) fn gpu_screenshot_async(
    instance: &wgpu::Instance,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    texture_width: u32,
    texture_height: u32,
    output_width: u32,
    output_height: u32,
    response_tx: mpsc::Sender<fission_test_driver::TestResponse>,
) {
    let (texture_width, texture_height) = clamp_copy_extent_to_texture(
        texture_width,
        texture_height,
        texture.width(),
        texture.height(),
    );
    if output_width == 0 || output_height == 0 {
        let _ = response_tx.send(fission_test_driver::TestResponse::Error {
            message: "zero-size viewport".into(),
        });
        return;
    }
    let bytes_per_pixel = 4_u32;
    let unpadded_bytes_per_row = texture_width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
    let buffer_size = (padded_bytes_per_row * texture_height) as u64;
    let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let out_of_memory_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("WebGPU screenshot staging"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("WebGPU screenshot copy"),
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
    // Request this staging buffer's map after submitting its copy. Mapping
    // already waits for GPU work that uses the buffer, not the whole queue.
    let mapped =
        js_sys::Promise::new(&mut |resolve, reject| {
            let resolve = resolve.clone();
            let reject = reject.clone();
            encoder.map_buffer_on_submit(&staging, wgpu::MapMode::Read, .., move |result| {
                match result {
                    Ok(()) => {
                        let _ = resolve.call0(&wasm_bindgen::JsValue::UNDEFINED);
                    }
                    Err(error) => {
                        let _ = reject.call1(
                            &wasm_bindgen::JsValue::UNDEFINED,
                            &wasm_bindgen::JsValue::from_str(&error.to_string()),
                        );
                    }
                }
            });
        });
    // Attach both Promise handlers before submission can report a map error.
    let mapped = wasm_bindgen_futures::JsFuture::from(mapped);
    queue.submit(Some(encoder.finish()));
    let out_of_memory_error = out_of_memory_scope.pop();
    let validation_error = validation_scope.pop();
    let instance_keepalive = instance.clone();
    let device_keepalive = device.clone();
    let queue_keepalive = queue.clone();

    wasm_bindgen_futures::spawn_local(async move {
        let _keepalive = (instance_keepalive, device_keepalive, queue_keepalive);
        record_web_test_diagnostic(486, texture_height);
        let phase = Cell::new("GPU error scopes");
        let readback_phase = &phase;
        let readback = async move {
            // The map is already in flight while these two scopes resolve.
            let (memory_error, validation_error) =
                futures_util::future::join(out_of_memory_error, validation_error).await;
            let gpu_error = memory_error.or(validation_error);
            if let Some(error) = gpu_error {
                return fission_test_driver::TestResponse::Error {
                    message: format!(
                        "renderer_screenshot_unavailable: WebGPU screenshot readback GPU error for {texture_width}x{texture_height} ({buffer_size} bytes, {padded_bytes_per_row} bytes per row): {error}"
                    ),
                };
            }

            readback_phase.set("staging buffer map");
            record_web_test_diagnostic(487, texture_height);
            match mapped.await {
                Ok(_) => {
                    record_web_test_diagnostic(488, texture_height);
                    let data = staging.slice(..).get_mapped_range();
                    let mut rgba =
                        Vec::with_capacity((texture_width * texture_height * 4) as usize);
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
                        None,
                    )
                }
                Err(error) => fission_test_driver::TestResponse::Error {
                    message: format!(
                        "renderer_screenshot_unavailable: WebGPU buffer map failed for {texture_width}x{texture_height} readback ({buffer_size} bytes, {padded_bytes_per_row} bytes per row): {error:?}"
                    ),
                },
            }
        };
        let timeout = async {
            let _ =
                wasm_bindgen_futures::JsFuture::from(browser_timeout_promise(READBACK_TIMEOUT_MS))
                    .await;
        };
        let response = match futures_util::future::select(Box::pin(readback), Box::pin(timeout))
            .await
        {
            futures_util::future::Either::Left((response, _timeout)) => response,
            futures_util::future::Either::Right(((), _readback)) => {
                record_web_test_diagnostic(489, texture_height);
                fission_test_driver::TestResponse::Error {
                    message: format!(
                        "renderer_screenshot_unavailable: WebGPU readback for {texture_width}x{texture_height} timed out after {READBACK_TIMEOUT_MS}ms awaiting {}; no replacement image was generated",
                        phase.get()
                    ),
                }
            }
        };
        record_web_test_diagnostic(490, texture_height);
        let _ = response_tx.send(response);
    });
}

#[cfg(target_arch = "wasm32")]
fn browser_timeout_promise(timeout_ms: i32) -> js_sys::Promise {
    use wasm_bindgen::JsCast;

    js_sys::Promise::new(&mut |resolve, _reject| {
        let Some(window) = web_sys::window() else {
            let _ = resolve.call0(&wasm_bindgen::JsValue::UNDEFINED);
            return;
        };
        let resolve_timer = resolve.clone();
        let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
            let _ = resolve_timer.call0(&wasm_bindgen::JsValue::UNDEFINED);
        });
        if window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref(),
                timeout_ms,
            )
            .is_err()
        {
            let _ = resolve.call0(&wasm_bindgen::JsValue::UNDEFINED);
        }
    })
}
