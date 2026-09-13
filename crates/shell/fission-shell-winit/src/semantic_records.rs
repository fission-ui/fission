//! Semantic node records and selector resolution for the live test control commands.

use super::*;

pub(crate) fn bounds_from_rect(rect: LayoutRect) -> fission_test_driver::Bounds {
    fission_test_driver::Bounds {
        x: rect.x(),
        y: rect.y(),
        width: rect.width(),
        height: rect.height(),
    }
}

pub(crate) fn visibility_state(
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

pub(crate) fn is_semantic_node(ir: &CoreIR, id: WidgetId) -> bool {
    ir.nodes
        .get(&id)
        .is_some_and(|node| matches!(node.op, fission_ir::Op::Semantics(_)))
}

pub(crate) fn nearest_semantic_parent(ir: &CoreIR, id: WidgetId) -> Option<WidgetId> {
    let mut current = ir.nodes.get(&id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        if is_semantic_node(ir, parent_id) {
            return Some(parent_id);
        }
        current = ir.nodes.get(&parent_id).and_then(|node| node.parent);
    }
    None
}

pub(crate) fn is_descendant_of(ir: &CoreIR, node_id: WidgetId, ancestor_id: WidgetId) -> bool {
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
pub(crate) struct SemanticRecord {
    pub(crate) id: WidgetId,
    pub(crate) semantics: Semantics,
    pub(crate) node: fission_test_driver::SemanticNode,
}

pub(crate) fn collect_semantic_records(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
) -> Vec<SemanticRecord> {
    let mut semantic_ids: Vec<WidgetId> = ir
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            (matches!(node.op, fission_ir::Op::Semantics(_))
                && !fission_core::hit_test::is_interaction_inert(ir, *id))
            .then_some(*id)
        })
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
                sequential_focusable: semantics.is_sequentially_focusable(),
                text_editable: semantics.supports_text_editing(),
                disabled: semantics.disabled,
                read_only: semantics.read_only,
                checked: semantics.checked,
                selected: semantics.selected,
                expanded: semantics.expanded,
                has_popup: semantics.has_popup.map(|value| format!("{value:?}")),
                orientation: semantics.orientation.map(|value| format!("{value:?}")),
                modal: semantics.modal,
                required: semantics.required,
                invalid: matches!(
                    semantics.validation_state,
                    fission_ir::TextFieldValidationState::Invalid
                ),
                controls: semantics.controls.iter().map(ToString::to_string).collect(),
                labelled_by: semantics
                    .labelled_by
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                described_by: semantics
                    .described_by
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                active_descendant: semantics.active_descendant.map(|id| id.to_string()),
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

pub(crate) fn selector_matches(
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

pub(crate) fn parse_widget_selector(widget_id: &str) -> WidgetId {
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

pub(crate) fn selector_failure(
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

pub(crate) fn resolve_selector_record(
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

pub(crate) fn selector_center(record: &SemanticRecord) -> Option<LayoutPoint> {
    let bounds = record.node.visible_bounds?;
    (bounds.width > 0.0 && bounds.height > 0.0).then(|| {
        LayoutPoint::new(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        )
    })
}
