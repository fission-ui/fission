//! Rendering scenes on the CPU.
//!
//! Used wherever a GPU is unavailable or deliberately bypassed: the Canvas2D web fallback,
//! software adapters, headless capture. It encodes through the same [`VelloRenderer`] as the
//! GPU path, so a frame rendered here matches one rendered on the GPU instead of being an
//! approximation of it.

use std::sync::Arc;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use anyhow::{anyhow, Result};
use fission_render::{Color as RenderColor, RenderScene, Renderer};
use vello_cpu::kurbo::{Affine, Rect};
use vello_cpu::peniko::{Color, ImageAlphaType};
use vello_cpu::{PaintType, Pixmap, RenderContext, Resources};

use crate::{CpuPainter, Painter, VelloRenderer, VelloTextMeasurer};

/// Reusable state for rendering frames with Vello's CPU backend.
///
/// Keep one instance per software renderer so Vello can retain glyph
/// preparation resources across frames. The scene recording context is reset
/// between frames and resized only when the output dimensions change.
pub struct CpuRenderer {
    context: Option<RenderContext>,
    context_size: Option<(u16, u16)>,
    resources: Resources,
    backdrop_filters_enabled: bool,
}

/// Time spent in the main software-rendering stages for one frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CpuRenderTimings {
    /// Building the CPU scene from the retained render tree.
    pub scene_recording: Duration,
    /// Flushing and rasterizing that scene into the output pixmap.
    pub rasterization: Duration,
    /// Copying the rendered pixmap into its RGBA8 output buffer.
    pub output_copy: Duration,
}

impl CpuRenderer {
    /// Create a CPU renderer with empty retained resources.
    pub fn new() -> Self {
        Self {
            context: None,
            context_size: None,
            resources: Resources::new(),
            backdrop_filters_enabled: true,
        }
    }

    /// Enable or disable backdrop filters for software-rendered output.
    ///
    /// Disable them when this renderer serves an interactive fallback that cannot afford to blur
    /// a full-window backdrop. The remaining scene, including translucent scrims, is unchanged.
    pub fn with_backdrop_filters(mut self, enabled: bool) -> Self {
        self.backdrop_filters_enabled = enabled;
        self
    }

    /// Render `scene` into premultiplied RGBA8 bytes over `background`.
    pub fn render_to_rgba8(
        &mut self,
        scene: &RenderScene,
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f64,
        measurer: Arc<VelloTextMeasurer>,
    ) -> Result<Vec<u8>> {
        self.render_to_rgba8_with_timings(scene, width, height, background, scale_factor, measurer)
            .map(|(rgba, _)| rgba)
    }

    /// Render a frame and report scene-recording, rasterization, and output-copy time.
    pub fn render_to_rgba8_with_timings(
        &mut self,
        scene: &RenderScene,
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f64,
        measurer: Arc<VelloTextMeasurer>,
    ) -> Result<(Vec<u8>, CpuRenderTimings)> {
        let width = pixmap_dimension(width, "width")?;
        let height = pixmap_dimension(height, "height")?;
        let dimensions = (width, height);

        match self.context.as_mut() {
            Some(context) if self.context_size == Some(dimensions) => context.reset(),
            Some(context) => context.reset_and_resize(width, height),
            None => self.context = Some(RenderContext::new(width, height)),
        }
        self.context_size = Some(dimensions);
        // Images are registered for this frame while glyph preparation data is
        // retained by Resources for later frames.
        self.resources.clear_images();

        let (pixmap, mut timings) = render_scene_to_pixmap(
            self.context.as_mut().expect("context initialized above"),
            &mut self.resources,
            scene,
            width,
            height,
            background,
            scale_factor,
            measurer,
            self.backdrop_filters_enabled,
        )?;
        let copy_started = Instant::now();
        let rgba = pixmap.take_rgba8(ImageAlphaType::AlphaPremultiplied);
        timings.output_copy = copy_started.elapsed();
        Ok((rgba, timings))
    }
}

impl Default for CpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

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
    render_scene_to_pixmap(
        &mut ctx,
        &mut resources,
        scene,
        width,
        height,
        background,
        scale_factor,
        measurer,
        true,
    )
    .map(|(pixmap, _)| pixmap)
}

fn render_scene_to_pixmap(
    ctx: &mut RenderContext,
    resources: &mut Resources,
    scene: &RenderScene,
    width: u16,
    height: u16,
    background: RenderColor,
    scale_factor: f64,
    measurer: Arc<VelloTextMeasurer>,
    backdrop_filters_enabled: bool,
) -> Result<(Pixmap, CpuRenderTimings)> {
    let recording_started = Instant::now();
    let mut pixmap = Pixmap::new(width, height);
    {
        let mut painter = CpuPainter { ctx, resources };
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
        VelloRenderer::new(&mut painter, measurer, scale_factor)
            .with_backdrop_filters(backdrop_filters_enabled)
            .render_scene(scene)?;
    }
    let scene_recording = recording_started.elapsed();

    let raster_started = Instant::now();
    ctx.flush();
    ctx.render(&mut pixmap, resources);
    let rasterization = raster_started.elapsed();
    Ok((
        pixmap,
        CpuRenderTimings {
            scene_recording,
            rasterization,
            output_copy: Duration::ZERO,
        },
    ))
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
    CpuRenderer::new().render_to_rgba8(scene, width, height, background, scale_factor, measurer)
}

fn pixmap_dimension(value: u32, axis: &str) -> Result<u16> {
    u16::try_from(value.max(1))
        .map_err(|_| anyhow!("CPU render target {axis} {value} exceeds the 65535 pixel limit"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_layout::LayoutRect;
    use parley::FontContext;
    use std::sync::Mutex;

    fn measurer() -> Arc<VelloTextMeasurer> {
        Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
            FontContext::new(),
        ))))
    }

    #[test]
    fn reused_context_matches_one_shot_after_resize() {
        let scene = RenderScene::new(LayoutRect::new(0.0, 0.0, 24.0, 16.0));
        let background = RenderColor {
            r: 12,
            g: 34,
            b: 56,
            a: 255,
        };
        let measurer = measurer();
        let mut renderer = CpuRenderer::new();

        for (width, height) in [(24, 16), (48, 32), (24, 16)] {
            let reused = renderer
                .render_to_rgba8(&scene, width, height, background, 1.0, measurer.clone())
                .unwrap();
            let one_shot =
                render_to_rgba8(&scene, width, height, background, 1.0, measurer.clone()).unwrap();

            assert_eq!(reused, one_shot);
        }
    }
}
