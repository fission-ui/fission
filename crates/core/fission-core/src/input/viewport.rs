use std::collections::HashMap;
use std::time::Instant;

use fission_ir::{
    ActionEntry, CoreIR, LayoutOp, Op, ViewportBoundary, ViewportPanAxis, ViewportTransform,
    ViewportZoomPolicy, WidgetId,
};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot};

use super::scoped_action_input;
use crate::event::{
    InputEvent, PointerButton, PointerEvent, PointerId, PointerKind, PointerPhase, ScrollDeltaMode,
};
use crate::{ActionEnvelope, ActionId, ActionInput};

const PAN_THRESHOLD_SQUARED: f32 = 25.0;
const LINE_DELTA_POINTS: f32 = 16.0;
const WHEEL_ZOOM_SENSITIVITY: f32 = 0.002;

/// Lifecycle stage for a viewport interaction delivered to a reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportInteractionPhase {
    Start,
    Update,
    End,
    Cancel,
}

/// Physical gesture family that changed an interactive viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportInputKind {
    Mouse,
    Touch,
    Stylus,
    Wheel,
    Magnify,
    Unknown,
}

/// Live event facts accompanying an interactive-viewport action.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportInteraction {
    pub node_id: WidgetId,
    pub phase: ViewportInteractionPhase,
    pub transform: ViewportTransform,
    pub viewport_focal_point: LayoutPoint,
    pub world_focal_point: LayoutPoint,
    pub pan_delta: LayoutPoint,
    pub scale_factor: f32,
    pub input_kind: ViewportInputKind,
    pub modifiers: u8,
}

#[derive(Debug, Clone)]
pub struct ViewportRuntimeState {
    pub transform: ViewportTransform,
    last_declared_transform: Option<ViewportTransform>,
    contacts: HashMap<PointerId, Contact>,
    pending_start: Option<LayoutPoint>,
    last_centroid: Option<LayoutPoint>,
    last_distance: Option<f32>,
    interacting: bool,
    velocity: LayoutPoint,
    last_motion_at: Option<Instant>,
    inertia_tick: Option<Instant>,
}

impl ViewportRuntimeState {
    fn new(initial: ViewportTransform, controlled: Option<ViewportTransform>) -> Self {
        Self {
            transform: controlled.unwrap_or(initial).normalized(),
            last_declared_transform: controlled,
            contacts: HashMap::new(),
            pending_start: None,
            last_centroid: None,
            last_distance: None,
            interacting: false,
            velocity: LayoutPoint::ZERO,
            last_motion_at: None,
            inertia_tick: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Contact {
    point: LayoutPoint,
    kind: PointerKind,
    child_claimed: bool,
}

/// Runtime-owned camera and active-contact state, keyed by viewer identity.
#[derive(Debug, Clone, Default)]
pub struct ViewportStateMap {
    states: HashMap<WidgetId, ViewportRuntimeState>,
    captures: HashMap<PointerId, WidgetId>,
}

impl ViewportStateMap {
    pub fn transform(&self, id: WidgetId) -> Option<ViewportTransform> {
        self.states.get(&id).map(|state| state.transform)
    }

    pub fn set_transform(&mut self, id: WidgetId, transform: ViewportTransform) {
        if let Some(state) = self.states.get_mut(&id) {
            state.transform = transform.normalized();
        } else {
            self.states
                .insert(id, ViewportRuntimeState::new(transform, None));
        }
    }

    pub fn reconcile(&mut self, ir: &CoreIR) {
        let mut active = std::collections::HashSet::new();
        for (id, node) in &ir.nodes {
            let Op::Layout(LayoutOp::InteractiveViewport {
                initial_transform,
                controlled_transform,
                ..
            }) = &node.op
            else {
                continue;
            };
            active.insert(*id);
            let state = self.states.entry(*id).or_insert_with(|| {
                ViewportRuntimeState::new(*initial_transform, *controlled_transform)
            });
            if let Some(transform) = controlled_transform {
                // A controlled transform is application authority on every
                // rebuild, including when its value did not change.
                state.transform = transform.normalized();
            }
            state.last_declared_transform = *controlled_transform;
        }
        self.states.retain(|id, _| active.contains(id));
        self.captures.retain(|_, id| active.contains(id));
    }

    pub fn advance_inertia(&mut self, ir: &CoreIR, layout: &LayoutSnapshot) -> bool {
        let now = Instant::now();
        let mut active = false;
        for (id, state) in &mut self.states {
            let Some(last_tick) = state.inertia_tick else {
                continue;
            };
            let Some(config) = viewport_config(ir, *id) else {
                state.inertia_tick = None;
                continue;
            };
            if config.friction <= 0.0 || !state.contacts.is_empty() {
                state.inertia_tick = None;
                continue;
            }
            let elapsed = now.duration_since(last_tick).as_secs_f32().min(0.05);
            state.inertia_tick = Some(now);
            let decay = (-config.friction * elapsed * 1_000_000.0).exp();
            state.velocity.x *= decay;
            state.velocity.y *= decay;
            if state.velocity.x.hypot(state.velocity.y) < 5.0 {
                state.inertia_tick = None;
                state.velocity = LayoutPoint::ZERO;
                continue;
            }
            let delta = constrained_pan(
                LayoutPoint::new(state.velocity.x * elapsed, state.velocity.y * elapsed),
                config.pan_axis,
            );
            state.transform.translation[0] += delta.x;
            state.transform.translation[1] += delta.y;
            state.transform =
                clamp_boundary(state.transform, config.boundary, viewer_rect(layout, *id));
            active = true;
        }
        active
    }

    pub fn retain_active(&mut self, active: &std::collections::HashSet<WidgetId>) {
        self.states.retain(|id, _| active.contains(id));
        self.captures.retain(|_, id| active.contains(id));
    }
}

pub struct ViewportControllerContext<'a> {
    pub ir: &'a CoreIR,
    pub layout: &'a LayoutSnapshot,
    pub scroll: &'a crate::ScrollStateMap,
    pub viewport: &'a mut ViewportStateMap,
    pub gesture: &'a mut crate::env::GestureState,
    pub dispatched_actions: Vec<(WidgetId, ActionEnvelope, ActionInput)>,
}

pub struct ViewportController;

impl ViewportController {
    pub fn handle_event(
        &mut self,
        ctx: &mut ViewportControllerContext<'_>,
        event: &InputEvent,
    ) -> bool {
        match event {
            InputEvent::Pointer(PointerEvent::Down {
                pointer_id,
                kind,
                point,
                button,
                modifiers,
            }) if matches!(button, PointerButton::Primary) => {
                self.pointer_down(ctx, *pointer_id, *kind, *point, *modifiers)
            }
            InputEvent::Pointer(PointerEvent::Move {
                pointer_id,
                point,
                modifiers,
                ..
            }) => self.pointer_move(ctx, *pointer_id, *point, *modifiers),
            InputEvent::Pointer(PointerEvent::Up {
                pointer_id,
                point,
                modifiers,
                ..
            }) => self.pointer_end(ctx, *pointer_id, *point, *modifiers, false),
            InputEvent::Pointer(PointerEvent::Cancel {
                pointer_id,
                point,
                modifiers,
                ..
            }) => self.pointer_end(ctx, *pointer_id, *point, *modifiers, true),
            InputEvent::Pointer(PointerEvent::Scroll {
                point,
                delta,
                delta_mode,
                modifiers,
                ..
            }) => self.scroll(ctx, *point, *delta, *delta_mode, *modifiers),
            InputEvent::Pointer(PointerEvent::Magnify {
                point,
                scale_factor,
                phase,
                modifiers,
            }) => self.magnify(ctx, *point, *scale_factor, *phase, *modifiers),
            _ => false,
        }
    }
}

impl ViewportController {
    fn pointer_down(
        &self,
        ctx: &mut ViewportControllerContext<'_>,
        pointer_id: PointerId,
        kind: PointerKind,
        point: LayoutPoint,
        modifiers: u8,
    ) -> bool {
        let Some((hit, target)) =
            hit_and_nearest_viewport(ctx.ir, ctx.layout, ctx.scroll, ctx.viewport, point)
        else {
            return false;
        };
        let child_claimed = child_drag_claims(ctx.ir, hit, target);
        ctx.viewport.captures.insert(pointer_id, target);
        let Some(state) = ctx.viewport.states.get_mut(&target) else {
            return false;
        };
        state.inertia_tick = None;
        state.velocity = LayoutPoint::ZERO;
        state.last_motion_at = Some(Instant::now());
        state.contacts.insert(
            pointer_id,
            Contact {
                point,
                kind,
                child_claimed,
            },
        );
        if state.contacts.len() == 1 {
            state.pending_start = Some(point);
            state.last_centroid = Some(point);
            state.last_distance = None;
            // Preserve taps and child drags until the pointer crosses the pan threshold.
            return false;
        }

        state.pending_start = None;
        for contact in state.contacts.values_mut() {
            contact.child_claimed = false;
        }
        let (centroid, distance) = contact_geometry(&state.contacts);
        state.last_centroid = Some(centroid);
        state.last_distance = distance;
        state.interacting = true;
        let _ = state;
        crate::input::gesture::cancel_active_drag_for_viewport(
            ctx.ir,
            ctx.layout,
            ctx.viewport,
            ctx.gesture,
            centroid,
            &mut ctx.dispatched_actions,
        );
        clear_generic_gesture(ctx);
        dispatch_viewport_action(
            ctx,
            target,
            InteractionAction::Start,
            centroid,
            LayoutPoint::ZERO,
            1.0,
            input_kind(kind),
            modifiers,
        );
        true
    }

    fn pointer_move(
        &self,
        ctx: &mut ViewportControllerContext<'_>,
        pointer_id: PointerId,
        point: LayoutPoint,
        modifiers: u8,
    ) -> bool {
        let Some(target) = ctx.viewport.captures.get(&pointer_id).copied() else {
            return false;
        };
        let Some(config) = viewport_config(ctx.ir, target) else {
            return false;
        };
        let Some(state) = ctx.viewport.states.get_mut(&target) else {
            return false;
        };
        let (kind, child_claimed) = {
            let Some(contact) = state.contacts.get_mut(&pointer_id) else {
                return false;
            };
            contact.point = point;
            (input_kind(contact.kind), contact.child_claimed)
        };

        if state.contacts.len() == 1 && child_claimed {
            return false;
        }

        let (centroid, distance) = contact_geometry(&state.contacts);
        let mut started = false;
        if !state.interacting {
            let start = state.pending_start.unwrap_or(centroid);
            let dx = centroid.x - start.x;
            let dy = centroid.y - start.y;
            if dx * dx + dy * dy <= PAN_THRESHOLD_SQUARED {
                return false;
            }
            state.interacting = true;
            started = true;
        }

        let previous_centroid = state.last_centroid.unwrap_or(centroid);
        let mut pan = LayoutPoint::new(
            centroid.x - previous_centroid.x,
            centroid.y - previous_centroid.y,
        );
        pan = constrained_pan(pan, config.pan_axis);
        let scale_factor = match (distance, state.last_distance) {
            (Some(next), Some(previous)) if previous > 0.0 => next / previous,
            _ => 1.0,
        };
        let mut transform = state.transform;
        transform.translation[0] += pan.x;
        transform.translation[1] += pan.y;
        if scale_factor.is_finite() && scale_factor > 0.0 {
            let next_scale =
                (transform.scale * scale_factor).clamp(config.min_scale, config.max_scale);
            transform =
                transform.with_scale_around(local_point(ctx.layout, target, centroid), next_scale);
        }
        state.transform =
            clamp_boundary(transform, config.boundary, viewer_rect(ctx.layout, target));
        state.last_centroid = Some(centroid);
        state.last_distance = distance;
        let now = Instant::now();
        if let Some(previous) = state.last_motion_at {
            let elapsed = now.duration_since(previous).as_secs_f32();
            if elapsed > 0.0 {
                state.velocity = LayoutPoint::new(pan.x / elapsed, pan.y / elapsed);
            }
        }
        state.last_motion_at = Some(now);
        let _ = state;
        if started {
            clear_generic_gesture(ctx);
            dispatch_viewport_action(
                ctx,
                target,
                InteractionAction::Start,
                centroid,
                LayoutPoint::ZERO,
                1.0,
                kind,
                modifiers,
            );
        }
        dispatch_viewport_action(
            ctx,
            target,
            InteractionAction::Update,
            centroid,
            pan,
            scale_factor,
            kind,
            modifiers,
        );
        true
    }

    fn pointer_end(
        &self,
        ctx: &mut ViewportControllerContext<'_>,
        pointer_id: PointerId,
        point: LayoutPoint,
        modifiers: u8,
        cancelled: bool,
    ) -> bool {
        let Some(target) = ctx.viewport.captures.remove(&pointer_id) else {
            return false;
        };
        let friction = viewport_config(ctx.ir, target)
            .map(|config| config.friction)
            .unwrap_or(0.0);
        let Some(state) = ctx.viewport.states.get_mut(&target) else {
            return false;
        };
        let kind = state
            .contacts
            .remove(&pointer_id)
            .map(|contact| input_kind(contact.kind))
            .unwrap_or(ViewportInputKind::Unknown);
        let was_interacting = state.interacting;
        if state.contacts.is_empty() {
            state.pending_start = None;
            state.last_centroid = None;
            state.last_distance = None;
            state.interacting = false;
            state.last_motion_at = None;
            if !cancelled && friction > 0.0 && state.velocity.x.hypot(state.velocity.y) >= 5.0 {
                state.inertia_tick = Some(Instant::now());
            } else {
                state.inertia_tick = None;
                state.velocity = LayoutPoint::ZERO;
            }
            if was_interacting {
                dispatch_viewport_action(
                    ctx,
                    target,
                    if cancelled {
                        InteractionAction::Cancel
                    } else {
                        InteractionAction::End
                    },
                    point,
                    LayoutPoint::ZERO,
                    1.0,
                    kind,
                    modifiers,
                );
            }
            return was_interacting;
        }

        // Reset the gesture baseline so a 2 -> 1 transition cannot jump.
        let (centroid, distance) = contact_geometry(&state.contacts);
        state.last_centroid = Some(centroid);
        state.last_distance = distance;
        state.pending_start = None;
        was_interacting
    }

    fn scroll(
        &self,
        ctx: &mut ViewportControllerContext<'_>,
        point: LayoutPoint,
        delta: LayoutPoint,
        delta_mode: ScrollDeltaMode,
        modifiers: u8,
    ) -> bool {
        let Some(target) = nearest_viewport(ctx.ir, ctx.layout, ctx.scroll, ctx.viewport, point)
        else {
            return false;
        };
        let Some(config) = viewport_config(ctx.ir, target) else {
            return false;
        };
        let multiplier = if matches!(delta_mode, ScrollDeltaMode::Line) {
            LINE_DELTA_POINTS
        } else {
            1.0
        };
        let delta = LayoutPoint::new(delta.x * multiplier, delta.y * multiplier);
        let modified = modifiers & (4 | 8) != 0;
        let zoom = matches!(config.zoom_policy, ViewportZoomPolicy::WheelAndTrackpad)
            || (matches!(config.zoom_policy, ViewportZoomPolicy::WheelWithModifier) && modified);
        if zoom {
            let factor = (-delta.y * WHEEL_ZOOM_SENSITIVITY).exp();
            return self.apply_discrete_transform(
                ctx,
                target,
                point,
                LayoutPoint::ZERO,
                factor,
                ViewportInputKind::Wheel,
                modifiers,
            );
        }
        if matches!(config.pan_axis, ViewportPanAxis::None) {
            return false;
        }
        self.apply_discrete_transform(
            ctx,
            target,
            point,
            constrained_pan(LayoutPoint::new(-delta.x, -delta.y), config.pan_axis),
            1.0,
            ViewportInputKind::Wheel,
            modifiers,
        )
    }

    fn magnify(
        &self,
        ctx: &mut ViewportControllerContext<'_>,
        point: LayoutPoint,
        scale_factor: f32,
        phase: PointerPhase,
        modifiers: u8,
    ) -> bool {
        let Some(target) = nearest_viewport(ctx.ir, ctx.layout, ctx.scroll, ctx.viewport, point)
        else {
            return false;
        };
        let Some(config) = viewport_config(ctx.ir, target) else {
            return false;
        };
        if matches!(config.zoom_policy, ViewportZoomPolicy::Disabled) {
            return false;
        }
        let action = match phase {
            PointerPhase::Started => InteractionAction::Start,
            PointerPhase::Moved => InteractionAction::Update,
            PointerPhase::Ended => InteractionAction::End,
            PointerPhase::Cancelled => InteractionAction::Cancel,
        };
        if matches!(action, InteractionAction::Update) {
            self.apply_transform(ctx, target, point, LayoutPoint::ZERO, scale_factor);
        }
        dispatch_viewport_action(
            ctx,
            target,
            action,
            point,
            LayoutPoint::ZERO,
            scale_factor,
            ViewportInputKind::Magnify,
            modifiers,
        );
        true
    }

    fn apply_discrete_transform(
        &self,
        ctx: &mut ViewportControllerContext<'_>,
        target: WidgetId,
        focal: LayoutPoint,
        pan: LayoutPoint,
        scale_factor: f32,
        kind: ViewportInputKind,
        modifiers: u8,
    ) -> bool {
        dispatch_viewport_action(
            ctx,
            target,
            InteractionAction::Start,
            focal,
            LayoutPoint::ZERO,
            1.0,
            kind,
            modifiers,
        );
        self.apply_transform(ctx, target, focal, pan, scale_factor);
        dispatch_viewport_action(
            ctx,
            target,
            InteractionAction::Update,
            focal,
            pan,
            scale_factor,
            kind,
            modifiers,
        );
        dispatch_viewport_action(
            ctx,
            target,
            InteractionAction::End,
            focal,
            LayoutPoint::ZERO,
            1.0,
            kind,
            modifiers,
        );
        true
    }

    fn apply_transform(
        &self,
        ctx: &mut ViewportControllerContext<'_>,
        target: WidgetId,
        focal: LayoutPoint,
        pan: LayoutPoint,
        scale_factor: f32,
    ) {
        let Some(config) = viewport_config(ctx.ir, target) else {
            return;
        };
        let Some(state) = ctx.viewport.states.get_mut(&target) else {
            return;
        };
        let mut transform = state.transform;
        transform.translation[0] += pan.x;
        transform.translation[1] += pan.y;
        if scale_factor.is_finite() && scale_factor > 0.0 {
            transform = transform.with_scale_around(
                local_point(ctx.layout, target, focal),
                (transform.scale * scale_factor).clamp(config.min_scale, config.max_scale),
            );
        }
        state.transform =
            clamp_boundary(transform, config.boundary, viewer_rect(ctx.layout, target));
    }
}

#[derive(Clone, Copy)]
struct ViewportConfig {
    pan_axis: ViewportPanAxis,
    boundary: ViewportBoundary,
    zoom_policy: ViewportZoomPolicy,
    min_scale: f32,
    max_scale: f32,
    friction: f32,
}

fn viewport_config(ir: &CoreIR, id: WidgetId) -> Option<ViewportConfig> {
    let Op::Layout(LayoutOp::InteractiveViewport {
        pan_axis,
        boundary,
        zoom_policy,
        min_scale,
        max_scale,
        friction,
        ..
    }) = &ir.nodes.get(&id)?.op
    else {
        return None;
    };
    Some(ViewportConfig {
        pan_axis: *pan_axis,
        boundary: *boundary,
        zoom_policy: *zoom_policy,
        min_scale: *min_scale,
        max_scale: *max_scale,
        friction: *friction,
    })
}

fn nearest_viewport(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll: &crate::ScrollStateMap,
    viewport: &ViewportStateMap,
    point: LayoutPoint,
) -> Option<WidgetId> {
    hit_and_nearest_viewport(ir, layout, scroll, viewport, point).map(|(_, viewer)| viewer)
}

fn hit_and_nearest_viewport(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll: &crate::ScrollStateMap,
    viewport: &ViewportStateMap,
    point: LayoutPoint,
) -> Option<(WidgetId, WidgetId)> {
    let hit = crate::hit_test::hit_test_with_viewports(ir, layout, scroll, viewport, point)?;
    let mut current = Some(hit);
    while let Some(id) = current {
        let node = ir.nodes.get(&id)?;
        if matches!(node.op, Op::Layout(LayoutOp::InteractiveViewport { .. })) {
            return Some((hit, id));
        }
        current = node.parent;
    }
    None
}

fn child_drag_claims(ir: &CoreIR, hit: WidgetId, viewer: WidgetId) -> bool {
    let mut current = Some(hit);
    while let Some(id) = current {
        if id == viewer {
            return false;
        }
        let Some(node) = ir.nodes.get(&id) else {
            return false;
        };
        if let Op::Semantics(semantics) = &node.op {
            if semantics.actions.entries.iter().any(|entry| {
                matches!(
                    entry.trigger,
                    fission_ir::ActionTrigger::DragStart | fission_ir::ActionTrigger::DragUpdate
                )
            }) {
                return true;
            }
        }
        current = node.parent;
    }
    false
}

fn contact_geometry(contacts: &HashMap<PointerId, Contact>) -> (LayoutPoint, Option<f32>) {
    let mut points: Vec<_> = contacts
        .iter()
        .map(|(id, contact)| (*id, contact.point))
        .collect();
    points.sort_by_key(|(id, _)| id.0);
    let count = points.len().max(1) as f32;
    let centroid = LayoutPoint::new(
        points.iter().map(|(_, point)| point.x).sum::<f32>() / count,
        points.iter().map(|(_, point)| point.y).sum::<f32>() / count,
    );
    let distance = (points.len() >= 2).then(|| {
        let dx = points[1].1.x - points[0].1.x;
        let dy = points[1].1.y - points[0].1.y;
        (dx * dx + dy * dy).sqrt()
    });
    (centroid, distance)
}

fn constrained_pan(delta: LayoutPoint, axis: ViewportPanAxis) -> LayoutPoint {
    match axis {
        ViewportPanAxis::None => LayoutPoint::ZERO,
        ViewportPanAxis::Horizontal => LayoutPoint::new(delta.x, 0.0),
        ViewportPanAxis::Vertical => LayoutPoint::new(0.0, delta.y),
        ViewportPanAxis::Both => delta,
    }
}

fn clamp_boundary(
    mut transform: ViewportTransform,
    boundary: ViewportBoundary,
    viewport: LayoutRect,
) -> ViewportTransform {
    let ViewportBoundary::Finite {
        min_x,
        min_y,
        max_x,
        max_y,
        margin,
    } = boundary
    else {
        return transform;
    };
    let min_tx = viewport.width() - (max_x + margin.right) * transform.scale;
    let max_tx = -(min_x - margin.left) * transform.scale;
    let min_ty = viewport.height() - (max_y + margin.bottom) * transform.scale;
    let max_ty = -(min_y - margin.top) * transform.scale;
    transform.translation[0] = clamp_or_center(transform.translation[0], min_tx, max_tx);
    transform.translation[1] = clamp_or_center(transform.translation[1], min_ty, max_ty);
    transform
}

fn clamp_or_center(value: f32, min: f32, max: f32) -> f32 {
    if min <= max {
        value.clamp(min, max)
    } else {
        (min + max) * 0.5
    }
}

fn viewer_rect(layout: &LayoutSnapshot, id: WidgetId) -> LayoutRect {
    layout
        .get_node_rect(id)
        .unwrap_or(LayoutRect::new(0.0, 0.0, 0.0, 0.0))
}

fn local_point(layout: &LayoutSnapshot, id: WidgetId, point: LayoutPoint) -> [f32; 2] {
    let rect = viewer_rect(layout, id);
    [point.x - rect.origin.x, point.y - rect.origin.y]
}

fn input_kind(kind: PointerKind) -> ViewportInputKind {
    match kind {
        PointerKind::Mouse => ViewportInputKind::Mouse,
        PointerKind::Touch => ViewportInputKind::Touch,
        PointerKind::Stylus => ViewportInputKind::Stylus,
        PointerKind::Unknown => ViewportInputKind::Unknown,
    }
}

#[derive(Clone, Copy)]
enum InteractionAction {
    Start,
    Update,
    End,
    Cancel,
}

fn dispatch_viewport_action(
    ctx: &mut ViewportControllerContext<'_>,
    target: WidgetId,
    action: InteractionAction,
    focal: LayoutPoint,
    pan_delta: LayoutPoint,
    scale_factor: f32,
    input_kind: ViewportInputKind,
    modifiers: u8,
) {
    let Some(node) = ctx.ir.nodes.get(&target) else {
        return;
    };
    let Op::Layout(LayoutOp::InteractiveViewport {
        on_interaction_start,
        on_interaction_update,
        on_interaction_end,
        ..
    }) = &node.op
    else {
        return;
    };
    let entry: Option<&ActionEntry> = match action {
        InteractionAction::Start => on_interaction_start.as_ref(),
        InteractionAction::Update => on_interaction_update.as_ref(),
        InteractionAction::End | InteractionAction::Cancel => on_interaction_end.as_ref(),
    };
    let Some(entry) = entry else { return };
    let Some(state) = ctx.viewport.states.get(&target) else {
        return;
    };
    let local_focal = local_point(ctx.layout, target, focal);
    let world = state.transform.screen_to_world(local_focal);
    let input = ActionInput::ViewportInteraction(ViewportInteraction {
        node_id: target,
        phase: match action {
            InteractionAction::Start => ViewportInteractionPhase::Start,
            InteractionAction::Update => ViewportInteractionPhase::Update,
            InteractionAction::End => ViewportInteractionPhase::End,
            InteractionAction::Cancel => ViewportInteractionPhase::Cancel,
        },
        transform: state.transform,
        viewport_focal_point: LayoutPoint::new(local_focal[0], local_focal[1]),
        world_focal_point: LayoutPoint::new(world[0], world[1]),
        pan_delta,
        scale_factor,
        input_kind,
        modifiers,
    });
    ctx.dispatched_actions.push((
        target,
        ActionEnvelope {
            id: ActionId::from_u128(entry.action_id),
            payload: entry.payload_data.clone().unwrap_or_default(),
        },
        scoped_action_input(ctx.ir, target, input),
    ));
}

fn clear_generic_gesture(ctx: &mut ViewportControllerContext<'_>) {
    ctx.gesture.start_point = None;
    ctx.gesture.last_point = None;
    ctx.gesture.is_panning = false;
    ctx.gesture.target_node = None;
    ctx.gesture.dragging_payload = None;
    ctx.gesture.drag_session = None;
    ctx.gesture.pressed_button = None;
}
