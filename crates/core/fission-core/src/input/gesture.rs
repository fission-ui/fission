use super::{ControllerContext, InputController};
use crate::event::{ExternalDragEvent, InputEvent, PointerEvent};
use crate::scrollbar::{
    scrollbar_drag_offset, scrollbar_drag_offset_with_grab, scrollbar_geometry_for_node,
    scrollbar_hit_test, scrollbar_point_for_node, ScrollbarDragState, ScrollbarHitKind,
};
use crate::{Action, ActionEnvelope, ActionId, ActionInput, DragSessionPayload, DragSessionState};
use fission_ir::op::RichTextAnnotation;
use fission_ir::{semantics::ActionTrigger, Op, WidgetId};
use fission_layout::{LayoutPoint, LayoutSnapshot};

fn drag_cancel_route(
    ir: &fission_ir::CoreIR,
    start_node: WidgetId,
) -> Option<(WidgetId, ActionTrigger)> {
    let mut current_id = Some(start_node);
    while let Some(node_id) = current_id {
        let node = ir.nodes.get(&node_id)?;
        if let Op::Semantics(semantics) = &node.op {
            let trigger = [ActionTrigger::DragCancel, ActionTrigger::DragEnd]
                .into_iter()
                .find(|trigger| {
                    semantics
                        .actions
                        .entries
                        .iter()
                        .any(|entry| entry.trigger == *trigger)
                });
            if let Some(trigger) = trigger {
                return Some((node_id, trigger));
            }
        }
        current_id = node.parent;
    }
    None
}

pub(crate) fn cancel_active_drag_for_viewport(
    ir: &fission_ir::CoreIR,
    layout: &LayoutSnapshot,
    viewport: &crate::input::viewport::ViewportStateMap,
    gesture: &crate::env::GestureState,
    point: LayoutPoint,
    dispatched_actions: &mut Vec<(WidgetId, ActionEnvelope, ActionInput)>,
) {
    let Some(start_node) = gesture.target_node.filter(|_| gesture.is_panning) else {
        return;
    };
    let Some((node_id, trigger)) = drag_cancel_route(ir, start_node) else {
        return;
    };
    let Some(node) = ir.nodes.get(&node_id) else {
        return;
    };
    let Op::Semantics(semantics) = &node.op else {
        return;
    };
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == trigger)
    else {
        return;
    };
    let input = if let Some(target) = &semantics.canvas_target {
        ActionInput::CanvasInteraction(crate::input::canvas::canvas_interaction(
            node_id,
            target,
            crate::input::canvas::CanvasInteractionPhase::Cancel,
            point,
            LayoutPoint::ZERO,
            gesture.start_point,
            layout,
            viewport,
            gesture.pointer_kind,
            gesture.modifiers,
        ))
    } else {
        ActionInput::Pointer {
            x: point.x,
            y: point.y,
            delta_x: 0.0,
            delta_y: 0.0,
        }
    };
    dispatched_actions.push((
        node_id,
        ActionEnvelope {
            id: ActionId::from_u128(entry.action_id),
            payload: entry.payload_data.clone().unwrap_or_default(),
        },
        crate::input::scoped_action_input(ir, node_id, input),
    ));
}

pub struct GestureController;

impl InputController for GestureController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Pointer(pe) => {
                match pe {
                    PointerEvent::Down {
                        point,
                        button,
                        kind,
                        modifiers,
                        ..
                    } => {
                        // GestureState currently models one active pointer-button
                        // sequence. Do not let an additional button press replace
                        // the button that must match the eventual release.
                        if ctx.gesture.pressed_button.is_some() {
                            return true;
                        }

                        ctx.gesture.start_point = Some(*point);
                        ctx.gesture.last_point = Some(*point);
                        ctx.gesture.is_panning = false;
                        ctx.gesture.pressed_button = Some(button.clone());
                        ctx.gesture.pointer_kind = *kind;
                        ctx.gesture.modifiers = *modifiers;
                        ctx.gesture.scrollbar_drag = None;

                        if matches!(button, crate::event::PointerButton::Primary) {
                            if let Some(hit) =
                                scrollbar_hit_test(ctx.ir, ctx.layout, ctx.scroll, *point)
                            {
                                let pointer_to_thumb_start = match hit.kind {
                                    ScrollbarHitKind::Thumb => hit.pointer_to_thumb_start,
                                    ScrollbarHitKind::Rail => hit.geometry.thumb_extent() * 0.5,
                                };
                                let new_offset = match hit.kind {
                                    ScrollbarHitKind::Thumb => hit.geometry.offset,
                                    ScrollbarHitKind::Rail => {
                                        scrollbar_drag_offset(hit.geometry, hit.layout_point)
                                    }
                                };
                                ctx.scroll.set_offset(hit.geometry.node_id, new_offset);
                                ctx.gesture.target_node = Some(hit.geometry.node_id);
                                ctx.gesture.dragging_payload = None;
                                ctx.gesture.scrollbar_drag = Some(ScrollbarDragState {
                                    node_id: hit.geometry.node_id,
                                    pointer_to_thumb_start,
                                });
                                return true;
                            }
                        }

                        if let Some(hit) = crate::hit_test::hit_test_with_viewports(
                            ctx.ir,
                            ctx.layout,
                            ctx.scroll,
                            ctx.viewport,
                            *point,
                        ) {
                            ctx.gesture.target_node = Some(hit);
                            ctx.gesture.dragging_payload =
                                matches!(button, crate::event::PointerButton::Primary)
                                    .then(|| self.find_drag_payload(ctx, hit))
                                    .flatten();
                        } else {
                            ctx.gesture.target_node = None;
                            ctx.gesture.dragging_payload = None;
                        }
                    }
                    PointerEvent::Move {
                        point,
                        kind,
                        modifiers,
                        ..
                    } => {
                        ctx.gesture.pointer_kind = *kind;
                        ctx.gesture.modifiers = *modifiers;
                        if !matches!(
                            ctx.gesture.pressed_button,
                            Some(crate::event::PointerButton::Primary)
                        ) {
                            return false;
                        }

                        if let Some(drag) = ctx.gesture.scrollbar_drag {
                            if let Some(geometry) = scrollbar_geometry_for_node(
                                ctx.ir,
                                ctx.layout,
                                ctx.scroll,
                                drag.node_id,
                            ) {
                                let new_offset = scrollbar_drag_offset_with_grab(
                                    geometry,
                                    scrollbar_point_for_node(
                                        ctx.ir,
                                        ctx.scroll,
                                        drag.node_id,
                                        *point,
                                    ),
                                    drag.pointer_to_thumb_start,
                                );
                                ctx.scroll.set_offset(drag.node_id, new_offset);
                            }
                            ctx.gesture.last_point = Some(*point);
                            return true;
                        }

                        if let Some(start) = ctx.gesture.start_point {
                            let dx = point.x - start.x;
                            let dy = point.y - start.y;
                            let dist_sq = dx * dx + dy * dy;
                            let threshold = 5.0 * 5.0;

                            if !ctx.gesture.is_panning && dist_sq > threshold {
                                ctx.gesture.is_panning = true;
                                if let Some(payload) = ctx.gesture.dragging_payload.clone() {
                                    let target = ctx.gesture.target_node;
                                    let source_identifier =
                                        target.and_then(|id| self.semantic_identifier(ctx, id));
                                    ctx.gesture.drag_session = Some(DragSessionState {
                                        source_node: target,
                                        source_identifier,
                                        payload: DragSessionPayload::Internal(payload),
                                        point: *point,
                                        target_node: None,
                                        target_identifier: None,
                                    });
                                    self.update_drag_target(ctx, *point);
                                }
                                // Dispatch DragStart now
                                if let Some(target) = ctx.gesture.target_node {
                                    self.dispatch_trigger(
                                        ctx,
                                        target,
                                        ActionTrigger::DragStart,
                                        *point,
                                        None,
                                    );
                                }
                            }

                            if ctx.gesture.is_panning {
                                if let Some(session) = ctx.gesture.drag_session.as_mut() {
                                    session.point = *point;
                                }
                                self.update_drag_target(ctx, *point);

                                let last = ctx.gesture.last_point.unwrap_or(start);
                                let delta = LayoutPoint {
                                    x: point.x - last.x,
                                    y: point.y - last.y,
                                };
                                ctx.gesture.last_point = Some(*point);

                                // Try dispatching DragUpdate
                                let dispatched = if let Some(target) = ctx.gesture.target_node {
                                    self.dispatch_trigger(
                                        ctx,
                                        target,
                                        ActionTrigger::DragUpdate,
                                        *point,
                                        Some(delta),
                                    )
                                } else {
                                    false
                                };

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
                    PointerEvent::Up {
                        point,
                        button,
                        kind,
                        modifiers,
                        ..
                    } => {
                        ctx.gesture.pointer_kind = *kind;
                        ctx.gesture.modifiers = *modifiers;
                        let scrollbar_drag = ctx.gesture.scrollbar_drag.take();
                        let mut handled = false;
                        let pressed_button = ctx.gesture.pressed_button.clone();
                        let buttons_match = pressed_button.as_ref() == Some(button);
                        let was_primary =
                            matches!(pressed_button, Some(crate::event::PointerButton::Primary));
                        let was_secondary =
                            matches!(pressed_button, Some(crate::event::PointerButton::Secondary));

                        if pressed_button.is_some() && !buttons_match {
                            self.reset_pointer_sequence(ctx, *point);
                            return true;
                        }

                        if buttons_match && ctx.gesture.is_panning {
                            // Internal Drop
                            if let Some(payload) = ctx.gesture.dragging_payload.take() {
                                if let Some(up_hit) = crate::hit_test::hit_test_with_viewports(
                                    ctx.ir,
                                    ctx.layout,
                                    ctx.scroll,
                                    ctx.viewport,
                                    *point,
                                ) {
                                    let _ = self.dispatch_internal_drop(
                                        ctx, up_hit, payload, *point, *modifiers,
                                    );
                                }
                            }

                            if let Some(target) = ctx.gesture.target_node {
                                self.dispatch_trigger(
                                    ctx,
                                    target,
                                    ActionTrigger::DragEnd,
                                    *point,
                                    None,
                                );
                            }
                            handled = true;
                        } else if buttons_match && was_secondary {
                            // Secondary click (right-click)
                            if let Some(target) = ctx.gesture.target_node {
                                if let Some(up_hit) = crate::hit_test::hit_test_with_viewports(
                                    ctx.ir,
                                    ctx.layout,
                                    ctx.scroll,
                                    ctx.viewport,
                                    *point,
                                ) {
                                    if up_hit == target
                                        || self.is_descendant(ctx, up_hit, target)
                                        || self.is_descendant(ctx, target, up_hit)
                                    {
                                        if let Some(menu_owner) =
                                            self.find_context_menu_owner(ctx, up_hit)
                                        {
                                            ctx.context_menu.open(menu_owner, *point);
                                            handled = true;
                                        }

                                        let rich_text_path = self.path_for_node(ctx, up_hit);
                                        if !handled {
                                            if let Some((annotation_node_id, annotation)) =
                                                crate::input::hover::resolve_rich_text_annotation_at_point(
                                                    ctx,
                                                    &rich_text_path,
                                                    *point,
                                                )
                                            {
                                                handled = self.dispatch_annotation_trigger(
                                                    ctx,
                                                    annotation_node_id,
                                                    &annotation,
                                                    ActionTrigger::SecondaryClick,
                                                    *point,
                                                );
                                            }
                                        }

                                        if !handled
                                            && self.dispatch_trigger(
                                                ctx,
                                                target,
                                                ActionTrigger::SecondaryClick,
                                                *point,
                                                None,
                                            )
                                        {
                                            handled = true;
                                        }
                                    }
                                }
                            }
                        } else if buttons_match && was_primary {
                            // Tap (primary click)
                            if let Some(target) = ctx.gesture.target_node {
                                if let Some(up_hit) = crate::hit_test::hit_test_with_viewports(
                                    ctx.ir,
                                    ctx.layout,
                                    ctx.scroll,
                                    ctx.viewport,
                                    *point,
                                ) {
                                    if up_hit == target
                                        || self.is_descendant(ctx, up_hit, target)
                                        || self.is_descendant(ctx, target, up_hit)
                                    {
                                        let rich_text_path = self.path_for_node(ctx, up_hit);
                                        if let Some((annotation_node_id, annotation)) =
                                            crate::input::hover::resolve_rich_text_annotation_at_point(
                                                ctx,
                                                &rich_text_path,
                                                *point,
                                            )
                                        {
                                            handled = self.dispatch_annotation_trigger(
                                                ctx,
                                                annotation_node_id,
                                                &annotation,
                                                ActionTrigger::Default,
                                                *point,
                                            );
                                        }

                                        if !handled
                                            && self.dispatch_trigger(
                                                ctx,
                                                target,
                                                ActionTrigger::Default,
                                                *point,
                                                None,
                                            )
                                        {
                                            handled = true;
                                        }
                                    }
                                }
                            }
                        }

                        if !was_secondary {
                            ctx.context_menu.close();
                        }
                        self.reset_pointer_sequence(ctx, *point);
                        if scrollbar_drag.is_some() {
                            ctx.gesture.target_node = None;
                            return true;
                        }
                        return handled;
                    }
                    PointerEvent::Cancel {
                        point,
                        kind,
                        modifiers,
                        ..
                    } => {
                        let had_sequence = ctx.gesture.pressed_button.is_some()
                            || ctx.gesture.drag_session.is_some()
                            || ctx.gesture.scrollbar_drag.is_some();
                        ctx.gesture.pointer_kind = *kind;
                        ctx.gesture.modifiers = *modifiers;
                        if ctx.gesture.is_panning {
                            if let Some(target) = ctx.gesture.target_node {
                                if let Some((owner, trigger)) = drag_cancel_route(ctx.ir, target) {
                                    self.dispatch_trigger_with_phase(
                                        ctx,
                                        owner,
                                        trigger,
                                        *point,
                                        None,
                                        Some(crate::input::canvas::CanvasInteractionPhase::Cancel),
                                    );
                                }
                            }
                        }
                        ctx.gesture.scrollbar_drag = None;
                        self.reset_pointer_sequence(ctx, *point);
                        ctx.gesture.target_node = None;
                        return had_sequence;
                    }
                    _ => {}
                }
            }
            InputEvent::ExternalDrag(event) => match event {
                ExternalDragEvent::Hover { point, paths, .. } => {
                    ctx.gesture.drag_session = Some(DragSessionState {
                        source_node: None,
                        source_identifier: None,
                        payload: DragSessionPayload::ExternalFiles(paths.clone()),
                        point: *point,
                        target_node: ctx
                            .gesture
                            .drag_session
                            .as_ref()
                            .and_then(|s| s.target_node),
                        target_identifier: ctx
                            .gesture
                            .drag_session
                            .as_ref()
                            .and_then(|s| s.target_identifier.clone()),
                    });
                    self.update_drag_target(ctx, *point);
                    return true;
                }
                ExternalDragEvent::Cancel => {
                    let point = ctx
                        .gesture
                        .drag_session
                        .as_ref()
                        .map(|session| session.point)
                        .unwrap_or(LayoutPoint::ZERO);
                    self.clear_drag_target(ctx, point);
                    ctx.gesture.drag_session = None;
                    return true;
                }
                ExternalDragEvent::Drop {
                    point,
                    paths,
                    modifiers,
                } => {
                    ctx.gesture.drag_session = Some(DragSessionState {
                        source_node: None,
                        source_identifier: None,
                        payload: DragSessionPayload::ExternalFiles(paths.clone()),
                        point: *point,
                        target_node: ctx
                            .gesture
                            .drag_session
                            .as_ref()
                            .and_then(|s| s.target_node),
                        target_identifier: ctx
                            .gesture
                            .drag_session
                            .as_ref()
                            .and_then(|s| s.target_identifier.clone()),
                    });
                    self.update_drag_target(ctx, *point);
                    if let Some(target) = ctx
                        .gesture
                        .drag_session
                        .as_ref()
                        .and_then(|s| s.target_node)
                    {
                        let _ = self.dispatch_external_drop(
                            ctx,
                            target,
                            paths.clone(),
                            *point,
                            *modifiers,
                        );
                    }
                    self.clear_drag_target(ctx, *point);
                    ctx.gesture.drag_session = None;
                    return true;
                }
            },
            InputEvent::ContextMenuRequested { point, .. } => {
                return self.handle_context_menu_request(ctx, *point);
            }
            _ => {}
        }
        false
    }
}

impl GestureController {
    fn handle_context_menu_request(
        &mut self,
        ctx: &mut ControllerContext,
        point: LayoutPoint,
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
        if let Some(menu_owner) = self.find_context_menu_owner(ctx, hit) {
            ctx.context_menu.open(menu_owner, point);
            return true;
        }
        let rich_text_path = self.path_for_node(ctx, hit);
        if let Some((annotation_node_id, annotation)) =
            crate::input::hover::resolve_rich_text_annotation_at_point(ctx, &rich_text_path, point)
        {
            if self.dispatch_annotation_trigger(
                ctx,
                annotation_node_id,
                &annotation,
                ActionTrigger::SecondaryClick,
                point,
            ) {
                return true;
            }
        }
        self.dispatch_trigger(ctx, hit, ActionTrigger::SecondaryClick, point, None)
    }

    fn reset_pointer_sequence(&self, ctx: &mut ControllerContext, point: LayoutPoint) {
        ctx.gesture.start_point = None;
        ctx.gesture.is_panning = false;
        ctx.gesture.dragging_payload = None;
        self.clear_drag_target(ctx, point);
        ctx.gesture.drag_session = None;
        ctx.gesture.pressed_button = None;
    }

    fn path_for_node(&self, ctx: &ControllerContext, node_id: WidgetId) -> Vec<WidgetId> {
        let mut path = Vec::new();
        let mut curr = Some(node_id);
        while let Some(id) = curr {
            path.push(id);
            curr = ctx.ir.nodes.get(&id).and_then(|node| node.parent);
        }
        path
    }

    fn is_descendant(&self, ctx: &ControllerContext, child: WidgetId, ancestor: WidgetId) -> bool {
        let mut curr = Some(child);
        while let Some(id) = curr {
            if id == ancestor {
                return true;
            }
            if let Some(node) = ctx.ir.nodes.get(&id) {
                curr = node.parent;
            } else {
                break;
            }
        }
        false
    }

    fn find_context_menu_owner(
        &self,
        ctx: &ControllerContext,
        start_node: WidgetId,
    ) -> Option<WidgetId> {
        let mut current_id = Some(start_node);
        while let Some(node_id) = current_id {
            let Some(node) = ctx.ir.nodes.get(&node_id) else {
                break;
            };
            if let Op::Semantics(semantics) = &node.op {
                if semantics.context_menu && !semantics.disabled {
                    return Some(node_id);
                }
            }
            current_id = node.parent;
        }
        None
    }

    fn dispatch_annotation_trigger(
        &self,
        ctx: &mut ControllerContext,
        node_id: WidgetId,
        annotation: &RichTextAnnotation,
        trigger: ActionTrigger,
        point: LayoutPoint,
    ) -> bool {
        let Some(action_entry) = annotation
            .actions
            .iter()
            .find(|entry| entry.trigger == trigger)
        else {
            return false;
        };
        let Some(payload) = &action_entry.payload_data else {
            return false;
        };

        let input = crate::input::scoped_action_input(
            ctx.ir,
            node_id,
            ActionInput::Pointer {
                x: point.x,
                y: point.y,
                delta_x: 0.0,
                delta_y: 0.0,
            },
        );
        ctx.dispatched_actions.push((
            node_id,
            ActionEnvelope {
                id: ActionId::from_u128(action_entry.action_id),
                payload: payload.clone(),
            },
            input,
        ));
        true
    }

    fn find_drag_payload(&self, ctx: &ControllerContext, start_node: WidgetId) -> Option<Vec<u8>> {
        let mut current_id = Some(start_node);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    if let Some(p) = &sem.drag_payload {
                        return Some(p.clone());
                    }
                }
                current_id = node.parent;
            } else {
                break;
            }
        }
        None
    }

    fn semantic_identifier(&self, ctx: &ControllerContext, start_node: WidgetId) -> Option<String> {
        let mut current_id = Some(start_node);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    if let Some(identifier) = &sem.identifier {
                        return Some(identifier.clone());
                    }
                }
                current_id = node.parent;
            } else {
                break;
            }
        }
        None
    }

    fn find_drop_target(&self, ctx: &ControllerContext, start_node: WidgetId) -> Option<WidgetId> {
        let mut current_id = Some(start_node);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    if sem
                        .actions
                        .entries
                        .iter()
                        .any(|entry| entry.trigger == ActionTrigger::Drop)
                    {
                        return Some(node_id);
                    }
                }
                current_id = node.parent;
            } else {
                break;
            }
        }
        None
    }

    fn update_drag_target(&self, ctx: &mut ControllerContext, point: LayoutPoint) {
        let next_target = crate::hit_test::hit_test_with_viewports(
            ctx.ir,
            ctx.layout,
            ctx.scroll,
            ctx.viewport,
            point,
        )
        .and_then(|hit| self.find_drop_target(ctx, hit));

        let previous_target = ctx
            .gesture
            .drag_session
            .as_ref()
            .and_then(|s| s.target_node);
        if previous_target == next_target {
            return;
        }

        if let Some(previous) = previous_target {
            self.dispatch_trigger(ctx, previous, ActionTrigger::DragLeave, point, None);
        }
        if let Some(next) = next_target {
            self.dispatch_trigger(ctx, next, ActionTrigger::DragEnter, point, None);
        }

        let next_identifier = next_target.and_then(|id| self.semantic_identifier(ctx, id));
        if let Some(session) = ctx.gesture.drag_session.as_mut() {
            session.target_node = next_target;
            session.target_identifier = next_identifier;
        }
    }

    fn clear_drag_target(&self, ctx: &mut ControllerContext, point: LayoutPoint) {
        if let Some(previous) = ctx
            .gesture
            .drag_session
            .as_ref()
            .and_then(|s| s.target_node)
        {
            self.dispatch_trigger(ctx, previous, ActionTrigger::DragLeave, point, None);
        }
        if let Some(session) = ctx.gesture.drag_session.as_mut() {
            session.target_node = None;
            session.target_identifier = None;
        }
    }

    fn dispatch_internal_drop(
        &self,
        ctx: &mut ControllerContext,
        target_node: WidgetId,
        payload: Vec<u8>,
        point: LayoutPoint,
        modifiers: u8,
    ) -> bool {
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

                            let input = crate::input::scoped_action_input(
                                ctx.ir,
                                node_id,
                                ActionInput::InternalDrop {
                                    payload: payload.clone(),
                                    x: point.x,
                                    y: point.y,
                                    modifiers,
                                },
                            );

                            ctx.dispatched_actions.push((node_id, envelope, input));
                            return true;
                        }
                    }
                }
                current_id = node.parent;
            } else {
                break;
            }
        }
        false
    }

    fn dispatch_external_drop(
        &self,
        ctx: &mut ControllerContext,
        target_node: WidgetId,
        paths: Vec<String>,
        point: LayoutPoint,
        modifiers: u8,
    ) -> bool {
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

                            let input = crate::input::scoped_action_input(
                                ctx.ir,
                                node_id,
                                ActionInput::Drop {
                                    paths: paths.clone(),
                                    x: point.x,
                                    y: point.y,
                                    modifiers,
                                },
                            );

                            ctx.dispatched_actions.push((node_id, envelope, input));
                            return true;
                        }
                    }
                }
                current_id = node.parent;
            } else {
                break;
            }
        }
        false
    }

    fn dispatch_trigger(
        &self,
        ctx: &mut ControllerContext,
        start_node: WidgetId,
        trigger: ActionTrigger,
        point: LayoutPoint,
        delta: Option<LayoutPoint>,
    ) -> bool {
        self.dispatch_trigger_with_phase(ctx, start_node, trigger, point, delta, None)
    }

    fn dispatch_trigger_with_phase(
        &self,
        ctx: &mut ControllerContext,
        start_node: WidgetId,
        trigger: ActionTrigger,
        point: LayoutPoint,
        delta: Option<LayoutPoint>,
        phase: Option<crate::input::canvas::CanvasInteractionPhase>,
    ) -> bool {
        let mut current_id = Some(start_node);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(sem) = &node.op {
                    let mut handled = false;
                    for entry in &sem.actions.entries {
                        if entry.trigger == trigger {
                            let envelope = ActionEnvelope {
                                id: ActionId::from_u128(entry.action_id),
                                payload: entry.payload_data.clone().unwrap_or_default(),
                            };

                            let delta = delta.unwrap_or(LayoutPoint::ZERO);
                            let input = if let Some(target) = &sem.canvas_target {
                                ActionInput::CanvasInteraction(
                                    crate::input::canvas::canvas_interaction(
                                        node_id,
                                        target,
                                        phase.unwrap_or_else(|| canvas_phase(trigger)),
                                        point,
                                        delta,
                                        ctx.gesture.start_point,
                                        ctx.layout,
                                        ctx.viewport,
                                        ctx.gesture.pointer_kind,
                                        ctx.gesture.modifiers,
                                    ),
                                )
                            } else {
                                ActionInput::Pointer {
                                    x: point.x,
                                    y: point.y,
                                    delta_x: delta.x,
                                    delta_y: delta.y,
                                }
                            };
                            let input = crate::input::scoped_action_input(ctx.ir, node_id, input);

                            ctx.dispatched_actions.push((node_id, envelope, input));
                            handled = true;
                            break;
                        }
                    }
                    if trigger == ActionTrigger::Default {
                        if let Some(hyperlink) = &sem.hyperlink {
                            let navigation_already_bound =
                                sem.actions.entries.iter().any(|entry| {
                                    entry.trigger == ActionTrigger::Default
                                        && ActionId::from_u128(entry.action_id)
                                            == crate::NavigationRequested::static_id()
                                });
                            if !navigation_already_bound {
                                ctx.dispatched_actions.push((
                                    node_id,
                                    crate::NavigationRequested::new(
                                        crate::NavigationCommand::Open(hyperlink.clone()),
                                    )
                                    .into(),
                                    crate::input::scoped_action_input(
                                        ctx.ir,
                                        node_id,
                                        ActionInput::Pointer {
                                            x: point.x,
                                            y: point.y,
                                            delta_x: 0.0,
                                            delta_y: 0.0,
                                        },
                                    ),
                                ));
                            }
                            handled = true;
                        }
                    }
                    if handled {
                        return true;
                    }
                }
                current_id = node.parent;
            } else {
                break;
            }
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
                    if let fission_ir::Op::Layout(fission_ir::op::LayoutOp::Scroll {
                        direction,
                        ..
                    }) = &node.op
                    {
                        let current_offset = ctx.scroll.get_offset(id);
                        let move_val = match direction {
                            fission_ir::op::FlexDirection::Row => -delta.x,
                            fission_ir::op::FlexDirection::Column => -delta.y,
                        };

                        let mut new_offset = current_offset + move_val;

                        if let Some(geom) = ctx.layout.get_node_geometry(id) {
                            let max_offset =
                                if matches!(direction, fission_ir::op::FlexDirection::Row) {
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
                } else {
                    break;
                }
            }
        }
        false
    }
}

fn canvas_phase(trigger: ActionTrigger) -> crate::input::canvas::CanvasInteractionPhase {
    use crate::input::canvas::CanvasInteractionPhase;
    match trigger {
        ActionTrigger::DragStart => CanvasInteractionPhase::Start,
        ActionTrigger::DragUpdate => CanvasInteractionPhase::Update,
        ActionTrigger::DragEnd => CanvasInteractionPhase::End,
        ActionTrigger::DragCancel => CanvasInteractionPhase::Cancel,
        _ => CanvasInteractionPhase::Activate,
    }
}
