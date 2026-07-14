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
                            let payload = slider_payload(entry.payload_data.as_deref(), new_val);
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

fn slider_payload(template: Option<&[u8]>, new_value: f32) -> Vec<u8> {
    let Some(template) = template else {
        return serde_json::to_vec(&new_value).expect("slider value serialization failed");
    };
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(template) else {
        return serde_json::to_vec(&new_value).expect("slider value serialization failed");
    };
    let Some(number) = serde_json::Number::from_f64(new_value as f64) else {
        return serde_json::to_vec(&new_value).expect("slider value serialization failed");
    };

    if replace_numeric_payload(&mut value, number) {
        serde_json::to_vec(&value).expect("slider action serialization failed")
    } else {
        serde_json::to_vec(&new_value).expect("slider value serialization failed")
    }
}

fn replace_numeric_payload(value: &mut serde_json::Value, number: serde_json::Number) -> bool {
    match value {
        serde_json::Value::Number(slot) => {
            *slot = number;
            true
        }
        serde_json::Value::Array(items) if items.len() == 1 && items[0].is_number() => {
            items[0] = serde_json::Value::Number(number);
            true
        }
        serde_json::Value::Object(fields) => {
            if let Some(slot) = fields.get_mut("value").filter(|slot| slot.is_number()) {
                *slot = serde_json::Value::Number(number);
                return true;
            }
            let mut numeric_keys = fields
                .iter()
                .filter_map(|(key, field)| field.is_number().then_some(key.clone()))
                .collect::<Vec<_>>();
            if numeric_keys.len() == 1 {
                let key = numeric_keys.pop().expect("one numeric key");
                if let Some(slot) = fields.get_mut(&key) {
                    *slot = serde_json::Value::Number(number);
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::slider_payload;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(transparent)]
    struct TransparentSliderValue(f32);

    #[derive(Debug, Deserialize, PartialEq)]
    struct NamedSliderValue {
        value: f32,
    }

    #[test]
    fn slider_payload_preserves_transparent_action_shape() {
        let payload = slider_payload(Some(b"0.0"), 75.0);
        let action: TransparentSliderValue = serde_json::from_slice(&payload).unwrap();

        assert_eq!(action, TransparentSliderValue(75.0));
    }

    #[test]
    fn slider_payload_preserves_named_value_action_shape() {
        let payload = slider_payload(Some(br#"{"value":0.0}"#), 25.0);
        let action: NamedSliderValue = serde_json::from_slice(&payload).unwrap();

        assert_eq!(action, NamedSliderValue { value: 25.0 });
    }
}
