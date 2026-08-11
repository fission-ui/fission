use super::*;

impl<'a> VelloRenderer<'a> {
    pub(super) fn paragraph_base_style(
        base_size: f32,
        base_color: RenderColor,
        underline: bool,
    ) -> RenderTextStyle {
        RenderTextStyle {
            font_size: base_size,
            color: base_color,
            underline,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: fission_ir::op::FontStyle::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
        }
    }

    pub(super) fn resolve_ellipsis_style(
        &self,
        line_range: std::ops::Range<usize>,
        base_style: &RenderTextStyle,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) -> RenderTextStyle {
        styles
            .iter()
            .rev()
            .find(|(range, _)| range.start < line_range.end && range.end > line_range.start)
            .map(|(_, style)| style.clone())
            .unwrap_or_else(|| base_style.clone())
    }

    pub(super) fn ellipsis_metrics(&self, style: &RenderTextStyle) -> (f32, f32) {
        let ellipsis = "...";
        if text_style_requires_rich_layout(style) {
            let layout = self.measurer.layout_rich(
                ellipsis,
                style.font_size,
                style.color,
                &[(0..ellipsis.len(), style.clone())],
                &[],
                None,
            );
            let metrics = layout
                .lines()
                .next()
                .map(|line| (line.metrics().advance, line.metrics().baseline));
            if let Some(metrics) = metrics {
                return metrics;
            }
        } else {
            let layout = self.measurer.get_layout(ellipsis, style.font_size, None);
            let metrics = layout
                .lines()
                .next()
                .map(|line| (line.metrics().advance, line.metrics().baseline));
            if let Some(metrics) = metrics {
                return metrics;
            }
        }

        (style.font_size, style.font_size)
    }

    #[cfg(test)]
    pub(super) fn paragraph_layout(
        &self,
        text: &str,
        base_style: &RenderTextStyle,
        wrap: bool,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) -> parley::layout::Layout<ParleyBrush> {
        let prepared =
            prepare_paragraph_layout(text, base_style, paragraph, inline_boxes, styles, None);
        self.paragraph_layout_from_prepared(&prepared, wrap, bounds, paragraph)
    }

    pub(super) fn paragraph_layout_from_prepared(
        &self,
        prepared: &PreparedParagraphLayout,
        wrap: bool,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
    ) -> parley::layout::Layout<ParleyBrush> {
        let mut layout = (*self.measurer.layout_rich(
            &prepared.text,
            prepared.base_style.font_size,
            prepared.base_style.color,
            &prepared.styles,
            &prepared.inline_boxes,
            if wrap && bounds.width() > 0.0 {
                Some(bounds.width() as f32)
            } else {
                None
            },
        ))
        .clone();

        if let Some(alignment_width) = paragraph_alignment_width(&layout, bounds, paragraph) {
            layout.align(
                Some(alignment_width),
                paragraph_alignment(paragraph.text_align),
                paragraph_alignment_options(paragraph.text_align),
            );
        }

        layout
    }

    #[cfg(test)]
    pub(super) fn paragraph_annotation_at_point(
        &self,
        text: &str,
        base_style: &RenderTextStyle,
        wrap: bool,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        annotations: &[fission_ir::op::RichTextAnnotation],
        x: f32,
        y: f32,
    ) -> Option<fission_ir::op::RichTextAnnotation> {
        if annotations.is_empty() {
            return None;
        }

        let prepared =
            prepare_paragraph_layout(text, base_style, paragraph, inline_boxes, styles, None);
        let layout = self.paragraph_layout_from_prepared(&prepared, wrap, bounds, paragraph);
        let total_lines = layout.lines().count();
        let visible_lines = paragraph
            .max_lines
            .map(|lines| lines.min(total_lines))
            .unwrap_or(total_lines);
        let local_y = y - paragraph_y_offset(
            layout.lines().next().as_ref(),
            paragraph.text_height_behavior,
            visible_lines == 1,
        );
        let idx = crate::text::VelloTextMeasurer::hit_test_layout_index_at_point(
            &prepared.text,
            &layout,
            x,
            local_y,
        )?;
        let raw_idx = idx
            .saturating_sub(prepared.text_byte_offset)
            .min(text.len());
        crate::text::resolve_rich_text_annotation_at_index(text, annotations, raw_idx)
    }

    pub(super) fn draw_paragraph_line(
        &mut self,
        line: &parley::layout::Line<'_, ParleyBrush>,
        position: fission_render::LayoutPoint,
        top_y: f32,
        line_height: f32,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        clip: Option<TextClip>,
    ) {
        if let Some(clip) = clip {
            if !clip.intersects_y(top_y, top_y + line_height) {
                return;
            }
        }
        self.draw_paragraph_line_backgrounds(line, position, top_y, line_height, styles, clip);
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let run_left = glyph_run.offset();
                let run_right = glyph_run.offset() + glyph_run.advance();
                if let Some(clip) = clip {
                    if !clip.intersects_x(run_left, run_right) {
                        continue;
                    }
                }
                let style = glyph_run.style();
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let brush_data = style.brush.clone();
                let color = Color::from_rgba8(
                    brush_data.0[0],
                    brush_data.0[1],
                    brush_data.0[2],
                    brush_data.0[3],
                );

                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let glyphs = glyph_run
                    .glyphs()
                    .filter_map(|g| {
                        let gx = x + g.x;
                        let gy = y - g.y;
                        x += g.advance;
                        let glyph_right = gx + g.advance.max(1.0);
                        if clip
                            .map(|clip| clip.intersects_x(gx, glyph_right))
                            .unwrap_or(true)
                        {
                            Some(Glyph {
                                id: g.id as u32,
                                x: gx,
                                y: gy,
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if glyphs.is_empty() {
                    continue;
                }

                self.scene
                    .draw_glyphs(font)
                    .font_size(font_size)
                    .transform(
                        self.current_transform
                            * Affine::translate((position.x as f64, position.y as f64)),
                    )
                    .brush(color)
                    .draw(Fill::NonZero, glyphs.into_iter());

                if let Some(decoration) = &style.underline {
                    let metrics = run.metrics();
                    let offset = decoration.offset.unwrap_or(metrics.underline_offset);
                    let size = decoration.size.unwrap_or(metrics.underline_size).max(1.0);
                    let deco_brush = decoration.brush.clone();
                    let deco_color = Color::from_rgba8(
                        deco_brush.0[0],
                        deco_brush.0[1],
                        deco_brush.0[2],
                        deco_brush.0[3],
                    );

                    let x0 = clip.map(|clip| run_left.max(clip.left)).unwrap_or(run_left);
                    let x1 = clip
                        .map(|clip| run_right.min(clip.right))
                        .unwrap_or(run_right);
                    if x1 <= x0 {
                        continue;
                    }
                    let x0 = position.x as f64 + x0 as f64;
                    let x1 = position.x as f64 + x1 as f64;
                    let y0 = position.y as f64 + (glyph_run.baseline() + offset) as f64;
                    let rect = Rect::new(x0, y0, x1, y0 + size as f64);
                    self.scene.fill(
                        Fill::NonZero,
                        self.current_transform,
                        deco_color,
                        None,
                        &rect,
                    );
                }
            }
        }
    }

    pub(super) fn draw_paragraph_line_backgrounds(
        &mut self,
        line: &parley::layout::Line<'_, ParleyBrush>,
        position: fission_render::LayoutPoint,
        top_y: f32,
        line_height: f32,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        clip: Option<TextClip>,
    ) {
        if !styles
            .iter()
            .any(|(_, style)| style.background_color.is_some())
        {
            return;
        }

        for run in line.runs() {
            let run_text_range = run.text_range();
            for (range, style) in styles.iter() {
                let Some(bg) = &style.background_color else {
                    continue;
                };
                let overlap_start = range.start.max(run_text_range.start);
                let overlap_end = range.end.min(run_text_range.end);
                if overlap_start >= overlap_end {
                    continue;
                }

                let segments = text_background_segments_for_cluster_ranges(
                    run.visual_clusters().filter_map(|cluster| {
                        let left = cluster.visual_offset()?;
                        Some((cluster.text_range(), left, left + cluster.advance()))
                    }),
                    range,
                    clip,
                );
                if segments.is_empty() {
                    continue;
                }

                let bg_color = Color::from_rgba8(bg.r, bg.g, bg.b, bg.a);
                let y0 = position.y as f64 + top_y as f64;
                for segment in segments {
                    let x0 = position.x as f64 + segment.left as f64;
                    let x1 = position.x as f64 + segment.right as f64;
                    let bg_rect = Rect::new(x0, y0, x1, y0 + line_height as f64);
                    self.scene.fill(
                        Fill::NonZero,
                        self.current_transform,
                        bg_color,
                        None,
                        &bg_rect,
                    );
                }
            }
        }
    }

    pub(super) fn draw_paragraph_line_with_fade(
        &mut self,
        line: &parley::layout::Line<'_, ParleyBrush>,
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
        top_y: f32,
        line_height: f32,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        clip: Option<TextClip>,
        fade: ParagraphFade,
    ) {
        let line_top = position.y + top_y;
        let line_bottom = line_top + line_height;

        match fade {
            ParagraphFade::Right { start, end } => {
                let body_end = start.max(0.0);
                if body_end > 0.0 {
                    let clip_rect = Rect::new(
                        bounds.x() as f64,
                        line_top as f64,
                        (position.x + body_end).min(bounds.right()) as f64,
                        line_bottom as f64,
                    );
                    self.with_clip_rect(clip_rect, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }

                let fade_width = end - start;
                for slice in 0..PARAGRAPH_FADE_SLICE_COUNT {
                    let slice_start =
                        start + fade_width * slice as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let slice_end =
                        start + fade_width * (slice + 1) as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let alpha = 1.0 - (slice as f32 + 0.5) / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let clip_rect = Rect::new(
                        (position.x + slice_start).max(bounds.x()) as f64,
                        line_top as f64,
                        (position.x + slice_end).min(bounds.right()) as f64,
                        line_bottom as f64,
                    );
                    self.with_alpha_clip_rect(clip_rect, alpha, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }
            }
            ParagraphFade::Bottom { start, end } => {
                if start > 0.0 {
                    let clip_rect = Rect::new(
                        bounds.x() as f64,
                        line_top as f64,
                        bounds.right() as f64,
                        (line_top + start).min(bounds.bottom()) as f64,
                    );
                    self.with_clip_rect(clip_rect, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }

                let fade_height = end - start;
                for slice in 0..PARAGRAPH_FADE_SLICE_COUNT {
                    let slice_start =
                        start + fade_height * slice as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let slice_end = start
                        + fade_height * (slice + 1) as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let alpha = 1.0 - (slice as f32 + 0.5) / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let clip_rect = Rect::new(
                        bounds.x() as f64,
                        (line_top + slice_start).max(bounds.y()) as f64,
                        bounds.right() as f64,
                        (line_top + slice_end).min(bounds.bottom()) as f64,
                    );
                    self.with_alpha_clip_rect(clip_rect, alpha, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }
            }
        }
    }

    pub(super) fn render_paragraph_text(
        &mut self,
        text: &str,
        base_style: &RenderTextStyle,
        wrap: bool,
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        caret_index: Option<usize>,
        caret_color: Option<RenderColor>,
        caret_width: Option<f32>,
        caret_height: Option<f32>,
        caret_radius: Option<f32>,
    ) {
        let prepared = prepare_paragraph_layout(
            text,
            base_style,
            paragraph,
            inline_boxes,
            styles,
            caret_index,
        );
        let layout = self.paragraph_layout_from_prepared(&prepared, wrap, bounds, paragraph);
        let lines: Vec<_> = layout.lines().collect();
        let total_lines = lines.len();
        let visible_lines = paragraph
            .max_lines
            .map(|lines| lines.min(total_lines))
            .unwrap_or(total_lines);
        let draw_position = fission_render::LayoutPoint::new(
            position.x,
            position.y
                + paragraph_y_offset(
                    lines.first(),
                    paragraph.text_height_behavior,
                    visible_lines == 1,
                ),
        );
        let text_clip = self.text_clip(draw_position, bounds);

        for (line_idx, line) in lines.iter().take(visible_lines).enumerate() {
            let metrics = *line.metrics();
            let line_height = metrics
                .line_height
                .max(metrics.ascent + metrics.descent)
                .max(1.0);
            let top_y = metrics.baseline - metrics.ascent;
            let is_last_visible_line = line_idx + 1 == visible_lines;
            let (top_trim, bottom_trim) = paragraph_line_trim(
                line,
                paragraph.text_height_behavior,
                line_idx == 0,
                is_last_visible_line,
            );
            let visual_line_height = (line_height - top_trim - bottom_trim).max(1.0);
            let visual_bounds =
                paragraph_line_visual_bounds(line).unwrap_or(ParagraphLineVisualBounds {
                    left: metrics.offset,
                    right: metrics.offset + metrics.advance,
                });
            let line_width = (visual_bounds.right - visual_bounds.left).max(0.0);
            let has_more_lines = line_idx + 1 < total_lines;
            let overflows_horizontally = bounds.width() > 0.0 && line_width > bounds.width();
            let show_ellipsis = matches!(paragraph.overflow, TextOverflow::Ellipsis)
                && is_last_visible_line
                && (has_more_lines || overflows_horizontally);
            let fade = paragraph_fade(
                paragraph,
                bounds,
                visual_line_height,
                line_width,
                is_last_visible_line,
                has_more_lines,
                overflows_horizontally,
            );

            let ellipsis = show_ellipsis.then(|| {
                let style = self.resolve_ellipsis_style(
                    line.text_range(),
                    &prepared.base_style,
                    &prepared.styles,
                );
                let (width, baseline) = self.ellipsis_metrics(&style);
                let line_end = if bounds.width() > 0.0 {
                    visual_bounds.right.min(bounds.width()).max(0.0)
                } else {
                    visual_bounds.right.max(0.0)
                };
                let left = (line_end - width).max(0.0);
                (style, width, baseline, left)
            });

            if let Some((_, _, _, ellipsis_left)) = ellipsis.as_ref() {
                let clip_rect = Rect::new(
                    bounds.x() as f64,
                    draw_position.y as f64 + top_y as f64,
                    (draw_position.x + *ellipsis_left).max(bounds.x()) as f64,
                    draw_position.y as f64 + top_y as f64 + visual_line_height as f64,
                );
                self.with_clip_rect(clip_rect, |this| {
                    this.draw_paragraph_line(
                        line,
                        draw_position,
                        top_y,
                        visual_line_height,
                        &prepared.styles,
                        text_clip,
                    );
                });
            } else if let Some(fade) = fade {
                self.draw_paragraph_line_with_fade(
                    line,
                    draw_position,
                    bounds,
                    top_y,
                    visual_line_height,
                    &prepared.styles,
                    text_clip,
                    fade,
                );
            } else {
                self.draw_paragraph_line(
                    line,
                    draw_position,
                    top_y,
                    visual_line_height,
                    &prepared.styles,
                    text_clip,
                );
            }

            if let Some((style, width, baseline, ellipsis_left)) = ellipsis {
                let ellipsis_position = fission_render::LayoutPoint::new(
                    draw_position.x + ellipsis_left,
                    draw_position.y + metrics.baseline - baseline,
                );
                let ellipsis_bounds = fission_render::LayoutRect::new(
                    ellipsis_position.x,
                    ellipsis_position.y,
                    width,
                    visual_line_height,
                );
                if text_style_requires_rich_layout(&style) {
                    let ellipsis_styles = vec![(0..3, style.clone())];
                    self.render_text(
                        "...",
                        style.font_size,
                        style.color,
                        style.underline,
                        false,
                        ellipsis_position,
                        ellipsis_bounds,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        &[],
                        &ellipsis_styles,
                    );
                } else {
                    self.render_text(
                        "...",
                        style.font_size,
                        style.color,
                        style.underline,
                        false,
                        ellipsis_position,
                        ellipsis_bounds,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        &[],
                        &[],
                    );
                }
            }
        }

        if let Some(idx) = prepared.caret_index {
            self.draw_caret(
                &layout,
                idx,
                position,
                &prepared.text,
                prepared.base_style.font_size,
                caret_color.unwrap_or(prepared.base_style.color),
                caret_width.unwrap_or(2.0),
                caret_height,
                caret_radius,
                paragraph,
            );
        }
    }

    pub(super) fn render_text(
        &mut self,
        text: &str,
        base_size: f32,
        base_color: RenderColor,
        underline: bool,
        wrap: bool,
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
        caret_index: Option<usize>,
        caret_color: Option<RenderColor>,
        caret_width: Option<f32>,
        caret_height: Option<f32>,
        caret_radius: Option<f32>,
        paragraph_style: Option<TextParagraphStyle>,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) {
        let paragraph = paragraph_style
            .or_else(|| {
                if caret_index.is_none() {
                    decode_text_paragraph_style(caret_width)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if paragraph != TextParagraphStyle::default() {
            let base_style = Self::paragraph_base_style(base_size, base_color, underline);
            let owned_styles;
            let paragraph_styles = if styles.is_empty() && !text.is_empty() {
                owned_styles = vec![(0..text.len(), base_style.clone())];
                owned_styles.as_slice()
            } else {
                styles
            };
            self.render_paragraph_text(
                text,
                &base_style,
                wrap,
                position,
                bounds,
                paragraph,
                inline_boxes,
                paragraph_styles,
                caret_index,
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
            );
            return;
        }

        let text_clip = self.text_clip(position, bounds);

        // Fast path for simple text using cache
        if styles.is_empty() && inline_boxes.is_empty() {
            let layout = self.measurer.get_layout(
                text,
                base_size,
                if wrap && bounds.width() > 0.0 {
                    Some(bounds.width() as f32)
                } else {
                    None
                },
            );

            // Draw Glyphs (Reused layout logic)
            for line in layout.lines() {
                let metrics = *line.metrics();
                let line_height = metrics
                    .line_height
                    .max(metrics.ascent + metrics.descent)
                    .max(1.0);
                let line_top = metrics.baseline - metrics.ascent;
                if let Some(clip) = text_clip {
                    if !clip.intersects_y(line_top, line_top + line_height) {
                        continue;
                    }
                }
                for item in line.items() {
                    if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                        let run_left = glyph_run.offset();
                        let run_right = glyph_run.offset() + glyph_run.advance();
                        if let Some(clip) = text_clip {
                            if !clip.intersects_x(run_left, run_right) {
                                continue;
                            }
                        }
                        let run = glyph_run.run();
                        let font = run.font();
                        let font_size = run.font_size();

                        // Override color from base_color since cached layout is color-agnostic
                        let color = Color::from_rgba8(
                            base_color.r,
                            base_color.g,
                            base_color.b,
                            base_color.a,
                        );

                        let mut x = glyph_run.offset();
                        let y = glyph_run.baseline();

                        let glyphs = glyph_run
                            .glyphs()
                            .filter_map(|g| {
                                let gx = x + g.x;
                                let gy = y - g.y;
                                x += g.advance;
                                let glyph_right = gx + g.advance.max(1.0);
                                if text_clip
                                    .map(|clip| clip.intersects_x(gx, glyph_right))
                                    .unwrap_or(true)
                                {
                                    Some(Glyph {
                                        id: g.id as u32,
                                        x: gx,
                                        y: gy,
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        if glyphs.is_empty() {
                            continue;
                        }

                        self.scene
                            .draw_glyphs(font)
                            .font_size(font_size)
                            .transform(
                                self.current_transform
                                    * Affine::translate((position.x as f64, position.y as f64)),
                            )
                            .brush(color)
                            .draw(Fill::NonZero, glyphs.into_iter());

                        if underline {
                            let metrics = run.metrics();
                            let offset = metrics.underline_offset;
                            let size = metrics.underline_size.max(1.0);
                            let x0 = text_clip
                                .map(|clip| run_left.max(clip.left))
                                .unwrap_or(run_left);
                            let x1 = text_clip
                                .map(|clip| run_right.min(clip.right))
                                .unwrap_or(run_right);
                            if x1 <= x0 {
                                continue;
                            }
                            let x0 = position.x as f64 + x0 as f64;
                            let x1 = position.x as f64 + x1 as f64;
                            let y0 = position.y as f64 + (glyph_run.baseline() + offset) as f64;
                            let rect = Rect::new(x0, y0, x1, y0 + size as f64);
                            self.scene.fill(
                                Fill::NonZero,
                                self.current_transform,
                                color,
                                None,
                                &rect,
                            );
                        }
                    }
                }
            }
            if let Some(idx) = caret_index {
                self.draw_caret(
                    &layout,
                    idx,
                    position,
                    text,
                    base_size,
                    caret_color.unwrap_or(base_color),
                    caret_width.unwrap_or(2.0),
                    caret_height,
                    caret_radius,
                    paragraph,
                );
            }
            return;
        }

        // Slow path for rich text
        let layout = self.measurer.layout_rich(
            text,
            base_size,
            base_color,
            styles,
            inline_boxes,
            if wrap && bounds.width() > 0.0 {
                Some(bounds.width() as f32)
            } else {
                None
            },
        );

        // Draw Glyphs for rich text (uses brushes from layout)
        for line in layout.lines() {
            let metrics = *line.metrics();
            let line_height = metrics
                .line_height
                .max(metrics.ascent + metrics.descent)
                .max(1.0);
            let line_top = metrics.baseline - metrics.ascent;
            if let Some(clip) = text_clip {
                if !clip.intersects_y(line_top, line_top + line_height) {
                    continue;
                }
            }
            self.draw_paragraph_line_backgrounds(
                &line,
                position,
                line_top,
                line_height,
                styles,
                text_clip,
            );
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let run_left = glyph_run.offset();
                    let run_right = glyph_run.offset() + glyph_run.advance();
                    if let Some(clip) = text_clip {
                        if !clip.intersects_x(run_left, run_right) {
                            continue;
                        }
                    }
                    let style = glyph_run.style();
                    let run = glyph_run.run();
                    let font = run.font();
                    let font_size = run.font_size();
                    let brush_data = style.brush.clone();
                    let color = Color::from_rgba8(
                        brush_data.0[0],
                        brush_data.0[1],
                        brush_data.0[2],
                        brush_data.0[3],
                    );

                    let mut x = glyph_run.offset();
                    let y = glyph_run.baseline();

                    let glyphs = glyph_run
                        .glyphs()
                        .filter_map(|g| {
                            let gx = x + g.x;
                            let gy = y - g.y;
                            x += g.advance;
                            let glyph_right = gx + g.advance.max(1.0);
                            if text_clip
                                .map(|clip| clip.intersects_x(gx, glyph_right))
                                .unwrap_or(true)
                            {
                                Some(Glyph {
                                    id: g.id as u32,
                                    x: gx,
                                    y: gy,
                                })
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    if glyphs.is_empty() {
                        continue;
                    }

                    self.scene
                        .draw_glyphs(font)
                        .font_size(font_size)
                        .transform(
                            self.current_transform
                                * Affine::translate((position.x as f64, position.y as f64)),
                        )
                        .brush(color)
                        .draw(Fill::NonZero, glyphs.into_iter());

                    if let Some(decoration) = &style.underline {
                        let metrics = run.metrics();
                        let offset = decoration.offset.unwrap_or(metrics.underline_offset);
                        let size = decoration.size.unwrap_or(metrics.underline_size).max(1.0);
                        let deco_brush = decoration.brush.clone();
                        let deco_color = Color::from_rgba8(
                            deco_brush.0[0],
                            deco_brush.0[1],
                            deco_brush.0[2],
                            deco_brush.0[3],
                        );

                        let x0 = text_clip
                            .map(|clip| run_left.max(clip.left))
                            .unwrap_or(run_left);
                        let x1 = text_clip
                            .map(|clip| run_right.min(clip.right))
                            .unwrap_or(run_right);
                        if x1 <= x0 {
                            continue;
                        }
                        let x0 = position.x as f64 + x0 as f64;
                        let x1 = position.x as f64 + x1 as f64;
                        let y0 = position.y as f64 + (glyph_run.baseline() + offset) as f64;
                        let rect = Rect::new(x0, y0, x1, y0 + size as f64);
                        self.scene.fill(
                            Fill::NonZero,
                            self.current_transform,
                            deco_color,
                            None,
                            &rect,
                        );
                    }
                }
            }
        }

        if let Some(idx) = caret_index {
            self.draw_caret(
                &layout,
                idx,
                position,
                text,
                base_size,
                caret_color.unwrap_or(base_color),
                caret_width.unwrap_or(2.0),
                caret_height,
                caret_radius,
                paragraph,
            );
        }
    }

    fn next_char_boundary(text: &str, idx: usize) -> usize {
        if idx >= text.len() {
            return text.len();
        }
        if !text.is_char_boundary(idx) {
            return text.len();
        }
        let mut it = text[idx..].char_indices();
        let _ = it.next();
        if let Some((off, _)) = it.next() {
            idx + off
        } else {
            text.len()
        }
    }

    pub(super) fn draw_caret(
        &mut self,
        layout: &parley::layout::Layout<ParleyBrush>,
        idx: usize,
        position: fission_render::LayoutPoint,
        text: &str,
        base_size: f32,
        caret_color: RenderColor,
        caret_width: f32,
        caret_height: Option<f32>,
        caret_radius: Option<f32>,
        paragraph: TextParagraphStyle,
    ) {
        let mut caret_drawn = false;
        let lines_count = layout.lines().count();
        let paragraph_y_offset = paragraph_y_offset(
            layout.lines().next().as_ref(),
            paragraph.text_height_behavior,
            lines_count == 1,
        );

        for (i, line) in layout.lines().enumerate() {
            let range = line.text_range();
            let is_last_line = i == lines_count - 1;

            if (idx >= range.start && idx < range.end) || (is_last_line && idx == range.end) {
                let mut x_pos = 0.0;
                for item in line.items() {
                    if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                        let style_run_range = glyph_run.run().text_range();
                        let line_range = line.text_range();
                        let start = style_run_range.start.max(line_range.start);
                        let end = style_run_range.end.min(line_range.end);
                        let run_range = start..end;

                        if idx >= run_range.start && idx <= run_range.end {
                            let mut local_x = glyph_run.offset();
                            if idx == run_range.start {
                                x_pos = local_x;
                                break;
                            }
                            let mut current_char_idx = run_range.start;
                            for glyph in glyph_run.glyphs() {
                                if current_char_idx >= idx {
                                    break;
                                }
                                local_x += glyph.advance;
                                current_char_idx = Self::next_char_boundary(text, current_char_idx)
                                    .min(run_range.end);
                            }
                            x_pos = local_x;
                        } else if idx > run_range.end {
                            x_pos = glyph_run.offset() + glyph_run.advance();
                        }
                    }
                }

                let metrics = line.metrics();
                let line_height = metrics
                    .line_height
                    .max(metrics.ascent + metrics.descent)
                    .max(1.0);
                let (top_trim, bottom_trim) = paragraph_line_trim(
                    &line,
                    paragraph.text_height_behavior,
                    i == 0,
                    is_last_line,
                );
                let visual_line_height = (line_height - top_trim - bottom_trim).max(1.0);
                let baseline_y = metrics.baseline;
                let visual_bounds =
                    paragraph_line_visual_bounds(&line).unwrap_or(ParagraphLineVisualBounds {
                        left: metrics.offset,
                        right: metrics.offset + metrics.advance,
                    });
                x_pos += visual_bounds.left - metrics.offset;

                let top_y = baseline_y - metrics.ascent;
                let caret_draw_height = caret_height
                    .unwrap_or(visual_line_height)
                    .clamp(1.0, visual_line_height.max(1.0));
                let caret_top = top_y - top_trim + ((visual_line_height - caret_draw_height) * 0.5);

                let caret_shape = RoundedRect::from_rect(
                    Rect::new(
                        position.x as f64 + x_pos as f64,
                        position.y as f64 + paragraph_y_offset as f64 + caret_top as f64,
                        position.x as f64 + x_pos as f64 + caret_width as f64,
                        position.y as f64
                            + paragraph_y_offset as f64
                            + caret_top as f64
                            + caret_draw_height as f64,
                    ),
                    caret_radius.unwrap_or(0.0).max(0.0) as f64,
                );

                self.scene.fill(
                    Fill::NonZero,
                    self.current_transform,
                    Color::from_rgba8(caret_color.r, caret_color.g, caret_color.b, caret_color.a),
                    None,
                    &caret_shape,
                );
                caret_drawn = true;
                break;
            }
        }
        if !caret_drawn && idx == 0 && text.is_empty() {
            let mut top_y = position.y as f64;
            let mut height = paragraph
                .strut_line_height
                .unwrap_or(base_size * 1.2)
                .max(1.0) as f64;
            if let Some(line) = layout.lines().next() {
                let metrics = line.metrics();
                top_y = position.y as f64
                    + paragraph_y_offset as f64
                    + (metrics.baseline - metrics.ascent) as f64;
                height = metrics
                    .line_height
                    .max(metrics.ascent + metrics.descent)
                    .max(1.0) as f64;
            }
            let draw_height = caret_height
                .unwrap_or(height as f32)
                .clamp(1.0, height as f32) as f64;
            let caret_top = top_y + ((height - draw_height) * 0.5);
            let caret_shape = RoundedRect::from_rect(
                Rect::new(
                    position.x as f64,
                    caret_top,
                    position.x as f64 + caret_width as f64,
                    caret_top + draw_height,
                ),
                caret_radius.unwrap_or(0.0).max(0.0) as f64,
            );
            self.scene.fill(
                Fill::NonZero,
                self.current_transform,
                Color::from_rgba8(caret_color.r, caret_color.g, caret_color.b, caret_color.a),
                None,
                &caret_shape,
            );
        }
    }
}
