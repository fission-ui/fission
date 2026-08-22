use super::*;

#[cfg(target_arch = "wasm32")]
pub(super) fn create_webgpu_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
    use_indirect_dispatch: bool,
) -> anyhow::Result<MainRenderer> {
    let request = request
        .for_target(RendererTarget::Web)
        .map_err(anyhow::Error::new)?;
    if request.uses_canvaskit() {
        return Err(anyhow::anyhow!(
            "webgpu renderer disabled by renderer request"
        ));
    }
    let renderer = VelloSceneRenderer::new(
        &device_handle.device,
        RendererOptions {
            use_cpu: false,
            use_indirect_dispatch,
            antialiasing_support: AaSupport::all(),
            num_init_threads: None,
            pipeline_cache: None,
        },
    )
    .map_err(|error| anyhow::anyhow!("failed to create webgpu Vello renderer: {error}"))?;
    let texture_compositor =
        TextureLayerCompositor::new(&device_handle.device, wgpu::TextureFormat::Rgba8Unorm);
    Ok(MainRenderer::Vello {
        renderer,
        texture_compositor,
        render_mode: RenderMode::Gpu,
    })
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn webgpu_preflight_dispatch_modes() -> [bool; 2] {
    // Indirect dispatch remains the preferred Vello path. Direct dispatch is
    // the conservative fully-GPU fallback when browser validation or pixel
    // readback rejects indirect command generation.
    [true, false]
}

#[cfg(target_arch = "wasm32")]
pub(super) async fn create_validated_webgpu_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
) -> anyhow::Result<MainRenderer> {
    let device = &device_handle.device;
    let mut failures = Vec::new();

    for use_indirect_dispatch in webgpu_preflight_dispatch_modes() {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        device.push_error_scope(wgpu::ErrorFilter::Internal);

        let result =
            match create_webgpu_main_renderer(device_handle, request, use_indirect_dispatch) {
                Ok(mut renderer) => preflight_webgpu_main_renderer(device_handle, &mut renderer)
                    .await
                    .map(|()| renderer),
                Err(error) => Err(error),
            };

        let mut gpu_errors = Vec::new();
        for stage in ["internal", "out-of-memory", "validation"] {
            if let Some(error) = device.pop_error_scope().await {
                gpu_errors.push(format!("{stage}: {error}"));
            }
        }

        match (result, gpu_errors.is_empty()) {
            (Ok(renderer), true) => {
                log::info!(
                    "Fission WebGPU Vello pixel preflight passed: indirect_dispatch={use_indirect_dispatch}"
                );
                return Ok(renderer);
            }
            (result, _) => {
                let mode = if use_indirect_dispatch {
                    "indirect"
                } else {
                    "direct"
                };
                let mut reasons = Vec::new();
                if let Err(error) = result {
                    reasons.push(error.to_string());
                }
                reasons.extend(gpu_errors);
                let failure = format!("{mode} dispatch: {}", reasons.join("; "));
                log::warn!("Fission WebGPU Vello pixel preflight failed: {failure}");
                failures.push(failure);
            }
        }
    }

    Err(anyhow::anyhow!(
        "webgpu Vello pixel preflight failed: {}",
        failures.join(" | ")
    ))
}

#[cfg(target_arch = "wasm32")]
async fn preflight_webgpu_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    renderer: &mut MainRenderer,
) -> anyhow::Result<()> {
    let MainRenderer::Vello { renderer, .. } = renderer;
    let texture = device_handle
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Fission WebGPU Vello preflight target"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut scene = Scene::new();
    scene.fill(
        vello::peniko::Fill::NonZero,
        vello::kurbo::Affine::IDENTITY,
        vello::peniko::Color::WHITE,
        None,
        &vello::kurbo::Rect::new(0.0, 0.0, 16.0, 16.0),
    );
    let workload_profile = vello::RenderWorkloadProfile {
        target: vello::TargetProfile {
            width_px: 16,
            height_px: 16,
            scale_factor: 1.0,
            dirty_tiles: None,
        },
        coverage: vello::TileCoverageProfile {
            tile_width: 16,
            tile_height: 16,
            target_tiles: 1,
            visible_tiles: 1,
            total_draw_tile_coverage: 1,
            total_path_tile_coverage: 1,
            max_ops_per_tile: 1,
            max_blend_depth: 0,
        },
        scene: vello::SceneComplexityProfile {
            draw_ops: 1,
            path_ops: 1,
            path_points: 4,
            estimated_path_segments: 4,
            ..Default::default()
        },
        ..Default::default()
    };
    renderer
        .render_to_texture_with_workload_profile(
            &device_handle.device,
            &device_handle.queue,
            &scene,
            &view,
            &vello::RenderParams {
                base_color: vello::peniko::Color::BLACK,
                width: 16,
                height: 16,
                antialiasing_method: vello::AaConfig::Area,
            },
            Some(&workload_profile),
        )
        .map_err(|error| {
            anyhow::anyhow!("webgpu profiled Vello preflight submission failed: {error}")
        })?;

    const BYTES_PER_ROW: u32 = 256;
    const HEIGHT: u32 = 16;
    let readback = device_handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Fission WebGPU Vello preflight readback"),
        size: u64::from(BYTES_PER_ROW * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder =
        device_handle
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Fission WebGPU Vello preflight readback encoder"),
            });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(BYTES_PER_ROW),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: 16,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    device_handle.queue.submit(Some(encoder.finish()));

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let resolve = resolve.clone();
        let reject = reject.clone();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| match result {
                Ok(()) => {
                    let _ = resolve.call0(&wasm_bindgen::JsValue::UNDEFINED);
                }
                Err(error) => {
                    let _ = reject.call1(
                        &wasm_bindgen::JsValue::UNDEFINED,
                        &wasm_bindgen::JsValue::from_str(&error.to_string()),
                    );
                }
            });
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| anyhow::anyhow!("webgpu Vello preflight readback failed: {error:?}"))?;

    let mapped = readback.slice(..).get_mapped_range();
    let mut non_black = 0_u32;
    let mut non_transparent = 0_u32;
    for row in mapped
        .chunks_exact(BYTES_PER_ROW as usize)
        .take(HEIGHT as usize)
    {
        for pixel in row[..16 * 4].chunks_exact(4) {
            non_black = non_black.saturating_add(u32::from(pixel[..3] != [0, 0, 0]));
            non_transparent = non_transparent.saturating_add(u32::from(pixel[3] != 0));
        }
    }
    drop(mapped);
    readback.unmap();

    if non_black == 0 || non_transparent == 0 {
        return Err(anyhow::anyhow!(
            "webgpu Vello preflight produced an empty texture (non_black={non_black}, non_transparent={non_transparent})"
        ));
    }
    Ok(())
}
