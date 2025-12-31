use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Radio {
    pub id: Option<NodeId>,
    pub checked: bool,
    pub on_select: Option<ActionEnvelope>,
    pub label: Option<String>,
}

impl Radio {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Radio(self)
    }
}

impl Lower for Radio {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let size = 18.0;
        let radius = size / 2.0;
        let border_color = tokens.colors.text_secondary;
        let active_color = tokens.colors.primary;
        let text_color = tokens.colors.text_primary;

        // Outer Circle
        let outer_paint = if self.checked {
            Op::Paint(PaintOp::DrawRect {
                fill: None, 
                stroke: Some(Stroke { color: active_color, width: 2.0 }),
                corner_radius: radius,
                shadow: None,
            })
        } else {
            Op::Paint(PaintOp::DrawRect {
                fill: None,
                stroke: Some(Stroke { color: border_color, width: 1.5 }),
                corner_radius: radius,
                shadow: None,
            })
        };
        let outer_node = NodeBuilder::new(cx.next_node_id(), outer_paint).build(cx);

        // Inner Dot
        let dot_node = if self.checked {
            let dot_size = 10.0;
            let dot = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: active_color }),
                stroke: None,
                corner_radius: dot_size / 2.0,
                shadow: None,
            })).build(cx);
            let mut dot_box = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box {
                width: Some(dot_size), height: Some(dot_size), 
                min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4],
                flex_grow: 0.0, flex_shrink: 0.0,
                aspect_ratio: None,
            }));
            dot_box.add_child(dot);
            Some(dot_box.build(cx))
        } else { None };

        let mut radio_box = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box { width: Some(size), height: Some(size), min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0; 4], flex_grow: 0.0, flex_shrink: 0.0, aspect_ratio: None }),
        );
        radio_box.add_child(outer_node);
        if let Some(d) = dot_node { radio_box.add_child(d); }
        let radio_final = radio_box.build(cx);

        // Label
        let label_id = if let Some(text) = &self.label {
            let text_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText { 
                    text: text.clone(), 
                    size: tokens.typography.body_medium_size, 
                    color: text_color, 
                    underline: false, 
                    caret_index: None 
                }),
            ).build(cx);
            let mut layout = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [tokens.spacing.s, 0.0, 0.0, 0.0], flex_grow: 0.0, flex_shrink: 0.0, aspect_ratio: None }), 
            );
            layout.add_child(text_id);
            Some(layout.build(cx))
        } else { None };

        let layout_id = cx.next_node_id();
        let mut row = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Flex { direction: fission_ir::FlexDirection::Row, wrap: fission_ir::op::FlexWrap::NoWrap, flex_grow: 0.0, flex_shrink: 0.0, padding: [0.0; 4], gap: None }),
        );
        row.add_child(radio_final);
        if let Some(l) = label_id { row.add_child(l); }
        row.build(cx);

        cx.pop_scope();

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Checkbox, // Reuse Checkbox for Radio behavior?
            label: self.label.clone(),
            value: Some(if self.checked { "true".into() } else { "false".into() }),
            actions: Default::default(),
            focusable: true,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: Some(self.checked),
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
        };
        if let Some(action) = &self.on_select {
             semantics.actions.entries.push(fission_ir::ActionEntry { 
                 trigger: fission_ir::semantics::ActionTrigger::Default,
                 action_id: action.id.as_u128(), 
                 payload_data: Some(action.payload.clone()) 
             });
        }
        
        let mut sem_node = NodeBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}


