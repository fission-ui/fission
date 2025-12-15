use super::traits::{Lower, LowerDyn};
use super::widgets::{Button, Column, Image, Row, Scroll, Text, Video};
use crate::lowering::LoweringContext;
use fission_ir::{NodeId, Op, StructuralOp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Node {
    Row(Row),
    Column(Column),
    Text(Text),
    Button(Button),
    Scroll(Scroll),
    Image(Image),
    Video(Video),
    Custom(CustomNode),
}

impl Node {
    pub fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        match self {
            Node::Row(w) => w.lower(cx),
            Node::Column(w) => w.lower(cx),
            Node::Text(w) => w.lower(cx),
            Node::Button(w) => w.lower(cx),
            Node::Scroll(w) => w.lower(cx),
            Node::Image(w) => w.lower(cx),
            Node::Video(w) => w.lower(cx),
            Node::Custom(w) => {
                let lowerer = w.lowerer.as_ref().expect("CustomNode lowerer must be set");
                let child_id = lowerer.lower_dyn(cx);
                let wrapper = cx.next_node_id();
                let mut builder = crate::lowering::NodeBuilder::new(
                    wrapper,
                    Op::Structural(StructuralOp::Group {
                        stable_hash: lowerer.stable_key(),
                    }),
                );
                builder.add_child(child_id);
                builder.build(cx)
            }
        }
    }
}

impl From<Row> for Node {
    fn from(w: Row) -> Self {
        Node::Row(w)
    }
}
impl From<Column> for Node {
    fn from(w: Column) -> Self {
        Node::Column(w)
    }
}
impl From<Text> for Node {
    fn from(w: Text) -> Self {
        Node::Text(w)
    }
}
impl From<Button> for Node {
    fn from(w: Button) -> Self {
        Node::Button(w)
    }
}
impl From<Scroll> for Node {
    fn from(w: Scroll) -> Self {
        Node::Scroll(w)
    }
}
impl From<Image> for Node {
    fn from(w: Image) -> Self {
        Node::Image(w)
    }
}
impl From<Video> for Node {
    fn from(w: Video) -> Self {
        Node::Video(w)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn LowerDyn>>,
}
