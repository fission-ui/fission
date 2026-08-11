use super::*;

pub(super) fn rects_intersect(a: LayoutRect, b: LayoutRect) -> bool {
    a.x() < b.right() && a.right() > b.x() && a.y() < b.bottom() && a.bottom() > b.y()
}

pub(super) fn visual_rect_for_node(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
    node_id: WidgetId,
) -> Option<LayoutRect> {
    let mut rect = snap.get_node_rect(node_id)?;
    let mut current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ir.nodes.get(&parent_id) else {
            break;
        };
        if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) = &parent.op {
            let offset = scroll.get_offset(parent_id);
            match direction {
                fission_ir::FlexDirection::Row => rect.origin.x -= offset,
                fission_ir::FlexDirection::Column => rect.origin.y -= offset,
            }
        }
        current = parent.parent;
    }
    Some(rect)
}

pub(super) fn rect_visible_in_scroll_ancestors(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
    node_id: WidgetId,
    rect: LayoutRect,
) -> bool {
    let viewport = LayoutRect::new(
        0.0,
        0.0,
        snap.viewport_size.width,
        snap.viewport_size.height,
    );
    if !rects_intersect(rect, viewport) {
        return false;
    }

    let mut current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ir.nodes.get(&parent_id) else {
            break;
        };
        if matches!(
            parent.op,
            fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. })
                | fission_ir::Op::Layout(fission_ir::LayoutOp::Clip { .. })
        ) {
            let Some(parent_rect) = visual_rect_for_node(ir, snap, scroll, parent_id) else {
                return false;
            };
            if !rects_intersect(rect, parent_rect) {
                return false;
            }
        }
        current = parent.parent;
    }

    true
}

pub(super) fn intersect_rect(a: LayoutRect, b: LayoutRect) -> Option<LayoutRect> {
    let left = a.x().max(b.x());
    let top = a.y().max(b.y());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    (right > left && bottom > top).then(|| LayoutRect::new(left, top, right - left, bottom - top))
}

pub(super) fn clipped_visible_rect_for_node(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
    node_id: WidgetId,
) -> Option<LayoutRect> {
    let viewport = LayoutRect::new(
        0.0,
        0.0,
        snap.viewport_size.width,
        snap.viewport_size.height,
    );
    let mut visible = intersect_rect(visual_rect_for_node(ir, snap, scroll, node_id)?, viewport)?;

    let mut current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ir.nodes.get(&parent_id) else {
            break;
        };
        if matches!(
            parent.op,
            fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. })
                | fission_ir::Op::Layout(fission_ir::LayoutOp::Clip { .. })
        ) {
            let parent_rect = visual_rect_for_node(ir, snap, scroll, parent_id)?;
            visible = intersect_rect(visible, parent_rect)?;
        }
        current = parent.parent;
    }

    Some(visible)
}

pub(super) fn bounds_from_rect(rect: LayoutRect) -> fission_test_driver::Bounds {
    fission_test_driver::Bounds {
        x: rect.x(),
        y: rect.y(),
        width: rect.width(),
        height: rect.height(),
    }
}

pub(super) fn visibility_state(
    visual: Option<LayoutRect>,
    visible: Option<LayoutRect>,
) -> fission_test_driver::VisibilityState {
    let Some(visual) = visual else {
        return fission_test_driver::VisibilityState::Hidden;
    };
    let Some(visible) = visible else {
        return fission_test_driver::VisibilityState::Hidden;
    };
    if visible.width() <= 0.0 || visible.height() <= 0.0 {
        return fission_test_driver::VisibilityState::Hidden;
    }
    let fully_visible = (visible.x() - visual.x()).abs() < 0.5
        && (visible.y() - visual.y()).abs() < 0.5
        && (visible.width() - visual.width()).abs() < 0.5
        && (visible.height() - visual.height()).abs() < 0.5;
    if fully_visible {
        fission_test_driver::VisibilityState::FullyVisible
    } else {
        fission_test_driver::VisibilityState::PartiallyVisible
    }
}

pub(super) fn is_semantic_node(ir: &CoreIR, id: WidgetId) -> bool {
    ir.nodes
        .get(&id)
        .is_some_and(|node| matches!(node.op, fission_ir::Op::Semantics(_)))
}

pub(super) fn nearest_semantic_parent(ir: &CoreIR, id: WidgetId) -> Option<WidgetId> {
    let mut current = ir.nodes.get(&id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        if is_semantic_node(ir, parent_id) {
            return Some(parent_id);
        }
        current = ir.nodes.get(&parent_id).and_then(|node| node.parent);
    }
    None
}

pub(super) fn is_descendant_of(ir: &CoreIR, node_id: WidgetId, ancestor_id: WidgetId) -> bool {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        if current_id == ancestor_id {
            return true;
        }
        current = ir.nodes.get(&current_id).and_then(|node| node.parent);
    }
    false
}

#[derive(Clone)]
pub(super) struct SemanticRecord {
    pub(super) id: WidgetId,
    semantics: Semantics,
    node: fission_test_driver::SemanticNode,
}

pub(super) fn collect_semantic_records(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
) -> Vec<SemanticRecord> {
    let mut semantic_ids: Vec<WidgetId> = ir
        .nodes
        .iter()
        .filter_map(|(id, node)| matches!(node.op, fission_ir::Op::Semantics(_)).then_some(*id))
        .collect();
    semantic_ids.sort_by_key(|id| id.as_u128());

    let mut semantic_children: HashMap<WidgetId, Vec<String>> = HashMap::new();
    for id in &semantic_ids {
        if let Some(parent_id) = nearest_semantic_parent(ir, *id) {
            semantic_children
                .entry(parent_id)
                .or_default()
                .push(id.to_string());
        }
    }

    semantic_ids
        .into_iter()
        .filter_map(|id| {
            let node = ir.nodes.get(&id)?;
            let fission_ir::Op::Semantics(semantics) = &node.op else {
                return None;
            };
            let logical = snap
                .get_node_rect(id)
                .unwrap_or_else(|| LayoutRect::new(0.0, 0.0, 0.0, 0.0));
            let visual = visual_rect_for_node(ir, snap, scroll, id);
            let visible = clipped_visible_rect_for_node(ir, snap, scroll, id);
            let visibility = visibility_state(visual, visible);
            let visible_bounds = visible.map(bounds_from_rect);
            let legacy_bounds = visible_bounds.unwrap_or(fission_test_driver::Bounds {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            });
            let value_present = semantics
                .value
                .as_deref()
                .map(|value| !value.is_empty())
                .unwrap_or(false);
            let value = if semantics.masked {
                None
            } else {
                semantics.value.clone()
            };
            let semantic_node = fission_test_driver::SemanticNode {
                identifier: semantics.identifier.clone(),
                widget_id: id.to_string(),
                stable_node_id: id.to_string(),
                parent: nearest_semantic_parent(ir, id).map(|parent| parent.to_string()),
                children: semantic_children.remove(&id).unwrap_or_default(),
                role: format!("{:?}", semantics.role),
                label: semantics.label.clone(),
                value,
                value_present,
                focusable: semantics.focusable,
                disabled: semantics.disabled,
                read_only: semantics.read_only,
                checked: semantics.checked,
                actions: semantics
                    .actions
                    .entries
                    .iter()
                    .map(|entry| format!("{:?}", entry.trigger))
                    .collect(),
                text_selection: semantics.text_selection,
                masked: semantics.masked,
                scrollable_x: semantics.scrollable_x,
                scrollable_y: semantics.scrollable_y,
                logical_bounds: bounds_from_rect(logical),
                visible_bounds,
                visibility,
                x: legacy_bounds.x,
                y: legacy_bounds.y,
                width: legacy_bounds.width,
                height: legacy_bounds.height,
            };
            Some(SemanticRecord {
                id,
                semantics: semantics.clone(),
                node: semantic_node,
            })
        })
        .collect()
}

pub(super) fn selector_matches(
    record: &SemanticRecord,
    selector: &fission_test_driver::Selector,
) -> bool {
    match selector {
        fission_test_driver::Selector::SemanticIdentifier { identifier }
        | fission_test_driver::Selector::AccessibilityIdentifier { identifier } => {
            record.node.identifier.as_deref() == Some(identifier.as_str())
        }
        fission_test_driver::Selector::TestId { test_id } => {
            record.node.identifier.as_deref() == Some(test_id.as_str())
        }
        fission_test_driver::Selector::WidgetId { widget_id } => {
            record.id == parse_widget_selector(widget_id)
        }
        fission_test_driver::Selector::RoleLabel { role, label } => {
            record.node.role.eq_ignore_ascii_case(role)
                && record.node.label.as_deref() == Some(label.as_str())
        }
        fission_test_driver::Selector::Label { label } => {
            record.node.label.as_deref() == Some(label.as_str())
        }
    }
}

pub(super) fn parse_widget_selector(widget_id: &str) -> WidgetId {
    let trimmed = widget_id
        .strip_prefix("WidgetId(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(widget_id)
        .trim_start_matches("0x");
    if trimmed.len() == 32 {
        if let Ok(raw) = u128::from_str_radix(trimmed, 16) {
            return WidgetId::from_u128(raw);
        }
    }
    WidgetId::explicit(widget_id)
}

pub(super) fn selector_failure(
    query: fission_test_driver::SelectorQuery,
    kind: fission_test_driver::SelectorFailureKind,
    message: impl Into<String>,
    records: Vec<(SemanticRecord, Option<String>)>,
) -> fission_test_driver::TestResponse {
    fission_test_driver::TestResponse::SelectorError {
        failure: fission_test_driver::SelectorFailure {
            kind,
            selector: query,
            candidates: records
                .into_iter()
                .take(50)
                .map(
                    |(record, rejected_reason)| fission_test_driver::SelectorCandidate {
                        node: record.node,
                        rejected_reason,
                    },
                )
                .collect(),
            message: message.into(),
        },
    }
}

pub(super) fn resolve_selector_record(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
    query: &fission_test_driver::SelectorQuery,
) -> std::result::Result<SemanticRecord, fission_test_driver::TestResponse> {
    let (Some(ir), Some(snap)) = (&pipeline.prev_ir, &pipeline.last_snapshot) else {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        ));
    };

    let all = collect_semantic_records(ir, snap, scroll);
    let scoped = if let Some(scope_query) = &query.scope {
        let scope = resolve_selector_record(pipeline, scroll, scope_query)?;
        all.into_iter()
            .filter(|record| is_descendant_of(ir, record.id, scope.id))
            .collect()
    } else {
        all
    };

    let matched: Vec<SemanticRecord> = scoped
        .iter()
        .filter(|record| selector_matches(record, &query.selector))
        .cloned()
        .collect();
    if matched.is_empty() {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::NoMatch,
            "selector did not match any semantic node",
            scoped
                .into_iter()
                .map(|record| (record, Some("selector did not match".into())))
                .collect(),
        ));
    }

    let visible_matched: Vec<SemanticRecord> = matched
        .iter()
        .filter(|record| {
            query.include_hidden
                || record.node.visibility != fission_test_driver::VisibilityState::Hidden
        })
        .cloned()
        .collect();
    if visible_matched.is_empty() {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::FoundButNotVisible,
            "selector matched node(s), but none are visible",
            matched
                .into_iter()
                .map(|record| (record, Some("matched but hidden".into())))
                .collect(),
        ));
    }

    if let Some(index) = query.index {
        return visible_matched.get(index).cloned().ok_or_else(|| {
            selector_failure(
                query.clone(),
                fission_test_driver::SelectorFailureKind::NoMatch,
                format!("selector matched fewer than {} node(s)", index + 1),
                visible_matched
                    .into_iter()
                    .map(|record| (record, Some("candidate index out of range".into())))
                    .collect(),
            )
        });
    }

    if query.include_hidden && visible_matched.len() > 1 {
        let laid_out = visible_matched
            .iter()
            .filter(|record| {
                record.node.logical_bounds.width > 0.0 || record.node.logical_bounds.height > 0.0
            })
            .cloned()
            .collect::<Vec<_>>();
        if let [record] = laid_out.as_slice() {
            return Ok(record.clone());
        }
    }

    if visible_matched.len() > 1 {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Ambiguous,
            "selector matched multiple semantic nodes; provide index or scope",
            visible_matched
                .into_iter()
                .map(|record| (record, Some("ambiguous match".into())))
                .collect(),
        ));
    }

    Ok(visible_matched.into_iter().next().unwrap())
}

pub(super) fn selector_center(record: &SemanticRecord) -> Option<LayoutPoint> {
    let bounds = record.node.visible_bounds?;
    (bounds.width > 0.0 && bounds.height > 0.0).then(|| {
        LayoutPoint::new(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        )
    })
}

pub(super) fn dispatch_semantics_action(
    ir: &CoreIR,
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    trigger: ActionTrigger,
    input: ActionInput,
) -> bool {
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == trigger)
    else {
        return false;
    };
    let input = scoped_action_input_for_node(ir, target, input);
    runtime
        .dispatch_with_input(
            ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload: entry.payload_data.clone().unwrap_or_default(),
            },
            target,
            &input,
        )
        .is_ok()
}

pub(super) fn scoped_action_input_for_node(
    ir: &CoreIR,
    target: WidgetId,
    input: ActionInput,
) -> ActionInput {
    let mut current_id = Some(target);
    while let Some(id) = current_id {
        let Some(node) = ir.nodes.get(&id) else {
            break;
        };
        if let Op::Semantics(semantics) = &node.op {
            if let Some(scope_id) = semantics.action_scope_id {
                return ActionInput::scoped_raw(scope_id, target, input);
            }
        }
        current_id = node.parent;
    }
    input
}

pub(super) fn dispatch_text_change(
    ir: &CoreIR,
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    new_text: String,
) -> bool {
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Change)
    else {
        return false;
    };
    let Ok(payload) = serde_json::to_vec(&new_text) else {
        return false;
    };
    let input = scoped_action_input_for_node(ir, target, ActionInput::None);
    runtime
        .dispatch_with_input(
            ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload,
            },
            target,
            &input,
        )
        .is_ok()
}

pub(super) fn dispatch_cursor_change(
    ir: &CoreIR,
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    caret: usize,
    anchor: usize,
) -> bool {
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::CursorChange)
    else {
        return false;
    };
    let cursor_changed = fission_core::action::CursorChanged { caret, anchor };
    let Ok(payload) = serde_json::to_vec(&cursor_changed) else {
        return false;
    };
    let input = scoped_action_input_for_node(ir, target, ActionInput::None);
    runtime
        .dispatch_with_input(
            ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload,
            },
            target,
            &input,
        )
        .is_ok()
}

pub(super) fn set_focus_for_test(
    runtime: &mut Runtime,
    ir: &CoreIR,
    target: WidgetId,
    semantics: &Semantics,
) -> bool {
    if !semantics.focusable || semantics.disabled {
        return false;
    }
    let previous = runtime.runtime_state.interaction.focused;
    if previous == Some(target) {
        return true;
    }
    if let Some(previous_id) = previous {
        if let Some(previous_semantics) = ir.nodes.get(&previous_id).and_then(|node| {
            if let fission_ir::Op::Semantics(semantics) = &node.op {
                Some(semantics.clone())
            } else {
                None
            }
        }) {
            let _ = dispatch_semantics_action(
                ir,
                runtime,
                previous_id,
                &previous_semantics,
                ActionTrigger::Blur,
                ActionInput::None,
            );
        }
    }
    runtime.runtime_state.interaction.set_focused(Some(target));
    let _ = dispatch_semantics_action(
        ir,
        runtime,
        target,
        semantics,
        ActionTrigger::Focus,
        ActionInput::None,
    );
    true
}

pub(super) fn set_text_value_for_test(
    runtime: &mut Runtime,
    ir: &CoreIR,
    record: &SemanticRecord,
    value: &str,
) -> bool {
    if record.semantics.role != Role::TextInput
        || record.semantics.disabled
        || record.semantics.read_only
    {
        return false;
    }
    set_focus_for_test(runtime, ir, record.id, &record.semantics);
    runtime.runtime_state.text_edit.sync_from_runtime(
        record.id,
        record.semantics.value.as_deref().unwrap_or_default(),
        None,
        None,
    );
    {
        let state = runtime
            .runtime_state
            .text_edit
            .get_mut_or_default(record.id);
        let old_len = state.buffer.len_bytes();
        state.buffer.replace(0..old_len, value);
        state.caret = value.len();
        state.anchor = value.len();
        state.pending_model_sync = true;
        state.clear_preedit();
    }
    let mut changed =
        dispatch_text_change(ir, runtime, record.id, &record.semantics, value.to_string());
    changed |= dispatch_cursor_change(
        ir,
        runtime,
        record.id,
        &record.semantics,
        value.len(),
        value.len(),
    );
    changed
}

pub(super) fn resolve_selector_response(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
    query: &fission_test_driver::SelectorQuery,
) -> fission_test_driver::TestResponse {
    match resolve_selector_record(pipeline, scroll, query) {
        Ok(record) => fission_test_driver::TestResponse::SelectorResolved { node: record.node },
        Err(error) => error,
    }
}

pub(super) fn handle_scroll_into_view_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let mut hidden_query = query.clone();
    hidden_query.include_hidden = true;
    match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, &hidden_query) {
        Ok(record) => {
            runtime.queue_scroll_into_view(ScrollIntoViewRequest {
                container: None,
                target: record.id,
                axis: ScrollAxis::Both,
                alignment: ScrollAlignment::Nearest,
                padding: [8.0, 8.0, 8.0, 8.0],
                behavior: ScrollBehavior::Instant,
                if_needed: true,
            });
            fission_test_driver::TestResponse::SelectorResolved { node: record.node }
        }
        Err(error) => error,
    }
}

pub(super) fn handle_pointer_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    button: PointerButton,
    click: bool,
) -> fission_test_driver::TestResponse {
    let (Some(ir), Some(snap)) = (&pipeline.prev_ir, &pipeline.last_snapshot) else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        );
    };
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    let Some(point) = selector_center(&record) else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::FoundButNotVisible,
            "selector target is not visible",
            vec![(record, Some("not visible".into()))],
        );
    };
    let event = if click {
        PointerEvent::Down {
            point,
            button: button.clone(),
            modifiers: 0,
        }
    } else {
        PointerEvent::Move {
            point,
            modifiers: 0,
        }
    };
    let _ = runtime.handle_input(InputEvent::Pointer(event), ir, snap);
    if click {
        let _ = runtime.handle_input(
            InputEvent::Pointer(PointerEvent::Up {
                point,
                button,
                modifiers: 0,
            }),
            ir,
            snap,
        );
    }
    fission_test_driver::TestResponse::Ok {}
}

pub(super) fn handle_activate_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if !dispatch_semantics_action(
        pipeline
            .prev_ir
            .as_ref()
            .expect("selector resolved from rendered IR"),
        runtime,
        record.id,
        &record.semantics,
        ActionTrigger::Default,
        ActionInput::None,
    ) {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target has no default semantic action",
            vec![(record, Some("missing Default action".into()))],
        );
    }
    fission_test_driver::TestResponse::Ok {}
}

pub(super) fn handle_focus_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let Some(ir) = pipeline.prev_ir.as_ref() else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        );
    };
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if !record.semantics.focusable {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target is not focusable",
            vec![(record, Some("not focusable".into()))],
        );
    }
    set_focus_for_test(runtime, ir, record.id, &record.semantics);
    fission_test_driver::TestResponse::Ok {}
}

pub(super) fn handle_fill_text_selector(
    query: &fission_test_driver::SelectorQuery,
    text: &str,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let Some(ir) = pipeline.prev_ir.as_ref() else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        );
    };
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if record.semantics.read_only {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::ReadOnly,
            "selector target is read-only",
            vec![(record, Some("read-only".into()))],
        );
    }
    if record.semantics.role != Role::TextInput {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target is not a text input",
            vec![(record, Some("not a text input".into()))],
        );
    }
    set_text_value_for_test(runtime, ir, &record, text);
    fission_test_driver::TestResponse::Ok {}
}

pub(super) fn handle_toggle_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if !matches!(record.semantics.role, Role::Checkbox | Role::Switch) {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target is not a checkbox or switch",
            vec![(record, Some("not toggleable".into()))],
        );
    }
    if !dispatch_semantics_action(
        pipeline
            .prev_ir
            .as_ref()
            .expect("selector resolved from rendered IR"),
        runtime,
        record.id,
        &record.semantics,
        ActionTrigger::Default,
        ActionInput::None,
    ) {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target has no toggle action",
            vec![(record, Some("missing Default action".into()))],
        );
    }
    fission_test_driver::TestResponse::Ok {}
}

/// Build the response for a GetText query.
pub(super) fn build_get_text_response(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
) -> fission_test_driver::TestResponse {
    use fission_test_driver::{TestResponse, TextItem};
    let mut items = Vec::new();
    if let (Some(ir), Some(snap)) = (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref()) {
        let mut reachable = std::collections::HashSet::new();
        let mut stack = ir.root.into_iter().collect::<Vec<_>>();
        while let Some(node_id) = stack.pop() {
            if !reachable.insert(node_id) {
                continue;
            }
            if let Some(node) = ir.nodes.get(&node_id) {
                stack.extend(node.children.iter().copied());
            }
        }

        for id in reachable {
            let Some(node) = ir.nodes.get(&id) else {
                continue;
            };
            let text_content = match &node.op {
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) => {
                    Some(text.clone())
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    Some(runs.iter().map(|r| r.text.clone()).collect::<String>())
                }
                _ => None,
            };
            if let Some(text) = text_content {
                if text.is_empty() {
                    continue;
                }
                let check_id = node.parent.unwrap_or(id);
                let rect = visual_rect_for_node(ir, snap, scroll, check_id)
                    .or_else(|| visual_rect_for_node(ir, snap, scroll, id));
                let (x, y, w, h) = rect
                    .filter(|r| rect_visible_in_scroll_ancestors(ir, snap, scroll, id, *r))
                    .map(|r| (r.x(), r.y(), r.width(), r.height()))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                if w <= 0.0 || h <= 0.0 {
                    continue;
                }
                items.push(TextItem {
                    text,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
    }
    TestResponse::Text { items }
}

pub(super) fn find_visible_text_center(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
    text: &str,
) -> Option<(f32, f32)> {
    let fission_test_driver::TestResponse::Text { items } =
        build_get_text_response(pipeline, scroll)
    else {
        return None;
    };
    items
        .into_iter()
        .find(|item| item.text.contains(text) && item.width > 0.0 && item.height > 0.0)
        .map(|item| (item.x + item.width / 2.0, item.y + item.height / 2.0))
}

/// Build the response for a GetTree query.
pub(super) fn build_get_tree_response(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
) -> fission_test_driver::TestResponse {
    use fission_test_driver::TestResponse;
    if let (Some(ir), Some(snap)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let nodes = collect_semantic_records(ir, snap, scroll)
            .into_iter()
            .map(|record| record.node)
            .collect();
        TestResponse::Tree { nodes }
    } else {
        TestResponse::Tree { nodes: Vec::new() }
    }
}

/// Handle TapText — find text in the IR, tap at its center.
pub(super) fn handle_tap_text(
    text: &str,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    use fission_test_driver::TestResponse;
    if let (Some(ir), Some(snap)) = (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref()) {
        if let Some((cx, cy)) =
            find_visible_text_center(pipeline, &runtime.runtime_state.scroll, text)
        {
            let point = LayoutPoint::new(cx, cy);
            let _ = runtime.handle_input(
                InputEvent::Pointer(PointerEvent::Down {
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                ir,
                snap,
            );
            let _ = runtime.handle_input(
                InputEvent::Pointer(PointerEvent::Up {
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                ir,
                snap,
            );
            TestResponse::Ok {}
        } else {
            TestResponse::Error {
                message: format!("text '{}' not found", text),
            }
        }
    } else {
        TestResponse::Error {
            message: "no frame rendered yet".into(),
        }
    }
}

pub(super) fn wrap_portal_for_viewport(
    id: Option<WidgetId>,
    node: fission_core::Widget,
    env: &Env,
) -> fission_core::Widget {
    let builder = fission_core::ui::Container::new(node)
        .width(env.viewport_size.width)
        .height(env.viewport_size.height);
    if let Some(id) = id {
        builder.id(fission_ir::WidgetId::derived(id.as_u128(), &[0x0000_F001]))
    } else {
        builder.into()
    }
}

pub(super) fn texture_plan_fits_device_limits(
    plan: &crate::pipeline::CompositorTexturePlan,
    scale_factor: f64,
    max_texture_dimension_2d: u32,
) -> bool {
    if plan.scene.is_some() {
        let width = ((plan.bounds.size.width as f64 * scale_factor).ceil() as u32).max(1);
        let height = ((plan.bounds.size.height as f64 * scale_factor).ceil() as u32).max(1);
        if width > max_texture_dimension_2d || height > max_texture_dimension_2d {
            return false;
        }
    }
    plan.children
        .iter()
        .all(|child| texture_plan_fits_device_limits(child, scale_factor, max_texture_dimension_2d))
}

pub(super) fn texture_plans_fit_device_limits(
    plans: &[crate::pipeline::CompositorTexturePlan],
    scale_factor: f64,
    max_texture_dimension_2d: u32,
) -> bool {
    plans
        .iter()
        .all(|plan| texture_plan_fits_device_limits(plan, scale_factor, max_texture_dimension_2d))
}
