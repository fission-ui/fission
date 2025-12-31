use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{op::{FlexDirection, LayoutOp, Op}, NodeId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyColumn {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub item_height: f32,
}

impl LazyColumn {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::LazyColumn(self)
    }
}

impl Lower for LazyColumn {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let scroll_id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        // Get layout info from previous frame
        let viewport_height = if let Some(layout) = cx.layout {
            if let Some(geom) = layout.get_node_geometry(scroll_id) {
                geom.rect.height()
            } else {
                600.0 // Default/Fallback
            }
        } else {
            600.0
        };
        
        let scroll_offset = cx.runtime_state.scroll.get_offset(scroll_id);
        
        let item_h = self.item_height.max(1.0);
        let total_count = self.children.len();
        
        let start_index = (scroll_offset / item_h).floor() as usize;
        let visible_count = (viewport_height / item_h).ceil() as usize + 1; // +1 buffer
        let end_index = (start_index + visible_count).min(total_count);
        let start_index = start_index.min(total_count); // Clamp
        
        // Column
        let col_id = cx.next_node_id(); // Reserve ID for column wrapper
        
        // Build children
        let mut column_children = Vec::new();
        
        // Top Spacer
        if start_index > 0 {
            // ...
        }
        
        // Visible Items
        // We do NOT push `col_id` as a shared scope because children need stable IDs regardless of skip.
        // Instead, we derive a unique scope for each child index.
        
        for (i, child) in self.children.iter().enumerate().skip(start_index).take(end_index - start_index) {
             let child_scope = NodeId::derived(col_id.as_u128(), &[i as u32]);
             cx.push_scope(child_scope);
             column_children.push(child.lower(cx));
             cx.pop_scope();
        }
        
        // No pop_scope needed for col_id since we didn't push it.
        
        // Bottom Spacer
        // ...
        
        let mut col = NodeBuilder::new(
            col_id, 
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: fission_ir::op::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 0.0,
                padding: [0.0; 4],
                gap: None,
            }),
        );
        col.add_children(column_children);
        let col_id = col.build(cx);
        
        // Scroll
        let mut scroll = NodeBuilder::new(
            scroll_id,
            Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Column,
                show_scrollbar: true,
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
            })
        );
        scroll.add_child(col_id);
        scroll.build(cx)
    }
}


