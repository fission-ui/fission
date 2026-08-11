use super::*;

impl TextInputController {
    pub(super) fn is_apple_platform() -> bool {
        cfg!(target_os = "macos") || cfg!(target_os = "ios")
    }

    pub(super) fn runtime_config(
        ctx: &ControllerContext,
        focused_id: WidgetId,
    ) -> Option<crate::ui::widgets::text_input::TextInputRuntimeConfig> {
        ctx.ir
            .custom_render_objects
            .get(&focused_id)
            .and_then(downcast_text_input_runtime_config)
            .cloned()
    }

    pub(super) fn drag_start_behavior(
        ctx: &ControllerContext,
        focused_id: WidgetId,
    ) -> DragStartBehavior {
        Self::runtime_config(ctx, focused_id)
            .map(|cfg| cfg.drag_start_behavior)
            .unwrap_or_default()
    }

    pub(super) fn sync_runtime_state(
        ctx: &mut ControllerContext,
        focused_id: WidgetId,
        semantic_value: &str,
    ) {
        let runtime = Self::runtime_config(ctx, focused_id);
        ctx.text_edit.sync_from_runtime(
            focused_id,
            semantic_value,
            runtime
                .as_ref()
                .and_then(|cfg| cfg.restoration_id.as_deref()),
            runtime
                .as_ref()
                .and_then(|cfg| cfg.undo_controller.as_ref().map(|undo| undo.capacity)),
        );
    }

    pub(super) fn persist_runtime_state(ctx: &mut ControllerContext, focused_id: WidgetId) {
        let runtime = Self::runtime_config(ctx, focused_id);
        ctx.text_edit.persist_restoration(
            focused_id,
            runtime
                .as_ref()
                .and_then(|cfg| cfg.restoration_id.as_deref()),
        );
    }

    pub(super) fn has_shift(modifiers: u8) -> bool {
        (modifiers & MOD_SHIFT) != 0
    }

    pub(super) fn has_alt(modifiers: u8) -> bool {
        (modifiers & MOD_ALT) != 0
    }

    pub(super) fn has_ctrl(modifiers: u8) -> bool {
        (modifiers & MOD_CTRL) != 0
    }

    pub(super) fn has_super(modifiers: u8) -> bool {
        (modifiers & MOD_SUPER) != 0
    }

    pub(super) fn has_primary_shortcut(modifiers: u8) -> bool {
        if Self::is_apple_platform() {
            Self::has_super(modifiers)
        } else {
            Self::has_ctrl(modifiers)
        }
    }

    pub(super) fn has_word_modifier(modifiers: u8) -> bool {
        if Self::is_apple_platform() {
            Self::has_alt(modifiers)
        } else {
            Self::has_ctrl(modifiers)
        }
    }

    pub(super) fn primary_shortcut_modifier() -> u8 {
        if Self::is_apple_platform() {
            MOD_SUPER
        } else {
            MOD_CTRL
        }
    }

    pub(super) fn node_or_ancestor_matches(
        ir: &fission_ir::CoreIR,
        node_id: WidgetId,
        expected: WidgetId,
    ) -> bool {
        let mut current = Some(node_id);
        while let Some(id) = current {
            if id == expected {
                return true;
            }
            current = ir.nodes.get(&id).and_then(|node| node.parent);
        }
        false
    }

    pub(super) fn toolbar_action_hit(
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
        hit_node_id: WidgetId,
    ) -> Option<TextContextMenuAction> {
        for action in [
            TextContextMenuAction::Copy,
            TextContextMenuAction::Cut,
            TextContextMenuAction::Paste,
            TextContextMenuAction::SelectAll,
        ] {
            if Self::node_or_ancestor_matches(
                ir,
                hit_node_id,
                text_input_toolbar_button_id(focused_id, action),
            ) {
                return Some(action);
            }
        }
        None
    }

    pub(super) fn selection_handle_hit(
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
        hit_node_id: WidgetId,
    ) -> Option<TextSelectionHandleKind> {
        for kind in [
            TextSelectionHandleKind::Caret,
            TextSelectionHandleKind::Start,
            TextSelectionHandleKind::End,
        ] {
            if Self::node_or_ancestor_matches(
                ir,
                hit_node_id,
                text_input_selection_handle_id(focused_id, kind),
            ) {
                return Some(kind);
            }
        }
        None
    }

    pub(super) fn execute_toolbar_action(
        &mut self,
        ctx: &mut ControllerContext,
        action: TextContextMenuAction,
    ) -> bool {
        match action {
            TextContextMenuAction::Copy => {
                self.handle_key(ctx, KeyCode::Char('c'), Self::primary_shortcut_modifier())
            }
            TextContextMenuAction::Cut => {
                self.handle_key(ctx, KeyCode::Char('x'), Self::primary_shortcut_modifier())
            }
            TextContextMenuAction::Paste => {
                self.handle_key(ctx, KeyCode::Char('v'), Self::primary_shortcut_modifier())
            }
            TextContextMenuAction::SelectAll => {
                self.handle_key(ctx, KeyCode::Char('a'), Self::primary_shortcut_modifier())
            }
        }
    }

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
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
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
        let (mut caret_x, caret_y) =
            measurer.get_caret_position(metric_text, font_size, render_width, metric_index);
        let line_metrics = measurer.get_line_metrics(metric_text, font_size, render_width);
        let (line_index, line_metric) = Self::line_metric_for_index(&line_metrics, metric_index)?;
        let is_last_line = line_index + 1 == line_metrics.len();
        if let Some(width) = render_width {
            caret_x +=
                Self::paragraph_line_x_offset(paragraph, width, line_metric.width, is_last_line);
        }

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
        let local_y = (scroll_geom.rect.origin.y - wrapper_geom.rect.origin.y)
            + visible_y
            + line_metric.height.max(1.0);

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
        let Some((scroll_id, _text_node_id, scroll_direction)) =
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
            ctx.ir,
            focused_id,
            &wrapper_geom,
            &scroll_geom,
            scroll_direction,
            scroll_offset,
            &metric_text,
            map_metric_index(caret),
        );
        let anchor_point = Self::local_text_point_for_index(
            measurer,
            ctx.ir,
            focused_id,
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
                if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
                    Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
                {
                    if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
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

    pub(super) fn truncate_to_chars(text: &str, max_chars: usize) -> String {
        text.chars().take(max_chars).collect()
    }

    pub(super) fn apply_text_capitalization(
        mode: TextCapitalization,
        prefix: &str,
        inserted: &str,
    ) -> String {
        match mode {
            TextCapitalization::None => inserted.to_string(),
            TextCapitalization::Characters => inserted.to_uppercase(),
            TextCapitalization::Words => {
                let starts_new_word = prefix
                    .chars()
                    .next_back()
                    .map(|ch| ch.is_whitespace() || ch.is_ascii_punctuation())
                    .unwrap_or(true);
                if starts_new_word {
                    let mut chars = inserted.chars();
                    if let Some(first) = chars.next() {
                        let mut out = first.to_uppercase().to_string();
                        out.push_str(chars.as_str());
                        out
                    } else {
                        String::new()
                    }
                } else {
                    inserted.to_string()
                }
            }
            TextCapitalization::Sentences => {
                let starts_sentence = prefix
                    .chars()
                    .rev()
                    .find(|ch| !ch.is_whitespace())
                    .map(|ch| matches!(ch, '.' | '!' | '?'))
                    .unwrap_or(true);
                if starts_sentence {
                    let mut chars = inserted.chars();
                    if let Some(first) = chars.next() {
                        let mut out = first.to_uppercase().to_string();
                        out.push_str(chars.as_str());
                        out
                    } else {
                        String::new()
                    }
                } else {
                    inserted.to_string()
                }
            }
        }
    }

    pub(super) fn apply_input_type_filter(
        input_type: TextInputType,
        text: &str,
        multiline: bool,
    ) -> String {
        let mut filtered = String::new();
        for ch in text.chars() {
            let allowed = match input_type {
                TextInputType::Text | TextInputType::Name => multiline || ch != '\n',
                TextInputType::Multiline => true,
                TextInputType::Number => ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-' | '+'),
                TextInputType::EmailAddress => !ch.is_whitespace(),
                TextInputType::Url => !ch.is_whitespace(),
                TextInputType::Phone => {
                    ch.is_ascii_digit() || matches!(ch, '+' | '-' | '(' | ')' | ' ')
                }
            };
            if allowed {
                filtered.push(ch);
            }
        }
        if !multiline {
            filtered = filtered.replace('\n', "");
        }
        filtered
    }

    pub(super) fn apply_formatters(
        text: &str,
        formatters: &[InputFormatter],
        multiline: bool,
    ) -> String {
        let mut out = text.to_string();
        for formatter in formatters {
            match formatter {
                InputFormatter::DigitsOnly => {
                    out = out.chars().filter(|ch| ch.is_ascii_digit()).collect();
                }
                InputFormatter::AsciiOnly => {
                    out = out.chars().filter(|ch| ch.is_ascii()).collect();
                }
                InputFormatter::InternalLowercase => {
                    out = out.to_lowercase();
                }
                InputFormatter::Uppercase => {
                    out = out.to_uppercase();
                }
                InputFormatter::TrimWhitespace => {
                    out = out.trim().to_string();
                }
                InputFormatter::SingleLine => {
                    out = out.replace('\n', "");
                }
            }
        }
        if !multiline {
            out = out.replace('\n', "");
        }
        out
    }

    pub(super) fn prepare_inserted_text(
        semantics: &Semantics,
        current_value: &str,
        replace_start: usize,
        replace_end: usize,
        raw_text: &str,
    ) -> Option<String> {
        let replace_start = replace_start.min(current_value.len());
        let replace_end = replace_end.min(current_value.len()).max(replace_start);

        let mut inserted =
            Self::apply_input_type_filter(semantics.text_input_type, raw_text, semantics.multiline);
        inserted = Self::apply_text_capitalization(
            semantics.text_capitalization,
            &current_value[..replace_start],
            &inserted,
        );
        inserted =
            Self::apply_formatters(&inserted, &semantics.input_formatters, semantics.multiline);

        if let Some(mask) = &semantics.input_mask {
            inserted = inserted
                .chars()
                .filter(|ch| mask.is_valid_char(*ch))
                .collect();
        }

        if semantics.max_length_enforcement == MaxLengthEnforcement::Enforced {
            if let Some(max_length) = semantics.max_length {
                let current_chars = current_value.chars().count();
                let replaced_chars = current_value[replace_start..replace_end].chars().count();
                let available =
                    max_length.saturating_sub(current_chars.saturating_sub(replaced_chars));
                inserted = Self::truncate_to_chars(&inserted, available);
            }
        }

        if inserted.is_empty() {
            None
        } else {
            Some(inserted)
        }
    }
}
