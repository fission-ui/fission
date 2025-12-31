use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Switch {
    pub id: Option<NodeId>,
    pub checked: bool,
    pub on_toggle: Option<ActionEnvelope>,
}

impl Switch {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Switch(self)
    }
}

impl Lower for Switch {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let width = 36.0;
        let height = 20.0;
        let thumb_size = 16.0;
        let padding = 2.0;
        
        let track_color = if self.checked { tokens.colors.primary } else { tokens.colors.border };
        let thumb_color = tokens.colors.on_primary;

        // Track
        let track_paint = Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill { color: track_color }),
            stroke: None,
            corner_radius: height / 2.0,
            shadow: None,
        });
        let track_node = NodeBuilder::new(cx.next_node_id(), track_paint).build(cx);

        // Thumb
        let thumb_paint = Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill { color: thumb_color }),
            stroke: None,
            corner_radius: thumb_size / 2.0,
            shadow: Some(fission_ir::op::BoxShadow { 
                color: Color { r:0, g:0, b:0, a:50 }, 
                blur_radius: 2.0, 
                offset: (0.0, 1.0) 
            }),
        });
        let thumb_paint_node = NodeBuilder::new(cx.next_node_id(), thumb_paint).build(cx);
        
        let left_padding = if self.checked { width - thumb_size - padding } else { padding };

        let mut thumb_wrapper = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: Some(thumb_size), height: Some(thumb_size),
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            })
        );
        thumb_wrapper.add_child(thumb_paint_node);
        let thumb_id = thumb_wrapper.build(cx);

        // ZStack for Track + Content
        let layout_id = cx.next_node_id();
        let bg_id = {
            let mut bg_fill = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
            bg_fill.add_child(track_node);
            bg_fill.build(cx)
        };
        
        let content_id = {
            let mut thumb_track = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box {
                    width: Some(width), height: Some(height),
                    min_width: None, max_width: None, min_height: None, max_height: None,
                    padding: [left_padding, 0.0, padding, 0.0],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                })
            );
            thumb_track.add_child(thumb_id);
            thumb_track.build(cx)
        };
        
        let mut root = NodeBuilder::new(layout_id, Op::Layout(LayoutOp::ZStack));
        root.add_child(bg_id);
        root.add_child(content_id);
        root.build(cx);

        cx.pop_scope();

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Switch,
            label: None,
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
        };
        if let Some(action) = &self.on_toggle {
             semantics.actions.entries.push(fission_ir::ActionEntry { action_id: action.id.as_u128(), payload_data: Some(action.payload.clone()) });
        }
        
        let mut sem_node = NodeBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}


