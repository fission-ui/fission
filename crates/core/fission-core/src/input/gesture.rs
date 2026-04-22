use super::{ControllerContext, InputController};
use crate::event::{InputEvent, PointerEvent};
use crate::{ActionEnvelope, ActionId, ActionInput};
use fission_ir::{NodeId, Op, semantics::ActionTrigger};
use fission_layout::LayoutPoint;

pub struct GestureController;

impl InputController for GestureController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Pointer(pe) => {
                match pe {
                    PointerEvent::Down { point, button } => {
                        ctx.gesture.start_point = Some(*point);
                        ctx.gesture.last_point = Some(*point);
                        ctx.gesture.is_panning = false;
                        ctx.gesture.pressed_button = Some(button.clone());
                        
                        if let Some(hit) = crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point) {
                            ctx.gesture.target_node = Some(hit);
                            ctx.gesture.dragging_payload = self.find_drag_payload(ctx, hit);
                        } else {
                            ctx.gesture.target_node = None;
                            ctx.gesture.dragging_payload = None;
                        }
                    }
                    PointerEvent::Move { point } => {
                        if let Some(start) = ctx.gesture.start_point {
                            let dx = point.x - start.x;
                            let dy = point.y - start.y;
                            let dist_sq = dx*dx + dy*dy;
                            let threshold = 5.0 * 5.0; 
                            
                            if !ctx.gesture.is_panning && dist_sq > threshold {
                                ctx.gesture.is_panning = true;
                                // Dispatch DragStart now
                                if let Some(target) = ctx.gesture.target_node {
                                    self.dispatch_trigger(ctx, target, ActionTrigger::DragStart, *point, None);
                                }
                            }
                            
                            if ctx.gesture.is_panning {
                                let last = ctx.gesture.last_point.unwrap_or(start);
                                let delta = LayoutPoint { x: point.x - last.x, y: point.y - last.y };
                                ctx.gesture.last_point = Some(*point);
                                
                                // Try dispatching DragUpdate
                                let dispatched = if let Some(target) = ctx.gesture.target_node {
                                    self.dispatch_trigger(ctx, target, ActionTrigger::DragUpdate, *point, Some(delta))
                                } else { false };
                                
                                if dispatched {
                                    return true;
                                }
                                
                                // Fallback to Scroll Panning if DragUpdate not handled
                                if self.handle_pan_update(ctx, delta) {
                                    return true; 
                                }
                            }
                        }
                    }
                    PointerEvent::Up { point, .. } => {
                        let mut handled = false;
                        let was_secondary = matches!(
                            ctx.gesture.pressed_button,
                            Some(crate::event::PointerButton::Secondary)
                        );
                        if ctx.gesture.is_panning {
                            // Internal Drop
                            if let Some(payload) = ctx.gesture.dragging_payload.take() {
                                if let Some(up_hit) = crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point) {
                                    if self.dispatch_internal_drop(ctx, up_hit, payload, *point) {
                                        handled = true;
                                    }
                                }
                            }

                            if let Some(target) = ctx.gesture.target_node {
                                self.dispatch_trigger(ctx, target, ActionTrigger::DragEnd, *point, None);
                            }
                            handled = true;
                        } else if was_secondary {
                            // Secondary click (right-click)
                            if let Some(target) = ctx.gesture.target_node {
                                if let Some(up_hit) = crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point) {
                                    if up_hit == target || self.is_descendant(ctx, up_hit, target) || self.is_descendant(ctx, target, up_hit) {
                                        if self.dispatch_trigger(ctx, target, ActionTrigger::SecondaryClick, *point, None) {
                                            handled = true;
                                        }
                                    }
                                }
                            }
                        } else {
                            // Tap (primary click)
                            if let Some(target) = ctx.gesture.target_node {
                                if let Some(up_hit) = crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point) {
                                    if up_hit == target || self.is_descendant(ctx, up_hit, target) || self.is_descendant(ctx, target, up_hit) {
                                        if self.dispatch_trigger(ctx, target, ActionTrigger::Default, *point, None) {
                                            handled = true;
                                        }
                                    }
                                }
                            }
                        }

                        ctx.gesture.start_point = None;
                        ctx.gesture.is_panning = false;
                        ctx.gesture.dragging_payload = None;
                        ctx.gesture.pressed_button = None;
                        return handled;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }
}

impl GestureController {
    fn is_descendant(&self, ctx: &ControllerContext, child: NodeId, ancestor: NodeId) -> bool {
        let mut curr = Some(child);
        while let Some(id) = curr {
            if id == ancestor { return true; }
            if let Some(node) = ctx.ir.nodes.get(&id) {
                curr = node.parent;
            } else { break; }
        }
        false
    }

    fn find_drag_payload(&self, ctx: &ControllerContext, start_node: NodeId) -> Option<Vec<u8>> {
        let mut current_id = Some(start_node);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    if let Some(p) = &sem.drag_payload {
                        return Some(p.clone());
                    }
                }
                current_id = node.parent;
            } else { break; }
        }
        None
    }

    fn dispatch_internal_drop(&self, ctx: &mut ControllerContext, target_node: NodeId, payload: Vec<u8>, point: LayoutPoint) -> bool {
        let mut current_id = Some(target_node);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    for entry in &sem.actions.entries {
                        if entry.trigger == ActionTrigger::Drop {
                            let envelope = ActionEnvelope {
                                id: ActionId::from_u128(entry.action_id),
                                payload: entry.payload_data.clone().unwrap_or_default(),
                            };
                            
                            let input = ActionInput::InternalDrop { 
                                payload: payload.clone(),
                                x: point.x, 
                                y: point.y, 
                            };
                            
                            ctx.dispatched_actions.push((node_id, envelope, input));
                            return true;
                        }
                    }
                }
                current_id = node.parent;
            } else { break; }
        }
        false
    }

    fn dispatch_trigger(&self, ctx: &mut ControllerContext, start_node: NodeId, trigger: ActionTrigger, point: LayoutPoint, delta: Option<LayoutPoint>) -> bool {
        let mut current_id = Some(start_node);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    for entry in &sem.actions.entries {
                        if entry.trigger == trigger {
                            let envelope = ActionEnvelope {
                                id: ActionId::from_u128(entry.action_id),
                                payload: entry.payload_data.clone().unwrap_or_default(),
                            };
                            
                            let input = ActionInput::Pointer { 
                                x: point.x, 
                                y: point.y, 
                                delta_x: delta.map(|d| d.x).unwrap_or(0.0), 
                                delta_y: delta.map(|d| d.y).unwrap_or(0.0), 
                            };
                            
                            ctx.dispatched_actions.push((node_id, envelope, input));
                            return true; 
                        }
                    }
                }
                current_id = node.parent;
            } else { break; }
        }
        false
    }

    fn handle_pan_update(&self, ctx: &mut ControllerContext, delta: LayoutPoint) -> bool {
        if let Some(target) = ctx.gesture.target_node {
            let mut current = Some(target);
            while let Some(id) = current {
                if let Some(node) = ctx.ir.nodes.get(&id) {
                    if let fission_ir::Op::Semantics(sem) = &node.op {
                        if sem.draggable {
                            return false; 
                        }
                    }
                    if let fission_ir::Op::Layout(fission_ir::op::LayoutOp::Scroll { direction, .. }) = &node.op {
                        let current_offset = ctx.scroll.get_offset(id);
                        let move_val = match direction {
                            fission_ir::op::FlexDirection::Row => -delta.x,
                            fission_ir::op::FlexDirection::Column => -delta.y,
                        };
                        
                        let mut new_offset = current_offset + move_val;
                        
                        if let Some(geom) = ctx.layout.get_node_geometry(id) {
                            let max_offset = if matches!(direction, fission_ir::op::FlexDirection::Row) {
                                (geom.content_size.width - geom.rect.width()).max(0.0)
                            } else {
                                (geom.content_size.height - geom.rect.height()).max(0.0)
                            };
                            new_offset = new_offset.clamp(0.0, max_offset);
                        }
                        
                        ctx.scroll.set_offset(id, new_offset);
                        return true;
                    }
                    current = node.parent;
                } else { break; }
            }
        }
        false
    }
}
