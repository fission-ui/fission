use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent, PointerId};
use crate::input::scoped_action_input;
use crate::ui::custom_render::{downcast_render_object, CustomRenderObject};
use crate::{ActionEnvelope, ActionInput, InteractionStateMap, ScrollStateMap};
use fission_ir::{CoreIR, WidgetId};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot};

/// Identifies the thumb that proposed a range-slider change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RangeSliderThumb {
    /// The thumb controlling the lower value.
    Start,
    /// The thumb controlling the upper value.
    End,
}

/// Identifies the input path that proposed a range-slider change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RangeSliderChangeSource {
    /// A primary pointer press or drag.
    Pointer,
    /// A focused thumb's keyboard command.
    Keyboard,
    /// A platform accessibility action.
    Accessibility,
}

/// Runtime input delivered separately from a range slider's bound action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RangeSliderChanged {
    /// Stable identity of the complete range slider, not an individual thumb.
    pub node_id: WidgetId,
    /// Proposed controlled lower value.
    pub start: f32,
    /// Proposed controlled upper value.
    pub end: f32,
    /// Thumb responsible for this proposal.
    pub active_thumb: RangeSliderThumb,
    /// Input path responsible for the proposal.
    pub source: RangeSliderChangeSource,
    /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
    pub modifiers: u8,
}

/// Lowered runtime configuration for one retained range slider.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct RangeSliderRuntimeConfig {
    pub node_id: WidgetId,
    pub start_thumb_id: WidgetId,
    pub end_thumb_id: WidgetId,
    pub start: f32,
    pub end: f32,
    pub min: f32,
    pub max: f32,
    pub step: Option<f32>,
    pub on_change: Option<ActionEnvelope>,
}

impl CustomRenderObject for RangeSliderRuntimeConfig {
    fn range_slider_config(&self) -> Option<&RangeSliderRuntimeConfig> {
        Some(self)
    }
}

/// Runtime-owned pointer capture and active-thumb state.
#[derive(Debug, Clone, Default)]
pub struct RangeSliderStateMap {
    captures: HashMap<PointerId, (WidgetId, RangeSliderThumb)>,
    active_thumbs: HashMap<WidgetId, RangeSliderThumb>,
}

impl RangeSliderStateMap {
    pub fn reconcile(&mut self, ir: &CoreIR) {
        let active = ir
            .custom_render_objects
            .iter()
            .filter_map(|(id, object)| {
                downcast_render_object(object)
                    .and_then(|object| object.range_slider_config())
                    .map(|_| *id)
            })
            .collect::<HashSet<_>>();
        self.captures
            .retain(|_, (node_id, _)| active.contains(node_id));
        self.active_thumbs.retain(|id, _| active.contains(id));
    }
}

pub struct RangeSliderControllerContext<'a> {
    pub ir: &'a CoreIR,
    pub layout: &'a LayoutSnapshot,
    pub scroll: &'a ScrollStateMap,
    pub viewport: &'a crate::input::viewport::ViewportStateMap,
    pub interaction: &'a mut InteractionStateMap,
    pub state: &'a mut RangeSliderStateMap,
    pub dispatched_actions: Vec<(WidgetId, ActionEnvelope, ActionInput)>,
}

pub struct RangeSliderController;

impl RangeSliderController {
    pub fn handle_event(
        &mut self,
        ctx: &mut RangeSliderControllerContext<'_>,
        event: &InputEvent,
    ) -> bool {
        match event {
            InputEvent::Pointer(PointerEvent::Down {
                pointer_id,
                point,
                button: PointerButton::Primary,
                modifiers,
                ..
            }) => self.pointer_down(ctx, *pointer_id, *point, *modifiers),
            InputEvent::Pointer(PointerEvent::Move {
                pointer_id,
                point,
                modifiers,
                ..
            }) => self.pointer_update(ctx, *pointer_id, *point, *modifiers, false),
            InputEvent::Pointer(PointerEvent::Up {
                pointer_id,
                point,
                button: PointerButton::Primary,
                modifiers,
                ..
            }) => self.pointer_update(ctx, *pointer_id, *point, *modifiers, true),
            InputEvent::Pointer(PointerEvent::Cancel { pointer_id, .. }) => {
                ctx.state.captures.remove(pointer_id).is_some()
            }
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => self.keyboard(ctx, key_code, *modifiers),
            _ => false,
        }
    }

    fn pointer_down(
        &self,
        ctx: &mut RangeSliderControllerContext<'_>,
        pointer_id: PointerId,
        point: LayoutPoint,
        modifiers: u8,
    ) -> bool {
        let Some(hit) = crate::hit_test::hit_test_with_viewports(
            ctx.ir,
            ctx.layout,
            ctx.scroll,
            ctx.viewport,
            point,
        ) else {
            return false;
        };
        let Some((owner, config)) = range_slider_config_for_node(ctx.ir, hit) else {
            return false;
        };
        let Some(rect) = ctx.layout.get_node_rect(owner) else {
            return false;
        };
        let point = crate::hit_test::point_to_node_space(
            ctx.ir,
            ctx.layout,
            ctx.scroll,
            ctx.viewport,
            owner,
            point,
        );
        let thumb = nearest_thumb(
            &config,
            rect,
            point.x,
            ctx.state.active_thumbs.get(&owner).copied(),
        );
        ctx.state.captures.insert(pointer_id, (owner, thumb));
        ctx.state.active_thumbs.insert(owner, thumb);
        let thumb_id = match thumb {
            RangeSliderThumb::Start => config.start_thumb_id,
            RangeSliderThumb::End => config.end_thumb_id,
        };
        ctx.interaction.set_focused(Some(thumb_id));
        ctx.interaction.set_pressed(thumb_id, true);
        propose_from_point(ctx, &config, rect, point.x, thumb, modifiers)
    }

    fn pointer_update(
        &self,
        ctx: &mut RangeSliderControllerContext<'_>,
        pointer_id: PointerId,
        point: LayoutPoint,
        modifiers: u8,
        release: bool,
    ) -> bool {
        let Some((owner, thumb)) = ctx.state.captures.get(&pointer_id).copied() else {
            return false;
        };
        let Some(config) = range_slider_config(ctx.ir, owner).cloned() else {
            ctx.state.captures.remove(&pointer_id);
            return false;
        };
        let Some(rect) = ctx.layout.get_node_rect(owner) else {
            ctx.state.captures.remove(&pointer_id);
            return false;
        };
        let point = crate::hit_test::point_to_node_space(
            ctx.ir,
            ctx.layout,
            ctx.scroll,
            ctx.viewport,
            owner,
            point,
        );
        let handled = propose_from_point(ctx, &config, rect, point.x, thumb, modifiers);
        if release {
            ctx.state.captures.remove(&pointer_id);
        }
        handled
    }

    fn keyboard(
        &self,
        ctx: &mut RangeSliderControllerContext<'_>,
        key: &KeyCode,
        modifiers: u8,
    ) -> bool {
        let Some(focused) = ctx.interaction.focused else {
            return false;
        };
        let Some((owner, config)) = range_slider_config_for_node(ctx.ir, focused) else {
            return false;
        };
        let thumb = if focused == config.start_thumb_id {
            RangeSliderThumb::Start
        } else if focused == config.end_thumb_id {
            RangeSliderThumb::End
        } else {
            ctx.state
                .active_thumbs
                .get(&owner)
                .copied()
                .unwrap_or(RangeSliderThumb::Start)
        };
        let (min, max, start, end) = normalized_values(&config);
        let step = keyboard_step(&config);
        let current = match thumb {
            RangeSliderThumb::Start => start,
            RangeSliderThumb::End => end,
        };
        let proposed = match key {
            KeyCode::Left | KeyCode::Down => current - step,
            KeyCode::Right | KeyCode::Up => current + step,
            KeyCode::PageDown => current - step * 10.0,
            KeyCode::PageUp => current + step * 10.0,
            KeyCode::Home => min,
            KeyCode::End => max,
            _ => return false,
        };
        dispatch_change(
            ctx,
            owner,
            &config,
            thumb,
            proposed,
            RangeSliderChangeSource::Keyboard,
            modifiers,
        )
    }
}

fn propose_from_point(
    ctx: &mut RangeSliderControllerContext<'_>,
    config: &RangeSliderRuntimeConfig,
    rect: LayoutRect,
    point_x: f32,
    thumb: RangeSliderThumb,
    modifiers: u8,
) -> bool {
    if rect.width() <= 0.0 {
        return true;
    }
    let (min, max, _, _) = normalized_values(config);
    let fraction = ((point_x - rect.x()) / rect.width()).clamp(0.0, 1.0);
    let proposed = min + fraction * (max - min);
    dispatch_change(
        ctx,
        config.node_id,
        config,
        thumb,
        proposed,
        RangeSliderChangeSource::Pointer,
        modifiers,
    )
}

fn dispatch_change(
    ctx: &mut RangeSliderControllerContext<'_>,
    owner: WidgetId,
    config: &RangeSliderRuntimeConfig,
    thumb: RangeSliderThumb,
    proposed: f32,
    source: RangeSliderChangeSource,
    modifiers: u8,
) -> bool {
    let Some(action) = &config.on_change else {
        return true;
    };
    let change = changed_values(config, thumb, proposed, source, modifiers);
    let input = scoped_action_input(ctx.ir, owner, ActionInput::RangeSliderChanged(change));
    ctx.dispatched_actions.push((owner, action.clone(), input));
    true
}

pub fn prepare_range_slider_change(
    ir: &CoreIR,
    target: WidgetId,
    proposed: f32,
    source: RangeSliderChangeSource,
    modifiers: u8,
) -> Option<(WidgetId, ActionEnvelope, ActionInput)> {
    let (owner, config) = range_slider_config_for_node(ir, target)?;
    let thumb = if target == config.end_thumb_id {
        RangeSliderThumb::End
    } else {
        RangeSliderThumb::Start
    };
    let action = config.on_change.clone()?;
    let change = changed_values(&config, thumb, proposed, source, modifiers);
    let input = scoped_action_input(ir, owner, ActionInput::RangeSliderChanged(change));
    Some((owner, action, input))
}

/// Builds one payload-preserving increment or decrement for an accessible
/// range thumb.
#[doc(hidden)]
pub fn prepare_range_slider_adjustment(
    ir: &CoreIR,
    target: WidgetId,
    direction: f32,
    source: RangeSliderChangeSource,
    modifiers: u8,
) -> Option<(WidgetId, ActionEnvelope, ActionInput)> {
    let (owner, config) = range_slider_config_for_node(ir, target)?;
    let thumb = if target == config.end_thumb_id {
        RangeSliderThumb::End
    } else {
        RangeSliderThumb::Start
    };
    let (_, _, start, end) = normalized_values(&config);
    let current = match thumb {
        RangeSliderThumb::Start => start,
        RangeSliderThumb::End => end,
    };
    let action = config.on_change.clone()?;
    let change = changed_values(
        &config,
        thumb,
        current + keyboard_step(&config) * direction,
        source,
        modifiers,
    );
    let input = scoped_action_input(ir, owner, ActionInput::RangeSliderChanged(change));
    Some((owner, action, input))
}

fn changed_values(
    config: &RangeSliderRuntimeConfig,
    thumb: RangeSliderThumb,
    proposed: f32,
    source: RangeSliderChangeSource,
    modifiers: u8,
) -> RangeSliderChanged {
    let (min, max, mut start, mut end) = normalized_values(config);
    let proposed = snap(proposed.clamp(min, max), config.step, min).clamp(min, max);
    match thumb {
        RangeSliderThumb::Start => start = proposed.min(end),
        RangeSliderThumb::End => end = proposed.max(start),
    }
    RangeSliderChanged {
        node_id: config.node_id,
        start,
        end,
        active_thumb: thumb,
        source,
        modifiers,
    }
}

fn nearest_thumb(
    config: &RangeSliderRuntimeConfig,
    rect: LayoutRect,
    point_x: f32,
    previous: Option<RangeSliderThumb>,
) -> RangeSliderThumb {
    let (min, max, start, end) = normalized_values(config);
    let width = rect.width().max(0.0);
    let range = (max - min).max(f32::EPSILON);
    let start_x = rect.x() + (start - min) / range * width;
    let end_x = rect.x() + (end - min) / range * width;
    let start_distance = (point_x - start_x).abs();
    let end_distance = (point_x - end_x).abs();
    if (start_distance - end_distance).abs() <= f32::EPSILON {
        previous.unwrap_or(RangeSliderThumb::End)
    } else if start_distance < end_distance {
        RangeSliderThumb::Start
    } else {
        RangeSliderThumb::End
    }
}

fn normalized_values(config: &RangeSliderRuntimeConfig) -> (f32, f32, f32, f32) {
    let min = config.min.min(config.max);
    let max = config.min.max(config.max);
    let first = config.start.clamp(min, max);
    let second = config.end.clamp(min, max);
    (min, max, first.min(second), first.max(second))
}

fn keyboard_step(config: &RangeSliderRuntimeConfig) -> f32 {
    config
        .step
        .filter(|step| step.is_finite() && *step > 0.0)
        .unwrap_or_else(|| ((config.max - config.min).abs() / 100.0).max(f32::EPSILON))
}

fn snap(value: f32, step: Option<f32>, min: f32) -> f32 {
    let Some(step) = step.filter(|step| step.is_finite() && *step > 0.0) else {
        return value;
    };
    (min + ((value - min) / step).round() * step).max(min)
}

pub fn range_slider_config_for_node(
    ir: &CoreIR,
    target: WidgetId,
) -> Option<(WidgetId, RangeSliderRuntimeConfig)> {
    let mut current = Some(target);
    while let Some(id) = current {
        if let Some(config) = range_slider_config(ir, id) {
            return Some((id, config.clone()));
        }
        current = ir.nodes.get(&id).and_then(|node| node.parent);
    }
    None
}

fn range_slider_config(ir: &CoreIR, id: WidgetId) -> Option<&RangeSliderRuntimeConfig> {
    ir.custom_render_objects
        .get(&id)
        .and_then(downcast_render_object)
        .and_then(|object| object.range_slider_config())
}
