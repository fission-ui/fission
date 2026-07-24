use image::{ImageBuffer, ImageEncoder, Rgba};

pub(crate) fn prepare_image_files(count: usize) -> anyhow::Result<Vec<String>> {
    let count = std::env::var("FISSION_REPRO_IMAGE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(count)
        .max(1);
    let pixels = std::env::var("FISSION_REPRO_IMAGE_PIXELS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1024u32);
    let dir = std::env::temp_dir().join(format!("fission-motion-memory-repro-{pixels}"));
    std::fs::create_dir_all(&dir)?;

    (0..count)
        .map(|index| {
            let path = dir.join(format!("image-{index:03}.png"));
            if !path.exists() {
                write_repro_image(&path, pixels, index as u32)?;
            }
            Ok(path.to_string_lossy().into_owned())
        })
        .collect()
}

fn write_repro_image(path: &std::path::Path, pixels: u32, seed: u32) -> anyhow::Result<()> {
    let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(pixels, pixels);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let r = ((x.wrapping_add(seed * 17)) % 256) as u8;
        let g = ((y.wrapping_mul(3).wrapping_add(seed * 29)) % 256) as u8;
        let b = ((x.wrapping_add(y).wrapping_add(seed * 41)) % 256) as u8;
        *pixel = Rgba([r, g, b, 255]);
    }

    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    image::codecs::png::PngEncoder::new(writer).write_image(
        image.as_raw(),
        pixels,
        pixels,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(())
}
