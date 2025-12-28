use super::{ControllerContext, InputController};
use crate::event::{InputEvent, PointerEvent};
use crate::{ActionEnvelope, ActionId};
use fission_ir::{op::Op, semantics::Role, NodeId};
use serde_json;

pub struct SliderController;

impl InputController for SliderController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                if let Some(hit_id) = crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point) {
                    let mut current_id = Some(hit_id);
                    while let Some(node_id) = current_id {
                        if let Some(node) = ctx.ir.nodes.get(&node_id) {
                            if let Op::Semantics(sem) = &node.op {
                                if sem.role == Role::Slider {
                                    ctx.interaction.set_focused(Some(node_id));
                                    ctx.interaction.set_pressed(node_id, true);
                                    
                                    self.update_value(ctx, node_id, point.x);
                                    return true;
                                }
                            }
                            current_id = node.parent;
                        } else { break; }
                    }
                }
            }
            InputEvent::Pointer(PointerEvent::Move { point }) => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if ctx.interaction.is_pressed(focused_id) {
                         if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                            if let Op::Semantics(sem) = &node.op {
                                if sem.role == Role::Slider {
                                    self.update_value(ctx, focused_id, point.x);
                                    return true;
                                }
                            }
                         }
                    }
                }
            }
            _ => {}
        }
        false
    }
}

impl SliderController {
    fn update_value(&self, ctx: &mut ControllerContext, node_id: NodeId, point_x: f32) {
        if let Some(geom) = ctx.layout.get_node_geometry(node_id) {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    let min = sem.min_value.unwrap_or(0.0);
                    let max = sem.max_value.unwrap_or(1.0);
                    
                    // Note: Slider Semantics node might wrap the Layout node directly, 
                    // or be an ancestor. Usually Semantics wraps Layout.
                    // The geom for Semantics node should match the Layout child?
                    // In `Slider::lower`, we wrap `layout_id` in `sem_node`.
                    // But `sem_node` itself doesn't have geometry in Taffy map if it's just Semantics op?
                    // `fission-layout` visits ALL nodes. 
                    // `LayoutEngine::compute_layout` extracts geometry for all nodes.
                    // But `Semantics` Op doesn't produce Taffy node usually?
                    // Wait, `compute_style` matches `LayoutOp`.
                    // `compute_style` has `_ => Display::Flex`.
                    // So Semantics nodes ARE layout nodes (Flex, default).
                    // They wrap their child.
                    // So they have geometry.
                    // The geometry of Semantics node should match its child (Flex Item).
                    
                    let width = geom.rect.width();
                    if width > 0.0 {
                        let local_x = point_x - geom.rect.x();
                        let t = (local_x / width).clamp(0.0, 1.0);
                        let new_val = min + t * (max - min);
                        
                        if let Some(entry) = sem.actions.entries.first() {
                            let payload = serde_json::to_vec(&new_val).unwrap();
                            let envelope = ActionEnvelope {
                                id: ActionId::from_u128(entry.action_id),
                                payload,
                            };
                            ctx.dispatched_actions.push((node_id, envelope));
                        }
                    }
                }
            }
        }
    }
}
