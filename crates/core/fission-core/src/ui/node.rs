use super::custom_render::CustomRenderObject;
use super::traits::{Lower, LowerDyn};
use super::widgets::{
    Align, Button, Checkbox, Clip, Column, Container, GestureDetector, FocusScope, Grid, GridItem, Icon, Image, LazyColumn, Overlay, Positioned, Radio, Row, SafeArea, Scroll, Slider, Spacer,
    Switch, Text, TextInput, Transform, Video, ZStack,
};
use crate::lowering::LoweringContext;
use fission_ir::{NodeId, Op, StructuralOp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Node {
    Row(Row),
    Column(Column),
    Align(Align),
    FocusScope(FocusScope),
    Clip(Clip),
    Text(Text),
    Transform(Transform),
    Button(Button),
    TextInput(TextInput),
    Scroll(Scroll),
    Image(Image),
    Video(Video),
    ZStack(ZStack),
    Overlay(Overlay),
    Container(Container),
    GestureDetector(GestureDetector),
    Grid(Grid),
    GridItem(GridItem),
    Checkbox(Checkbox),
    Switch(Switch),
    Radio(Radio),
    SafeArea(SafeArea),
    Positioned(Positioned),
    Spacer(Spacer),
    Slider(Slider),
    LazyColumn(LazyColumn),
    Icon(Icon),
    Custom(CustomNode),
}

impl Node {
    pub fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        match self {
            Node::Row(w) => w.lower(cx),
            Node::Column(w) => w.lower(cx),
            Node::Align(w) => w.lower(cx),
            Node::FocusScope(w) => w.lower(cx),
            Node::Clip(w) => w.lower(cx),
            Node::Text(w) => w.lower(cx),
            Node::Transform(w) => w.lower(cx),
            Node::Button(w) => w.lower(cx),
            Node::TextInput(w) => w.lower(cx),
            Node::Scroll(w) => w.lower(cx),
            Node::Image(w) => w.lower(cx),
            Node::Video(w) => w.lower(cx),
            Node::ZStack(w) => w.lower(cx),
            Node::Overlay(w) => w.lower(cx),
            Node::Container(w) => w.lower(cx),
            Node::GestureDetector(w) => w.lower(cx),
            Node::Grid(w) => w.lower(cx),
            Node::GridItem(w) => w.lower(cx),
            Node::Checkbox(w) => w.lower(cx),
            Node::Switch(w) => w.lower(cx),
            Node::Radio(w) => w.lower(cx),
            Node::SafeArea(w) => w.lower(cx),
            Node::Positioned(w) => w.lower(cx),
            Node::Spacer(w) => w.lower(cx),
            Node::Slider(w) => w.lower(cx),
            Node::LazyColumn(w) => w.lower(cx),
            Node::Icon(w) => w.lower(cx),
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
                let node_id = builder.build(cx);

                // If the custom node carries a render object, store it in the
                // IR so that hit-testing and event handling can find it later.
                // We wrap the `Arc<dyn CustomRenderObject>` in a `RenderObjectHolder`
                // so it can be stored as `Arc<dyn Any + Send + Sync>` in the
                // dependency-free IR crate and downcast back later.
                if let Some(render_obj) = &w.render_object {
                    let holder = crate::ui::custom_render::RenderObjectHolder(render_obj.clone());
                    let erased: fission_ir::AnyRenderObject = Arc::new(holder);
                    // Register the render object at the wrapper AND every node in
                    // the lowered subtree so the parent-walk from any hit descendant
                    // finds it regardless of tree depth.
                    cx.ir.custom_render_objects.insert(node_id, erased.clone());
                    fn register_subtree(ir: &mut fission_ir::CoreIR, node_id: fission_ir::NodeId, erased: &fission_ir::AnyRenderObject) {
                        ir.custom_render_objects.insert(node_id, erased.clone());
                        if let Some(children) = ir.nodes.get(&node_id).map(|n| n.children.clone()) {
                            for child_id in children {
                                register_subtree(ir, child_id, erased);
                            }
                        }
                    }
                    register_subtree(&mut cx.ir, child_id, &erased);
                }

                node_id
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
impl From<Align> for Node {
    fn from(w: Align) -> Self {
        Node::Align(w)
    }
}
impl From<FocusScope> for Node {
    fn from(w: FocusScope) -> Self {
        Node::FocusScope(w)
    }
}
impl From<Clip> for Node {
    fn from(w: Clip) -> Self {
        Node::Clip(w)
    }
}
impl From<Text> for Node {
    fn from(w: Text) -> Self {
        Node::Text(w)
    }
}
impl From<Transform> for Node {
    fn from(w: Transform) -> Self {
        Node::Transform(w)
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
impl From<GestureDetector> for Node {
    fn from(w: GestureDetector) -> Self {
        Node::GestureDetector(w)
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
impl From<SafeArea> for Node {
    fn from(w: SafeArea) -> Self {
        Node::SafeArea(w)
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
impl From<Icon> for Node {
    fn from(w: Icon) -> Self {
        Node::Icon(w)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn LowerDyn>>,
    /// Optional render object that participates in hit-testing, event handling,
    /// and painting.  When `None`, the node behaves exactly as before (lowering
    /// only via `LowerDyn`).
    #[serde(skip)]
    pub render_object: Option<Arc<dyn CustomRenderObject>>,
}
