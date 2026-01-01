use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use crate::{ActionEnvelope, Env, InteractionStateMap};
use fission_ir::{
    op::{BoxShadow, Color as IrColor, Fill, LayoutOp, Op, PaintOp, Stroke},
    ActionEntry, ActionSet, NodeId, Role, Semantics,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ButtonVariant {
    #[default]
    Filled,
    Outline,
    Ghost,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ButtonContentAlign {
    #[default]
    Center,
    Start,
    End,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Button {
    pub id: Option<NodeId>,
    pub child: Option<Box<Node>>,
    pub on_press: Option<ActionEnvelope>,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub padding: Option<[f32; 4]>,
    pub style: Option<ButtonStyleOverride>,
    pub variant: ButtonVariant,
    #[serde(default)]
    pub content_align: ButtonContentAlign,
    pub disabled: bool,
}

impl Button {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Button(self)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonStyleOverride {}

struct ButtonStyleResolved {
    background_color: Option<IrColor>,
    text_color: IrColor,
    padding_horizontal: f32,
    height: f32,
    corner_radius: f32,
    shadow: Option<BoxShadow>,
    stroke: Option<Stroke>,
}

impl Button {
    fn resolve_style(
        &self,
        env: &Env,
        interaction: &InteractionStateMap,
        self_id: NodeId,
    ) -> ButtonStyleResolved {
        let default_style = &env.theme.components.button;
        let tokens = &env.theme.tokens.colors;

        let is_hovered = interaction.is_hovered(self_id) && !self.disabled;
        let is_pressed = interaction.is_pressed(self_id) && !self.disabled;
        let is_focused = interaction.is_focused(self_id) && !self.disabled;

        let (bg_color, text_color, border_stroke) = if self.disabled {
            (
                if self.variant == ButtonVariant::Filled { Some(tokens.border) } else { None }, // Grey bg or transparent
                tokens.text_secondary, // Grey text
                if self.variant == ButtonVariant::Outline { Some(Stroke { color: tokens.border, width: 1.0 }) } else { None },
            )
        } else {
            match self.variant {
                ButtonVariant::Filled => (
                    Some(tokens.primary),
                    tokens.on_primary,
                    if is_focused { default_style.focus_stroke } else { None },
                ),
                ButtonVariant::Outline => (
                    if is_hovered { Some(tokens.surface) } else { None },
                    tokens.primary,
                    Some(Stroke { color: tokens.border, width: 1.0 }),
                ),
                ButtonVariant::Ghost => (
                    if is_hovered { Some(tokens.surface) } else { None },
                    tokens.primary,
                    None,
                ),
            }
        };

        let shadow = if self.variant == ButtonVariant::Filled {
            if is_pressed {
                default_style.elevation_pressed
            } else if is_hovered {
                default_style.elevation_hover
            } else {
                default_style.elevation_rest
            }
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
            stroke: border_stroke,
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
            
        semantics.disabled = self.disabled;

        if let Some(action_envelope) = &self.on_press {
            if !self.disabled {
                semantics.actions.entries.push(ActionEntry {
                    trigger: fission_ir::semantics::ActionTrigger::Default,
                    action_id: action_envelope.id.as_u128(),
                    payload_data: Some(action_envelope.payload.clone()),
                });
            }
        }

        Some(semantics)
    }
}

impl Lower for Button {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let button_id = self.id.unwrap_or_else(|| cx.next_node_id());

        let resolved_style = self.resolve_style(cx.env, &cx.runtime_state.interaction, button_id);
        
        cx.push_scope(button_id);

        let background_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: resolved_style.background_color.map(|c| Fill { color: c }),
                stroke: resolved_style.stroke,
                corner_radius: resolved_style.corner_radius,
                shadow: resolved_style.shadow,
            }),
        )
        .build(cx);

        let mut button_builder = NodeBuilder::new(
            button_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: if self.height.is_some() { None } else { Some(resolved_style.height) },
                max_height: None,
                padding: self.padding.unwrap_or([
                    resolved_style.padding_horizontal,
                    resolved_style.padding_horizontal,
                    0.0,
                    0.0,
                ]),
                flex_grow: 0.0,
                flex_shrink: 1.0, // Allow shrink
                aspect_ratio: None,
            }),
        );
        button_builder.add_child(background_id);

        if let Some(child_widget) = &self.child {
            let child_id = if let Node::Text(mut text_widget) = *child_widget.clone() {
                text_widget.color = Some(resolved_style.text_color);
                text_widget.lower(cx)
            } else {
                child_widget.lower(cx)
            };
            let aligned_id = match self.content_align {
                ButtonContentAlign::Center => {
                    // Center the content within the button's box (vertically + horizontally).
                    let mut align_builder =
                        NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Align));
                    align_builder.add_child(child_id);
                    align_builder.build(cx)
                }
                ButtonContentAlign::Start | ButtonContentAlign::End => {
                    let justify = match self.content_align {
                        ButtonContentAlign::Start => fission_ir::op::JustifyContent::Start,
                        ButtonContentAlign::End => fission_ir::op::JustifyContent::End,
                        ButtonContentAlign::Center => fission_ir::op::JustifyContent::Center,
                    };
                    let mut flex_builder = NodeBuilder::new(
                        cx.next_node_id(),
                        Op::Layout(LayoutOp::Flex {
                            direction: fission_ir::FlexDirection::Row,
                            wrap: fission_ir::FlexWrap::NoWrap,
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            padding: [0.0; 4],
                            gap: None,
                            align_items: fission_ir::op::AlignItems::Center,
                            justify_content: justify,
                        }),
                    );
                    flex_builder.add_child(child_id);
                    flex_builder.build(cx)
                }
            };
            button_builder.add_child(aligned_id);
        }
        
        cx.pop_scope();

        let button_id = button_builder.build(cx);

        if let Some(semantics_op) = self.build_semantics() {
            let mut semantics_builder =
                NodeBuilder::new(cx.next_node_id(), Op::Semantics(semantics_op));
            semantics_builder.add_child(button_id);
            return semantics_builder.build(cx);
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
        multiline: false,
        masked: false,
        input_mask: None,
        ime_preedit_range: None,
        checked: None,
        disabled: false,
        draggable: false,
        scrollable_x: false,
        scrollable_y: false,
        min_value: None,
        max_value: None,
        current_value: None,
        is_focus_scope: false,
        is_focus_barrier: false,
        drag_payload: None,
        hero_tag: None,
        focus_index: None,
    }
}
