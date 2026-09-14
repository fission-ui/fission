//! Rendering scenes on the GPU.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use fission_render::{Color as RenderColor, RenderScene, Renderer};
use vello_cpu::kurbo::{Affine, Rect};
use vello_cpu::peniko::Color;
use vello_cpu::PaintType;

use crate::{GpuImageCache, GpuPainter, GpuUploader, Painter, VelloRenderer, VelloTextMeasurer};

/// Renders Fission scenes into wgpu textures.
///
/// Owns the renderer and everything that must persist between frames for it to stay cheap: the
/// glyph and image atlases, the uploaded-image cache and a scene whose allocations are reused.
pub struct GpuSceneRenderer {
    renderer: vello_gpu::Renderer,
    resources: vello_gpu::Resources,
    images: GpuImageCache,
    scene: vello_gpu::Scene,
    format: wgpu::TextureFormat,
}

impl GpuSceneRenderer {
    /// Create a renderer for targets of `format`.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let (renderer, resources) = vello_gpu::Renderer::new(
            device,
            &vello_gpu::RenderTargetConfig {
                format,
                width: 1,
                height: 1,
            },
        );
        Self {
            renderer,
            resources,
            images: GpuImageCache::default(),
            scene: vello_gpu::Scene::new(1, 1),
            format,
        }
    }

    /// The target format this renderer was created for.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Render `scene` into `view`.
    ///
    /// With a `background` the target is filled with it first. Without one the scene is
    /// composited over whatever the target already holds, which is how the compositor draws a
    /// layer onto a seeded base.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &RenderScene,
        measurer: Arc<VelloTextMeasurer>,
        scale_factor: f64,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        background: Option<RenderColor>,
    ) -> Result<()> {
        let width = target_dimension(width, "width")?;
        let height = target_dimension(height, "height")?;
        if self.scene.width() != width || self.scene.height() != height {
            self.scene.reset_and_resize(width, height);
        } else {
            self.scene.reset();
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Fission scene"),
        });
        {
            let mut painter = GpuPainter {
                scene: &mut self.scene,
                resources: &mut self.resources,
                images: &mut self.images,
                uploader: GpuUploader {
                    renderer: &mut self.renderer,
                    device,
                    queue,
                    encoder: &mut encoder,
                },
            };
            // Drawn into the scene rather than used as a clear colour, so a backdrop filter at
            // the root sees the background, as it does on the CPU.
            if let Some(background) = background {
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
            }
            VelloRenderer::new(&mut painter, measurer, scale_factor).render_scene(scene)?;
        }
        self.images
            .end_frame(&mut self.renderer, &mut self.resources, &mut encoder);

        let target_init = if background.is_some() {
            vello_gpu::TargetInit::Clear(vello_gpu::ClearSettings::default())
        } else {
            vello_gpu::TargetInit::SrcOver
        };
        self.renderer
            .render(
                &self.scene,
                &mut self.resources,
                device,
                queue,
                &mut encoder,
                &vello_gpu::RenderSize { width, height },
                view,
                None,
                &vello_gpu::TextureBindings::new(),
                target_init,
            )
            .map_err(|error| anyhow!("vello_gpu failed to render the scene: {error:?}"))?;
        queue.submit([encoder.finish()]);
        Ok(())
    }
}

fn target_dimension(value: u32, axis: &str) -> Result<u16> {
    u16::try_from(value.max(1))
        .map_err(|_| anyhow!("GPU render target {axis} {value} exceeds the 65535 pixel limit"))
}
