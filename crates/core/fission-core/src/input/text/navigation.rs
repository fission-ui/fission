use super::*;

impl TextInputController {
    pub(crate) fn ime_cursor_area(
        ctx: &mut ControllerContext,
        text_root: WidgetId,
    ) -> Option<fission_layout::LayoutRect> {
        let measurer = ctx.measurer?;
        let node = ctx.ir.nodes.get(&text_root)?;
        let semantics = match &node.op {
            Op::Semantics(semantics) => semantics,
            _ => return None,
        };

        let (scroll_id, _text_op_node_id, scroll_direction) =
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
        let (caret_x, caret_y) =
            measurer.get_caret_position(&metric_text, font_size, render_width, metric_caret_idx);

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
        let line_x =
            Self::paragraph_line_x_offset(paragraph, viewport_size.width, line_width, is_last_line);

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

            if let Some((scroll_id, _text_op_node_id, scroll_direction)) =
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

                    let (caret_x, caret_y) = measurer.get_caret_position(
                        &metric_text,
                        font_size,
                        measurer_width,
                        metric_caret_idx,
                    );

                    let mut offset = ctx.scroll.get_offset(scroll_id);

                    if scroll_direction == op::FlexDirection::Row {
                        // Handle horizontal scrolling for single-line text
                        let line_width = measurer
                            .get_line_metrics(&metric_text, font_size, None)
                            .first()
                            .map(|line| line.width)
                            .unwrap_or_else(|| measurer.measure(&metric_text, font_size, None).0);
                        let caret_left = caret_x
                            + Self::paragraph_line_x_offset(
                                paragraph,
                                viewport_size.width,
                                line_width,
                                false,
                            );
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
                        let caret_height = measurer
                            .measure("Tg", font_size, Some(viewport_size.width))
                            .1;
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
                        let content_h = scroll_geom.content_size.height.max(viewport_size.height);
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
            if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_w = scroll_geom.rect.size.width;
                    let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);

                    let (current_caret_x, _current_caret_y) =
                        measurer.get_caret_position(value, font_size, Some(viewport_w), caret);

                    let line_metrics =
                        measurer.get_line_metrics(value, font_size, Some(viewport_w));

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

                        let mut new_caret_pos = measurer.hit_test(
                            value,
                            font_size,
                            Some(viewport_w),
                            current_caret_x,
                            target_y,
                        );

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
            if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_w = scroll_geom.rect.size.width;
                    let viewport_h = scroll_geom.rect.size.height.max(1.0);
                    let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);
                    let (current_caret_x, _current_caret_y) =
                        measurer.get_caret_position(value, font_size, Some(viewport_w), caret);
                    let line_metrics =
                        measurer.get_line_metrics(value, font_size, Some(viewport_w));

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
                        let mut new_caret_pos = measurer.hit_test(
                            value,
                            font_size,
                            Some(viewport_w),
                            current_caret_x,
                            target_y,
                        );
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
