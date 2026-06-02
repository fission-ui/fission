use super::{ControllerContext, InputController};
use crate::event::{InputEvent, PointerEvent};
use crate::{ActionEnvelope, ActionId};
use fission_ir::{op::Op, semantics::Role, WidgetId};
use serde_json;

pub struct SliderController;

impl InputController for SliderController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                if let Some(hit_id) =
                    crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point)
                {
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
                        } else {
                            break;
                        }
                    }
                }
            }
            InputEvent::Pointer(PointerEvent::Move { point, .. }) => {
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
    fn update_value(&self, ctx: &mut ControllerContext, node_id: WidgetId, point_x: f32) {
        if let Some(geom) = ctx.layout.get_node_geometry(node_id) {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    let min = sem.min_value.unwrap_or(0.0);
                    let max = sem.max_value.unwrap_or(1.0);

                    // Note: Slider semantics nodes often wrap the layout node directly.
                    // Layout traversal records geometry for all nodes, including semantics
                    // wrappers, so the semantics geometry should match its child.

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
                            let input = crate::input::scoped_action_input(
                                ctx.ir,
                                node_id,
                                crate::ActionInput::None,
                            );
                            ctx.dispatched_actions.push((node_id, envelope, input));
                        }
                    }
                }
            }
        }
    }
}
