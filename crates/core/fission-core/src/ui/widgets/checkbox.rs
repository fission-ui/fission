use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Checkbox {
    pub id: Option<NodeId>,
    pub checked: bool,
    pub on_toggle: Option<ActionEnvelope>,
    pub label: Option<String>,
}

impl Lower for Checkbox {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let size = 18.0;
        let radius = tokens.radii.small;
        let border_color = tokens.colors.text_secondary;
        let active_color = tokens.colors.primary;
        let text_color = tokens.colors.text_primary;

        // Square indicator
        let square_id = cx.next_node_id();
        
        let bg_paint = if self.checked {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: active_color }),
                stroke: None,
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
        let bg_node = NodeBuilder::new(cx.next_node_id(), bg_paint).build(cx);
        
        // Checkmark
        let check_node = if self.checked {
            let check = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: tokens.colors.on_primary }),
                stroke: None,
                corner_radius: 1.0,
                shadow: None,
            })).build(cx);
            let mut check_box = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Box {
                width: Some(10.0), height: Some(10.0), 
                min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0;4]
            }));
            check_box.add_child(check);
            Some(check_box.build(cx))
        } else { None };

        let mut square_box = NodeBuilder::new(
            square_id,
            Op::Layout(LayoutOp::Box { width: Some(size), height: Some(size), min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0; 4] }),
        );
        square_box.add_child(bg_node);
        if let Some(c) = check_node { square_box.add_child(c); }
        let square_final = square_box.build(cx);

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
                Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [tokens.spacing.s, 0.0, 0.0, 0.0] }), 
            );
            layout.add_child(text_id);
            Some(layout.build(cx))
        } else { None };

        let layout_id = cx.next_node_id();
        let mut row = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Flex { direction: fission_ir::FlexDirection::Row, flex_grow: 0.0, flex_shrink: 0.0, padding: [0.0; 4], gap: None }),
        );
        row.add_child(square_final);
        if let Some(l) = label_id { row.add_child(l); }
        row.build(cx);

        cx.pop_scope();

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Checkbox,
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
        };
        if let Some(action) = &self.on_toggle {
             semantics.actions.entries.push(fission_ir::ActionEntry { action_id: action.id.as_u128(), payload_data: Some(action.payload.clone()) });
        }
        
        let mut sem_node = NodeBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}
