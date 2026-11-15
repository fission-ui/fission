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
use vello_cpu::{PaintType, Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};

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
    exact_root_clip_elision: bool,
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
            exact_root_clip_elision: false,
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

    /// Skip an exact scene viewport clip layer when the CPU target already has
    /// the same dimensions. This is intended for software fallbacks that render
    /// directly to their final viewport-sized target.
    pub fn with_exact_root_clip_elision(mut self, enabled: bool) -> Self {
        self.exact_root_clip_elision = enabled;
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
        let trace_web = crate::web_test_trace_enabled();

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
            self.exact_root_clip_elision
                && !self.backdrop_filters_enabled
                && target_matches_scene_bounds(scene, width, height, scale_factor),
        )?;
        let copy_started = Instant::now();
        if trace_web {
            crate::trace_web_test_stage(376, 0);
        }
        let rgba = pixmap.take_rgba8(ImageAlphaType::AlphaPremultiplied);
        timings.output_copy = copy_started.elapsed();
        if trace_web {
            crate::trace_web_test_stage(377, timings.output_copy.as_millis() as u32);
        }
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
        false,
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
    exact_root_clip_elision: bool,
) -> Result<(Pixmap, CpuRenderTimings)> {
    let trace_web = crate::web_test_trace_enabled();
    let recording_started = Instant::now();
    if trace_web {
        crate::trace_web_test_stage(370, 0);
    }
    let mut pixmap = Pixmap::new(width, height);
    let background = Color::from_rgba8(background.r, background.g, background.b, background.a);
    {
        let mut painter = CpuPainter { ctx, resources };
        if backdrop_filters_enabled {
            // Drawn into the scene rather than used as a clear colour, so a backdrop filter at
            // the root sees the background exactly as it would on the GPU.
            painter.fill_rect(
                Affine::IDENTITY,
                PaintType::from(background),
                &Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
            );
        }
        VelloRenderer::new(&mut painter, measurer, scale_factor)
            .with_backdrop_filters(backdrop_filters_enabled)
            .with_exact_root_clip_elision(exact_root_clip_elision)
            .render_scene(scene)?;
    }
    let scene_recording = recording_started.elapsed();
    if trace_web {
        crate::trace_web_test_stage(371, scene_recording.as_millis() as u32);
        crate::trace_web_test_stage(372, 0);
    }

    let raster_started = Instant::now();
    ctx.flush();
    if trace_web {
        crate::trace_web_test_stage(373, raster_started.elapsed().as_millis() as u32);
        crate::trace_web_test_stage(374, 0);
    }
    if backdrop_filters_enabled {
        ctx.render(&mut pixmap, resources);
    } else {
        // The Canvas2D fallback omits backdrop filters, so the background can be installed as
        // the target's initial color. This avoids rasterizing and compositing an extra full-window
        // rectangle on every software frame.
        ctx.render_with(
            &mut pixmap,
            resources,
            RasterizerSettings {
                target_init: TargetInit::Clear(background),
                ..RasterizerSettings::default()
            },
        );
    }
    let rasterization = raster_started.elapsed();
    if trace_web {
        crate::trace_web_test_stage(375, rasterization.as_millis() as u32);
    }
    Ok((
        pixmap,
        CpuRenderTimings {
            scene_recording,
            rasterization,
            output_copy: Duration::ZERO,
        },
    ))
}

fn target_matches_scene_bounds(
    scene: &RenderScene,
    width: u16,
    height: u16,
    scale_factor: f64,
) -> bool {
    if !scale_factor.is_finite()
        || scale_factor <= 0.0
        || scene.bounds.origin.x != 0.0
        || scene.bounds.origin.y != 0.0
    {
        return false;
    }

    let expected_width = f64::from(scene.bounds.size.width) * scale_factor;
    let expected_height = f64::from(scene.bounds.size.height) * scale_factor;
    expected_width.is_finite()
        && expected_height.is_finite()
        && expected_width.round() == f64::from(width)
        && expected_height.round() == f64::from(height)
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

/// Render a scene for screenshot fallback when the active renderer is GPU-backed.
///
/// A WebGPU screenshot may need to be reconstructed from the retained scene because the browser
/// cannot reliably capture its swapchain. Keep this best-effort CPU path responsive by omitting
/// backdrop filters, which can require a full-window software blur. The translucent scrim and
/// the rest of the scene are preserved.
pub fn render_to_rgba8_for_screenshot(
    scene: &RenderScene,
    width: u32,
    height: u32,
    background: RenderColor,
    scale_factor: f64,
    measurer: Arc<VelloTextMeasurer>,
) -> Result<Vec<u8>> {
    CpuRenderer::new()
        .with_backdrop_filters(false)
        .render_to_rgba8(scene, width, height, background, scale_factor, measurer)
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

    #[test]
    fn software_fallback_clear_matches_a_rasterized_background() {
        let bounds = LayoutRect::new(0.0, 0.0, 24.0, 16.0);
        let mut list = fission_render::DisplayList::new(bounds);
        list.push(fission_render::DisplayOp::DrawRect {
            rect: LayoutRect::new(4.0, 4.0, 12.0, 12.0),
            fill: Some(fission_render::Fill::Solid(fission_render::Color {
                r: 240,
                g: 80,
                b: 24,
                a: 128,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: LayoutRect::new(4.0, 4.0, 12.0, 12.0),
            node_id: None,
            corner_radii: None,
            border_sides: None,
        });
        let scene = RenderScene::from_display_list(list);
        let background = RenderColor {
            r: 12,
            g: 34,
            b: 56,
            a: 255,
        };
        let measurer = measurer();
        let rasterized_background = CpuRenderer::new()
            .render_to_rgba8(&scene, 24, 16, background, 1.0, measurer.clone())
            .unwrap();
        let cleared_background = CpuRenderer::new()
            .with_backdrop_filters(false)
            .render_to_rgba8(&scene, 24, 16, background, 1.0, measurer)
            .unwrap();

        assert_eq!(cleared_background, rasterized_background);
    }

    #[test]
    fn screenshot_fallback_omits_full_window_backdrop_filters() {
        let bounds = LayoutRect::new(0.0, 0.0, 48.0, 32.0);
        let mut list = fission_render::DisplayList::new(bounds);
        list.push(fission_render::DisplayOp::DrawRect {
            rect: LayoutRect::new(0.0, 0.0, 24.0, 32.0),
            fill: Some(fission_render::Fill::Solid(fission_render::Color {
                r: 240,
                g: 80,
                b: 24,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: LayoutRect::new(0.0, 0.0, 24.0, 32.0),
            node_id: None,
            corner_radii: None,
            border_sides: None,
        });
        list.push(fission_render::DisplayOp::DrawRect {
            rect: LayoutRect::new(24.0, 0.0, 24.0, 32.0),
            fill: Some(fission_render::Fill::Solid(fission_render::Color {
                r: 24,
                g: 80,
                b: 240,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: LayoutRect::new(24.0, 0.0, 24.0, 32.0),
            node_id: None,
            corner_radii: None,
            border_sides: None,
        });
        list.push(fission_render::DisplayOp::BackdropFilter {
            rect: bounds,
            filter: fission_ir::op::BackdropFilter::Blur(4.0),
            corner_radius: 0.0,
            bounds,
            node_id: None,
            corner_radii: None,
        });
        let scene = RenderScene::from_display_list(list);
        let background = RenderColor {
            r: 12,
            g: 34,
            b: 56,
            a: 255,
        };
        let measurer = measurer();

        let screenshot =
            render_to_rgba8_for_screenshot(&scene, 48, 32, background, 1.0, measurer.clone())
                .unwrap();
        let without_backdrop = CpuRenderer::new()
            .with_backdrop_filters(false)
            .render_to_rgba8(&scene, 48, 32, background, 1.0, measurer.clone())
            .unwrap();
        let with_backdrop = CpuRenderer::new()
            .render_to_rgba8(&scene, 48, 32, background, 1.0, measurer)
            .unwrap();

        assert_eq!(screenshot, without_backdrop);
        assert_ne!(screenshot, with_backdrop);
    }

    #[test]
    fn exact_root_viewport_clip_elision_preserves_target_pixels() {
        let bounds = LayoutRect::new(0.0, 0.0, 24.0, 16.0);
        let mut content = fission_render::DisplayList::new(bounds);
        content.push(fission_render::DisplayOp::DrawRect {
            rect: LayoutRect::new(-4.0, 3.0, 32.0, 10.0),
            fill: Some(fission_render::Fill::Solid(fission_render::Color {
                r: 240,
                g: 80,
                b: 24,
                a: 128,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: LayoutRect::new(-4.0, 3.0, 32.0, 10.0),
            node_id: None,
            corner_radii: None,
            border_sides: None,
        });
        let mut root = fission_render::RenderLayer::new(bounds);
        root.style.clip = Some(fission_render::LayerClip::Rect(bounds));
        root.children
            .push(fission_render::RenderNode::Paint(content));
        let mut scene = RenderScene::new(bounds);
        scene.roots.push(fission_render::RenderNode::Layer(root));
        assert!(target_matches_scene_bounds(&scene, 24, 16, 1.0));
        assert!(!target_matches_scene_bounds(&scene, 48, 16, 1.0));
        let background = RenderColor {
            r: 12,
            g: 34,
            b: 56,
            a: 255,
        };
        let measurer = measurer();

        let clipped = CpuRenderer::new()
            .with_backdrop_filters(false)
            .render_to_rgba8(&scene, 24, 16, background, 1.0, measurer.clone())
            .unwrap();
        let clip_elided = CpuRenderer::new()
            .with_backdrop_filters(false)
            .with_exact_root_clip_elision(true)
            .render_to_rgba8(&scene, 24, 16, background, 1.0, measurer)
            .unwrap();

        assert_eq!(clip_elided, clipped);
    }
}
