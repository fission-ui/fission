use fission_core::internal::{InternalLowerer, InternalLoweringCx, InternalRenderNode};
use fission_core::ui::{Container, Widget};

pub use fission_3d_model::{
    decode_scene3d_submission, encode_scene3d_submission, try_encode_scene3d_submission,
    validate_scene3d_primitives, validate_scene3d_source, Point3D, Primitive3D, Scene3DModel,
    Scene3DSource, Scene3DSubmissionError, Scene3DValidationError,
};
use fission_ir::op::{EmbedKind, LayoutOp};
use serde::{Deserialize, Serialize};

/// Compatibility exports for the existing public wgpu renderer path.
///
/// New host integrations should depend on `fission-render-wgpu3d` directly.
/// This module remains unconditional so existing renderer consumers keep their
/// API, including dependencies that previously disabled default features.
/// Backend-neutral consumers should use `fission-3d-model`.
pub mod render;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene3D {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub primitives: Vec<Primitive3D>,
}

impl Scene3D {
    pub fn new() -> Self {
        Self {
            width: None,
            height: None,
            primitives: Vec::new(),
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    pub fn add_primitive(mut self, primitive: Primitive3D) -> Self {
        self.primitives.push(primitive);
        self
    }
}

impl Scene3DSource for Scene3D {
    fn primitives(&self) -> &[Primitive3D] {
        &self.primitives
    }
}

impl From<Scene3D> for Widget {
    fn from(component: Scene3D) -> Self {
        let this = &component;
        let mut container = Container::new(fission_core::internal::custom_render_widget(
            InternalRenderNode {
                debug_tag: "fission_3d::Scene3D".into(),
                lowerer: Some(std::sync::Arc::new(Scene3DInternalLowerer {
                    scene: this.clone(),
                })),
                render_object: None,
            },
        ));
        if let Some(w) = this.width {
            container = container.width(w);
        } else {
            container = container.flex_grow(1.0);
        }
        if let Some(h) = this.height {
            container = container.height(h);
        } else {
            if this.width.is_none() {
                container = container.flex_grow(1.0);
            }
        }
        container.into()
    }
}

#[derive(Debug)]
pub struct Scene3DInternalLowerer {
    pub scene: Scene3D,
}

impl InternalLowerer for Scene3DInternalLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> fission_ir::WidgetId {
        let node_id = cx.next_node_id();

        let w = self
            .scene
            .width
            .unwrap_or_else(|| (cx.env.viewport_size.width - 264.0).max(400.0));
        let h = self
            .scene
            .height
            .unwrap_or_else(|| (cx.env.viewport_size.height - 200.0).max(300.0));

        // The shell recognizes this neutral payload as external 3D content.
        // A selected general-GPU adapter owns rendering; no renderer or GPU
        // implementation type crosses this scene-model boundary.

        let payload = encode_scene3d_submission(&self.scene.primitives);
        let op = fission_ir::Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload),
            widget_id: node_id,
            width: Some(w),
            height: Some(h),
        });

        cx.insert_node(node_id, op, vec![])
    }
}
