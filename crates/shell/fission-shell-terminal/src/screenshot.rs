use crate::frame::{TerminalColor, TerminalFrame};
use ab_glyph::{point, Font, FontArc, PxScale, ScaleFont};
use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenshotOptions {
    pub cell_width: u32,
    pub cell_height: u32,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            cell_width: 10,
            cell_height: 18,
        }
    }
}

pub fn write_frame_png(
    frame: &TerminalFrame,
    path: impl AsRef<Path>,
    options: ScreenshotOptions,
) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let width = u32::from(frame.width).saturating_mul(options.cell_width);
    let height = u32::from(frame.height).saturating_mul(options.cell_height);
    let mut image = RgbaImage::new(width.max(1), height.max(1));
    let font = FontArc::try_from_slice(fission_theme::fonts::default_font_bytes())
        .context("failed to load default Fission font for terminal screenshot")?;
    let scale = PxScale::from((options.cell_height as f32 * 0.78).max(8.0));
    let scaled = font.as_scaled(scale);
    let baseline = ((options.cell_height as f32 - scale.y) * 0.5).max(0.0) + scaled.ascent();

    for y in 0..frame.height {
        for x in 0..frame.width {
            let Some(cell) = frame.get(x, y) else {
                continue;
            };
            fill_cell(
                &mut image,
                u32::from(x).saturating_mul(options.cell_width),
                u32::from(y).saturating_mul(options.cell_height),
                options.cell_width,
                options.cell_height,
                cell.style.bg,
            );
        }
    }

    for y in 0..frame.height {
        for x in 0..frame.width {
            let Some(cell) = frame.get(x, y) else {
                continue;
            };
            if cell.ch == ' ' {
                continue;
            }
            draw_char(
                &mut image,
                &font,
                scale,
                cell.ch,
                u32::from(x).saturating_mul(options.cell_width) as f32,
                u32::from(y).saturating_mul(options.cell_height) as f32 + baseline,
                cell.style.fg,
            );
        }
    }

    image
        .save(path)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn fill_cell(image: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, color: TerminalColor) {
    let rgba = Rgba([color.r, color.g, color.b, 255]);
    for py in y..(y + height).min(image.height()) {
        for px in x..(x + width).min(image.width()) {
            image.put_pixel(px, py, rgba);
        }
    }
}

fn draw_char(
    image: &mut RgbaImage,
    font: &FontArc,
    scale: PxScale,
    ch: char,
    x: f32,
    baseline: f32,
    color: TerminalColor,
) {
    let glyph = font
        .glyph_id(ch)
        .with_scale_and_position(scale, point(x, baseline));
    let Some(outlined) = font.outline_glyph(glyph) else {
        return;
    };
    let bounds = outlined.px_bounds();
    outlined.draw(|gx, gy, coverage| {
        let px = bounds.min.x.floor() as i32 + gx as i32;
        let py = bounds.min.y.floor() as i32 + gy as i32;
        if px < 0 || py < 0 {
            return;
        }
        let px = px as u32;
        let py = py as u32;
        if px >= image.width() || py >= image.height() {
            return;
        }
        let dst = image.get_pixel(px, py).0;
        let alpha = coverage.clamp(0.0, 1.0);
        let inv = 1.0 - alpha;
        image.put_pixel(
            px,
            py,
            Rgba([
                (color.r as f32 * alpha + dst[0] as f32 * inv) as u8,
                (color.g as f32 * alpha + dst[1] as f32 * inv) as u8,
                (color.b as f32 * alpha + dst[2] as f32 * inv) as u8,
                255,
            ]),
        );
    });
}
