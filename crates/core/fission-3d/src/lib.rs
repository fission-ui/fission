pub mod render;
mod scene_graph;
use fission_core::internal::{InternalLowerer, InternalLoweringCx, InternalRenderNode};
use fission_core::op::Color;
use fission_core::ui::{Container, Widget};
pub use scene_graph::{
    Node3D, Node3DId, ResolvedNode3D, Rotation3D, Scene3DDiagnostic, Scene3DIR, Transform3D,
};

use fission_ir::op::{EmbedKind, LayoutOp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// Projection used by a retained 3D scene camera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CameraProjection3D {
    Perspective {
        vertical_fov_radians: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        vertical_size: f32,
        near: f32,
        far: f32,
    },
}

impl Default for CameraProjection3D {
    fn default() -> Self {
        Self::Perspective {
            vertical_fov_radians: 45.0_f32.to_radians(),
            near: 0.1,
            far: 1_000.0,
        }
    }
}

/// Explicit camera configuration carried through the scene payload.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera3D {
    pub eye: Point3D,
    pub target: Point3D,
    pub up: Point3D,
    pub projection: CameraProjection3D,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            eye: Point3D::new(4.0, 3.0, 6.0),
            target: Point3D::new(0.0, 0.0, 0.0),
            up: Point3D::new(0.0, 1.0, 0.0),
            projection: CameraProjection3D::default(),
        }
    }
}

impl Camera3D {
    pub fn perspective(
        eye: Point3D,
        target: Point3D,
        vertical_fov_radians: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            eye,
            target,
            up: Point3D::new(0.0, 1.0, 0.0),
            projection: CameraProjection3D::Perspective {
                vertical_fov_radians,
                near,
                far,
            },
        }
    }

    pub fn orthographic(
        eye: Point3D,
        target: Point3D,
        vertical_size: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            eye,
            target,
            up: Point3D::new(0.0, 1.0, 0.0),
            projection: CameraProjection3D::Orthographic {
                vertical_size,
                near,
                far,
            },
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        if !self.eye.is_finite() || !self.target.is_finite() || !self.up.is_finite() {
            return false;
        }
        let view = Point3D::new(
            self.target.x - self.eye.x,
            self.target.y - self.eye.y,
            self.target.z - self.eye.z,
        );
        let eye_target_distance = view.x.powi(2) + view.y.powi(2) + view.z.powi(2);
        let up_length = self.up.x.powi(2) + self.up.y.powi(2) + self.up.z.powi(2);
        if eye_target_distance <= f32::EPSILON || up_length <= f32::EPSILON {
            return false;
        }
        let view_cross_up = Point3D::new(
            view.y * self.up.z - view.z * self.up.y,
            view.z * self.up.x - view.x * self.up.z,
            view.x * self.up.y - view.y * self.up.x,
        );
        let cross_length =
            view_cross_up.x.powi(2) + view_cross_up.y.powi(2) + view_cross_up.z.powi(2);
        if !cross_length.is_finite()
            || cross_length <= f32::EPSILON * eye_target_distance * up_length
        {
            return false;
        }
        match self.projection {
            CameraProjection3D::Perspective {
                vertical_fov_radians,
                near,
                far,
            } => {
                vertical_fov_radians.is_finite()
                    && vertical_fov_radians > 0.0
                    && vertical_fov_radians < std::f32::consts::PI
                    && near.is_finite()
                    && far.is_finite()
                    && near > 0.0
                    && far > near
            }
            CameraProjection3D::Orthographic {
                vertical_size,
                near,
                far,
            } => {
                vertical_size.is_finite()
                    && vertical_size > 0.0
                    && near.is_finite()
                    && far.is_finite()
                    && far > near
            }
        }
    }
}

impl Point3D {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Primitive3D {
    Cube {
        center: Point3D,
        size: f32,
        color: Color,
    },
    Sphere {
        center: Point3D,
        radius: f32,
        color: Color,
    },
    Mesh {
        vertices: Vec<Point3D>,
        indices: Vec<u32>,
        color: Color,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene3D {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub camera: Camera3D,
    pub primitives: Vec<Primitive3D>,
    pub nodes: Vec<Node3D>,
    resolved_nodes: Vec<ResolvedNode3D>,
}

impl Scene3D {
    pub fn new() -> Self {
        Self {
            width: None,
            height: None,
            camera: Camera3D::default(),
            primitives: Vec::new(),
            nodes: Vec::new(),
            resolved_nodes: Vec::new(),
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

    pub fn camera(mut self, camera: Camera3D) -> Self {
        self.camera = camera;
        self
    }

    pub fn add_node(mut self, node: Node3D) -> Self {
        self.nodes.push(node);
        self
    }

    /// Runs structural validation and resolves retained node transforms.
    pub fn finish(&self) -> Scene3DIR {
        scene_graph::resolve_nodes(&self.nodes)
    }

    pub(crate) fn render_nodes(&self) -> std::borrow::Cow<'_, [ResolvedNode3D]> {
        if self.resolved_nodes.is_empty() && !self.nodes.is_empty() {
            std::borrow::Cow::Owned(self.finish().nodes)
        } else {
            std::borrow::Cow::Borrowed(&self.resolved_nodes)
        }
    }
}

/// Versioned renderer payload for one embedded 3D scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene3DPayload {
    pub magic: [u8; 4],
    pub version: u16,
    pub camera: Camera3D,
    pub primitives: Vec<Primitive3D>,
}

impl Scene3DPayload {
    pub const MAGIC: [u8; 4] = *b"F3D\0";
    pub const VERSION: u16 = 1;

    pub fn from_scene(scene: &Scene3D) -> Self {
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            camera: scene.camera,
            primitives: scene.primitives.clone(),
        }
    }

    pub fn into_scene(self, width: f32, height: f32) -> Option<Scene3D> {
        (self.magic == Self::MAGIC && self.version == Self::VERSION && self.camera.is_valid())
            .then_some(Scene3D {
                width: Some(width),
                height: Some(height),
                camera: self.camera,
                primitives: self.primitives,
                nodes: Vec::new(),
                resolved_nodes: Vec::new(),
            })
    }
}

/// Current payload carrying the closed retained-node IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene3DPayloadV2 {
    pub magic: [u8; 4],
    pub version: u16,
    pub camera: Camera3D,
    pub primitives: Vec<Primitive3D>,
    pub scene_ir: Scene3DIR,
}

impl Scene3DPayloadV2 {
    pub const MAGIC: [u8; 4] = *b"F3D\0";
    pub const VERSION: u16 = 2;

    pub fn from_scene(scene: &Scene3D) -> Self {
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            camera: scene.camera,
            primitives: scene.primitives.clone(),
            scene_ir: scene.finish(),
        }
    }

    pub fn into_scene(self, width: f32, height: f32) -> Option<Scene3D> {
        (self.magic == Self::MAGIC && self.version == Self::VERSION && self.camera.is_valid())
            .then_some(Scene3D {
                width: Some(width),
                height: Some(height),
                camera: self.camera,
                primitives: self.primitives,
                nodes: Vec::new(),
                resolved_nodes: self.scene_ir.nodes,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{Camera3D, CameraProjection3D, Point3D, Scene3D, Scene3DPayload};

    #[test]
    fn scene_payload_round_trips_camera_and_bounds() {
        let camera = Camera3D::orthographic(
            Point3D::new(0.0, 4.0, 8.0),
            Point3D::new(0.0, 0.0, 0.0),
            12.0,
            0.1,
            100.0,
        );
        let encoded =
            bincode::serialize(&Scene3DPayload::from_scene(&Scene3D::new().camera(camera)))
                .expect("payload should serialize");
        let decoded: Scene3DPayload =
            bincode::deserialize(&encoded).expect("payload should deserialize");
        let scene = decoded
            .into_scene(640.0, 480.0)
            .expect("current payload should be accepted");

        assert_eq!(scene.width, Some(640.0));
        assert_eq!(scene.height, Some(480.0));
        assert_eq!(scene.camera, camera);
    }

    #[test]
    fn scene_payload_rejects_unknown_version() {
        let payload = Scene3DPayload {
            magic: Scene3DPayload::MAGIC,
            version: Scene3DPayload::VERSION + 1,
            camera: Camera3D::default(),
            primitives: Vec::new(),
        };

        assert!(payload.into_scene(640.0, 480.0).is_none());
    }

    #[test]
    fn camera_rejects_degenerate_view_bases() {
        let same_eye_and_target = Camera3D::perspective(
            Point3D::new(1.0, 2.0, 3.0),
            Point3D::new(1.0, 2.0, 3.0),
            1.0,
            0.1,
            100.0,
        );
        assert!(!same_eye_and_target.is_valid());

        let parallel_up = Camera3D {
            eye: Point3D::new(0.0, 0.0, 2.0),
            target: Point3D::new(0.0, 0.0, 0.0),
            up: Point3D::new(0.0, 0.0, 1.0),
            projection: CameraProjection3D::default(),
        };
        assert!(!parallel_up.is_valid());
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

        let payload = bincode::serialize(&Scene3DPayloadV2::from_scene(&self.scene))
            .expect("Scene3D payload serialization is infallible for owned scene data");
        let op = fission_ir::Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload),
            widget_id: fission_ir::WidgetId::explicit("fission_3d_scene"),
            width: Some(w),
            height: Some(h),
        });

        cx.insert_node(node_id, op, vec![])
    }
}
