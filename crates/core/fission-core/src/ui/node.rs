use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::lowering::LoweringContext;
use fission_ir::NodeId;
use super::traits::{Lower, LowerDyn};
use super::widgets::{Button, Text, Row, Column, Scroll, Image};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Node {
    Row(Row),
    Column(Column),
    Text(Text),
    Button(Button),
    Scroll(Scroll),
    Image(Image),
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
            Node::Custom(w) => w.lowerer.as_ref().expect("CustomNode lowerer must be set").lower_dyn(cx),
        }
    }
}

impl From<Row> for Node { fn from(w: Row) -> Self { Node::Row(w) } }
impl From<Column> for Node { fn from(w: Column) -> Self { Node::Column(w) } }
impl From<Text> for Node { fn from(w: Text) -> Self { Node::Text(w) } }
impl From<Button> for Node { fn from(w: Button) -> Self { Node::Button(w) } }
impl From<Scroll> for Node { fn from(w: Scroll) -> Self { Node::Scroll(w) } }
impl From<Image> for Node { fn from(w: Image) -> Self { Node::Image(w) } }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn LowerDyn>>,
}
