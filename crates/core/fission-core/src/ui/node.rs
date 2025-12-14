use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::lowering::LoweringContext;
use fission_ir::NodeId;
use super::traits::{Lower, LowerDyn};
use super::widgets::{Button, Text, Row, Column};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Node {
    Row(Row),
    Column(Column),
    Text(Text),
    Button(Button),
    Custom(CustomNode),
}

impl Node {
    pub fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        match self {
            Node::Row(w) => w.lower(cx),
            Node::Column(w) => w.lower(cx),
            Node::Text(w) => w.lower(cx),
            Node::Button(w) => w.lower(cx),
            Node::Custom(w) => w.lowerer.as_ref().expect("CustomNode lowerer must be set").lower_dyn(cx),
        }
    }
}

impl From<Row> for Node { fn from(w: Row) -> Self { Node::Row(w) } }
impl From<Column> for Node { fn from(w: Column) -> Self { Node::Column(w) } }
impl From<Text> for Node { fn from(w: Text) -> Self { Node::Text(w) } }
impl From<Button> for Node { fn from(w: Button) -> Self { Node::Button(w) } }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn LowerDyn>>,
}
