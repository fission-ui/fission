//! Post-layout scroll requests, active-descendant reveal, and container resolution.

use super::*;

impl Runtime {
    /// Queues a post-layout request to reveal a widget in a scroll container.
    pub fn queue_scroll_into_view(&mut self, request: ScrollIntoViewRequest) {
        self.pending_scroll_into_view.push(PendingScrollIntoView {
            request,
            retries_remaining: 1,
        });
    }

    pub(super) fn drain_post_layout_effects(&mut self) {
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
                Effect::Runtime(RuntimeEffect::SelectionRegion { region_id, command }) => {
                    self.pending_selection_regions.push((region_id, command));
                }
                Effect::Runtime(RuntimeEffect::TextEditing { input_id, command }) => {
                    self.pending_text_editing.push((input_id, command));
                }
                Effect::Runtime(RuntimeEffect::TextScroll { input_id, command }) => {
                    self.pending_text_scroll.push((input_id, command));
                }
                Effect::Runtime(RuntimeEffect::TextFormValidation { form_id }) => {
                    self.pending_text_form_validation.push(form_id);
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
        self.drain_post_layout_effects();

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

    pub(super) fn reveal_new_active_descendants(
        &mut self,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> bool {
        let active_descendants: HashMap<WidgetId, WidgetId> = ir
            .nodes
            .iter()
            .filter_map(|(owner, node)| match &node.op {
                Op::Semantics(_) => self
                    .effective_active_descendant(ir, *owner)
                    .map(|target| (*owner, target)),
                _ => None,
            })
            .collect();

        self.revealed_active_descendants.retain(|owner, target| {
            active_descendants
                .get(owner)
                .is_some_and(|active| active == target)
        });

        let mut needs_follow_up_frame = false;
        for (owner, target) in active_descendants {
            if self.revealed_active_descendants.get(&owner) == Some(&target) {
                continue;
            }

            if layout.get_node_geometry(target).is_none() {
                // This automatic reveal follows retained semantic state; it is
                // not a queued effect with a bounded retry lifecycle. A stale
                // relationship or a semantic-only target can therefore never
                // become revealable merely by requesting another identical
                // frame. Remember the pair and reconsider it only if the
                // active descendant changes or is removed and restored.
                self.revealed_active_descendants.insert(owner, target);
                continue;
            }

            let request = ScrollIntoViewRequest {
                container: None,
                target,
                axis: ScrollAxis::Both,
                alignment: ScrollAlignment::Center,
                padding: [0.0; 4],
                behavior: ScrollBehavior::Instant,
                if_needed: true,
            };

            let Some(_containers) = self.resolve_scroll_containers(&request, ir, layout) else {
                // An active descendant that is not inside a scroll container is
                // already fully governed by ordinary layout.
                self.revealed_active_descendants.insert(owner, target);
                continue;
            };

            match self.apply_scroll_into_view(&request, ir, layout) {
                ScrollIntoViewOutcome::Applied { changed } => {
                    self.revealed_active_descendants.insert(owner, target);
                    needs_follow_up_frame |= changed;
                }
                ScrollIntoViewOutcome::Ignored => {
                    self.revealed_active_descendants.insert(owner, target);
                }
                ScrollIntoViewOutcome::Retry => needs_follow_up_frame = true,
            }
        }

        needs_follow_up_frame
    }

    /// Resolves the active descendant currently exposed by `owner`.
    ///
    /// Editable combobox navigation is runtime-owned because platform focus
    /// remains on the text controller. Its current option takes precedence
    /// over the authored/lowered fallback, while other semantic owners retain
    /// their declared relationship unchanged. This is the single projection
    /// used by scrolling and platform accessibility bridges between rebuilds.
    pub fn effective_active_descendant(&self, ir: &CoreIR, owner: WidgetId) -> Option<WidgetId> {
        let semantic_active = ir.nodes.get(&owner).and_then(|node| match &node.op {
            Op::Semantics(semantics) => semantics.active_descendant,
            _ => None,
        });
        self.runtime_state
            .interaction
            .active_descendant(owner)
            .filter(|target| {
                crate::hit_test::is_valid_editable_combobox_active_descendant(ir, owner, *target)
            })
            .or(semantic_active)
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

        let Some(container_ids) = self.resolve_scroll_containers(request, ir, layout) else {
            Self::emit_scroll_container_chain_diag(request, ir, layout);
            Self::emit_scroll_into_view_diag("missing_container", request, None);
            return ScrollIntoViewOutcome::Retry;
        };

        let mut matched_container = false;
        let mut changed = false;
        for container_id in container_ids {
            if !Self::is_descendant_or_self(ir, request.target, container_id)
                && !Self::is_descendant_via_children(ir, request.target, container_id)
            {
                Self::emit_scroll_into_view_diag(
                    "target_not_descendant",
                    request,
                    Some(container_id),
                );
                continue;
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
                Self::emit_scroll_into_view_diag(
                    "not_scroll_container",
                    request,
                    Some(container_id),
                );
                continue;
            };

            if !Self::axis_matches(request.axis, direction) {
                Self::emit_scroll_into_view_diag("axis_mismatch", request, Some(container_id));
                continue;
            }

            matched_container = true;
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
                changed = true;
            }
        }

        if !matched_container {
            return ScrollIntoViewOutcome::Ignored;
        }

        ScrollIntoViewOutcome::Applied { changed }
    }

    pub(super) fn resolve_scroll_containers(
        &self,
        request: &ScrollIntoViewRequest,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> Option<Vec<WidgetId>> {
        if request.container.is_some() || request.axis != ScrollAxis::Both {
            return self
                .resolve_scroll_container(request, ir, layout)
                .map(|container| vec![container]);
        }

        // A Both-axis request may cross nested orthogonal scroll views. Resolve
        // the nearest matching container independently for each axis so an
        // already-aligned inner horizontal view cannot hide an outer vertical
        // view that still needs to reveal the target.
        let mut containers = Vec::with_capacity(2);
        for axis in [ScrollAxis::Vertical, ScrollAxis::Horizontal] {
            let mut axis_request = request.clone();
            axis_request.axis = axis;
            if let Some(container) = self.resolve_scroll_container(&axis_request, ir, layout) {
                if !containers.contains(&container) {
                    containers.push(container);
                }
            }
        }

        (!containers.is_empty()).then_some(containers)
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

        // Responsive alternatives can share a logical widget identity. CoreIR
        // keeps a single parent pointer for that identity, so the pointer can
        // refer to an unmeasured alternative even though another parent path
        // in the active tree contains the measured scroll node. Recover that
        // path from the root's child edges when the fast parent walk fails.
        Self::resolve_scroll_container_from_children(request, ir, layout)
    }

    pub(super) fn resolve_scroll_container_from_children(
        request: &ScrollIntoViewRequest,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) -> Option<WidgetId> {
        let root = ir.root?;
        let mut parents: HashMap<WidgetId, Vec<WidgetId>> = HashMap::new();
        let mut reachable = HashSet::new();
        let mut pending = vec![root];

        while let Some(parent_id) = pending.pop() {
            if !reachable.insert(parent_id) {
                continue;
            }
            let Some(parent) = ir.nodes.get(&parent_id) else {
                continue;
            };
            for child_id in &parent.children {
                parents.entry(*child_id).or_default().push(parent_id);
                pending.push(*child_id);
            }
        }

        // Walk upward by distance so a measured inner scroll is selected
        // before an outer one. Sorting makes shared-parent trees deterministic.
        let mut frontier = vec![request.target];
        let mut visited = HashSet::new();
        visited.insert(request.target);
        while !frontier.is_empty() {
            frontier.sort_unstable_by_key(|id| id.as_u128());
            let mut next = Vec::new();
            for child_id in frontier {
                let Some(ancestor_ids) = parents.get(&child_id) else {
                    continue;
                };
                for ancestor_id in ancestor_ids {
                    if !visited.insert(*ancestor_id) {
                        continue;
                    }
                    if let Some(direction) = Self::scroll_direction(ir, *ancestor_id) {
                        if Self::axis_matches(request.axis, direction)
                            && layout.get_node_geometry(*ancestor_id).is_some()
                        {
                            return Some(*ancestor_id);
                        }
                    }
                    next.push(*ancestor_id);
                }
            }
            frontier = next;
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

    pub(super) fn is_descendant_via_children(
        ir: &CoreIR,
        target: WidgetId,
        ancestor: WidgetId,
    ) -> bool {
        let mut pending = vec![ancestor];
        let mut visited = HashSet::new();
        while let Some(node_id) = pending.pop() {
            if node_id == target {
                return true;
            }
            if !visited.insert(node_id) {
                continue;
            }
            if let Some(node) = ir.nodes.get(&node_id) {
                pending.extend(node.children.iter().copied());
            }
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

    pub(super) fn emit_scroll_container_chain_diag(
        request: &ScrollIntoViewRequest,
        ir: &CoreIR,
        layout: &LayoutSnapshot,
    ) {
        let mut ancestors = Vec::new();
        let mut current = ir.nodes.get(&request.target).and_then(|node| node.parent);
        while let Some(node_id) = current {
            let Some(node) = ir.nodes.get(&node_id) else {
                ancestors.push(format!("{node_id:?}:missing_ir_node"));
                break;
            };
            let op = match &node.op {
                Op::Layout(LayoutOp::Scroll { direction, .. }) => {
                    format!("Scroll({direction:?})")
                }
                Op::Layout(LayoutOp::Responsive { .. }) => "Responsive".to_string(),
                Op::Layout(_) => "Layout".to_string(),
                Op::Semantics(_) => "Semantics".to_string(),
                Op::Structural(_) => "Structural".to_string(),
                Op::Paint(_) => "Paint".to_string(),
            };
            let measured = layout.get_node_geometry(node_id).is_some();
            ancestors.push(format!("{node_id:?}:{op}:measured={measured}"));
            current = node.parent;
        }
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::InputEvent {
                kind: format!(
                    "scroll_into_view:container_search:target={:?}:axis={:?}:ancestors=[{}]",
                    request.target,
                    request.axis,
                    ancestors.join(" -> ")
                ),
                target: Some(request.target.as_u128()),
                position: None,
            },
        );
    }
}
