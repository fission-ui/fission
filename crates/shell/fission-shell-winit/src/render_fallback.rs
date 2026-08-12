#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use fission_layout::TextMeasurer;
use fission_render::capabilities::{DisplayOpKind, GraphicsCapabilities};
#[cfg(target_arch = "wasm32")]
use fission_render::Color;
use fission_render::{DisplayList, DisplayOp, RenderNode, RenderScene};
use vello::wgpu;

#[cfg(target_arch = "wasm32")]
use crate::frame_submission::FrameSubmission;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SoftwareFallbackReason {
    BackdropFilter,
}

impl SoftwareFallbackReason {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::BackdropFilter => "backdrop-filter",
        }
    }
}

/// Plans a whole-frame software fallback only when the active backend cannot
/// encode the operation itself. The caller must validate the complete frame
/// against the software profile before executing this plan.
pub(super) fn required_software_fallback(
    scene: &RenderScene,
    active_capabilities: &GraphicsCapabilities,
) -> Option<SoftwareFallbackReason> {
    software_fallback_reason(scene).filter(|reason| match reason {
        SoftwareFallbackReason::BackdropFilter => {
            !active_capabilities.supports_display_op(DisplayOpKind::BackdropFilter)
        }
    })
}

pub(super) fn software_fallback_reason(scene: &RenderScene) -> Option<SoftwareFallbackReason> {
    scene.roots.iter().find_map(node_software_fallback_reason)
}

fn node_software_fallback_reason(node: &RenderNode) -> Option<SoftwareFallbackReason> {
    match node {
        RenderNode::Layer(layer) => layer
            .children
            .iter()
            .find_map(node_software_fallback_reason),
        RenderNode::Paint(list) => list_software_fallback_reason(list),
    }
}

fn list_software_fallback_reason(list: &DisplayList) -> Option<SoftwareFallbackReason> {
    list.ops.iter().find_map(|operation| match operation {
        DisplayOp::BackdropFilter { .. } => Some(SoftwareFallbackReason::BackdropFilter),
        DisplayOp::CachedScene { list, .. } => list_software_fallback_reason(list),
        _ => None,
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn render_host_scene_with_software(
    submission: &FrameSubmission,
    scene: &RenderScene,
    width: u32,
    height: u32,
    background: Color,
    scale_factor: f32,
    text_measurer: Arc<dyn TextMeasurer>,
) -> anyhow::Result<Vec<u8>> {
    let scene = submission.compose_host_scene(scene);
    fission_render_software::SoftwareRenderer::render_with_text_measurer(
        &scene,
        width,
        height,
        background,
        scale_factor,
        text_measurer,
    )
}

pub(super) fn upload_raster_frame(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    rgba: &[u8],
    width: u32,
    height: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_ir::op::BackdropFilter;
    use fission_render::{LayoutRect, RenderLayer};

    #[test]
    fn detects_backdrop_filters_inside_cached_nested_layers() {
        let bounds = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let mut cached = DisplayList::new(bounds);
        cached.push(DisplayOp::BackdropFilter {
            rect: bounds,
            filter: BackdropFilter::Blur(2.0),
            corner_radius: 0.0,
            bounds,
            node_id: None,
        });
        let mut outer = DisplayList::new(bounds);
        outer.push(DisplayOp::CachedScene {
            cache_key: 7,
            bounds,
            list: Box::new(cached),
        });
        let mut layer = RenderLayer::new(bounds);
        layer.children.push(RenderNode::Paint(outer));
        let mut scene = RenderScene::new(bounds);
        scene.roots.push(RenderNode::Layer(layer));

        assert_eq!(
            software_fallback_reason(&scene),
            Some(SoftwareFallbackReason::BackdropFilter)
        );

        let vello = crate::frame_submission::winit_vello_capabilities(
            fission_render::capabilities::RenderMode::Gpu,
        );
        assert_eq!(
            required_software_fallback(&scene, &vello),
            Some(SoftwareFallbackReason::BackdropFilter)
        );
    }
}
