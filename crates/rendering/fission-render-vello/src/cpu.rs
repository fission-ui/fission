//! Rendering scenes on the CPU.
//!
//! Used wherever a GPU is unavailable or deliberately bypassed: the Canvas2D web fallback,
//! software adapters, headless capture. It encodes through the same [`VelloRenderer`] as the
//! GPU path, so a frame rendered here matches one rendered on the GPU instead of being an
//! approximation of it.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use fission_render::{Color as RenderColor, RenderScene, Renderer};
use vello_cpu::kurbo::{Affine, Rect};
use vello_cpu::peniko::{Color, ImageAlphaType};
use vello_cpu::{PaintType, Pixmap, RenderContext, Resources};

use crate::{CpuPainter, Painter, VelloRenderer, VelloTextMeasurer};

/// Render `scene` into a pixmap over `background`.
pub fn render_to_pixmap(
    scene: &RenderScene,
    width: u32,
    height: u32,
    background: RenderColor,
    scale_factor: f64,
    measurer: Arc<VelloTextMeasurer>,
) -> Result<Pixmap> {
    let width = pixmap_dimension(width, "width")?;
    let height = pixmap_dimension(height, "height")?;

    let mut ctx = RenderContext::new(width, height);
    let mut resources = Resources::new();
    {
        let mut painter = CpuPainter {
            ctx: &mut ctx,
            resources: &mut resources,
        };
        // Drawn into the scene rather than used as a clear colour, so a backdrop filter at the
        // root sees the background exactly as it would on the GPU.
        painter.fill_rect(
            Affine::IDENTITY,
            PaintType::from(Color::from_rgba8(
                background.r,
                background.g,
                background.b,
                background.a,
            )),
            &Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
        );
        VelloRenderer::new(&mut painter, measurer, scale_factor).render_scene(scene)?;
    }

    ctx.flush();
    let mut pixmap = Pixmap::new(width, height);
    ctx.render(&mut pixmap, &mut resources);
    Ok(pixmap)
}

/// Render `scene` into premultiplied RGBA8 bytes over `background`.
pub fn render_to_rgba8(
    scene: &RenderScene,
    width: u32,
    height: u32,
    background: RenderColor,
    scale_factor: f64,
    measurer: Arc<VelloTextMeasurer>,
) -> Result<Vec<u8>> {
    Ok(
        render_to_pixmap(scene, width, height, background, scale_factor, measurer)?
            .take_rgba8(ImageAlphaType::AlphaPremultiplied),
    )
}

fn pixmap_dimension(value: u32, axis: &str) -> Result<u16> {
    u16::try_from(value.max(1))
        .map_err(|_| anyhow!("CPU render target {axis} {value} exceeds the 65535 pixel limit"))
}
