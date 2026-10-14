use super::{ControllerContext, InputController};
use crate::event::{InputEvent, KeyCode, KeyEvent, PointerEvent};
use crate::{ActionEnvelope, ActionId, ActionInput};
use fission_ir::{op::Op, semantics::ActionTrigger, semantics::Role, CoreIR, WidgetId};
use serde_json;

pub struct SliderController;

impl InputController for SliderController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Keyboard(KeyEvent::Down { key_code, .. }) => {
                let Some(node_id) = ctx.interaction.focused else {
                    return false;
                };
                let Some(semantics) = ctx.ir.nodes.get(&node_id).and_then(|node| match &node.op {
                    Op::Semantics(semantics) if semantics.role == Role::Slider => Some(semantics),
                    _ => None,
                }) else {
                    return false;
                };
                let Some(value) = slider_keyboard_value(semantics, key_code) else {
                    return false;
                };
                let Some(dispatch) = prepare_slider_change(ctx.ir, node_id, value) else {
                    return false;
                };
                ctx.dispatched_actions.push(dispatch);
                return true;
            }
            InputEvent::Pointer(PointerEvent::Down { point, button, .. })
                if matches!(button, crate::event::PointerButton::Primary) =>
            {
                if let Some(hit_id) = crate::hit_test::hit_test_with_viewports(
                    ctx.ir,
                    ctx.layout,
                    ctx.scroll,
                    ctx.viewport,
                    *point,
                ) {
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

fn slider_keyboard_value(semantics: &fission_ir::Semantics, key_code: &KeyCode) -> Option<f32> {
    let current = semantics.current_value?;
    let min = semantics.min_value.unwrap_or(0.0);
    let max = semantics.max_value.unwrap_or(1.0);
    let step = semantics
        .step_value
        .filter(|step| step.is_finite() && *step > 0.0)
        .unwrap_or_else(|| ((max - min).abs() / 100.0).max(f32::EPSILON));
    match key_code {
        KeyCode::Left | KeyCode::Down => Some(current - step),
        KeyCode::Right | KeyCode::Up => Some(current + step),
        KeyCode::PageDown => Some(current - step * 10.0),
        KeyCode::PageUp => Some(current + step * 10.0),
        KeyCode::Home => Some(min),
        KeyCode::End => Some(max),
        _ => None,
    }
}

/// Builds a payload-preserving slider change for an increment or decrement
/// requested by an accessibility adapter.
#[doc(hidden)]
pub fn prepare_slider_adjustment(
    ir: &CoreIR,
    target: WidgetId,
    direction: f32,
) -> Option<(WidgetId, ActionEnvelope, ActionInput)> {
    let node = ir.nodes.get(&target)?;
    let Op::Semantics(semantics) = &node.op else {
        return None;
    };
    let current = semantics.current_value?;
    let step = semantics
        .step_value
        .filter(|step| step.is_finite() && *step > 0.0)
        .unwrap_or_else(|| {
            let min = semantics.min_value.unwrap_or(0.0);
            let max = semantics.max_value.unwrap_or(1.0);
            ((max - min).abs() / 100.0).max(f32::EPSILON)
        });
    prepare_slider_change(ir, target, current + step * direction)
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

                        if let Some(dispatch) = prepare_slider_change(ctx.ir, node_id, new_val) {
                            ctx.dispatched_actions.push(dispatch);
                        }
                    }
                }
            }
        }
    }
}

/// Builds the payload-preserving action dispatched for one slider value change.
///
/// Shells with a platform-native range control use this boundary so they apply
/// the same validation, action shape and action-scope routing as retained input.
#[doc(hidden)]
pub fn prepare_slider_change(
    ir: &CoreIR,
    target: WidgetId,
    proposed: f32,
) -> Option<(WidgetId, ActionEnvelope, ActionInput)> {
    if !proposed.is_finite() {
        return None;
    }
    let node = ir.nodes.get(&target)?;
    let Op::Semantics(semantics) = &node.op else {
        return None;
    };
    if semantics.role != Role::Slider || semantics.disabled {
        return None;
    }
    let action = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Change)?;
    let min = semantics.min_value.unwrap_or(0.0);
    let max = semantics.max_value.unwrap_or(1.0);
    let value = normalize_slider_value(proposed, min, max, semantics.step_value);
    let envelope = ActionEnvelope {
        id: ActionId::from_u128(action.action_id),
        payload: slider_payload(action.payload_data.as_deref(), value),
    };
    let input = crate::input::scoped_action_input(ir, target, ActionInput::None);
    Some((target, envelope, input))
}

/// Clamps a proposed slider value and, for a discrete slider, snaps it to the
/// nearest step measured from the lower bound.
pub(crate) fn normalize_slider_value(proposed: f32, min: f32, max: f32, step: Option<f32>) -> f32 {
    let lower = min.min(max);
    let upper = min.max(max);
    let clamped = proposed.clamp(lower, upper);
    let Some(step) = step.filter(|step| step.is_finite() && *step > 0.0) else {
        return clamped;
    };

    let snapped = lower + ((clamped - lower) / step).round() * step;
    snapped.clamp(lower, upper)
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
    use super::{normalize_slider_value, slider_keyboard_value, slider_payload};
    use crate::event::KeyCode;
    use fission_ir::{Role, Semantics};
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

    #[test]
    fn continuous_values_are_only_clamped() {
        assert_eq!(normalize_slider_value(0.232, 0.0, 1.0, None), 0.232);
        assert_eq!(normalize_slider_value(2.0, 0.0, 1.0, None), 1.0);
    }

    #[test]
    fn discrete_values_snap_from_the_lower_bound() {
        assert!((normalize_slider_value(0.232, 0.0, 1.0, Some(0.1)) - 0.2).abs() < f32::EPSILON);
        assert!((normalize_slider_value(2.6, 1.0, 250.0, Some(1.0)) - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn keyboard_uses_discrete_step_or_one_percent_for_continuous_values() {
        let continuous = Semantics {
            role: Role::Slider,
            min_value: Some(0.0),
            max_value: Some(1.0),
            current_value: Some(0.23),
            ..Default::default()
        };
        assert!(
            (slider_keyboard_value(&continuous, &KeyCode::Right).unwrap() - 0.24).abs() < 0.0001
        );

        let discrete = Semantics {
            min_value: Some(1.0),
            max_value: Some(250.0),
            current_value: Some(20.0),
            step_value: Some(1.0),
            ..continuous
        };
        assert_eq!(slider_keyboard_value(&discrete, &KeyCode::Up), Some(21.0));
        assert_eq!(slider_keyboard_value(&discrete, &KeyCode::Home), Some(1.0));
        assert_eq!(slider_keyboard_value(&discrete, &KeyCode::End), Some(250.0));
    }
}
