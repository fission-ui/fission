use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{CompositeScalar, CompositeStyle, NodeId, Op, StructuralOp, WidgetNodeId};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Composite {
    pub id: Option<NodeId>,
    pub style: CompositeStyle,
    pub child: Box<Node>,
}

impl Default for Composite {
    fn default() -> Self {
        Self {
            id: None,
            style: CompositeStyle::default(),
            child: Box::new(crate::ui::widgets::spacer::Spacer::default().into_node()),
        }
    }
}

impl Composite {
    pub fn new(child: Node) -> Self {
        Self {
            child: Box::new(child),
            ..Default::default()
        }
    }

    pub fn opacity(mut self, value: f32) -> Self {
        self.style.opacity = Some(CompositeScalar::new(value));
        self
    }

    pub fn animated_opacity(mut self, target: WidgetNodeId, base: f32) -> Self {
        self.style.opacity = Some(CompositeScalar::new(base).animated(target));
        self
    }

    pub fn translate_x(mut self, value: f32) -> Self {
        self.style.translate_x = Some(CompositeScalar::new(value));
        self
    }

    pub fn animated_translate_x(mut self, target: WidgetNodeId, base: f32) -> Self {
        self.style.translate_x = Some(CompositeScalar::new(base).animated(target));
        self
    }

    pub fn translate_y(mut self, value: f32) -> Self {
        self.style.translate_y = Some(CompositeScalar::new(value));
        self
    }

    pub fn animated_translate_y(mut self, target: WidgetNodeId, base: f32) -> Self {
        self.style.translate_y = Some(CompositeScalar::new(base).animated(target));
        self
    }

    pub fn scale(mut self, value: f32) -> Self {
        self.style.scale = Some(CompositeScalar::new(value));
        self
    }

    pub fn animated_scale(mut self, target: WidgetNodeId, base: f32) -> Self {
        self.style.scale = Some(CompositeScalar::new(base).animated(target));
        self
    }

    pub fn rotation(mut self, value: f32) -> Self {
        self.style.rotation = Some(CompositeScalar::new(value));
        self
    }

    pub fn animated_rotation(mut self, target: WidgetNodeId, base: f32) -> Self {
        self.style.rotation = Some(CompositeScalar::new(base).animated(target));
        self
    }

    pub fn clip_to_bounds(mut self, clip: bool) -> Self {
        self.style.clip_to_bounds = clip;
        self
    }

    pub fn repaint_boundary(mut self, enabled: bool) -> Self {
        self.style.repaint_boundary = enabled;
        self
    }

    pub fn into_node(self) -> Node {
        Node::Composite(self)
    }
}

impl Lower for Composite {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        self.style.hash(&mut hasher);
        let stable_hash = hasher.finish();

        let mut builder = NodeBuilder::new(id, Op::Structural(StructuralOp::Group { stable_hash }))
            .composite(self.style.clone());
        builder.add_child(child_id);
        builder.build(cx)
    }
}
