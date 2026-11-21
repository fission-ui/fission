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

use anyhow::{Result, anyhow};
use fission_layout::LayoutRect;
use fission_render::{
    Color as RenderColor, LayerClip, RenderLayer, RenderNode, RenderScene, Renderer,
};
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
    retained_target: Option<Pixmap>,
    retained_background: Option<RenderColor>,
    retained_scale_factor: Option<f64>,
    backdrop_filters_enabled: bool,
    exact_root_clip_elision: bool,
}

/// Pixel rectangle updated by one CPU render pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CpuUpdateRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Pixels emitted for a full or damage-limited CPU frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuFrameUpdate {
    /// Premultiplied RGBA8 pixel rows for `rect`.
    pub rgba: Vec<u8>,
    /// Updated area in the full render target.
    pub rect: CpuUpdateRect,
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
            retained_target: None,
            retained_background: None,
            retained_scale_factor: None,
            backdrop_filters_enabled: true,
            exact_root_clip_elision: false,
        }
    }

    /// Enable or disable backdrop filters for software-rendered output.
    ///
    /// Disable them when this renderer serves an interactive fallback that cannot afford to blur
    /// a full-window backdrop. The remaining scene, including translucent scrims, is unchanged.
    pub fn with_backdrop_filters(mut self, enabled: bool) -> Self {
        if self.backdrop_filters_enabled != enabled {
            self.invalidate_retained_target();
        }
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

    /// Discard the incremental target after its presentation sink fails.
    ///
    /// The next render will rebuild the full frame before emitting pixels, so a
    /// failed canvas or texture update cannot leave stale pixels in a later
    /// damage-limited frame.
    pub fn invalidate_retained_target(&mut self) {
        self.retained_target = None;
        self.retained_background = None;
        self.retained_scale_factor = None;
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
        self.invalidate_retained_target();
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

    /// Render a frame, updating only `damage` when the previous target has the same size.
    ///
    /// The retained target is cleared and rerasterized inside the damage rectangle only; the
    /// returned pixel rows can therefore be uploaded to the matching texture subregion. A
    /// missing damage rectangle, target resize, or invalid damage falls back to a full frame.
    pub fn render_to_rgba8_update_with_timings(
        &mut self,
        scene: &RenderScene,
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f64,
        measurer: Arc<VelloTextMeasurer>,
        damage: Option<LayoutRect>,
    ) -> Result<(CpuFrameUpdate, CpuRenderTimings)> {
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
        self.resources.clear_images();

        let partial = damage
            .filter(|_| self.context_size == Some(dimensions))
            .filter(|_| {
                self.retained_target
                    .as_ref()
                    .is_some_and(|target| target.width() == width && target.height() == height)
            })
            .filter(|_| self.retained_background == Some(background))
            .filter(|_| self.retained_scale_factor == Some(scale_factor))
            .and_then(|rect| pixel_damage_rect(rect, scale_factor, width, height));

        if let Some((pixel_rect, logical_clip)) = partial {
            let mut target = self
                .retained_target
                .take()
                .expect("partial rendering requires a retained target");
            self.retained_background = None;
            self.retained_scale_factor = None;
            clear_pixmap_rect(&mut target, pixel_rect, background);
            let clipped_scene = scene_with_clip(scene, logical_clip);
            let recording_started = Instant::now();
            if trace_web {
                crate::trace_web_test_stage(370, 0);
            }
            {
                let mut painter = CpuPainter {
                    ctx: self.context.as_mut().expect("context initialized above"),
                    resources: &mut self.resources,
                };
                VelloRenderer::new(&mut painter, measurer, scale_factor)
                    .with_backdrop_filters(self.backdrop_filters_enabled)
                    .render_scene(&clipped_scene)?;
            }
            let scene_recording = recording_started.elapsed();
            if trace_web {
                crate::trace_web_test_stage(371, scene_recording.as_millis() as u32);
                crate::trace_web_test_stage(372, 0);
            }

            let raster_started = Instant::now();
            self.context
                .as_mut()
                .expect("context initialized above")
                .flush();
            if trace_web {
                crate::trace_web_test_stage(373, raster_started.elapsed().as_millis() as u32);
                crate::trace_web_test_stage(374, 0);
            }
            self.context
                .as_ref()
                .expect("context initialized above")
                .render_with(
                    &mut target,
                    &mut self.resources,
                    RasterizerSettings {
                        target_init: TargetInit::SrcOver,
                        ..RasterizerSettings::default()
                    },
                );
            let rasterization = raster_started.elapsed();
            if trace_web {
                crate::trace_web_test_stage(375, rasterization.as_millis() as u32);
            }

            let copy_started = Instant::now();
            let rgba = copy_pixmap_rect(&target, pixel_rect);
            let output_copy = copy_started.elapsed();
            self.retained_target = Some(target);
            self.retained_background = Some(background);
            self.retained_scale_factor = Some(scale_factor);
            if trace_web {
                crate::trace_web_test_stage(376, 0);
                crate::trace_web_test_stage(377, output_copy.as_millis() as u32);
            }
            return Ok((
                CpuFrameUpdate {
                    rgba,
                    rect: CpuUpdateRect {
                        x: u32::from(pixel_rect.x),
                        y: u32::from(pixel_rect.y),
                        width: u32::from(pixel_rect.width),
                        height: u32::from(pixel_rect.height),
                    },
                },
                CpuRenderTimings {
                    scene_recording,
                    rasterization,
                    output_copy,
                },
            ));
        }

        self.invalidate_retained_target();
        let (target, mut timings) = render_scene_to_pixmap(
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
        let rgba = target.data_as_u8_slice().to_vec();
        timings.output_copy = copy_started.elapsed();
        if trace_web {
            crate::trace_web_test_stage(377, timings.output_copy.as_millis() as u32);
        }
        self.retained_target = Some(target);
        self.retained_background = Some(background);
        self.retained_scale_factor = Some(scale_factor);
        Ok((
            CpuFrameUpdate {
                rgba,
                rect: CpuUpdateRect {
                    x: 0,
                    y: 0,
                    width: u32::from(width),
                    height: u32::from(height),
                },
            },
            timings,
        ))
    }
}

fn pixel_damage_rect(
    damage: LayoutRect,
    scale_factor: f64,
    width: u16,
    height: u16,
) -> Option<((u16, u16, u16, u16), LayoutRect)> {
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return None;
    }
    let x0 = (f64::from(damage.x()) * scale_factor).floor() as i64;
    let y0 = (f64::from(damage.y()) * scale_factor).floor() as i64;
    let x1 = (f64::from(damage.right()) * scale_factor).ceil() as i64;
    let y1 = (f64::from(damage.bottom()) * scale_factor).ceil() as i64;
    let max_x = i64::from(width);
    let max_y = i64::from(height);
    let left = x0.saturating_sub(1).clamp(0, max_x);
    let top = y0.saturating_sub(1).clamp(0, max_y);
    let right = x1.saturating_add(1).clamp(0, max_x);
    let bottom = y1.saturating_add(1).clamp(0, max_y);
    if right <= left || bottom <= top {
        return None;
    }

    let rect = (
        u16::try_from(left).ok()?,
        u16::try_from(top).ok()?,
        u16::try_from(right - left).ok()?,
        u16::try_from(bottom - top).ok()?,
    );
    let inverse_scale = 1.0 / scale_factor;
    let clip = LayoutRect::new(
        (left as f64 * inverse_scale) as f32,
        (top as f64 * inverse_scale) as f32,
        (f64::from(rect.2) * inverse_scale) as f32,
        (f64::from(rect.3) * inverse_scale) as f32,
    );
    Some((rect, clip))
}

fn clear_pixmap_rect(target: &mut Pixmap, rect: (u16, u16, u16, u16), background: RenderColor) {
    let (x, y, width, height) = rect;
    let row_stride = usize::from(target.width()) * 4;
    let left = usize::from(x) * 4;
    let right = left + usize::from(width) * 4;
    let alpha = u16::from(background.a);
    let color = [
        ((u16::from(background.r) * alpha + 127) / 255) as u8,
        ((u16::from(background.g) * alpha + 127) / 255) as u8,
        ((u16::from(background.b) * alpha + 127) / 255) as u8,
        background.a,
    ];
    let bytes = target.data_as_u8_slice_mut();
    for row in usize::from(y)..usize::from(y + height) {
        let row = &mut bytes[row * row_stride + left..row * row_stride + right];
        for pixel in row.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
    }
}

fn copy_pixmap_rect(target: &Pixmap, rect: (u16, u16, u16, u16)) -> Vec<u8> {
    let (x, y, width, height) = rect;
    let row_stride = usize::from(target.width()) * 4;
    let left = usize::from(x) * 4;
    let row_width = usize::from(width) * 4;
    let mut rgba = Vec::with_capacity(row_width * usize::from(height));
    let bytes = target.data_as_u8_slice();
    for row in usize::from(y)..usize::from(y + height) {
        rgba.extend_from_slice(
            &bytes[row * row_stride + left..row * row_stride + left + row_width],
        );
    }
    rgba
}

fn scene_with_clip(scene: &RenderScene, clip: LayoutRect) -> RenderScene {
    let mut root = RenderLayer::new(scene.bounds);
    root.style.clip = Some(LayerClip::Rect(clip));
    root.children.clone_from(&scene.roots);
    let mut clipped = RenderScene::new(scene.bounds);
    clipped.roots.push(RenderNode::Layer(root));
    clipped
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
    fn partial_render_matches_full_render_and_returns_only_the_rounded_damage() {
        fn scene(changed: &str) -> RenderScene {
            let bounds = LayoutRect::new(0.0, 0.0, 64.0, 48.0);
            let mut list = fission_render::DisplayList::new(bounds);
            list.push(fission_render::DisplayOp::DrawRect {
                rect: LayoutRect::new(3.0, 5.0, 12.0, 9.0),
                fill: Some(fission_render::Fill::Solid(RenderColor {
                    r: 20,
                    g: 40,
                    b: 60,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
                bounds: LayoutRect::new(3.0, 5.0, 12.0, 9.0),
                node_id: Some(fission_ir::WidgetId::derived(61, &[1])),
                corner_radii: None,
                border_sides: None,
            });
            list.push(fission_render::DisplayOp::DrawText {
                text: changed.to_string(),
                position: fission_render::LayoutPoint::new(25.3, 12.2),
                size: 12.0,
                color: RenderColor {
                    r: 24,
                    g: 32,
                    b: 48,
                    a: 255,
                },
                bounds: LayoutRect::new(25.3, 12.2, 15.1, 10.3),
                node_id: Some(fission_ir::WidgetId::derived(61, &[2])),
                underline: false,
                wrap: true,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: None,
                resolved_layout: None,
            });
            RenderScene::from_display_list(list)
        }

        let background = RenderColor {
            r: 248,
            g: 249,
            b: 252,
            a: 255,
        };
        let before = scene("short");
        let after = scene("typing update");
        let mut renderer = CpuRenderer::new();
        let (initial, _) = renderer
            .render_to_rgba8_update_with_timings(
                &before,
                80,
                60,
                background,
                1.25,
                measurer(),
                None,
            )
            .unwrap();
        let (partial, _) = renderer
            .render_to_rgba8_update_with_timings(
                &after,
                80,
                60,
                background,
                1.25,
                measurer(),
                Some(LayoutRect::new(25.3, 12.2, 15.1, 10.3)),
            )
            .unwrap();
        let full_after = CpuRenderer::new()
            .render_to_rgba8(&after, 80, 60, background, 1.25, measurer())
            .unwrap();

        assert!(partial.rect.x > 0 && partial.rect.y > 0);
        assert!(partial.rect.width < 80 && partial.rect.height < 60);
        assert!(partial.rgba.len() < initial.rgba.len());
        let mut composed = initial.rgba;
        for row in 0..partial.rect.height as usize {
            let source = row * partial.rect.width as usize * 4;
            let destination = ((partial.rect.y as usize + row) * 80 + partial.rect.x as usize) * 4;
            let row_bytes = partial.rect.width as usize * 4;
            composed[destination..destination + row_bytes]
                .copy_from_slice(&partial.rgba[source..source + row_bytes]);
        }
        assert_eq!(composed, full_after, "partial update left stale pixels");

        renderer.invalidate_retained_target();
        let (recovered, _) = renderer
            .render_to_rgba8_update_with_timings(
                &after,
                80,
                60,
                background,
                1.25,
                measurer(),
                Some(LayoutRect::new(25.3, 12.2, 15.1, 10.3)),
            )
            .unwrap();
        assert_eq!(
            recovered.rect,
            CpuUpdateRect {
                x: 0,
                y: 0,
                width: 80,
                height: 60,
            },
            "invalidating a failed presentation sink must force a full update"
        );
        assert_eq!(recovered.rgba, full_after);
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
