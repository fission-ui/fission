use super::*;

impl TextInputController {
    pub(super) fn handle_ime(
        &mut self,
        ctx: &mut ControllerContext,
        ime: &crate::event::ImeEvent,
    ) -> bool {
        match ime {
            crate::event::ImeEvent::Commit { text } => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.role == fission_ir::semantics::Role::TextInput {
                                if semantics.disabled || semantics.read_only {
                                    return true;
                                }
                                let (value, _caret, _anchor) = Self::resolve_editing_value(
                                    ctx,
                                    focused_id,
                                    semantics.value.as_deref().unwrap_or(""),
                                );
                                let st = ctx.text_edit.get_mut_or_default(focused_id);

                                let (start, end) = st
                                    .preedit
                                    .as_ref()
                                    .map(|preedit| preedit.range)
                                    .unwrap_or_else(|| st.selection_range());

                                if let Some(filtered_text) =
                                    Self::prepare_inserted_text(semantics, &value, start, end, text)
                                {
                                    let new_caret = start + filtered_text.len();
                                    let new_text = st.apply_edit(
                                        start..end,
                                        &filtered_text,
                                        new_caret,
                                        new_caret,
                                    );
                                    self.dispatch_change(ctx, semantics, focused_id, new_text);
                                    Self::dispatch_cursor_change(
                                        ctx, semantics, focused_id, new_caret, new_caret,
                                    );
                                } else {
                                    st.clear_preedit();
                                }

                                return true;
                            }
                        }
                    }
                }
            }
            crate::event::ImeEvent::Preedit { text, cursor } => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.disabled || semantics.read_only {
                                return true;
                            }
                            Self::sync_runtime_state(
                                ctx,
                                focused_id,
                                semantics.value.as_deref().unwrap_or(""),
                            );
                        }
                    }
                    let st = ctx.text_edit.get_mut_or_default(focused_id);
                    st.set_preedit(text.clone(), *cursor);
                    Self::auto_scroll_textinput(ctx, focused_id);
                    return true;
                }
            }
            crate::event::ImeEvent::Cancel => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.disabled || semantics.read_only {
                                return true;
                            }
                            Self::sync_runtime_state(
                                ctx,
                                focused_id,
                                semantics.value.as_deref().unwrap_or(""),
                            );
                        }
                    }
                    let st = ctx.text_edit.get_mut_or_default(focused_id);
                    st.clear_preedit();
                    Self::auto_scroll_textinput(ctx, focused_id);
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn dispatch_change(
        &self,
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        new_text: String,
    ) {
        Self::persist_runtime_state(ctx, node_id);
        if let Some(action_entry) = semantics.actions.entries.iter().find(|e| {
            matches!(
                e.trigger,
                fission_ir::semantics::ActionTrigger::Change
                    | fission_ir::semantics::ActionTrigger::NumberChange
            )
        }) {
            let payload = match action_entry.trigger {
                fission_ir::semantics::ActionTrigger::Change => serde_json::to_vec(&new_text)
                    .expect("serializing text input change payload should not fail"),
                fission_ir::semantics::ActionTrigger::NumberChange => {
                    let Ok(parsed) = new_text.trim().parse::<f32>() else {
                        return;
                    };
                    serde_json::to_vec(&parsed)
                        .expect("serializing numeric text input payload should not fail")
                }
                _ => unreachable!("filtered to text input change triggers"),
            };

            let envelope = ActionEnvelope {
                id: ActionId::from_u128(action_entry.action_id),
                payload,
            };
            let input =
                crate::input::scoped_action_input(ctx.ir, node_id, crate::ActionInput::None);
            ctx.dispatched_actions.push((node_id, envelope, input));

            // State update moved to handle_key to avoid double borrow

            Self::auto_scroll_textinput(ctx, node_id);
        }
    }

    pub(super) fn dispatch_cursor_change(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        new_caret: usize,
        new_anchor: usize,
    ) {
        // Deduplicate: skip dispatch if cursor position hasn't actually changed
        // since our last dispatch. This prevents unnecessary model updates that
        // would trigger extra rebuild cycles.
        if let Some(st) = ctx.text_edit.states.get(&node_id) {
            if st.last_dispatched_cursor == Some((new_caret, new_anchor)) {
                return;
            }
        }

        Self::persist_runtime_state(ctx, node_id);

        if let Some(action_entry) = semantics
            .actions
            .entries
            .iter()
            .find(|e| e.trigger == fission_ir::semantics::ActionTrigger::CursorChange)
        {
            // Record the dispatched position before dispatching
            if let Some(st) = ctx.text_edit.states.get_mut(&node_id) {
                st.last_dispatched_cursor = Some((new_caret, new_anchor));
            }

            let cursor_changed = crate::action::CursorChanged {
                caret: new_caret,
                anchor: new_anchor,
            };
            let payload = serde_json::to_vec(&cursor_changed).unwrap();
            let envelope = ActionEnvelope {
                id: ActionId::from_u128(action_entry.action_id),
                payload,
            };
            let input =
                crate::input::scoped_action_input(ctx.ir, node_id, crate::ActionInput::None);
            ctx.dispatched_actions.push((node_id, envelope, input));
        }
    }

    pub(super) fn dispatch_submit(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        current_value: &str,
    ) -> bool {
        let mut dispatched = false;
        for trigger in [
            fission_ir::semantics::ActionTrigger::EditingComplete,
            fission_ir::semantics::ActionTrigger::Submit,
        ] {
            dispatched |= Self::dispatch_action_for_trigger(
                ctx,
                semantics,
                node_id,
                trigger,
                Some(serde_json::to_vec(&current_value.to_string()).unwrap()),
            );
        }
        dispatched
    }

    pub(super) fn dispatch_action_for_trigger(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        trigger: fission_ir::semantics::ActionTrigger,
        fallback_payload: Option<Vec<u8>>,
    ) -> bool {
        let Some(action_entry) = semantics
            .actions
            .entries
            .iter()
            .find(|e| e.trigger == trigger)
        else {
            return false;
        };
        let payload = action_entry
            .payload_data
            .clone()
            .or(fallback_payload)
            .unwrap_or_else(|| serde_json::to_vec(&()).unwrap());
        let envelope = ActionEnvelope {
            id: ActionId::from_u128(action_entry.action_id),
            payload,
        };
        let input = crate::input::scoped_action_input(ctx.ir, node_id, crate::ActionInput::None);
        ctx.dispatched_actions.push((node_id, envelope, input));
        true
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
    /// same `available_width` that the renderer will use so both sides consume
    /// geometry from the selected paragraph profile. This ensures the Y-to-line
    /// and X-to-glyph mapping in hit-testing matches the rendered text without
    /// coupling core input handling to a particular text engine.
    pub(super) fn hit_test_text(
        measurer: &std::sync::Arc<dyn fission_layout::TextMeasurer>,
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
        prefer_plain_text: bool,
        text: &str,
        scroll_geom: &fission_layout::LayoutNodeGeometry,
        local_x: f32,
        local_y: f32,
    ) -> usize {
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
}
