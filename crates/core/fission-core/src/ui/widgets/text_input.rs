use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color as IrColor, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId, Role, Semantics, FlexDirection
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TextInput {
    pub id: Option<NodeId>,
    pub value: String,
    pub placeholder: Option<String>,
    pub on_change: Option<ActionEnvelope>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl Lower for TextInput {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let input_id = self.id.unwrap_or_else(|| cx.next_node_id());

        // Use the semantics node id (input_id) for focus checks so the caret reflects focus.
        let is_focused = cx.runtime_state.interaction.is_focused(input_id);

        // 1. Background (Paint) - AbsoluteFill
        let background_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: IrColor::WHITE }), 
                stroke: Some(Stroke { 
                    color: if is_focused { IrColor::BLUE } else { IrColor::BLACK }, 
                    width: if is_focused { 2.0 } else { 1.0 } 
                }),
                corner_radius: 4.0,
                shadow: None,
            })
        ).build(cx);

        // 2. Text (Paint)
        let mut display_text = String::new();
        let mut using_placeholder = false;
        let preedit_suffix = if is_focused {
            if let Some((id, txt)) = &cx.runtime_state.ime_preedit {
                if *id == input_id { Some(txt.clone()) } else { None }
            } else { None }
        } else { None };

        // Main text is only the committed value (+ preedit when present).
        // Placeholder is painted separately so it doesn't affect caret position.
        display_text = self.value.clone();
        if let Some(pre) = preedit_suffix.clone() {
            display_text.push_str(&pre);
        }
        if display_text.is_empty() && preedit_suffix.is_none() {
            using_placeholder = true;
        }
        
        let text_color = if preedit_suffix.is_some() { IrColor::BLUE } else { IrColor::BLACK };

        let text_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawText {
                text: display_text,
                size: 16.0,
                color: text_color,
            })
        ).build(cx);
        
        // Wrap Text in Layout Box to act as flex item
        let mut text_layout_builder = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box { width: None, height: None, padding: [0.0, 0.0, 0.0, 0.0] })
        );
        text_layout_builder.add_child(text_id);
        let text_layout_id = text_layout_builder.build(cx);

        // 3. Content container (Flex Row)
        let flex_id = cx.next_node_id();
        let mut flex_builder = NodeBuilder::new(
            flex_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0, 0.0, 0.0, 0.0],
            }),
        );
        
        // Wrapper (Box) with layout and visuals
        let wrapper_id = cx.next_node_id();
        let mut wrapper_builder = NodeBuilder::new(
            wrapper_id,
            Op::Layout(LayoutOp::Box {
                width: self.width.or(Some(200.0)),
                height: self.height.or(Some(40.0)),
                padding: [8.0, 8.0, 4.0, 4.0],
            }),
        );
        
        flex_builder.add_child(text_layout_id);

        if is_focused {
             let cursor_paint_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill { color: IrColor::BLACK }),
                    stroke: None,
                    corner_radius: 0.0,
                    shadow: None,
                })
             ).build(cx);
             
             let mut cursor_layout_builder = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box {
                    width: Some(2.0),
                    height: Some(16.0 * 1.2), // approximate line height from text size
                    padding: [0.0, 0.0, 0.0, 0.0],
                })
             );
             cursor_layout_builder.add_child(cursor_paint_id);
             let cursor_layout_id = cursor_layout_builder.build(cx);
             
             flex_builder.add_child(cursor_layout_id);
        }
        
        let flex_node_id = flex_builder.build(cx);

        // 3.5 Clip content using a horizontal Scroll viewport (no scrollbar). This keeps text/caret within bounds.
        let scroll_id = cx.next_node_id();
        // Compute inner viewport from wrapper padding and configured size
        let outer_w = self.width.unwrap_or(200.0);
        let outer_h = self.height.unwrap_or(40.0);
        let inner_w = (outer_w - (8.0 + 8.0)).max(0.0);
        let inner_h = (outer_h - (8.0 + 4.0)).max(0.0);
        let mut scroll_builder = NodeBuilder::new(
            scroll_id,
            Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Row,
                show_scrollbar: false,
                width: Some(inner_w),
                height: Some(inner_h),
                padding: [0.0, 0.0, 0.0, 0.0],
            }),
        );
        scroll_builder.add_child(flex_node_id);
        let scroll_node_id = scroll_builder.build(cx);
        
        wrapper_builder.add_child(background_id); // Background first (z-index)
        // Draw placeholder under the clipped content so it doesn't affect caret position
        if using_placeholder {
            let placeholder_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: self.placeholder.clone().unwrap_or_default(),
                    size: 16.0,
                    color: IrColor { r: 150, g: 150, b: 150, a: 255 },
                })
            ).build(cx);
            // Place placeholder in a box so it positions like main text origin
            let mut ph_box = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box { width: None, height: None, padding: [0.0;4] })
            );
            ph_box.add_child(placeholder_id);
            wrapper_builder.add_child(ph_box.build(cx));
        }
        wrapper_builder.add_child(scroll_node_id);  // Clipped content on top
        
        let final_id = wrapper_builder.build(cx);

        // 4. Semantics Wrapper (use input_id for semantics so focus id == input_id)
        let mut semantics = Semantics {
            role: Role::TextInput,
            label: None,
            value: Some(self.value.clone()),
            actions: Default::default(), 
            focusable: true,
        };
        if let Some(env) = &self.on_change {
             semantics.actions.entries.push(fission_ir::ActionEntry {
                 action_id: env.id.as_u128(),
                 payload_data: None,
             });
        }
        
        let mut semantics_builder = NodeBuilder::new(input_id, Op::Semantics(semantics));
        semantics_builder.add_child(final_id);
        
        semantics_builder.build(cx)
    }
}
