//! Drawing backends for the scene encoder.
//!
//! Fission renders the same display list on the GPU and on the CPU. Both renderers are built on
//! the same sparse-strips core, so the encoder is written once against [`Painter`] and each
//! backend only translates calls. That is what makes the CPU and GPU output agree: there is no
//! second implementation of text, gradients, borders or filters for them to disagree about.

use std::collections::HashMap;
use std::sync::Arc;

use vello_cpu::filter_effects::{Filter, FilterFunction};
use vello_cpu::kurbo::{Affine, BezPath, Rect, Stroke};
use vello_cpu::peniko::{BlendMode, Color, Compose, FontData, Mix};
use vello_cpu::{Glyph, ImageSource, PaintType, Pixmap};

/// The drawing operations the scene encoder needs.
///
/// Every call carries its transform rather than relying on ambient state, so a backend never
/// has to track which transform was last set.
pub trait Painter {
    /// Fill `path` with `paint`.
    fn fill_path(&mut self, transform: Affine, paint: PaintType, path: &BezPath);

    /// Fill `rect` with `paint`.
    fn fill_rect(&mut self, transform: Affine, paint: PaintType, rect: &Rect);

    /// Stroke `path` with `paint`.
    fn stroke_path(&mut self, transform: Affine, stroke: &Stroke, paint: PaintType, path: &BezPath);

    /// Fill a Gaussian-blurred rounded rectangle, or its inverse when `invert` is set.
    ///
    /// The inverse is what an inset box shadow is: opaque outside the rectangle and fading to
    /// transparent inside it.
    #[allow(clippy::too_many_arguments)]
    fn fill_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        color: Color,
        rect: &Rect,
        radius: f32,
        std_dev: f32,
        invert: bool,
    );

    /// Push a layer composited with `blend` and `opacity`, optionally clipped to `clip`.
    fn push_layer(
        &mut self,
        transform: Affine,
        clip: Option<&BezPath>,
        blend: BlendMode,
        opacity: f32,
    );

    /// Pop the most recently pushed layer.
    fn pop_layer(&mut self);

    /// Filter what is already drawn in the current layer beneath `clip`, in place.
    fn apply_backdrop_filter(&mut self, transform: Affine, clip: &BezPath, filter: Filter);

    /// Fill a run of glyphs that share a font and size.
    fn fill_glyphs(
        &mut self,
        transform: Affine,
        paint: PaintType,
        font: &FontData,
        font_size: f32,
        glyphs: &[Glyph],
    );

    /// An image source this backend can paint `image` with.
    ///
    /// The CPU paints pixmaps directly; the GPU has to upload them into its atlas first.
    fn image_source(&mut self, image: &Arc<Pixmap>) -> ImageSource;
}

/// Paints into a [`vello_cpu::RenderContext`].
pub struct CpuPainter<'a> {
    pub ctx: &'a mut vello_cpu::RenderContext,
    pub resources: &'a mut vello_cpu::Resources,
}

impl Painter for CpuPainter<'_> {
    fn fill_path(&mut self, transform: Affine, paint: PaintType, path: &BezPath) {
        self.ctx.set_transform(transform);
        self.ctx.set_paint(paint);
        self.ctx.fill_path(path);
    }

    fn fill_rect(&mut self, transform: Affine, paint: PaintType, rect: &Rect) {
        self.ctx.set_transform(transform);
        self.ctx.set_paint(paint);
        self.ctx.fill_rect(rect);
    }

    fn stroke_path(
        &mut self,
        transform: Affine,
        stroke: &Stroke,
        paint: PaintType,
        path: &BezPath,
    ) {
        self.ctx.set_transform(transform);
        self.ctx.set_paint(paint);
        self.ctx.set_stroke(stroke.clone());
        self.ctx.stroke_path(path);
    }

    fn fill_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        color: Color,
        rect: &Rect,
        radius: f32,
        std_dev: f32,
        invert: bool,
    ) {
        self.ctx.set_transform(transform);
        self.ctx.set_paint(color);
        self.ctx
            .fill_blurred_rounded_rect(rect, radius, std_dev, invert);
    }

    fn push_layer(
        &mut self,
        transform: Affine,
        clip: Option<&BezPath>,
        blend: BlendMode,
        opacity: f32,
    ) {
        self.ctx.set_transform(transform);
        self.ctx
            .push_layer(clip, Some(blend), Some(opacity), None, None);
    }

    fn pop_layer(&mut self) {
        self.ctx.pop_layer();
    }

    fn apply_backdrop_filter(&mut self, transform: Affine, clip: &BezPath, filter: Filter) {
        self.ctx.set_transform(transform);
        self.ctx.apply_backdrop_filter(clip, filter);
    }

    fn fill_glyphs(
        &mut self,
        transform: Affine,
        paint: PaintType,
        font: &FontData,
        font_size: f32,
        glyphs: &[Glyph],
    ) {
        self.ctx.set_transform(transform);
        self.ctx.set_paint(paint);
        self.ctx
            .glyph_run(self.resources, font)
            .font_size(font_size)
            .fill_glyphs(glyphs.iter().copied());
    }

    fn image_source(&mut self, image: &Arc<Pixmap>) -> ImageSource {
        ImageSource::Pixmap(Arc::clone(image))
    }
}

/// What the GPU painter needs to upload images while it encodes.
pub struct GpuUploader<'a> {
    pub renderer: &'a mut vello_gpu::Renderer,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
}

/// Paints into a [`vello_gpu::Scene`].
pub struct GpuPainter<'a> {
    pub scene: &'a mut vello_gpu::Scene,
    pub resources: &'a mut vello_gpu::Resources,
    pub images: &'a mut GpuImageCache,
    pub uploader: GpuUploader<'a>,
}

impl Painter for GpuPainter<'_> {
    fn fill_path(&mut self, transform: Affine, paint: PaintType, path: &BezPath) {
        self.scene.set_transform(transform);
        self.scene.set_paint(paint);
        self.scene.fill_path(path);
    }

    fn fill_rect(&mut self, transform: Affine, paint: PaintType, rect: &Rect) {
        self.scene.set_transform(transform);
        self.scene.set_paint(paint);
        self.scene.fill_rect(rect);
    }

    fn stroke_path(
        &mut self,
        transform: Affine,
        stroke: &Stroke,
        paint: PaintType,
        path: &BezPath,
    ) {
        self.scene.set_transform(transform);
        self.scene.set_paint(paint);
        self.scene.set_stroke(stroke.clone());
        self.scene.stroke_path(path);
    }

    fn fill_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        color: Color,
        rect: &Rect,
        radius: f32,
        std_dev: f32,
        invert: bool,
    ) {
        self.scene.set_transform(transform);
        self.scene.set_paint(color);
        self.scene
            .fill_blurred_rounded_rect(rect, radius, std_dev, invert);
    }

    fn push_layer(
        &mut self,
        transform: Affine,
        clip: Option<&BezPath>,
        blend: BlendMode,
        opacity: f32,
    ) {
        self.scene.set_transform(transform);
        self.scene
            .push_layer(clip, Some(blend), Some(opacity), None, None);
    }

    fn pop_layer(&mut self) {
        self.scene.pop_layer();
    }

    fn apply_backdrop_filter(&mut self, transform: Affine, clip: &BezPath, filter: Filter) {
        self.scene.set_transform(transform);
        self.scene.apply_backdrop_filter(clip, filter);
    }

    fn fill_glyphs(
        &mut self,
        transform: Affine,
        paint: PaintType,
        font: &FontData,
        font_size: f32,
        glyphs: &[Glyph],
    ) {
        self.scene.set_transform(transform);
        self.scene.set_paint(paint);
        self.scene
            .glyph_run(self.resources, font)
            .font_size(font_size)
            .fill_glyphs(glyphs.iter().copied());
    }

    fn image_source(&mut self, image: &Arc<Pixmap>) -> ImageSource {
        self.images
            .source_for(image, self.resources, &mut self.uploader)
    }
}

/// Images uploaded to a GPU renderer's atlas, keyed by the decoded pixmap they came from.
///
/// Decoded images live in Fission's shared image cache as `Arc<Pixmap>`. Holding the `Arc`
/// here keeps the pointer used as the key from being reused by a different image while the
/// upload is still live.
#[derive(Default)]
pub struct GpuImageCache {
    entries: HashMap<usize, GpuImageEntry>,
    frame: u64,
}

struct GpuImageEntry {
    id: vello_cpu::ImageId,
    may_have_transparency: bool,
    _image: Arc<Pixmap>,
    last_used: u64,
}

/// Frames an uploaded image may go unused before its atlas space is reclaimed.
const GPU_IMAGE_IDLE_FRAMES: u64 = 120;

impl GpuImageCache {
    fn source_for(
        &mut self,
        image: &Arc<Pixmap>,
        resources: &mut vello_gpu::Resources,
        uploader: &mut GpuUploader<'_>,
    ) -> ImageSource {
        let key = Arc::as_ptr(image) as usize;
        let frame = self.frame;
        let entry = self.entries.entry(key).or_insert_with(|| GpuImageEntry {
            id: uploader.renderer.upload_image(
                resources,
                uploader.device,
                uploader.queue,
                uploader.encoder,
                image,
            ),
            may_have_transparency: image.may_have_transparency(),
            _image: Arc::clone(image),
            last_used: frame,
        });
        entry.last_used = frame;
        ImageSource::OpaqueId {
            id: entry.id,
            may_have_transparency: entry.may_have_transparency,
        }
    }

    /// Advance to the next frame and release uploads that have gone unused for a while.
    pub fn end_frame(
        &mut self,
        renderer: &mut vello_gpu::Renderer,
        resources: &mut vello_gpu::Resources,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let frame = self.frame;
        self.entries.retain(|_, entry| {
            let keep = frame.saturating_sub(entry.last_used) <= GPU_IMAGE_IDLE_FRAMES;
            if !keep {
                renderer.destroy_image(resources, encoder, entry.id);
            }
            keep
        });
        self.frame += 1;
    }
}

/// Lower an IR backdrop filter to the renderer's filter vocabulary.
///
/// Returns `None` when the filter does nothing, so the encoder can skip the backdrop entirely
/// instead of paying for a layer that leaves the pixels unchanged.
pub fn backdrop_filter(filter: &fission_ir::op::BackdropFilter) -> Option<Filter> {
    use fission_ir::op::BackdropFilter as IrFilter;
    let functions: Vec<FilterFunction> = filter
        .flatten()
        .into_iter()
        .filter_map(|leaf| match leaf {
            IrFilter::Blur(sigma) if *sigma > 0.0 => Some(FilterFunction::Blur { radius: *sigma }),
            IrFilter::Blur(_) => None,
            IrFilter::Saturate(amount) => Some(FilterFunction::Saturate { amount: *amount }),
            IrFilter::Brightness(amount) => Some(FilterFunction::Brightness { amount: *amount }),
            IrFilter::Chain(_) => None,
        })
        .collect();
    (!functions.is_empty()).then(|| Filter::from_functions(functions))
}

/// Map an IR blend mode onto the renderer's.
///
/// Every mode maps across, including additive `Plus`, which the sparse strips renderers
/// implement as a Porter-Duff compose mode.
pub fn blend_mode(mode: fission_ir::BlendMode) -> BlendMode {
    use fission_ir::BlendMode as B;
    let mix = |mix: Mix| BlendMode::new(mix, Compose::SrcOver);
    match mode {
        B::Normal => mix(Mix::Normal),
        B::Multiply => mix(Mix::Multiply),
        B::Screen => mix(Mix::Screen),
        B::Overlay => mix(Mix::Overlay),
        B::Darken => mix(Mix::Darken),
        B::Lighten => mix(Mix::Lighten),
        B::ColorDodge => mix(Mix::ColorDodge),
        B::ColorBurn => mix(Mix::ColorBurn),
        B::HardLight => mix(Mix::HardLight),
        B::SoftLight => mix(Mix::SoftLight),
        B::Difference => mix(Mix::Difference),
        B::Exclusion => mix(Mix::Exclusion),
        B::Hue => mix(Mix::Hue),
        B::Saturation => mix(Mix::Saturation),
        B::Color => mix(Mix::Color),
        B::Luminosity => mix(Mix::Luminosity),
        B::Plus => BlendMode::new(Mix::Normal, Compose::Plus),
    }
}
