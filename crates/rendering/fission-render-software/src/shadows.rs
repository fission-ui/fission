use anyhow::{anyhow, Result};
use tiny_skia::{FillRule as TinyFillRule, Mask, Pixmap, Transform};

use crate::resources::{rect_path, rounded_rect_path};

pub(super) fn draw_software_box_shadow(
    surface: &mut Pixmap,
    rect: fission_render::LayoutRect,
    corner_radius: f32,
    shadow: &fission_render::BoxShadow,
    transform: Transform,
    clip: Option<&Mask>,
    scale_factor: f32,
) -> Result<()> {
    let width = surface.width();
    let height = surface.height();
    let sigma = shadow.blur_radius.max(0.0) * 0.5 * scale_factor;
    let coverage = if shadow.inset {
        let Some(original_path) = (if corner_radius > 0.0 {
            rounded_rect_path(rect, corner_radius)
        } else {
            rect_path(rect)
        }) else {
            return Ok(());
        };
        let mut original_mask = Mask::new(width, height)
            .ok_or_else(|| anyhow!("failed to allocate inset-shadow clip mask"))?;
        original_mask.fill_path(&original_path, TinyFillRule::Winding, true, transform);

        let spread = shadow.spread_radius;
        let hole = fission_render::LayoutRect::new(
            rect.origin.x + spread + shadow.offset.0,
            rect.origin.y + spread + shadow.offset.1,
            (rect.size.width - spread * 2.0).max(0.0),
            (rect.size.height - spread * 2.0).max(0.0),
        );
        let mut hole_mask = Mask::new(width, height)
            .ok_or_else(|| anyhow!("failed to allocate inset-shadow hole mask"))?;
        if let Some(hole_path) = if corner_radius > 0.0 {
            rounded_rect_path(hole, (corner_radius - spread).max(0.0))
        } else {
            rect_path(hole)
        } {
            hole_mask.fill_path(&hole_path, TinyFillRule::Winding, true, transform);
        }
        let outside = hole_mask
            .data()
            .iter()
            .map(|coverage| 255_u8.saturating_sub(*coverage))
            .collect::<Vec<_>>();
        let mut blurred = blur_coverage(outside, width, height, sigma)?;
        for (coverage, shape) in blurred.iter_mut().zip(original_mask.data()) {
            *coverage = (u16::from(*coverage) * u16::from(*shape) / 255) as u8;
        }
        blurred
    } else {
        let spread = shadow.spread_radius;
        let shadow_rect = fission_render::LayoutRect::new(
            rect.origin.x + shadow.offset.0 - spread,
            rect.origin.y + shadow.offset.1 - spread,
            (rect.size.width + spread * 2.0).max(0.0),
            (rect.size.height + spread * 2.0).max(0.0),
        );
        let Some(shadow_path) = (if corner_radius > 0.0 {
            rounded_rect_path(shadow_rect, (corner_radius + spread).max(0.0))
        } else {
            rect_path(shadow_rect)
        }) else {
            return Ok(());
        };
        let mut mask = Mask::new(width, height)
            .ok_or_else(|| anyhow!("failed to allocate drop-shadow mask"))?;
        mask.fill_path(&shadow_path, TinyFillRule::Winding, true, transform);
        blur_coverage(mask.data().to_vec(), width, height, sigma)?
    };

    blend_shadow_coverage(surface.data_mut(), &coverage, clip, shadow.color);
    Ok(())
}

pub(super) fn blur_coverage(
    coverage: Vec<u8>,
    width: u32,
    height: u32,
    sigma: f32,
) -> Result<Vec<u8>> {
    if sigma <= f32::EPSILON {
        return Ok(coverage);
    }
    let image = image::GrayImage::from_raw(width, height, coverage)
        .ok_or_else(|| anyhow!("invalid shadow mask dimensions"))?;
    Ok(image::imageops::blur(&image, sigma).into_raw())
}

pub(super) fn blend_shadow_coverage(
    destination: &mut [u8],
    coverage: &[u8],
    clip: Option<&Mask>,
    color: fission_render::Color,
) {
    let clip = clip.map(Mask::data);
    for (index, coverage) in coverage.iter().copied().enumerate() {
        let coverage = clip
            .map(|clip| u16::from(coverage) * u16::from(clip[index]) / 255)
            .unwrap_or_else(|| u16::from(coverage));
        let source_alpha = u16::from(color.a) * coverage / 255;
        if source_alpha == 0 {
            continue;
        }
        let inverse_alpha = 255 - source_alpha;
        let offset = index * 4;
        for (channel, source) in [color.r, color.g, color.b].into_iter().enumerate() {
            let source = u16::from(source) * source_alpha / 255;
            destination[offset + channel] =
                (source + u16::from(destination[offset + channel]) * inverse_alpha / 255) as u8;
        }
        destination[offset + 3] = (source_alpha
            + u16::from(destination[offset + 3]) * inverse_alpha / 255)
            .min(255) as u8;
    }
}
