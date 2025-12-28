use super::traits::{Lower, LowerDyn};
use super::widgets::{
    Button, Checkbox, Column, Container, Grid, GridItem, Image, LazyColumn, Overlay, Positioned, Radio, Row, Scroll, Slider, Spacer,
    Switch, Text, TextInput, Video, ZStack,
};
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
    TextInput(TextInput),
    Scroll(Scroll),
    Image(Image),
    Video(Video),
    ZStack(ZStack),
    Overlay(Overlay),
    Container(Container),
    Grid(Grid),
    GridItem(GridItem),
    Checkbox(Checkbox),
    Switch(Switch),
    Radio(Radio),
    Positioned(Positioned),
    Spacer(Spacer),
    Slider(Slider),
    LazyColumn(LazyColumn),
    Custom(CustomNode),
}

impl Node {
    pub fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        match self {
            Node::Row(w) => w.lower(cx),
            Node::Column(w) => w.lower(cx),
            Node::Text(w) => w.lower(cx),
            Node::Button(w) => w.lower(cx),
            Node::TextInput(w) => w.lower(cx),
            Node::Scroll(w) => w.lower(cx),
            Node::Image(w) => w.lower(cx),
            Node::Video(w) => w.lower(cx),
            Node::ZStack(w) => w.lower(cx),
            Node::Overlay(w) => w.lower(cx),
            Node::Container(w) => w.lower(cx),
            Node::Grid(w) => w.lower(cx),
            Node::GridItem(w) => w.lower(cx),
            Node::Checkbox(w) => w.lower(cx),
            Node::Switch(w) => w.lower(cx),
            Node::Radio(w) => w.lower(cx),
            Node::Positioned(w) => w.lower(cx),
            Node::Spacer(w) => w.lower(cx),
            Node::Slider(w) => w.lower(cx),
            Node::LazyColumn(w) => w.lower(cx),
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
impl From<TextInput> for Node {
    fn from(w: TextInput) -> Self {
        Node::TextInput(w)
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
impl From<ZStack> for Node {
    fn from(w: ZStack) -> Self {
        Node::ZStack(w)
    }
}
impl From<Overlay> for Node {
    fn from(w: Overlay) -> Self {
        Node::Overlay(w)
    }
}
impl From<Container> for Node {
    fn from(w: Container) -> Self {
        Node::Container(w)
    }
}
impl From<Grid> for Node {
    fn from(w: Grid) -> Self {
        Node::Grid(w)
    }
}
impl From<GridItem> for Node {
    fn from(w: GridItem) -> Self {
        Node::GridItem(w)
    }
}
impl From<Checkbox> for Node {
    fn from(w: Checkbox) -> Self {
        Node::Checkbox(w)
    }
}
impl From<Switch> for Node {
    fn from(w: Switch) -> Self {
        Node::Switch(w)
    }
}
impl From<Radio> for Node {
    fn from(w: Radio) -> Self {
        Node::Radio(w)
    }
}
impl From<Positioned> for Node {
    fn from(w: Positioned) -> Self {
        Node::Positioned(w)
    }
}
impl From<Spacer> for Node {
    fn from(w: Spacer) -> Self {
        Node::Spacer(w)
    }
}
impl From<Slider> for Node {
    fn from(w: Slider) -> Self {
        Node::Slider(w)
    }
}
impl From<LazyColumn> for Node {
    fn from(w: LazyColumn) -> Self {
        Node::LazyColumn(w)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn LowerDyn>>,
}
