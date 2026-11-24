use super::SurfaceBlitter;
use wgpu::{Device, Extent3d, Queue, Texture, TextureFormat};

fn texture(device: &Device, width: u32, height: u32, format: TextureFormat) -> Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("surface blitter regression target"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn upload(queue: &Queue, texture: &Texture, bytes: &[u8]) {
    queue.write_texture(
        texture.as_image_copy(),
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texture.width() * 4),
            rows_per_image: Some(texture.height()),
        },
        texture.size(),
    );
}

fn read_rgba(device: &Device, queue: &Queue, texture: &Texture) -> Vec<u8> {
    let row = (texture.width() * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("blitter regression readback"),
        size: u64::from(row) * u64::from(texture.height()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .expect("poll readback");
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("readback callback")
        .expect("map readback");
    let mapped = staging.slice(..).get_mapped_range();
    let mut result = Vec::new();
    for y in 0..texture.height() {
        let start = (y * row) as usize;
        result.extend_from_slice(&mapped[start..start + texture.width() as usize * 4]);
    }
    drop(mapped);
    staging.unmap();
    if texture.format() == TextureFormat::Bgra8Unorm {
        for pixel in result.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    result
}

fn copy(device: &Device, queue: &Queue, blitter: &SurfaceBlitter, target: &Texture) {
    let mut encoder = device.create_command_encoder(&Default::default());
    blitter.copy(&mut encoder, &target.create_view(&Default::default()));
    queue.submit([encoder.finish()]);
}

#[test]
fn surface_copy_preserves_orientation_alpha_and_reconfigured_sources() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("headless graphics adapter (Mesa software Vulkan is sufficient)");
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))
        .expect("headless graphics device");
    let source = texture(&device, 2, 2, TextureFormat::Rgba8Unorm);
    upload(
        &queue,
        &source,
        &[
            255, 0, 0, 255, 0, 128, 0, 128, 0, 0, 0, 0, 255, 255, 255, 255,
        ],
    );
    let target = texture(&device, 2, 2, TextureFormat::Rgba8Unorm);
    upload(&queue, &target, &[64, 32, 0, 255].repeat(4));
    let mut blitter = SurfaceBlitter::new(
        &device,
        target.format(),
        &source.create_view(&Default::default()),
        Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
    );
    copy(&device, &queue, &blitter, &target);
    let actual = read_rgba(&device, &queue, &target);
    let expected = [
        255_u8, 0, 0, 255, 32, 144, 0, 255, 64, 32, 0, 255, 255, 255, 255, 255,
    ];
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
        "premultiplied pixels must keep row orientation and blend correctly: {actual:?}"
    );

    let resized = texture(&device, 3, 1, TextureFormat::Rgba8Unorm);
    let colors = [255_u8, 0, 255, 255, 0, 255, 255, 255, 255, 255, 0, 255];
    upload(&queue, &resized, &colors);
    blitter.update_source(&device, &resized.create_view(&Default::default()));
    let new_target = texture(&device, 3, 1, TextureFormat::Rgba8Unorm);
    copy(&device, &queue, &blitter, &new_target);
    assert_eq!(read_rgba(&device, &queue, &new_target), colors);

    let reconfigured = texture(&device, 3, 1, TextureFormat::Bgra8Unorm);
    let blitter = SurfaceBlitter::new(
        &device,
        reconfigured.format(),
        &resized.create_view(&Default::default()),
        None,
    );
    copy(&device, &queue, &blitter, &reconfigured);
    assert_eq!(read_rgba(&device, &queue, &reconfigured), colors);
}
