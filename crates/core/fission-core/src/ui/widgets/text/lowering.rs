use super::*;

impl Lower for Text {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let owner_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let layout_node_id = if self.selectable {
            cx.next_node_id()
        } else {
            owner_id
        };
        let resolved_text = self.resolve_text(cx);
        let style = self.resolved_style(cx);
        let paragraph_style = paragraph_style_metadata(
            self.text_align,
            self.text_direction,
            self.text_width_basis,
            self.max_lines,
            self.overflow,
            self.strut_line_height,
            self.text_height_behavior,
        );
        let max_height = cap_max_height(
            self.max_height,
            self.max_lines,
            paragraph_line_height(
                resolve_line_height(style.font_size, style.line_height),
                self.strut_line_height,
            ),
        );
        let clip_to_bounds = should_clip_paragraph(self.max_lines, self.overflow);
        let runtime_selection = if self.selectable {
            cx.runtime_state.selectable_text.selection_range(owner_id)
        } else {
            None
        };
        let selection_range = runtime_selection.or(self.selection_range);

        // `Text` always paints through rich-text IR, so plain and rich text
        // share a single paragraph path.
        //
        // Style resolution falls back to the theme's sans family, so a resolved
        // style always carries a font family. Emitting a bare `DrawText` here
        // would drop that family and leave the renderer on the host's emergency
        // fallback face, which is exactly the design-authority leak this node is
        // meant to prevent. One paint primitive also means one shaping path per
        // shell rather than two that can disagree about wrapping and metrics.
        let runs = apply_selection_to_runs(
            vec![IrTextRun {
                text: resolved_text.clone(),
                style: style.clone(),
            }],
            selection_range,
            self.selection_color,
            self.selection_text_color,
        );
        let paint_node_id = IrBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                display_breaks: self.display_breaks.clone(),
                paragraph_styles: Vec::new(),
                wrap: self.wrap,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style,
            }),
        )
        .build(cx);

        let layout_node_id = wrap_paint_in_layout(
            cx,
            layout_node_id,
            paint_node_id,
            self.width,
            self.height,
            self.min_width,
            self.max_width,
            self.min_height,
            max_height,
            clip_to_bounds,
            self.flex_grow,
            self.flex_shrink,
        );

        if self.selectable {
            let visual_id = wrap_selectable_context_menu(
                cx,
                owner_id,
                layout_node_id,
                &self.context_menu,
                selection_range,
                &resolved_text,
            );
            let semantics = selectable_text_semantics(
                self.semantics.clone(),
                resolved_text,
                false,
                runtime_selection,
                self.context_menu.enabled,
            );
            let mut builder = IrBuilder::new(owner_id, Op::Semantics(semantics));
            builder.add_child(visual_id);
            builder.build(cx)
        } else {
            maybe_wrap_semantics(cx, layout_node_id, self.semantics.clone(), false)
        }
    }
}

impl Lower for RichText {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let owner_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let layout_node_id = if self.selectable {
            cx.next_node_id()
        } else {
            owner_id
        };
        let runs = self.lower_runs(cx);
        let plain_text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        let mut annotations = self.annotations.clone();
        let mut run_byte_start = 0;
        for run in &self.runs {
            let range = run_byte_start..run_byte_start + run.text.len();
            run_byte_start = range.end;
            if range.is_empty()
                || (run.semantics_label.is_none()
                    && run.semantics_identifier.is_none()
                    && run.spell_out.is_none()
                    && run.hyperlink.is_none())
            {
                continue;
            }

            let candidate = IrRichTextAnnotation {
                range: range.clone(),
                semantics_label: run.semantics_label.clone(),
                semantics_identifier: run.semantics_identifier.clone(),
                spell_out: run.spell_out,
                mouse_cursor: run.hyperlink.as_ref().map(|_| IrMouseCursor::Pointer),
                actions: Vec::new(),
                hyperlink: run.hyperlink.clone(),
            };
            if !annotations.iter().any(|annotation| {
                annotation.range == candidate.range
                    && annotation.semantics_identifier == candidate.semantics_identifier
                    && annotation.hyperlink == candidate.hyperlink
            }) {
                annotations.push(candidate);
            }
        }

        let mut rich_text_links = Vec::new();
        for (index, annotation) in annotations.iter_mut().enumerate() {
            let Some(hyperlink) = annotation.hyperlink.clone() else {
                continue;
            };
            let Some(link_text) = plain_text.get(annotation.range.clone()) else {
                continue;
            };
            if link_text.is_empty() {
                continue;
            }

            let semantics_identifier = annotation
                .semantics_identifier
                .clone()
                .unwrap_or_else(|| format!("rich-text-link:{}:{index}", owner_id.as_u128()));
            annotation.semantics_identifier = Some(semantics_identifier.clone());
            let semantics_id = WidgetId::derived(
                owner_id.as_u128(),
                &[
                    0x5254_4c4b,
                    annotation.range.start.min(u32::MAX as usize) as u32,
                    annotation.range.end.min(u32::MAX as usize) as u32,
                ],
            );
            rich_text_links.push((
                semantics_id,
                Semantics {
                    role: Role::Link,
                    label: Some(
                        annotation
                            .semantics_label
                            .clone()
                            .unwrap_or_else(|| link_text.to_owned()),
                    ),
                    identifier: Some(semantics_identifier),
                    hyperlink: Some(hyperlink),
                    focusable: true,
                    ..Semantics::default()
                },
            ));
        }
        let runtime_selection = if self.selectable {
            cx.runtime_state.selectable_text.selection_range(owner_id)
        } else {
            None
        };
        let selection_range = runtime_selection.or(self.selection_range);
        let runs = apply_selection_to_runs(
            runs,
            selection_range,
            self.selection_color,
            self.selection_text_color,
        );
        let paragraph_style = paragraph_style_metadata(
            self.text_align,
            self.text_direction,
            self.text_width_basis,
            self.max_lines,
            self.overflow,
            self.strut_line_height,
            self.text_height_behavior,
        );
        let max_height = cap_max_height(
            self.max_height,
            self.max_lines,
            rich_text_line_height(
                &runs,
                cx.env.theme.tokens.typography.body_medium_size,
                self.strut_line_height,
            ),
        );
        let clip_to_bounds = should_clip_paragraph(self.max_lines, self.overflow);
        let mut paint_builder = IrBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                display_breaks: self.display_breaks.clone(),
                paragraph_styles: Vec::new(),
                wrap: self.wrap,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style,
            }),
        );
        for inline_widget in &self.inline_widgets {
            let child_id = inline_widget.widget.lower(cx);
            paint_builder.add_child(child_id);
        }
        for (link_id, semantics) in rich_text_links {
            IrBuilder::new(link_id, Op::Semantics(semantics)).build(cx);
            paint_builder.add_child(link_id);
        }
        let paint_node_id = paint_builder.build(cx);
        if !annotations.is_empty() {
            cx.ir
                .custom_render_objects
                .insert(paint_node_id, Arc::new(annotations));
        }

        let layout_node_id = wrap_paint_in_layout(
            cx,
            layout_node_id,
            paint_node_id,
            self.width,
            self.height,
            self.min_width,
            self.max_width,
            self.min_height,
            max_height,
            clip_to_bounds,
            self.flex_grow,
            self.flex_shrink,
        );

        if self.selectable {
            let visual_id = wrap_selectable_context_menu(
                cx,
                owner_id,
                layout_node_id,
                &self.context_menu,
                selection_range,
                &plain_text,
            );
            let semantics = selectable_text_semantics(
                self.semantics.clone(),
                plain_text,
                true,
                runtime_selection,
                self.context_menu.enabled,
            );
            let mut builder = IrBuilder::new(owner_id, Op::Semantics(semantics));
            builder.add_child(visual_id);
            builder.build(cx)
        } else {
            maybe_wrap_semantics(cx, layout_node_id, self.semantics.clone(), true)
        }
    }
}
