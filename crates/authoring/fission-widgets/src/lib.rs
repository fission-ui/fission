pub use fission_core::ui::{Button, Checkbox, Column, Container, CustomNode, Grid, GridItem, Image, Node, Overlay, Radio, Row, Scroll, Switch, ZStack, Text, TextContent, TextInput, Video};
pub use fission_core::view::{Selector, View, Widget};
pub use fission_core::BuildCtx;

pub mod dropdown;
pub use dropdown::DropDown;

pub mod stack;
pub use stack::{HStack, VStack};

pub mod badge;
pub use badge::Badge;

pub mod tag;
pub use tag::Tag;

pub mod avatar;
pub use avatar::Avatar;

pub mod divider;
pub use divider::Divider;

pub mod card;
pub use card::Card;

pub mod progress;
pub use progress::ProgressBar;

pub mod spinner;
pub use spinner::Spinner;

pub mod tabs;
pub use tabs::{Tabs, TabItem};

pub mod accordion;
pub use accordion::{Accordion, AccordionItem};

pub mod positioned;
pub use positioned::Positioned;

pub mod popover;
pub use popover::Popover;

use fission_core::{lowering::NodeBuilder, op::StructuralOp, LowerDyn, LoweringContext, NodeId, Op};
use std::sync::Arc;

// Canvas (CustomPaint) convenience
pub struct CanvasLowerer {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub painter: Arc<dyn Fn(&mut LoweringContext) -> Vec<NodeId> + Send + Sync>,
}

impl std::fmt::Debug for CanvasLowerer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasLowerer")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl LowerDyn for CanvasLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let root = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            }),
        )
        .build(cx);

        let child_ids = (self.painter)(cx);
        let mut wrapper = NodeBuilder::new(root, Op::Structural(StructuralOp::Group { stable_hash: 0 }));
        for cid in child_ids {
            wrapper.add_child(cid);
        }
        wrapper.build(cx)
    }
}

pub fn canvas<F>(width: Option<f32>, height: Option<f32>, painter: F) -> Node
where
    F: Fn(&mut LoweringContext) -> Vec<NodeId> + Send + Sync + 'static,
{
    Node::Custom(fission_core::CustomNode {
        debug_tag: "Canvas".into(),
        lowerer: Some(Arc::new(CanvasLowerer {
            width,
            height,
            painter: Arc::new(painter),
        })),
    })
}

// Portal
#[derive(Debug, Clone)]
pub struct Portal {
    pub child: Node,
}

impl<S: fission_core::AppState> Widget<S> for Portal {
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        ctx.register_portal(self.child.clone());
        // Return invisible spacer
        Node::Custom(fission_core::CustomNode {
            debug_tag: "Spacer".into(),
            lowerer: Some(Arc::new(SizedBoxLowerer { width: None, height: None })),
        })
    }
}

// Spacer Helper
#[derive(Debug)]
struct SizedBoxLowerer {
    width: Option<f32>,
    height: Option<f32>,
}

impl LowerDyn for SizedBoxLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            }),
        )
        .build(cx)
    }
}