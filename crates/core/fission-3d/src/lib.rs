pub mod render;
use fission_core::{BuildCtx, View, Widget};
use fission_core::ui::{CustomNode, Node, Container};
use fission_core::op::Color;
use fission_ir::op::{EmbedKind, LayoutOp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Primitive3D {
    Cube { center: Point3D, size: f32, color: Color },
    Sphere { center: Point3D, radius: f32, color: Color },
    Mesh { vertices: Vec<Point3D>, indices: Vec<u32>, color: Color },
}

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

impl<S: fission_core::AppState> Widget<S> for Scene3D {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        let mut container = Container::new(
            Node::Custom(CustomNode {
                debug_tag: "fission_3d::Scene3D".into(),
                lowerer: Some(std::sync::Arc::new(Scene3DLowerer {
                    scene: self.clone(),
                })),
                render_object: None,
            })
        );
        if let Some(w) = self.width { container = container.width(w); } else { container = container.flex_grow(1.0); }
        if let Some(h) = self.height { container = container.height(h); } else { if self.width.is_none() { container = container.flex_grow(1.0); } }
        container.into_node()
    }
}

#[derive(Debug)]
pub struct Scene3DLowerer {
    pub scene: Scene3D,
}

impl fission_core::ui::traits::LowerDyn for Scene3DLowerer {
    fn lower_dyn(&self, cx: &mut fission_core::lowering::LoweringContext) -> fission_ir::NodeId {
        let node_id = cx.next_node_id();
        
        let w = self.scene.width.unwrap_or_else(|| {
            (cx.env.viewport_size.width - 264.0).max(400.0)
        });
        let h = self.scene.height.unwrap_or_else(|| {
            (cx.env.viewport_size.height - 200.0).max(300.0)
        });

        // In a real implementation, this would emit an EmbedKind::Surface3D
        // and fission-shell-desktop would intercept it to render a wgpu scene
        // For this milestone, we emit a 3D placeholder layout op.
        
        let payload = bincode::serialize(&self.scene.primitives).unwrap_or_default();
        let op = fission_ir::Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload),
            widget_id: fission_ir::WidgetNodeId::explicit("fission_3d_scene"),
            width: Some(w),
            height: Some(h),
        });

        cx.insert_node(node_id, op, vec![])
    }
}
