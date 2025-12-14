use crate::lowering::LoweringContext;
use crate::{ActionEnvelope, Env, InteractionStateMap};
use fission_ir::{
    op::{Color as IrColor, Fill, LayoutOp, Op, PaintOp, FlexDirection, BoxShadow},
    ActionEntry, ActionSet, NodeId, Role, Semantics
};
use fission_theme::{ButtonTheme, Theme};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::fmt::Debug;

pub trait Lower {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId;
}

pub trait LowerDyn: Send + Sync + Debug {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId;
    fn stable_key(&self) -> u64 { 0 }
}

#[derive(Clone, Debug, Serialize, Deserialize)] // Removed PartialEq
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

#[derive(Clone, Debug, Serialize, Deserialize)] // Removed PartialEq, Default
pub struct CustomNode {
    pub debug_tag: String,
    #[serde(skip)]
    pub lowerer: Option<Arc<dyn LowerDyn>>,
}

// --- Primitives ---

#[derive(Debug, Default, Clone, Serialize, Deserialize)] // Removed PartialEq
pub struct Row {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub semantics: Option<Semantics>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Lower for Row {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let mut child_ids = Vec::new();
        for child in &self.children {
            child_ids.push(child.lower(cx));
        }

        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                padding: [0.0; 4],
            }),
            child_ids,
        );

        if let Some(s) = &self.semantics {
            let semantics_id = cx.next_node_id();
            cx.add_node(semantics_id, Op::Semantics(s.clone()), vec![layout_id]);
            return semantics_id;
        }

        layout_id
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)] // Removed PartialEq
pub struct Column {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub semantics: Option<Semantics>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Lower for Column {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let mut child_ids = Vec::new();
        for child in &self.children {
            child_ids.push(child.lower(cx));
        }

        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                padding: [0.0; 4],
            }),
            child_ids,
        );

        if let Some(s) = &self.semantics {
            let semantics_id = cx.next_node_id();
            cx.add_node(semantics_id, Op::Semantics(s.clone()), vec![layout_id]);
            return semantics_id;
        }

        layout_id
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextContent {
    Literal(String),
    Key(String),
}

impl Default for TextContent {
    fn default() -> Self {
        TextContent::Literal(String::new())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)] // Removed PartialEq
pub struct Text {
    pub id: Option<NodeId>,
    pub content: TextContent,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub font_size: Option<f32>,
    pub color: Option<IrColor>,
}

impl Lower for Text {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_node_id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.add_node(
            layout_node_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                padding: [0.0; 4],
            }),
            vec![],
        );

        let resolved_text = match &self.content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
        };

        let paint_node_id = cx.next_node_id();
        cx.add_node(
            paint_node_id,
            Op::Paint(PaintOp::DrawText {
                text: resolved_text,
                size: self.font_size.unwrap_or(14.0),
                color: self.color.unwrap_or(IrColor::BLACK),
            }),
            vec![],
        );

        if let Some(layout_node) = cx.ir.nodes.get_mut(&layout_node_id) {
            layout_node.children.push(paint_node_id);
            if let Some(paint_node) = cx.ir.nodes.get_mut(&paint_node_id) {
                paint_node.parent = Some(layout_node_id);
            }
        }

        if let Some(s) = &self.semantics {
            let semantics_id = cx.next_node_id();
            cx.add_node(semantics_id, Op::Semantics(s.clone()), vec![layout_node_id]);
            return semantics_id;
        }

        layout_node_id
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)] // Removed PartialEq
pub struct Button {
    pub id: Option<NodeId>,
    pub child: Option<Box<Node>>,
    pub on_press: Option<ActionEnvelope>,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub style: Option<ButtonStyleOverride>, // Placeholder for overrides
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonStyleOverride {
    // Add fields as needed, e.g. color override
}

// Temporary struct for resolved style
struct ButtonStyleResolved {
    background_color: IrColor,
    text_color: IrColor,
    padding_horizontal: f32,
    height: f32,
    corner_radius: f32,
    shadow: Option<BoxShadow>,
}

impl Button {
    fn resolve_style(&self, env: &Env, interaction: &InteractionStateMap, self_id: NodeId) -> ButtonStyleResolved {
        let default_style = &env.theme.components.button;
        let tokens = &env.theme.tokens.colors;
        
        let is_hovered = interaction.is_hovered(self_id);
        let is_pressed = interaction.is_pressed(self_id);

        let bg_color = tokens.primary;

        let text_color = tokens.on_primary;

        let shadow = if is_pressed {
            default_style.elevation_pressed
        } else if is_hovered {
            default_style.elevation_hover
        } else {
            default_style.elevation_rest
        };

        ButtonStyleResolved {
            background_color: bg_color,
            text_color,
            padding_horizontal: default_style.padding_horizontal,
            height: default_style.height,
            corner_radius: default_style.radius,
            shadow,
        }
    }

    fn should_attach_semantics(&self) -> bool {
        self.semantics.is_some() || self.on_press.is_some()
    }

    fn build_semantics(&self) -> Option<Semantics> {
        if !self.should_attach_semantics() {
            return None;
        }

        let mut semantics = self
            .semantics
            .clone()
            .unwrap_or_else(default_button_semantics);

        if let Some(action_envelope) = &self.on_press {
            semantics.actions.entries.push(ActionEntry {
                action_id: action_envelope.id.as_u128(),
                payload_data: Some(action_envelope.payload.clone()),
            });
        }

        Some(semantics)
    }
}

impl Lower for Button {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let button_id = self.id.unwrap_or_else(|| cx.next_node_id());

        let resolved_style = self.resolve_style(cx.env, &cx.runtime_state.interaction, button_id);

        cx.add_node(
            button_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height.or(Some(resolved_style.height)),
                padding: [resolved_style.padding_horizontal, resolved_style.padding_horizontal, 0.0, 0.0],
            }),
            vec![],
        );

        let background_id = cx.next_node_id();
        cx.add_node(
            background_id,
            Op::Layout(LayoutOp::AbsoluteFill),
            vec![],
        );
        cx.add_node(
            background_id,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: resolved_style.background_color }),
                stroke: None,
                corner_radius: resolved_style.corner_radius,
                shadow: resolved_style.shadow,
            }),
            vec![],
        );

        if let Some(layout_node) = cx.ir.nodes.get_mut(&button_id) {
            layout_node.children.push(background_id);
            if let Some(bg_node) = cx.ir.nodes.get_mut(&background_id) {
                bg_node.parent = Some(button_id);
            }
        }

        let mut child_node_ids = Vec::new();
        if let Some(child_widget) = &self.child {
            if let Node::Text(mut text_widget) = *child_widget.clone() {
                text_widget.color = Some(resolved_style.text_color);
                child_node_ids.push(text_widget.lower(cx));
            } else { 
                child_node_ids.push(child_widget.lower(cx));
            }
        }

        if let Some(layout_node) = cx.ir.nodes.get_mut(&button_id) {
            layout_node.children.extend(child_node_ids.iter().cloned());
            for child_id in &child_node_ids {
                if let Some(child_node) = cx.ir.nodes.get_mut(child_id) {
                    child_node.parent = Some(button_id);
                }
            }
        }

        if let Some(semantics_op) = self.build_semantics() {
            let semantics_id = cx.next_node_id(); 
            cx.add_node(semantics_id, Op::Semantics(semantics_op), vec![button_id]);
            return semantics_id;
        }

        button_id
    }
}

fn default_button_semantics() -> Semantics {
    Semantics {
        role: Role::Button,
        label: None,
        value: None,
        actions: ActionSet::default(),
        focusable: true,
    }
}
