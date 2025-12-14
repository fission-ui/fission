use crate::{node::Node, Desugar, WidgetNodeId};
use fission_core::{ActionEnvelope, Env, InteractionStateMap, lowering::LoweringContext};
use fission_ir::{
    op::{Color as IrColor, Fill, LayoutOp, Op, PaintOp},
    ActionEntry, ActionSet, NodeId, Role, Semantics, 
};
use fission_theme::{ButtonTheme, Theme};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Button {
    pub id: Option<WidgetNodeId>,
    pub child: Option<Box<Node>>,
    pub on_press: Option<ActionEnvelope>,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl Button {
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

    // Helper to resolve the button's visual style based on theme and interaction state
    fn resolve_style(&self, env: &Env, interaction: &InteractionStateMap, self_id: NodeId) -> ButtonStyleResolved {
        let default_style = &env.theme.components.button;
        let tokens = &env.theme.tokens.colors;
        
        let is_hovered = interaction.is_hovered(self_id);
        let is_pressed = interaction.is_pressed(self_id);

        let bg_color = if is_pressed {
            tokens.primary // Example: Darker primary when pressed
        } else if is_hovered {
            tokens.surface // Example: Lighter surface when hovered
        } else {
            tokens.primary // Default primary color
        };

        let text_color = tokens.on_primary;

        ButtonStyleResolved {
            background_color: bg_color,
            text_color,
            padding_horizontal: default_style.padding_horizontal,
            height: default_style.height,
            corner_radius: default_style.radius,
            // shadow: None, // Removed
        }
    }
}

// Temporary struct to hold resolved style for a button (combines theme + overrides + state)
struct ButtonStyleResolved {
    background_color: IrColor,
    text_color: IrColor,
    padding_horizontal: f32,
    height: f32,
    corner_radius: f32,
    // shadow: Option<BoxShadow>, // Removed
}

impl Desugar for Button {
    fn desugar(&self, cx: &mut LoweringContext) -> NodeId {
        let button_id = self.id.unwrap_or_else(|| cx.next_node_id());

        let resolved_style = self.resolve_style(cx.env, &cx.runtime_state.interaction, button_id);

        // 1. Create a main LayoutOp::Box for the button's overall size and shape
        let button_layout_id = cx.next_node_id();
        cx.add_node(
            button_layout_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height.or(Some(resolved_style.height)),
            }),
            vec![],
        );

        // 2. Create the background rect (AbsoluteFill) as child of button_layout_id
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
                // shadow: resolved_style.shadow, // Removed
            }),
            vec![],
        );

        // Attach background to button_layout_id
        if let Some(layout_node) = cx.ir.nodes.get_mut(&button_layout_id) {
            layout_node.children.push(background_id);
            if let Some(bg_node) = cx.ir.nodes.get_mut(&background_id) {
                bg_node.parent = Some(button_layout_id);
            }
        }

        // 3. Desugar child (Text) and add as child of button_layout_id
        let mut child_node_ids = Vec::new(); // Renamed to avoid collision with child_ids
        if let Some(child_widget) = &self.child {
            // If child is Text, override its color from theme.
            if let Node::Text(mut text_widget) = *child_widget.clone() {
                text_widget.color = Some(resolved_style.text_color);
                child_node_ids.push(text_widget.desugar(cx));
            } else { // Other children desugar normally
                child_node_ids.push(child_widget.desugar(cx));
            }
        }

        // Add children to button_layout_id
        if let Some(layout_node) = cx.ir.nodes.get_mut(&button_layout_id) {
            layout_node.children.extend(child_node_ids.iter().cloned()); // Clone to use in next loop
            for child_id in &child_node_ids { // Iterate over cloned to set parents
                if let Some(child_node) = cx.ir.nodes.get_mut(child_id) {
                    child_node.parent = Some(button_layout_id);
                }
            }
        }

        // Semantics wrap the main button_layout_id
        if let Some(semantics_op) = self.build_semantics() {
            let semantics_id = self.id.unwrap_or_else(|| cx.next_node_id()); // Reuse button_id for semantics
            cx.add_node(semantics_id, Op::Semantics(semantics_op), vec![button_layout_id]);
            return semantics_id;
        }

        button_layout_id
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
