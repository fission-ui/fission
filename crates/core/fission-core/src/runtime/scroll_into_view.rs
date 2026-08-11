use super::*;

impl Runtime {
    /// Queues a runtime effect that must be resolved by the core runtime.
    ///
    /// Shells call this for effects that require runtime-owned state or a
    /// post-layout pass instead of a host capability executor.
    pub fn queue_runtime_effect(&mut self, effect: RuntimeEffect) -> bool {
        match effect {
            RuntimeEffect::ScrollIntoView(request) => {
                self.queue_scroll_into_view(request);
                true
            }
            RuntimeEffect::Cancel { .. } | RuntimeEffect::ReleaseResource { .. } => false,
        }
    }

    /// Queues a post-layout request to reveal a widget in a scroll container.
    pub fn queue_scroll_into_view(&mut self, request: ScrollIntoViewRequest) {
        self.pending_scroll_into_view.push(PendingScrollIntoView {
            request,
            retries_remaining: 1,
        });
    }

    pub(super) fn drain_scroll_into_view_effects(&mut self) {
        let pending = std::mem::take(&mut self.pending_effects);

        for env in pending {
            let EffectEnvelope {
                req_id,
                effect,
                on_ok,
                on_err,
                service_bindings,
                resource,
            } = env;

            match effect {
                Effect::Runtime(RuntimeEffect::ScrollIntoView(request)) => {
                    self.queue_scroll_into_view(request);
                }
                retained => self.pending_effects.push(EffectEnvelope {
                    req_id,
                    effect: retained,
                    on_ok,
                    on_err,
                    service_bindings,
                    resource,
                }),
            }
        }
    }

    pub(super) fn apply_pending_scroll_into_view(
        &mut self,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> bool {
        self.drain_scroll_into_view_effects();

        let mut needs_follow_up_frame = false;
        let pending = std::mem::take(&mut self.pending_scroll_into_view);

        for mut pending_request in pending {
            match self.apply_scroll_into_view(&pending_request.request, ir, layout) {
                ScrollIntoViewOutcome::Applied { changed } => {
                    needs_follow_up_frame |= changed;
                }
                ScrollIntoViewOutcome::Retry if pending_request.retries_remaining > 0 => {
                    pending_request.retries_remaining -= 1;
                    self.pending_scroll_into_view.push(pending_request);
                    needs_follow_up_frame = true;
                }
                ScrollIntoViewOutcome::Retry | ScrollIntoViewOutcome::Ignored => {}
            }
        }

        needs_follow_up_frame
    }

    pub(super) fn apply_scroll_into_view(
        &mut self,
        request: &ScrollIntoViewRequest,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> ScrollIntoViewOutcome {
        let Some(target_geom) = layout.get_node_geometry(request.target) else {
            Self::emit_scroll_into_view_diag("missing_target", request, None);
            return ScrollIntoViewOutcome::Retry;
        };

        let Some(container_id) = self.resolve_scroll_container(request, ir, layout) else {
            Self::emit_scroll_into_view_diag("missing_container", request, None);
            return ScrollIntoViewOutcome::Retry;
        };

        if !Self::is_descendant_or_self(ir, request.target, container_id) {
            Self::emit_scroll_into_view_diag("target_not_descendant", request, Some(container_id));
            return ScrollIntoViewOutcome::Ignored;
        }

        let Some(container_geom) = layout.get_node_geometry(container_id) else {
            Self::emit_scroll_into_view_diag(
                "missing_container_layout",
                request,
                Some(container_id),
            );
            return ScrollIntoViewOutcome::Retry;
        };

        let Some(direction) = Self::scroll_direction(ir, container_id) else {
            Self::emit_scroll_into_view_diag("not_scroll_container", request, Some(container_id));
            return ScrollIntoViewOutcome::Ignored;
        };

        if !Self::axis_matches(request.axis, direction) {
            Self::emit_scroll_into_view_diag("axis_mismatch", request, Some(container_id));
            return ScrollIntoViewOutcome::Ignored;
        }

        if matches!(request.behavior, ScrollBehavior::Smooth) {
            Self::emit_scroll_into_view_diag(
                "smooth_resolved_as_instant",
                request,
                Some(container_id),
            );
        }

        let current_offset = self.runtime_state.scroll.get_offset(container_id);
        let new_offset = match direction {
            FlexDirection::Column => Self::compute_scroll_offset(
                current_offset,
                target_geom.rect.y() - container_geom.rect.y(),
                target_geom.rect.height(),
                container_geom.rect.height(),
                container_geom.content_size.height,
                request.padding[2],
                request.padding[3],
                request.alignment,
                request.if_needed,
            ),
            FlexDirection::Row => Self::compute_scroll_offset(
                current_offset,
                target_geom.rect.x() - container_geom.rect.x(),
                target_geom.rect.width(),
                container_geom.rect.width(),
                container_geom.content_size.width,
                request.padding[0],
                request.padding[1],
                request.alignment,
                request.if_needed,
            ),
        };

        if (new_offset - current_offset).abs() > f32::EPSILON {
            self.runtime_state
                .scroll
                .set_offset(container_id, new_offset);
            ScrollIntoViewOutcome::Applied { changed: true }
        } else {
            ScrollIntoViewOutcome::Applied { changed: false }
        }
    }

    pub(super) fn resolve_scroll_container(
        &self,
        request: &ScrollIntoViewRequest,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> Option<WidgetId> {
        if let Some(container) = request.container {
            return ir
                .nodes
                .contains_key(&container)
                .then_some(container)
                .filter(|id| layout.get_node_geometry(*id).is_some());
        }

        let mut current = ir.nodes.get(&request.target)?.parent;
        while let Some(node_id) = current {
            if let Some(direction) = Self::scroll_direction(ir, node_id) {
                if Self::axis_matches(request.axis, direction)
                    && layout.get_node_geometry(node_id).is_some()
                {
                    return Some(node_id);
                }
            }
            current = ir.nodes.get(&node_id).and_then(|node| node.parent);
        }

        None
    }

    pub(super) fn scroll_direction(ir: &CoreIR, node_id: WidgetId) -> Option<FlexDirection> {
        match ir.nodes.get(&node_id).map(|node| &node.op) {
            Some(Op::Layout(LayoutOp::Scroll { direction, .. })) => Some(*direction),
            _ => None,
        }
    }

    pub(super) fn axis_matches(axis: ScrollAxis, direction: FlexDirection) -> bool {
        matches!(
            (axis, direction),
            (ScrollAxis::Both, _)
                | (ScrollAxis::Vertical, FlexDirection::Column)
                | (ScrollAxis::Horizontal, FlexDirection::Row)
        )
    }

    pub(super) fn is_descendant_or_self(ir: &CoreIR, target: WidgetId, ancestor: WidgetId) -> bool {
        let mut current = Some(target);
        while let Some(node_id) = current {
            if node_id == ancestor {
                return true;
            }
            current = ir.nodes.get(&node_id).and_then(|node| node.parent);
        }
        false
    }

    pub(super) fn compute_scroll_offset(
        current_offset: f32,
        target_content_start: f32,
        target_size: f32,
        viewport_size: f32,
        content_size: f32,
        padding_start: f32,
        padding_end: f32,
        alignment: ScrollAlignment,
        if_needed: bool,
    ) -> f32 {
        let current_offset = Self::finite_or_zero(current_offset).max(0.0);
        let viewport_size = Self::finite_or_zero(viewport_size).max(0.0);
        let content_size = Self::finite_or_zero(content_size).max(0.0);
        let target_size = Self::finite_or_zero(target_size).max(0.0);
        let padding_start = Self::finite_or_zero(padding_start).max(0.0);
        let padding_end = Self::finite_or_zero(padding_end).max(0.0);
        let max_offset = (content_size - viewport_size).max(0.0);

        if viewport_size <= f32::EPSILON || max_offset <= f32::EPSILON {
            return 0.0;
        }

        let target_start = Self::finite_or_zero(target_content_start);
        let target_end = target_start + target_size;
        let reveal_start = target_start - padding_start;
        let reveal_end = target_end + padding_end;
        let viewport_start = current_offset;
        let viewport_end = current_offset + viewport_size;

        if if_needed && reveal_start >= viewport_start && reveal_end <= viewport_end {
            return current_offset.min(max_offset);
        }

        let desired = match alignment {
            ScrollAlignment::Start => reveal_start,
            ScrollAlignment::Center => {
                let padded_viewport = (viewport_size - padding_start - padding_end).max(0.0);
                target_start - padding_start - (padded_viewport - target_size) * 0.5
            }
            ScrollAlignment::End => reveal_end - viewport_size,
            ScrollAlignment::Nearest => {
                if reveal_end - reveal_start > viewport_size {
                    reveal_start
                } else if reveal_start < viewport_start {
                    reveal_start
                } else if reveal_end > viewport_end {
                    reveal_end - viewport_size
                } else {
                    current_offset
                }
            }
            ScrollAlignment::Fraction(fraction) => {
                let fraction = Self::finite_or_zero(fraction).clamp(0.0, 1.0);
                let padded_viewport = (viewport_size - padding_start - padding_end).max(0.0);
                target_start - padding_start - (padded_viewport - target_size) * fraction
            }
        };

        Self::finite_or_zero(desired).clamp(0.0, max_offset)
    }

    pub(super) fn finite_or_zero(value: f32) -> f32 {
        if value.is_finite() {
            value
        } else {
            0.0
        }
    }

    pub(super) fn emit_scroll_into_view_diag(
        kind: &'static str,
        request: &ScrollIntoViewRequest,
        container: Option<WidgetId>,
    ) {
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::InputEvent {
                kind: format!(
                    "scroll_into_view:{kind}:target={:?}:container={:?}",
                    request.target,
                    container.or(request.container)
                ),
                target: Some(request.target.as_u128()),
                position: None,
            },
        );
    }
}
