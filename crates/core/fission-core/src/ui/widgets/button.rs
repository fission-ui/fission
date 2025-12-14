use serde::{Deserialize, Serialize};
use crate::lowering::LoweringContext;
use crate::{ActionEnvelope, Env, InteractionStateMap};
use fission_ir::{
    op::{Color as IrColor, Fill, LayoutOp, Op, PaintOp, BoxShadow, Stroke},
    ActionEntry, ActionSet, NodeId, Role, Semantics
};
use crate::ui::Node;
use crate::ui::traits::Lower;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Button {
    pub id: Option<NodeId>,
    pub child: Option<Box<Node>>,
    pub on_press: Option<ActionEnvelope>,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub style: Option<ButtonStyleOverride>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonStyleOverride {
}

struct ButtonStyleResolved {
    background_color: IrColor,
    text_color: IrColor,
    padding_horizontal: f32,
    height: f32,
    corner_radius: f32,
    shadow: Option<BoxShadow>,
    stroke: Option<Stroke>,
}

impl Button {
    fn resolve_style(&self, env: &Env, interaction: &InteractionStateMap, self_id: NodeId) -> ButtonStyleResolved {
        let default_style = &env.theme.components.button;
        let tokens = &env.theme.tokens.colors;
        
        let is_hovered = interaction.is_hovered(self_id);
        let is_pressed = interaction.is_pressed(self_id);
        let is_focused = interaction.is_focused(self_id);

        let bg_color = tokens.primary;
        let text_color = tokens.on_primary;

        let shadow = if is_pressed {
            default_style.elevation_pressed
        } else if is_hovered {
            default_style.elevation_hover
        } else {
            default_style.elevation_rest
        };

        let stroke = if is_focused {
            default_style.focus_stroke
        } else {
            None
        };

        ButtonStyleResolved {
            background_color: bg_color,
            text_color,
            padding_horizontal: default_style.padding_horizontal,
            height: default_style.height,
            corner_radius: default_style.radius,
            shadow,
            stroke,
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
                stroke: resolved_style.stroke,
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
