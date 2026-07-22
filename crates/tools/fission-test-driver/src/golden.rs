use anyhow::{bail, Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;

/// Thresholds used by deterministic golden-image comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoldenOptions {
    /// Maximum per-channel difference that still counts as unchanged.
    pub channel_tolerance: u8,
    /// Maximum percentage of pixels allowed to exceed the tolerance.
    pub max_changed_percent: f32,
}

impl Default for GoldenOptions {
    fn default() -> Self {
        Self {
            channel_tolerance: 2,
            max_changed_percent: 0.1,
        }
    }
}

/// Measurements produced by a golden-image comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoldenReport {
    pub width: u32,
    pub height: u32,
    pub changed_pixels: u64,
    pub total_pixels: u64,
    pub changed_percent: f32,
    pub maximum_channel_delta: u8,
}

impl GoldenReport {
    pub fn passed(self, options: GoldenOptions) -> bool {
        self.changed_percent <= options.max_changed_percent
    }
}

/// Compares PNG bytes against a golden file and optionally writes a heatmap.
pub fn compare_png_to_golden(
    actual_png: &[u8],
    golden_path: impl AsRef<Path>,
    diff_path: Option<impl AsRef<Path>>,
    options: GoldenOptions,
) -> Result<GoldenReport> {
    let actual = image::load_from_memory_with_format(actual_png, image::ImageFormat::Png)
        .context("decode actual PNG")?
        .to_rgba8();
    let expected = image::open(golden_path.as_ref())
        .with_context(|| format!("open golden image {}", golden_path.as_ref().display()))?
        .to_rgba8();
    if actual.dimensions() != expected.dimensions() {
        bail!(
            "golden dimensions differ: actual={:?} expected={:?}",
            actual.dimensions(),
            expected.dimensions()
        );
    }

    let (width, height) = actual.dimensions();
    let mut heatmap: RgbaImage = ImageBuffer::new(width, height);
    let mut changed_pixels = 0u64;
    let mut maximum_channel_delta = 0u8;
    for (x, y, actual_pixel) in actual.enumerate_pixels() {
        let expected_pixel = expected.get_pixel(x, y);
        let delta = channel_delta(*actual_pixel, *expected_pixel);
        let maximum = *delta.iter().max().unwrap_or(&0);
        maximum_channel_delta = maximum_channel_delta.max(maximum);
        let changed = delta.iter().any(|delta| *delta > options.channel_tolerance);
        if changed {
            changed_pixels += 1;
            heatmap.put_pixel(x, y, Rgba([maximum, 0, 0, 255]));
        } else {
            let luminance = expected_pixel.0[..3]
                .iter()
                .map(|channel| u16::from(*channel))
                .sum::<u16>()
                / 3;
            let subdued = (luminance / 4) as u8;
            heatmap.put_pixel(x, y, Rgba([subdued, subdued, subdued, 255]));
        }
    }
    if let Some(path) = diff_path {
        heatmap
            .save(path.as_ref())
            .with_context(|| format!("write diff heatmap {}", path.as_ref().display()))?;
    }
    let total_pixels = u64::from(width) * u64::from(height);
    let changed_percent = if total_pixels == 0 {
        0.0
    } else {
        changed_pixels as f32 * 100.0 / total_pixels as f32
    };
    Ok(GoldenReport {
        width,
        height,
        changed_pixels,
        total_pixels,
        changed_percent,
        maximum_channel_delta,
    })
}

fn channel_delta(actual: Rgba<u8>, expected: Rgba<u8>) -> [u8; 4] {
    std::array::from_fn(|index| actual.0[index].abs_diff(expected.0[index]))
}

#[cfg(test)]
mod tests {
    use super::{compare_png_to_golden, GoldenOptions};
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    #[test]
    fn reports_changed_pixels_and_writes_heatmap() {
        let temp = tempfile::tempdir().expect("temp directory");
        let golden_path = temp.path().join("golden.png");
        let diff_path = temp.path().join("diff.png");
        let golden = RgbaImage::from_pixel(2, 1, Rgba([10, 10, 10, 255]));
        golden.save(&golden_path).expect("save golden");
        let mut actual = golden.clone();
        actual.put_pixel(1, 0, Rgba([30, 10, 10, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(actual)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode actual");

        let report = compare_png_to_golden(
            &bytes,
            &golden_path,
            Some(&diff_path),
            GoldenOptions {
                channel_tolerance: 2,
                max_changed_percent: 50.0,
            },
        )
        .expect("compare images");

        assert_eq!(report.changed_pixels, 1);
        assert_eq!(report.changed_percent, 50.0);
        assert!(diff_path.exists());
    }
}
