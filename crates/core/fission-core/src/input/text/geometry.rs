use super::*;

impl TextInputController {
    pub(super) fn input_wrapper_geometry<'a>(
        ctx: &'a ControllerContext<'_>,
        focused_id: WidgetId,
    ) -> Option<&'a fission_layout::LayoutNodeGeometry> {
        let wrapper_id = ctx.ir.nodes.get(&focused_id)?.children.first().copied()?;
        ctx.layout.get_node_geometry(wrapper_id)
    }

    pub(super) fn text_local_point_from_screen(
        ctx: &ControllerContext<'_>,
        scroll_id: WidgetId,
        scroll_direction: FlexDirection,
        scroll_geom: &fission_layout::LayoutNodeGeometry,
        point: fission_layout::LayoutPoint,
    ) -> fission_layout::LayoutPoint {
        let mut ancestor_scroll_x = 0.0f32;
        let mut ancestor_scroll_y = 0.0f32;
        let mut walk = ctx.ir.nodes.get(&scroll_id).and_then(|node| node.parent);
        while let Some(parent_id) = walk {
            if let Some(parent_node) = ctx.ir.nodes.get(&parent_id) {
                if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &parent_node.op {
                    let offset = ctx.scroll.get_offset(parent_id);
                    match direction {
                        FlexDirection::Row => ancestor_scroll_x += offset,
                        FlexDirection::Column => ancestor_scroll_y += offset,
                    }
                }
                walk = parent_node.parent;
            } else {
                break;
            }
        }

        let own_scroll_offset = ctx.scroll.get_offset(scroll_id);
        let mut local_x = point.x - scroll_geom.rect.origin.x + ancestor_scroll_x;
        let mut local_y = point.y - scroll_geom.rect.origin.y + ancestor_scroll_y;
        match scroll_direction {
            FlexDirection::Row => local_x += own_scroll_offset,
            FlexDirection::Column => local_y += own_scroll_offset,
        }

        fission_layout::LayoutPoint::new(local_x, local_y)
    }

    pub(super) fn line_metric_for_index<'a>(
        line_metrics: &'a [fission_layout::LineMetric],
        caret_index: usize,
    ) -> Option<(usize, &'a fission_layout::LineMetric)> {
        line_metrics
            .iter()
            .enumerate()
            .find(|(_, line)| caret_index >= line.start_index && caret_index <= line.end_index)
            .or_else(|| line_metrics.iter().enumerate().last())
    }

    pub(super) fn local_text_point_for_index(
        measurer: &std::sync::Arc<dyn fission_layout::TextMeasurer>,
        layout: &fission_layout::LayoutSnapshot,
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
        text_op_node_id: WidgetId,
        wrapper_geom: &fission_layout::LayoutNodeGeometry,
        scroll_geom: &fission_layout::LayoutNodeGeometry,
        scroll_direction: FlexDirection,
        scroll_offset: f32,
        metric_text: &str,
        metric_index: usize,
    ) -> Option<fission_layout::LayoutPoint> {
        let font_size = Self::extract_font_size(ir, focused_id).unwrap_or(16.0);
        let paragraph = Self::extract_paragraph_style(ir, focused_id).unwrap_or_default();
        let render_width = if scroll_direction == FlexDirection::Column {
            Some(scroll_geom.rect.size.width)
        } else {
            None
        };
        let resolved = Self::resolved_paragraph_for_text(layout, text_op_node_id, metric_text);
        let (caret_x, caret_y, line_height) = if let Some(resolved) = resolved {
            let caret = resolved.caret(metric_index, false)?;
            (caret.position.x, caret.position.y, caret.height.max(1.0))
        } else {
            let (mut caret_x, caret_y) =
                measurer.get_caret_position(metric_text, font_size, render_width, metric_index);
            let line_metrics = measurer.get_line_metrics(metric_text, font_size, render_width);
            let (line_index, line_metric) =
                Self::line_metric_for_index(&line_metrics, metric_index)?;
            let is_last_line = line_index + 1 == line_metrics.len();
            if let Some(width) = render_width {
                caret_x += Self::paragraph_line_x_offset(
                    paragraph,
                    width,
                    line_metric.width,
                    is_last_line,
                );
            }
            (caret_x, caret_y, line_metric.height.max(1.0))
        };

        let visible_x = if scroll_direction == FlexDirection::Row {
            caret_x - scroll_offset
        } else {
            caret_x
        };
        let visible_y = if scroll_direction == FlexDirection::Column {
            caret_y - scroll_offset
        } else {
            caret_y
        };

        let local_x = (scroll_geom.rect.origin.x - wrapper_geom.rect.origin.x) + visible_x;
        let local_y =
            (scroll_geom.rect.origin.y - wrapper_geom.rect.origin.y) + visible_y + line_height;

        Some(fission_layout::LayoutPoint::new(local_x, local_y))
    }

    pub(super) fn clear_text_input_affordances(ctx: &mut ControllerContext, focused_id: WidgetId) {
        if let Some(state) = ctx.text_edit.states.get_mut(&focused_id) {
            state.affordances = Default::default();
        }
    }

    pub(super) fn sync_text_input_affordances(
        ctx: &mut ControllerContext,
        focused_id: WidgetId,
        semantics: &Semantics,
        value: &str,
        toolbar_visible: bool,
        toolbar_anchor_override: Option<fission_layout::LayoutPoint>,
    ) {
        let Some(measurer) = ctx.measurer else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };
        let Some(wrapper_geom) = Self::input_wrapper_geometry(ctx, focused_id).cloned() else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };
        let Some((scroll_id, text_node_id, scroll_direction)) =
            Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
        else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };
        let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id).cloned() else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };

        let display_value = Self::display_value_for_metrics(
            ctx,
            focused_id,
            semantics.value.as_deref().unwrap_or(value),
        );
        let metric_text = if semantics.masked {
            Self::mask_text_for_metrics(&display_value)
        } else {
            display_value.clone()
        };
        let (caret, anchor, active_handle) = {
            let state = ctx.text_edit.get_mut_or_default(focused_id);
            (state.caret, state.anchor, state.affordances.active_handle)
        };

        let map_metric_index = |index: usize| {
            if semantics.masked {
                Self::masked_byte_offset_from_source(&display_value, &metric_text, index)
            } else {
                index.min(metric_text.len())
            }
        };

        let scroll_offset = ctx.scroll.get_offset(scroll_id);
        let caret_point = Self::local_text_point_for_index(
            measurer,
            ctx.layout,
            ctx.ir,
            focused_id,
            text_node_id,
            &wrapper_geom,
            &scroll_geom,
            scroll_direction,
            scroll_offset,
            &metric_text,
            map_metric_index(caret),
        );
        let anchor_point = Self::local_text_point_for_index(
            measurer,
            ctx.layout,
            ctx.ir,
            focused_id,
            text_node_id,
            &wrapper_geom,
            &scroll_geom,
            scroll_direction,
            scroll_offset,
            &metric_text,
            map_metric_index(anchor),
        );

        let selection_range = if caret == anchor {
            None
        } else {
            Some((caret.min(anchor), caret.max(anchor)))
        };

        let toolbar_anchor = if let Some(override_point) = toolbar_anchor_override {
            Some(override_point)
        } else {
            match (caret_point, anchor_point, selection_range) {
                (Some(caret_point), Some(anchor_point), Some(_)) => {
                    Some(fission_layout::LayoutPoint::new(
                        (caret_point.x + anchor_point.x) * 0.5,
                        caret_point.y.min(anchor_point.y),
                    ))
                }
                (Some(point), _, None) => Some(point),
                _ => None,
            }
        };

        let state = ctx.text_edit.get_mut_or_default(focused_id);
        state.affordances.toolbar_visible = toolbar_visible;
        state.affordances.toolbar_anchor = toolbar_anchor;
        state.affordances.magnifier_visible = active_handle.is_some();
        state.affordances.magnifier_anchor = match active_handle {
            Some(TextSelectionHandleKind::Caret) => caret_point,
            Some(TextSelectionHandleKind::Start) => anchor_point,
            Some(TextSelectionHandleKind::End) => caret_point,
            None => None,
        };
        if selection_range.is_some() {
            let (start_point, end_point) = if caret <= anchor {
                (caret_point, anchor_point)
            } else {
                (anchor_point, caret_point)
            };
            state.affordances.caret_handle = None;
            state.affordances.selection_start_handle = start_point;
            state.affordances.selection_end_handle = end_point;
        } else {
            state.affordances.caret_handle = caret_point;
            state.affordances.selection_start_handle = None;
            state.affordances.selection_end_handle = None;
        }
    }

    pub(super) fn trim_line_end(value: &str, end: usize) -> usize {
        let end = end.min(value.len());
        if end > 0 && value.as_bytes()[end - 1] == b'\n' {
            end - 1
        } else {
            end
        }
    }

    pub(super) fn current_line_bounds(
        ctx: &ControllerContext,
        focused_id: WidgetId,
        semantics: &Semantics,
        value: &str,
        caret: usize,
    ) -> (usize, usize) {
        let caret = caret.min(value.len());
        if semantics.multiline {
            if let Some(measurer) = ctx.measurer {
                if let Some((scroll_id, text_op_node_id, _scroll_direction)) =
                    Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
                {
                    if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                        if let Some(paragraph) =
                            Self::resolved_paragraph_for_text(ctx.layout, text_op_node_id, value)
                        {
                            if let Some(line) = paragraph
                                .lines
                                .iter()
                                .find(|line| caret >= line.start_index && caret <= line.end_index)
                                .or_else(|| paragraph.lines.last())
                            {
                                let start = line.start_index.min(value.len());
                                let end = Self::trim_line_end(value, line.end_index);
                                return (start.min(end), end);
                            }
                        }
                        let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);
                        let line_metrics = measurer.get_line_metrics(
                            value,
                            font_size,
                            Some(scroll_geom.rect.size.width),
                        );
                        if let Some(line) = line_metrics
                            .iter()
                            .find(|line| caret >= line.start_index && caret <= line.end_index)
                            .or_else(|| line_metrics.last())
                        {
                            let start = line.start_index.min(value.len());
                            let end = Self::trim_line_end(value, line.end_index);
                            return (start.min(end), end);
                        }
                    }
                }
            }

            let start = value[..caret].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let end = value[caret..]
                .find('\n')
                .map(|offset| caret + offset)
                .unwrap_or(value.len());
            (start.min(end), end)
        } else {
            (0, value.len())
        }
    }

    pub(super) fn resolve_editing_value(
        ctx: &mut ControllerContext,
        focused_id: WidgetId,
        semantic_value: &str,
    ) -> (String, usize, usize) {
        Self::sync_runtime_state(ctx, focused_id, semantic_value);
        let st = ctx.text_edit.get_mut_or_default(focused_id);
        let value = st.committed_text();
        (value, st.caret, st.anchor)
    }

    pub(super) fn display_value_for_metrics(
        ctx: &mut ControllerContext,
        focused_id: WidgetId,
        semantic_value: &str,
    ) -> String {
        Self::sync_runtime_state(ctx, focused_id, semantic_value);
        let state = ctx.text_edit.get_mut_or_default(focused_id);
        state.display_text().0
    }

    pub(super) fn mask_text_for_metrics(text: &str) -> String {
        let mut masked = String::new();
        for _ in text.graphemes(true) {
            masked.push('•');
        }
        masked
    }

    pub(super) fn masked_byte_offset_from_source(
        source: &str,
        masked: &str,
        source_byte_offset: usize,
    ) -> usize {
        let clamped = source_byte_offset.min(source.len());
        let grapheme_count = source[..clamped].graphemes(true).count();
        masked
            .grapheme_indices(true)
            .nth(grapheme_count)
            .map(|(idx, _)| idx)
            .unwrap_or(masked.len())
    }

    pub(super) fn source_byte_offset_from_masked(
        source: &str,
        masked: &str,
        masked_byte_offset: usize,
    ) -> usize {
        let clamped = masked_byte_offset.min(masked.len());
        let grapheme_count = masked[..clamped].graphemes(true).count();
        source
            .grapheme_indices(true)
            .nth(grapheme_count)
            .map(|(idx, _)| idx)
            .unwrap_or(source.len())
    }

    pub(super) fn clamp_caret_to_value(value: &str, caret: usize) -> usize {
        if caret > value.len() {
            value.len()
        } else {
            caret
        }
    }

    pub(super) fn prev_grapheme_boundary(value: &str, idx: usize) -> usize {
        let mut last = 0;
        for (pos, _) in value.grapheme_indices(true) {
            if pos >= idx {
                break;
            }
            last = pos;
        }
        last
    }

    pub(super) fn next_grapheme_boundary(value: &str, idx: usize) -> usize {
        for (pos, _) in value.grapheme_indices(true) {
            if pos > idx {
                return pos;
            }
        }
        value.len()
    }

    pub(super) fn prev_word_boundary(value: &str, idx: usize) -> usize {
        let at = idx.min(value.len());
        let segments: Vec<(usize, &str)> = value.split_word_bound_indices().collect();
        for (start, segment) in segments.into_iter().rev() {
            let end = start + segment.len();
            if end > at {
                continue;
            }
            if segment.chars().any(|ch| ch.is_alphanumeric() || ch == '_') {
                return start;
            }
        }
        0
    }

    pub(super) fn next_word_boundary(value: &str, idx: usize) -> usize {
        let at = idx.min(value.len());
        for (start, segment) in value.split_word_bound_indices() {
            let end = start + segment.len();
            if end <= at {
                continue;
            }
            if segment.chars().any(|ch| ch.is_alphanumeric() || ch == '_') {
                return end;
            }
        }
        value.len()
    }

    pub(super) fn find_scroll_container_and_text_op(
        ir: &fission_ir::CoreIR,
        root: WidgetId,
        multiline_semantics: bool,
    ) -> Option<(WidgetId, WidgetId, op::FlexDirection)> {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if let Some(n) = ir.nodes.get(&id) {
                if let Op::Layout(op::LayoutOp::Scroll { direction, .. }) = &n.op {
                    let matches_multiline_config = (multiline_semantics
                        && *direction == op::FlexDirection::Column)
                        || (!multiline_semantics && *direction == op::FlexDirection::Row);
                    if matches_multiline_config {
                        let mut q = vec![id]; // Start BFS from scroll node to find text
                        while let Some(cid) = q.pop() {
                            if let Some(cn) = ir.nodes.get(&cid) {
                                if matches!(
                                    cn.op,
                                    Op::Paint(fission_ir::PaintOp::DrawText { .. })
                                        | Op::Paint(fission_ir::PaintOp::DrawRichText { .. })
                                ) {
                                    return Some((id, cid, *direction));
                                }
                                for &gc in &cn.children {
                                    q.push(gc);
                                }
                            }
                        }
                        return None; // Should find text inside. For now, assume it's directly related.
                    }
                }
                for &c in &n.children {
                    stack.push(c);
                }
            }
        }
        None
    }

    /// Extract rich text runs from the TextInput's DrawRichText child.
    pub(super) fn extract_rich_runs(
        ir: &fission_ir::CoreIR,
        semantics_id: WidgetId,
    ) -> Option<Vec<fission_ir::op::TextRun>> {
        fn walk(
            ir: &fission_ir::CoreIR,
            node_id: WidgetId,
            depth: usize,
        ) -> Option<Vec<fission_ir::op::TextRun>> {
            if depth > 20 {
                return None;
            }
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) if !runs.is_empty() => {
                    Some(runs.clone())
                }
                _ => {
                    for child_id in &node.children {
                        if let Some(r) = walk(ir, *child_id, depth + 1) {
                            return Some(r);
                        }
                    }
                    None
                }
            }
        }
        walk(ir, semantics_id, 0)
    }

    /// Extract the font size from the TextInput's DrawRichText or DrawText child.
    pub(super) fn extract_font_size(
        ir: &fission_ir::CoreIR,
        semantics_id: WidgetId,
    ) -> Option<f32> {
        // Walk children of the semantics node to find a text paint op
        fn walk(ir: &fission_ir::CoreIR, node_id: WidgetId, depth: usize) -> Option<f32> {
            if depth > 10 {
                return None;
            }
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawText { size, .. }) => Some(*size),
                Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    runs.first().map(|r| r.style.font_size)
                }
                _ => {
                    for child_id in &node.children {
                        if let Some(sz) = walk(ir, *child_id, depth + 1) {
                            return Some(sz);
                        }
                    }
                    None
                }
            }
        }
        walk(ir, semantics_id, 0)
    }

    /// Shared hit-test logic for both PointerDown and PointerMove.
    ///
    /// Uses the rich-text layout path when styled runs are available, passing the
    /// same `available_width` that the renderer will use so both sides build (or
    /// look up) the same Parley `Layout`.  This ensures the Y-to-line and X-to-
    /// glyph mapping in hit-testing exactly matches the rendered text.
    pub(super) fn hit_test_text(
        measurer: &std::sync::Arc<dyn fission_layout::TextMeasurer>,
        layout: &fission_layout::LayoutSnapshot,
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
        text_op_node_id: WidgetId,
        prefer_plain_text: bool,
        text: &str,
        scroll_geom: &fission_layout::LayoutNodeGeometry,
        local_x: f32,
        local_y: f32,
    ) -> usize {
        if let Some(paragraph) = Self::resolved_paragraph_for_text(layout, text_op_node_id, text) {
            return paragraph.hit_test(fission_layout::LayoutPoint::new(local_x, local_y));
        }

        let viewport_width = if scroll_geom.rect.size.width > 0.0 {
            Some(scroll_geom.rect.size.width)
        } else {
            None
        };
        let render_width = viewport_width;
        let font_size = Self::extract_font_size(ir, focused_id).unwrap_or(13.0);
        let paragraph = Self::extract_paragraph_style(ir, focused_id).unwrap_or_default();

        if paragraph.text_align != TextAlign::Start {
            let line_metrics = measurer.get_line_metrics(text, font_size, render_width);
            if let (Some(width), Some(line)) = (
                viewport_width,
                Self::line_metric_for_local_y(&line_metrics, local_y),
            ) {
                let aligned_x =
                    local_x - Self::paragraph_line_x_offset(paragraph, width, line.width, false);
                return measurer.hit_test(text, font_size, render_width, aligned_x, local_y);
            }
        }

        if !prefer_plain_text {
            if let Some(runs) = Self::extract_rich_runs(ir, focused_id) {
                return measurer.hit_test_rich(&runs, render_width, local_x, local_y);
            }
        }
        measurer.hit_test(text, font_size, render_width, local_x, local_y)
    }

    /// Returns a retained paragraph only when it describes the complete value
    /// currently being edited. A platform replacement can update the editing
    /// value before the next declarative layout pass; in that case the retained
    /// paragraph is stale and callers must measure the new value directly.
    pub(super) fn resolved_paragraph_for_text<'a>(
        layout: &'a fission_layout::LayoutSnapshot,
        text_op_node_id: WidgetId,
        metric_text: &str,
    ) -> Option<&'a fission_layout::ResolvedParagraphLayout> {
        let paragraph = layout.get_resolved_paragraph(text_op_node_id)?;
        let resolved_end = paragraph.caret_stops.iter().map(|stop| stop.index).max()?;
        (resolved_end == metric_text.len()).then_some(paragraph)
    }

    pub(super) fn caret_from_point_in_text_fallback(
        _value: &str,
        _font_size: f32,
        _viewport_x: f32,
        _viewport_w: f32,
        _content_w: f32,
        _scroll_offset: f32,
        _point_x: f32,
    ) -> usize {
        // Simplified fallback: always return 0 if no proper measurer is available.
        // In a real scenario, this would ideally not be hit in interactive UIs.
        0
    }

    pub(crate) fn ime_cursor_area(
        ctx: &mut ControllerContext,
        text_root: WidgetId,
    ) -> Option<fission_layout::LayoutRect> {
        let node = ctx.ir.nodes.get(&text_root)?;
        let semantics = match &node.op {
            Op::Semantics(semantics) => semantics,
            _ => return None,
        };

        let (scroll_id, text_op_node_id, scroll_direction) =
            Self::find_scroll_container_and_text_op(ctx.ir, text_root, semantics.multiline)?;
        let scroll_geom = ctx.layout.get_node_geometry(scroll_id)?;
        let viewport_size = scroll_geom.rect.size;
        let font_size = Self::extract_font_size(ctx.ir, text_root).unwrap_or(16.0);
        let display_value = Self::display_value_for_metrics(
            ctx,
            text_root,
            semantics.value.as_deref().unwrap_or(""),
        );
        let metric_text = if semantics.masked {
            Self::mask_text_for_metrics(&display_value)
        } else {
            display_value.clone()
        };

        let caret_idx = ctx
            .text_edit
            .get(text_root)
            .map(|state| {
                state
                    .display_preedit_cursor_range()
                    .map(|(_, end)| end)
                    .unwrap_or(state.caret)
            })
            .unwrap_or(0);
        let metric_caret_idx = if semantics.masked {
            Self::masked_byte_offset_from_source(&display_value, &metric_text, caret_idx)
        } else {
            caret_idx
        };

        let paragraph = Self::extract_paragraph_style(ctx.ir, text_root).unwrap_or_default();
        let render_width = if scroll_direction == op::FlexDirection::Column {
            Some(viewport_size.width)
        } else {
            None
        };
        let (caret_x, caret_y, line_height, line_x) = if let Some(resolved) =
            Self::resolved_paragraph_for_text(ctx.layout, text_op_node_id, &metric_text)
        {
            let caret = resolved.caret(metric_caret_idx, false)?;
            (
                caret.position.x,
                caret.position.y,
                caret.height.max(1.0),
                0.0,
            )
        } else {
            let measurer = ctx.measurer?;
            let (caret_x, caret_y) = measurer.get_caret_position(
                &metric_text,
                font_size,
                render_width,
                metric_caret_idx,
            );
            let line_metrics = measurer.get_line_metrics(&metric_text, font_size, render_width);
            let line = Self::line_metric_for_index(&line_metrics, metric_caret_idx)
                .map(|(_, line)| line)
                .or_else(|| Self::line_metric_for_local_y(&line_metrics, caret_y));
            let line_width = line
                .map(|line| line.width)
                .unwrap_or_else(|| measurer.measure(&metric_text, font_size, render_width).0);
            let line_height = line
                .map(|line| line.height.max(1.0))
                .unwrap_or_else(|| measurer.measure("Tg", font_size, render_width).1.max(1.0));
            let is_last_line = line_metrics
                .last()
                .is_some_and(|last| last.end_index <= metric_caret_idx);
            let line_x = Self::paragraph_line_x_offset(
                paragraph,
                viewport_size.width,
                line_width,
                is_last_line,
            );
            (caret_x, caret_y, line_height, line_x)
        };

        let mut origin_x = scroll_geom.rect.origin.x;
        let mut origin_y = scroll_geom.rect.origin.y;
        let mut walk = ctx.ir.nodes.get(&scroll_id).and_then(|node| node.parent);
        while let Some(parent_id) = walk {
            let Some(parent) = ctx.ir.nodes.get(&parent_id) else {
                break;
            };
            if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &parent.op {
                let offset = ctx.scroll.get_offset(parent_id);
                match direction {
                    FlexDirection::Row => origin_x -= offset,
                    FlexDirection::Column => origin_y -= offset,
                }
            }
            walk = parent.parent;
        }

        let own_offset = ctx.scroll.get_offset(scroll_id);
        let mut x = origin_x + line_x + caret_x;
        let mut y = origin_y + caret_y;
        match scroll_direction {
            op::FlexDirection::Row => x -= own_offset,
            op::FlexDirection::Column => y -= own_offset,
        }

        if !(x.is_finite() && y.is_finite() && line_height.is_finite()) {
            return None;
        }

        let right_limit = origin_x + viewport_size.width - 2.0;
        let bottom_limit = origin_y + viewport_size.height - line_height;
        if right_limit >= origin_x {
            x = x.clamp(origin_x, right_limit);
        }
        if bottom_limit >= origin_y {
            y = y.clamp(origin_y, bottom_limit);
        }

        Some(fission_layout::LayoutRect::new(x, y, 2.0, line_height))
    }

    pub(super) fn auto_scroll_textinput(ctx: &mut ControllerContext, text_root: WidgetId) {
        if Self::runtime_config(ctx, text_root)
            .is_some_and(|config| config.scroll_policy == TextScrollPolicy::Never)
        {
            return;
        }
        let font_size = Self::extract_font_size(ctx.ir, text_root).unwrap_or(16.0);
        if let Some(measurer) = ctx.measurer {
            // Need to get multiline status from semantics here
            let is_multiline = if let Some(node) = ctx.ir.nodes.get(&text_root) {
                if let Op::Semantics(sem) = &node.op {
                    sem.multiline
                } else {
                    false
                }
            } else {
                false
            };

            if let Some((scroll_id, text_op_node_id, scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, text_root, is_multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_size = scroll_geom.rect.size;

                    let (current_text_value, metric_text, masked, scroll_padding) =
                        if let Some(node) = ctx.ir.nodes.get(&text_root) {
                            if let Op::Semantics(sem) = &node.op {
                                let display_value = Self::display_value_for_metrics(
                                    ctx,
                                    text_root,
                                    sem.value.as_deref().unwrap_or(""),
                                );
                                let metric_text = if sem.masked {
                                    Self::mask_text_for_metrics(&display_value)
                                } else {
                                    display_value.clone()
                                };
                                (
                                    display_value,
                                    metric_text,
                                    sem.masked,
                                    sem.scroll_padding.unwrap_or([2.0, 3.0, 2.0, 3.0]),
                                )
                            } else {
                                (String::new(), String::new(), false, [2.0, 3.0, 2.0, 3.0])
                            }
                        } else {
                            (String::new(), String::new(), false, [2.0, 3.0, 2.0, 3.0])
                        };

                    let current_caret_idx = if let Some(st) = ctx.text_edit.get(text_root) {
                        st.display_preedit_cursor_range()
                            .map(|(_, end)| end)
                            .unwrap_or(st.caret)
                    } else {
                        0
                    };
                    let metric_caret_idx = if masked {
                        Self::masked_byte_offset_from_source(
                            &current_text_value,
                            &metric_text,
                            current_caret_idx,
                        )
                    } else {
                        current_caret_idx
                    };
                    let paragraph =
                        Self::extract_paragraph_style(ctx.ir, text_root).unwrap_or_default();
                    let measurer_width = if scroll_direction == op::FlexDirection::Column {
                        Some(viewport_size.width)
                    } else {
                        None
                    };

                    let resolved = Self::resolved_paragraph_for_text(
                        ctx.layout,
                        text_op_node_id,
                        &metric_text,
                    );
                    let resolved_caret =
                        resolved.and_then(|paragraph| paragraph.caret(metric_caret_idx, false));
                    let (caret_x, caret_y) = resolved_caret
                        .map(|caret| (caret.position.x, caret.position.y))
                        .unwrap_or_else(|| {
                            measurer.get_caret_position(
                                &metric_text,
                                font_size,
                                measurer_width,
                                metric_caret_idx,
                            )
                        });

                    let mut offset = ctx.scroll.get_offset(scroll_id);

                    if scroll_direction == op::FlexDirection::Row {
                        // Handle horizontal scrolling for single-line text
                        let line_width = resolved
                            .and_then(|paragraph| paragraph.lines.first())
                            .map(|line| line.width)
                            .or_else(|| {
                                measurer
                                    .get_line_metrics(&metric_text, font_size, None)
                                    .first()
                                    .map(|line| line.width)
                            })
                            .unwrap_or_else(|| measurer.measure(&metric_text, font_size, None).0);
                        let caret_left = caret_x
                            + if resolved.is_some() {
                                0.0
                            } else {
                                Self::paragraph_line_x_offset(
                                    paragraph,
                                    viewport_size.width,
                                    line_width,
                                    false,
                                )
                            };
                        let caret_width = 2.0f32;
                        let caret_right = caret_left + caret_width;

                        let margin_left = scroll_padding[0].max(0.0);
                        let margin_right = scroll_padding[1].max(0.0);

                        let visible_left = caret_left - offset;
                        let visible_right = caret_right - offset;

                        if visible_right > (viewport_size.width - margin_right) {
                            offset =
                                (caret_right - (viewport_size.width - margin_right)).max(0.0f32);
                        } else if visible_left < margin_left {
                            offset = (caret_left - margin_left).max(0.0f32);
                        }
                        let content_w = scroll_geom.content_size.width.max(viewport_size.width);
                        let max_offset = (content_w - viewport_size.width).max(0.0f32);
                        offset = offset.clamp(0.0f32, max_offset);
                        ctx.scroll.set_offset(scroll_id, offset);
                    } else {
                        // op::FlexDirection::Column
                        // Handle vertical scrolling for multi-line text
                        let caret_top = caret_y;
                        let caret_height = resolved_caret
                            .map(|caret| caret.height.max(1.0))
                            .unwrap_or_else(|| {
                                measurer
                                    .measure("Tg", font_size, Some(viewport_size.width))
                                    .1
                            });
                        let caret_bottom = caret_top + caret_height;

                        let margin_top = scroll_padding[2].max(0.0);
                        let margin_bottom = scroll_padding[3].max(0.0);

                        let visible_top = caret_top - offset;
                        let visible_bottom = caret_bottom - offset;

                        if visible_bottom > (viewport_size.height - margin_bottom) {
                            offset =
                                (caret_bottom - (viewport_size.height - margin_bottom)).max(0.0f32);
                        } else if visible_top < margin_top {
                            offset = (caret_top - margin_top).max(0.0f32);
                        }
                        // A complete platform replacement can arrive before the
                        // declarative rebuild has enlarged layout's content size.
                        // Measure the authoritative value now so caret visibility
                        // does not depend on waiting for a second input event.
                        let measured_content_h = resolved
                            .map(|paragraph| paragraph.size.height)
                            .unwrap_or_else(|| {
                                measurer
                                    .measure(&metric_text, font_size, Some(viewport_size.width))
                                    .1
                            });
                        let content_h = scroll_geom
                            .content_size
                            .height
                            .max(measured_content_h)
                            .max(viewport_size.height);
                        let max_offset = (content_h - viewport_size.height).max(0.0f32);
                        offset = offset.clamp(0.0f32, max_offset);
                        ctx.scroll.set_offset(scroll_id, offset);
                    }
                }
            }
        }
    }

    pub(super) fn handle_vertical_navigation(
        &mut self,
        ctx: &mut ControllerContext,
        focused_id: WidgetId,
        semantics: &Semantics,
        value: &str,
        caret: usize,
        modifiers: u8,
        is_up: bool,
    ) {
        if let Some(measurer) = ctx.measurer {
            let (text_op_node_id, viewport_w) =
                Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
                    .and_then(|(scroll_id, text_op_node_id, _)| {
                        ctx.layout
                            .get_node_geometry(scroll_id)
                            .map(|geometry| (text_op_node_id, geometry.rect.size.width))
                    })
                    .unwrap_or((focused_id, ctx.layout.viewport_size.width));
            let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);

            let resolved = Self::resolved_paragraph_for_text(ctx.layout, text_op_node_id, value);
            let current_caret_x = resolved
                .and_then(|paragraph| paragraph.caret(caret, false))
                .map(|caret| caret.position.x)
                .unwrap_or_else(|| {
                    measurer
                        .get_caret_position(value, font_size, Some(viewport_w), caret)
                        .0
                });

            let fallback_lines;
            let line_metrics = if let Some(paragraph) = resolved {
                paragraph.lines.as_slice()
            } else {
                fallback_lines = measurer.get_line_metrics(value, font_size, Some(viewport_w));
                fallback_lines.as_slice()
            };

            let mut current_line_idx = 0;
            for (idx, line) in line_metrics.iter().enumerate() {
                if caret >= line.start_index && caret <= line.end_index {
                    current_line_idx = idx;
                    // Don't break: if the caret sits at the boundary
                    // between two lines (end of line N == start of
                    // line N+1), prefer the later line so empty lines
                    // are reachable.
                }
            }

            let target_line_idx = if is_up {
                current_line_idx.saturating_sub(1)
            } else {
                (current_line_idx + 1).min(line_metrics.len().saturating_sub(1))
            };

            if let Some(target_line) = line_metrics.get(target_line_idx) {
                let target_y = target_line.baseline;

                let mut new_caret_pos = resolved
                    .map(|paragraph| {
                        paragraph
                            .hit_test(fission_layout::LayoutPoint::new(current_caret_x, target_y))
                    })
                    .unwrap_or_else(|| {
                        measurer.hit_test(
                            value,
                            font_size,
                            Some(viewport_w),
                            current_caret_x,
                            target_y,
                        )
                    });

                // Ensure we stay within the target line's bounds.
                // For empty lines (start_index == end_index), this
                // correctly places the cursor at start_index.
                new_caret_pos = new_caret_pos.clamp(
                    target_line.start_index,
                    target_line.end_index.max(target_line.start_index),
                );

                let st = ctx.text_edit.get_mut_or_default(focused_id);
                st.caret = new_caret_pos;
                if !Self::has_shift(modifiers) {
                    st.anchor = new_caret_pos;
                } // If no shift, collapse selection
                let final_anchor = st.anchor;
                Self::auto_scroll_textinput(ctx, focused_id);
                Self::dispatch_cursor_change(
                    ctx,
                    semantics,
                    focused_id,
                    new_caret_pos,
                    final_anchor,
                );
            }
        }
    }

    pub(super) fn handle_page_navigation(
        &mut self,
        ctx: &mut ControllerContext,
        focused_id: WidgetId,
        semantics: &Semantics,
        value: &str,
        caret: usize,
        modifiers: u8,
        is_page_up: bool,
    ) {
        if let Some(measurer) = ctx.measurer {
            if let Some((scroll_id, text_op_node_id, _scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_w = scroll_geom.rect.size.width;
                    let viewport_h = scroll_geom.rect.size.height.max(1.0);
                    let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);
                    let resolved =
                        Self::resolved_paragraph_for_text(ctx.layout, text_op_node_id, value);
                    let current_caret_x = resolved
                        .and_then(|paragraph| paragraph.caret(caret, false))
                        .map(|caret| caret.position.x)
                        .unwrap_or_else(|| {
                            measurer
                                .get_caret_position(value, font_size, Some(viewport_w), caret)
                                .0
                        });
                    let fallback_lines;
                    let line_metrics = if let Some(paragraph) = resolved {
                        paragraph.lines.as_slice()
                    } else {
                        fallback_lines =
                            measurer.get_line_metrics(value, font_size, Some(viewport_w));
                        fallback_lines.as_slice()
                    };

                    if line_metrics.is_empty() {
                        return;
                    }

                    let mut current_line_idx = 0usize;
                    for (idx, line) in line_metrics.iter().enumerate() {
                        if caret >= line.start_index && caret <= line.end_index {
                            current_line_idx = idx;
                        }
                    }

                    let line_height = line_metrics
                        .get(current_line_idx)
                        .map(|line| line.height.max(1.0))
                        .unwrap_or(20.0);
                    let lines_per_page = (viewport_h / line_height).floor().max(1.0) as isize;
                    let delta = if is_page_up {
                        -lines_per_page
                    } else {
                        lines_per_page
                    };
                    let target_line_idx = current_line_idx
                        .saturating_add_signed(delta)
                        .min(line_metrics.len().saturating_sub(1));

                    if let Some(target_line) = line_metrics.get(target_line_idx) {
                        let target_y = target_line.baseline;
                        let mut new_caret_pos = resolved
                            .map(|paragraph| {
                                paragraph.hit_test(fission_layout::LayoutPoint::new(
                                    current_caret_x,
                                    target_y,
                                ))
                            })
                            .unwrap_or_else(|| {
                                measurer.hit_test(
                                    value,
                                    font_size,
                                    Some(viewport_w),
                                    current_caret_x,
                                    target_y,
                                )
                            });
                        let target_end = Self::trim_line_end(
                            value,
                            target_line.end_index.max(target_line.start_index),
                        );
                        new_caret_pos = new_caret_pos.clamp(
                            target_line.start_index,
                            target_end.max(target_line.start_index),
                        );

                        let st = ctx.text_edit.get_mut_or_default(focused_id);
                        st.caret = new_caret_pos;
                        if !Self::has_shift(modifiers) {
                            st.anchor = new_caret_pos;
                        }
                        let final_anchor = st.anchor;
                        Self::auto_scroll_textinput(ctx, focused_id);
                        Self::dispatch_cursor_change(
                            ctx,
                            semantics,
                            focused_id,
                            new_caret_pos,
                            final_anchor,
                        );
                    }
                }
            }
        }
    }

    pub(super) fn extract_paragraph_style(
        ir: &fission_ir::CoreIR,
        semantics_id: WidgetId,
    ) -> Option<TextParagraphStyle> {
        fn walk(
            ir: &fission_ir::CoreIR,
            node_id: WidgetId,
            depth: usize,
        ) -> Option<TextParagraphStyle> {
            if depth > 10 {
                return None;
            }
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawText {
                    paragraph_style,
                    caret_width,
                    ..
                }) => paragraph_style.or_else(|| decode_text_paragraph_style(*caret_width)),
                Op::Paint(fission_ir::PaintOp::DrawRichText {
                    paragraph_style,
                    caret_width,
                    ..
                }) => paragraph_style.or_else(|| decode_text_paragraph_style(*caret_width)),
                _ => {
                    for child_id in &node.children {
                        if let Some(style) = walk(ir, *child_id, depth + 1) {
                            return Some(style);
                        }
                    }
                    None
                }
            }
        }
        walk(ir, semantics_id, 0)
    }

    pub(super) fn line_metric_for_local_y<'a>(
        line_metrics: &'a [fission_layout::LineMetric],
        local_y: f32,
    ) -> Option<&'a fission_layout::LineMetric> {
        if line_metrics.is_empty() {
            return None;
        }
        let mut line_top = 0.0f32;
        for (index, line) in line_metrics.iter().enumerate() {
            let line_height = line.height.max(1.0);
            let line_bottom = line_top + line_height;
            if local_y < line_bottom || index + 1 == line_metrics.len() {
                return Some(line);
            }
            line_top = line_bottom;
        }
        line_metrics.last()
    }

    pub(super) fn paragraph_line_x_offset(
        paragraph: TextParagraphStyle,
        bounds_width: f32,
        line_width: f32,
        is_last_line: bool,
    ) -> f32 {
        if bounds_width <= 0.0 {
            return 0.0;
        }

        match paragraph.text_align {
            TextAlign::Start | TextAlign::Left => 0.0,
            TextAlign::Center => (bounds_width - line_width) * 0.5,
            TextAlign::End | TextAlign::Right => bounds_width - line_width,
            TextAlign::Justify if is_last_line => 0.0,
            TextAlign::Justify => 0.0,
        }
    }
}

// This pub fn is no longer needed since Controller uses measurer directly in handle_event
// But other parts of code might still call it, so keep it.
pub fn caret_from_point_in_text(
    measurer: Option<&std::sync::Arc<dyn fission_layout::TextMeasurer>>,
    value: &str,
    font_size: f32,
    viewport_x: f32,
    viewport_w: f32,
    content_w: f32,
    scroll_offset: f32,
    point_x: f32,
) -> usize {
    let local_x = (point_x - viewport_x) + scroll_offset;
    if local_x <= 0.0 {
        return 0;
    }
    let max_x = content_w.max(viewport_w);
    if local_x >= max_x {
        return value.len();
    }

    if let Some(measurer) = measurer {
        // This function is for single line mostly. measurer.hit_test is better.
        // Single-line hit-testing should not wrap text to the viewport width.
        measurer.hit_test(value, font_size, None, local_x, 0.0)
    } else {
        TextInputController::caret_from_point_in_text_fallback(
            value,
            font_size,
            viewport_x,
            viewport_w,
            content_w,
            scroll_offset,
            point_x,
        )
    }
}
