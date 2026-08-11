//! Compatibility facade for the renderer API historically exported here.
//!
//! New host code uses `fission-render-wgpu3d` directly. Keeping this wrapper in
//! the authoring crate preserves the existing method signatures while the
//! neutral scene model and production renderer evolve independently.

pub use fission_render_wgpu3d::{Scene3DComposition, Scene3DSource, Scene3DViewport, Vertex};
use wgpu::{Device, Queue, TextureFormat, TextureView};

use crate::Scene3D;

/// Source-compatible wrapper around the separated wgpu renderer.
pub struct Scene3DRenderer {
    inner: fission_render_wgpu3d::Scene3DRenderer,
}

impl Scene3DRenderer {
    pub fn new(device: &Device, width: u32, height: u32, target_format: TextureFormat) -> Self {
        Self {
            inner: fission_render_wgpu3d::Scene3DRenderer::new(
                device,
                width,
                height,
                target_format,
            ),
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        self.inner.resize(device, width, height);
    }

    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        view: &TextureView,
        scene: &Scene3D,
    ) {
        self.inner.render(device, queue, view, scene);
    }

    pub fn render_in_rect(
        &mut self,
        device: &Device,
        queue: &Queue,
        view: &TextureView,
        scene: &Scene3D,
        viewport: Scene3DViewport,
    ) {
        self.inner
            .render_in_rect(device, queue, view, scene, viewport);
    }
}
