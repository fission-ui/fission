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
    pub width: f32,
    pub height: f32,
    pub primitives: Vec<Primitive3D>,
}

impl Scene3D {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            primitives: Vec::new(),
        }
    }

    pub fn add_primitive(mut self, primitive: Primitive3D) -> Self {
        self.primitives.push(primitive);
        self
    }
}

impl<S: fission_core::AppState> Widget<S> for Scene3D {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Container::new(
            Node::Custom(CustomNode {
                debug_tag: "fission_3d::Scene3D".into(),
                lowerer: Some(std::sync::Arc::new(Scene3DLowerer {
                    scene: self.clone(),
                })),
            })
        ).width(self.width).height(self.height).into_node()
    }
}

#[derive(Debug)]
pub struct Scene3DLowerer {
    pub scene: Scene3D,
}

impl fission_core::ui::traits::LowerDyn for Scene3DLowerer {
    fn lower_dyn(&self, cx: &mut fission_core::lowering::LoweringContext) -> fission_ir::NodeId {
        let node_id = cx.next_node_id();
        
        // In a real implementation, this would emit an EmbedKind::Surface3D
        // and fission-shell-desktop would intercept it to render a wgpu scene
        // For this milestone, we emit a 3D placeholder layout op.
        
        let op = fission_ir::Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom,
            widget_id: fission_ir::WidgetNodeId::explicit("fission_3d_scene"),
            width: Some(self.scene.width),
            height: Some(self.scene.height),
        });

        cx.insert_node(node_id, op, vec![])
    }
}
