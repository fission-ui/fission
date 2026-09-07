use anyhow::{anyhow, Result};
use fission_ir::op::{
    FontStyle, TextAlign, TextDirection, TextHeightBehavior, TextOverflow, TextParagraphStyle,
    TextWidthBasis,
};
use fission_layout::{LineMetric, TextMeasurer};
use fission_render::{
    Color as RenderColor, Fill, LayoutPoint, LayoutRect, ResolvedParagraphLayout, TextRun,
};
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle as FontdueTextStyle};
use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;
use tiny_skia::{FillRule as TinyFillRule, Mask, Pixmap, PixmapPaint, Transform};

use super::{fill_paint, rect_path, rgba_to_premul, SoftwareRenderer};
use crate::software_fonts::{default_font, packaged_font};

#[derive(Clone)]
struct SoftwareTextPaintRun {
    range: Range<usize>,
    font: Option<Arc<fontdue::Font>>,
    font_size: f32,
    color: RenderColor,
    underline: bool,
    resolved_font_eligible: bool,
}

impl SoftwareTextPaintRun {
    fn font(&self) -> &fontdue::Font {
        match self.font.as_deref() {
            Some(font) => font,
            None => default_font(),
        }
    }
}

fn paint_run_for_range<'a>(
    runs: &'a [SoftwareTextPaintRun],
    range: &Range<usize>,
) -> Option<&'a SoftwareTextPaintRun> {
    let index = runs.partition_point(|run| run.range.end <= range.start);
    runs.get(index)
        .filter(|run| run.range.start < range.end && run.range.end > range.start)
        .or_else(|| index.checked_sub(1).and_then(|index| runs.get(index)))
        .or_else(|| runs.last())
}

fn ellipsis_paint_run_for_range<'a>(
    runs: &'a [SoftwareTextPaintRun],
    range: &Range<usize>,
) -> Option<&'a SoftwareTextPaintRun> {
    let index = runs.partition_point(|run| run.range.start < range.end);
    index
        .checked_sub(1)
        .and_then(|index| runs.get(index))
        .filter(|run| run.range.end > range.start)
        .or_else(|| runs.last())
}

fn paragraph_geometry_is_resolved(paragraph: TextParagraphStyle) -> bool {
    paragraph.text_align == TextAlign::Start
        && paragraph.text_direction == TextDirection::Auto
        && paragraph.text_width_basis == TextWidthBasis::Parent
        && paragraph.strut_line_height.is_none()
        && paragraph.text_height_behavior == TextHeightBehavior::default()
        && paragraph.overflow != TextOverflow::Fade
}

fn resolved_paint_is_eligible(
    text: &str,
    layout: &ResolvedParagraphLayout,
    bounds: LayoutRect,
    wrap: bool,
    paragraph: TextParagraphStyle,
    runs: &[SoftwareTextPaintRun],
) -> bool {
    // The resolved snapshot carries positions and font-scoped glyph IDs, but not
    // the selected font face or paragraph alignment inputs. Use it only when the
    // bundled default face and default paragraph geometry can be verified. Every
    // other shape keeps the pre-existing source-layout fallback.
    if !wrap
        || !paragraph_geometry_is_resolved(paragraph)
        || runs.len() != 1
        || runs[0].range != (0..text.len())
        || !runs[0].resolved_font_eligible
        || !layout.inline_boxes.is_empty()
        || layout.clusters.iter().any(|cluster| cluster.is_rtl)
    {
        return false;
    }

    if text
        .chars()
        .filter(|character| !character.is_control())
        .any(|character| runs[0].font().lookup_glyph_index(character) == 0)
    {
        return false;
    }

    if layout.clusters.iter().any(|cluster| {
        cluster.line_index >= layout.lines.len()
            || cluster.start_index > cluster.end_index
            || cluster.end_index > text.len()
            || !text.is_char_boundary(cluster.start_index)
            || !text.is_char_boundary(cluster.end_index)
    }) || layout.glyphs.iter().any(|glyph| {
        let Some(cluster) = layout.clusters.get(glyph.cluster_index) else {
            return true;
        };
        let Some(cluster_text) = text.get(cluster.start_index..cluster.end_index) else {
            return true;
        };
        let mut characters = cluster_text.chars();
        let Some(character) = characters.next() else {
            return true;
        };
        character.is_control()
            || characters.next().is_some()
            || u16::try_from(glyph.id).is_err()
            || u32::from(runs[0].font().lookup_glyph_index(character)) != glyph.id
    }) {
        return false;
    }

    let visible_lines = visible_resolved_lines(layout, paragraph);
    let Some(last_visible_line) = visible_lines.checked_sub(1) else {
        return true;
    };
    let line_right = resolved_line_visual_right(layout, last_visible_line);
    let horizontal_ellipsis = paragraph.overflow == TextOverflow::Ellipsis
        && bounds.width() > 0.0
        && line_right > bounds.width();
    !horizontal_ellipsis
}

fn visible_resolved_lines(
    layout: &ResolvedParagraphLayout,
    paragraph: TextParagraphStyle,
) -> usize {
    paragraph
        .max_lines
        .map(|max_lines| max_lines.min(layout.lines.len()))
        .unwrap_or(layout.lines.len())
}

fn resolved_line_visual_right(layout: &ResolvedParagraphLayout, line_index: usize) -> f32 {
    layout
        .clusters
        .iter()
        .filter(|cluster| cluster.line_index == line_index)
        .map(|cluster| cluster.rect.right())
        .reduce(f32::max)
        .unwrap_or_else(|| {
            layout
                .lines
                .get(line_index)
                .map(|line| line.width)
                .unwrap_or(0.0)
        })
}

fn wrap_max_width(bounds_width: f32, font_size: f32, wrap: bool) -> Option<f32> {
    if !wrap || bounds_width <= 0.0 {
        return None;
    }
    // The retained text bounds track ink-box width more closely than advance width.
    // Give the software layout a small amount of slack so short labels do not wrap
    // spuriously when their final advance slightly exceeds the reported bounds.
    Some(bounds_width.ceil() + font_size * 0.5)
}

fn pipeline_wrap_breaks(
    measurer: Option<&dyn TextMeasurer>,
    text: &str,
    font_size: f32,
    bounds_width: f32,
    wrap: bool,
) -> Option<Vec<usize>> {
    if !wrap || bounds_width <= 0.0 {
        return None;
    }
    let measurer = measurer?;
    let lines = measurer.get_line_metrics(text, font_size, Some(bounds_width));
    if lines.is_empty() {
        return None;
    }
    Some(soft_wrap_breaks(text, &lines))
}

fn soft_wrap_breaks(text: &str, lines: &[LineMetric]) -> Vec<usize> {
    lines
        .windows(2)
        .filter_map(|pair| {
            let end = pair[0].end_index.min(text.len());
            let next_start = pair[1].start_index.min(text.len());
            if !text.is_char_boundary(end) || !text.is_char_boundary(next_start) {
                return None;
            }
            let gap_start = end.min(next_start);
            let gap_end = end.max(next_start);
            let already_broken = text[..end].ends_with('\r')
                || text[..end].ends_with('\n')
                || text[end..].starts_with('\r')
                || text[end..].starts_with('\n')
                || text[gap_start..gap_end]
                    .chars()
                    .any(|character| matches!(character, '\r' | '\n'));
            (!already_broken).then_some(end)
        })
        .collect()
}

fn insert_soft_wraps<'a>(text: &'a str, breaks: &[usize]) -> Cow<'a, str> {
    if breaks.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut wrapped = String::with_capacity(text.len() + breaks.len());
    let mut cursor = 0;
    for &break_at in breaks {
        if break_at < cursor || break_at > text.len() || !text.is_char_boundary(break_at) {
            continue;
        }
        wrapped.push_str(&text[cursor..break_at]);
        wrapped.push('\n');
        cursor = break_at;
    }
    wrapped.push_str(&text[cursor..]);
    Cow::Owned(wrapped)
}

impl SoftwareRenderer {
    pub(super) fn draw_text(
        &mut self,
        text: &str,
        position: LayoutPoint,
        size: f32,
        color: RenderColor,
        bounds: LayoutRect,
        wrap: bool,
        underline: bool,
        paragraph_style: Option<TextParagraphStyle>,
        resolved_layout: Option<&ResolvedParagraphLayout>,
    ) -> Result<()> {
        let paragraph = paragraph_style.unwrap_or_default();
        if paragraph.max_lines == Some(0) {
            return Ok(());
        }
        if let Some(resolved_layout) = resolved_layout {
            let paint_runs = [SoftwareTextPaintRun {
                range: 0..text.len(),
                font: None,
                font_size: size,
                color,
                underline,
                resolved_font_eligible: true,
            }];
            if self.draw_resolved_paragraph(
                text,
                resolved_layout,
                position,
                bounds,
                wrap,
                paragraph,
                &paint_runs,
            )? {
                return Ok(());
            }
        }

        let font = default_font();
        let fonts = [font];
        let pipeline_breaks = pipeline_wrap_breaks(
            self.text_measurer.as_deref(),
            text,
            size,
            bounds.width(),
            wrap,
        );
        let layout_text = pipeline_breaks
            .as_deref()
            .map(|breaks| insert_soft_wraps(text, breaks))
            .unwrap_or(Cow::Borrowed(text));
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position.x,
            y: position.y,
            max_width: if pipeline_breaks.is_some() {
                None
            } else {
                wrap_max_width(bounds.width(), size, wrap)
            },
            ..LayoutSettings::default()
        });
        layout.append(&fonts, &FontdueTextStyle::new(&layout_text, size, 0));
        self.draw_glyphs(&layout, |_, _| color)?;
        if underline {
            self.draw_layout_underlines(&layout, color, size)?;
        }
        Ok(())
    }

    pub(super) fn draw_rich_text(
        &mut self,
        runs: &[TextRun],
        position: LayoutPoint,
        bounds: LayoutRect,
        wrap: bool,
        paragraph_style: Option<TextParagraphStyle>,
        resolved_layout: Option<&ResolvedParagraphLayout>,
    ) -> Result<()> {
        let paragraph = paragraph_style.unwrap_or_default();
        if paragraph.max_lines == Some(0) {
            return Ok(());
        }
        let full_text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        let sole_default_run = runs.len() == 1
            && !runs[0].text.is_empty()
            && runs[0].style.font_family.is_none()
            && runs[0].style.font_weight == 400
            && runs[0].style.font_style == FontStyle::Normal
            && runs[0].style.background_color.is_none()
            && runs[0].style.typography == Default::default();
        let mut text_cursor = 0;
        let paint_runs = runs
            .iter()
            .map(|run| {
                let start = text_cursor;
                text_cursor += run.text.len();
                SoftwareTextPaintRun {
                    range: start..text_cursor,
                    font: packaged_font(
                        run.style.font_family.as_deref(),
                        run.style.font_weight,
                        run.style.font_style,
                    ),
                    font_size: run.style.font_size,
                    color: run.style.color,
                    underline: run.style.underline,
                    resolved_font_eligible: sole_default_run,
                }
            })
            .collect::<Vec<_>>();

        if let Some(resolved_layout) = resolved_layout {
            if self.draw_resolved_paragraph(
                &full_text,
                resolved_layout,
                position,
                bounds,
                wrap,
                paragraph,
                &paint_runs,
            )? {
                return Ok(());
            }
        }

        let fonts = paint_runs
            .iter()
            .map(SoftwareTextPaintRun::font)
            .collect::<Vec<_>>();
        let base_size = runs.first().map(|run| run.style.font_size).unwrap_or(14.0);
        let pipeline_breaks = pipeline_wrap_breaks(
            self.text_measurer.as_deref(),
            &full_text,
            base_size,
            bounds.width(),
            wrap,
        );
        let mut break_cursor = 0;
        let mut text_cursor = 0;
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position.x,
            y: position.y,
            max_width: if pipeline_breaks.is_some() {
                None
            } else {
                wrap_max_width(bounds.width(), base_size, wrap)
            },
            ..LayoutSettings::default()
        });
        for (font_index, run) in runs.iter().enumerate() {
            let run_start = text_cursor;
            let run_end = run_start + run.text.len();
            let rendered_text = if let Some(breaks) = pipeline_breaks.as_deref() {
                while break_cursor < breaks.len() && breaks[break_cursor] < run_start {
                    break_cursor += 1;
                }
                let first_break = break_cursor;
                while break_cursor < breaks.len() && breaks[break_cursor] <= run_end {
                    break_cursor += 1;
                }
                let local_breaks = breaks[first_break..break_cursor]
                    .iter()
                    .map(|break_at| break_at - run_start)
                    .collect::<Vec<_>>();
                insert_soft_wraps(&run.text, &local_breaks)
            } else {
                Cow::Borrowed(run.text.as_str())
            };
            layout.append(
                &fonts,
                &fontdue::layout::TextStyle::with_user_data(
                    &rendered_text,
                    run.style.font_size,
                    font_index,
                    (
                        run.style.color,
                        run.style.underline,
                        run.style.background_color,
                    ),
                ),
            );
            text_cursor = run_end;
        }
        self.draw_glyphs(&layout, |glyph, (color, _underline, bg)| {
            if bg.is_some() {
                let _ = glyph;
            }
            *color
        })?;
        if let Some(lines) = layout.lines() {
            for line in lines {
                for glyph in &layout.glyphs()[line.glyph_start..=line.glyph_end] {
                    let (color, underline, _) = glyph.user_data;
                    if underline {
                        let underline_rect = LayoutRect::new(
                            glyph.x,
                            line.baseline_y + 1.5,
                            glyph.width as f32,
                            (glyph.key.px / 14.0).max(1.0),
                        );
                        self.draw_rect(underline_rect, Some(&Fill::Solid(color)), None, 0.0, None)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_resolved_paragraph(
        &mut self,
        text: &str,
        layout: &ResolvedParagraphLayout,
        position: LayoutPoint,
        bounds: LayoutRect,
        wrap: bool,
        paragraph: TextParagraphStyle,
        runs: &[SoftwareTextPaintRun],
    ) -> Result<bool> {
        if layout.lines.is_empty()
            || layout.glyphs.is_empty()
            || !resolved_paint_is_eligible(text, layout, bounds, wrap, paragraph, runs)
        {
            return Ok(false);
        }

        // Parley shaped these glyphs while laying out the retained tree. Fontdue
        // rounds advances independently, so laying the source out again can move
        // the end of one resolved line through its clip. Rasterize at the retained
        // positions and keep line visibility/ellipsis as paint-only decisions.
        let visible_lines = visible_resolved_lines(layout, paragraph);
        if visible_lines == 0 {
            return Ok(true);
        }

        let last_visible_line = visible_lines - 1;
        let line = &layout.lines[last_visible_line];
        let line_right = resolved_line_visual_right(layout, last_visible_line);
        let has_more_lines = visible_lines < layout.lines.len();
        let overflows_horizontally = bounds.width() > 0.0 && line_right > bounds.width();
        let ellipsis = (paragraph.overflow == TextOverflow::Ellipsis
            && (has_more_lines || overflows_horizontally))
            .then(|| {
                let run = ellipsis_paint_run_for_range(runs, &(line.start_index..line.end_index))
                    .cloned()
                    .unwrap_or_else(|| runs[runs.len() - 1].clone());
                let width = self.ellipsis_advance(&run);
                let right = if bounds.width() > 0.0 {
                    line_right.min(bounds.width()).max(0.0)
                } else {
                    line_right.max(0.0)
                };
                ((right - width).max(0.0), width, run)
            });

        if let Some((ellipsis_left, _, _)) = ellipsis.as_ref() {
            if last_visible_line > 0 {
                self.draw_resolved_line_range(layout, 0..last_visible_line, position, runs)?;
            }
            let clip_right = position.x + *ellipsis_left;
            let clip_rect = LayoutRect::new(
                bounds.x(),
                bounds.y(),
                (clip_right - bounds.x()).max(0.0),
                bounds.height(),
            );
            self.with_temporary_clip_rect(clip_rect, |this| {
                this.draw_resolved_line_range(
                    layout,
                    last_visible_line..visible_lines,
                    position,
                    runs,
                )
            })?;
        } else {
            self.draw_resolved_line_range(layout, 0..visible_lines, position, runs)?;
        }

        if let Some((ellipsis_left, width, run)) = ellipsis {
            self.with_temporary_clip_rect(bounds, |this| {
                this.draw_resolved_ellipsis(
                    position.x + ellipsis_left,
                    position.y + line.baseline,
                    width,
                    &run,
                )
            })?;
        }

        Ok(true)
    }

    fn draw_resolved_line_range(
        &mut self,
        layout: &ResolvedParagraphLayout,
        line_range: Range<usize>,
        position: LayoutPoint,
        runs: &[SoftwareTextPaintRun],
    ) -> Result<()> {
        let transform = self.current_device_transform();
        let clip = self.current_clip().cloned();
        for glyph in &layout.glyphs {
            let Some(cluster) = layout.clusters.get(glyph.cluster_index) else {
                continue;
            };
            if !line_range.contains(&cluster.line_index) {
                continue;
            }
            let Some(run) = paint_run_for_range(runs, &(cluster.start_index..cluster.end_index))
            else {
                continue;
            };
            self.draw_resolved_glyph_bitmap(
                run.font(),
                glyph.id,
                run.font_size,
                LayoutPoint::new(position.x + glyph.position.x, position.y + glyph.position.y),
                run.color,
                transform,
                clip.as_ref(),
            )?;
        }

        let mut underlines = Vec::new();
        for cluster in layout
            .clusters
            .iter()
            .filter(|cluster| line_range.contains(&cluster.line_index))
        {
            let Some(run) = paint_run_for_range(runs, &(cluster.start_index..cluster.end_index))
            else {
                continue;
            };
            if !run.underline || cluster.rect.width() <= 0.0 {
                continue;
            }
            let Some(line) = layout.lines.get(cluster.line_index) else {
                continue;
            };
            underlines.push((
                LayoutRect::new(
                    position.x + cluster.rect.x(),
                    position.y + line.baseline + 1.5,
                    cluster.rect.width(),
                    (run.font_size / 14.0).max(1.0),
                ),
                run.color,
            ));
        }
        self.draw_resolved_underlines(&underlines, transform, clip.as_ref());
        Ok(())
    }

    fn draw_resolved_underlines(
        &mut self,
        underlines: &[(LayoutRect, RenderColor)],
        transform: Transform,
        clip: Option<&Mask>,
    ) {
        let surface = self.current_surface_mut();
        for (rect, color) in underlines {
            let Some(path) = rect_path(*rect) else {
                continue;
            };
            surface.fill_path(
                &path,
                &fill_paint(&Fill::Solid(*color), *rect),
                TinyFillRule::Winding,
                transform,
                clip,
            );
        }
    }

    fn ellipsis_advance(&self, run: &SoftwareTextPaintRun) -> f32 {
        let font = run.font();
        let glyph = font.lookup_glyph_index('.');
        (font.metrics_indexed(glyph, run.font_size).advance_width * 3.0).max(1.0)
    }

    fn draw_resolved_ellipsis(
        &mut self,
        x: f32,
        baseline: f32,
        width: f32,
        run: &SoftwareTextPaintRun,
    ) -> Result<()> {
        let font = run.font();
        let glyph = u32::from(font.lookup_glyph_index('.'));
        let advance = width / 3.0;
        let transform = self.current_device_transform();
        let clip = self.current_clip().cloned();
        for index in 0..3 {
            self.draw_resolved_glyph_bitmap(
                font,
                glyph,
                run.font_size,
                LayoutPoint::new(x + advance * index as f32, baseline),
                run.color,
                transform,
                clip.as_ref(),
            )?;
        }
        if run.underline {
            self.draw_rect(
                LayoutRect::new(x, baseline + 1.5, width, (run.font_size / 14.0).max(1.0)),
                Some(&Fill::Solid(run.color)),
                None,
                0.0,
                None,
            )?;
        }
        Ok(())
    }

    fn draw_resolved_glyph_bitmap(
        &mut self,
        font: &fontdue::Font,
        glyph_index: u32,
        font_size: f32,
        baseline: LayoutPoint,
        color: RenderColor,
        transform: Transform,
        clip: Option<&Mask>,
    ) -> Result<()> {
        let Ok(glyph_index) = u16::try_from(glyph_index) else {
            return Ok(());
        };
        let uniform_device_scale = transform.is_scale_translate()
            && transform.sx > 0.0
            && transform.sy > 0.0
            && (transform.sx - transform.sy).abs() < 0.01;
        let px = if uniform_device_scale {
            (font_size * transform.sx).max(1.0)
        } else {
            font_size.max(1.0)
        };
        let (metrics, bitmap) = font.rasterize_indexed(glyph_index, px);
        if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
            return Ok(());
        }

        let (draw_x, draw_y, draw_transform) = if uniform_device_scale {
            let baseline_x = baseline.x * transform.sx + transform.tx;
            let baseline_y = baseline.y * transform.sy + transform.ty;
            (
                baseline_x.round() as i32 + metrics.xmin,
                baseline_y.round() as i32 - metrics.ymin - metrics.height as i32,
                Transform::identity(),
            )
        } else {
            (
                (baseline.x + metrics.xmin as f32).round() as i32,
                (baseline.y - metrics.ymin as f32 - metrics.height as f32).round() as i32,
                transform,
            )
        };

        let mut rgba = Vec::with_capacity(metrics.width * metrics.height * 4);
        for coverage in bitmap {
            let premul = rgba_to_premul(color, coverage);
            rgba.extend_from_slice(&[premul.red(), premul.green(), premul.blue(), premul.alpha()]);
        }
        let size = tiny_skia::IntSize::from_wh(metrics.width as u32, metrics.height as u32)
            .ok_or_else(|| anyhow!("invalid glyph pixmap size"))?;
        let pixmap =
            Pixmap::from_vec(rgba, size).ok_or_else(|| anyhow!("failed to create glyph pixmap"))?;
        self.current_surface_mut().draw_pixmap(
            draw_x,
            draw_y,
            pixmap.as_ref(),
            &PixmapPaint::default(),
            draw_transform,
            clip,
        );
        Ok(())
    }

    fn draw_layout_underlines<U: Copy + Clone>(
        &mut self,
        layout: &Layout<U>,
        color: RenderColor,
        size: f32,
    ) -> Result<()> {
        if let Some(lines) = layout.lines() {
            for line in lines {
                if line.glyph_start > line.glyph_end || line.glyph_end >= layout.glyphs().len() {
                    continue;
                }
                let first = &layout.glyphs()[line.glyph_start];
                let last = &layout.glyphs()[line.glyph_end];
                let underline_rect = LayoutRect::new(
                    first.x,
                    line.baseline_y + 1.5,
                    (last.x + last.width as f32 - first.x).max(1.0),
                    (size / 14.0).max(1.0),
                );
                self.draw_rect(underline_rect, Some(&Fill::Solid(color)), None, 0.0, None)?;
            }
        }
        Ok(())
    }

    fn draw_glyphs<U: Copy + Clone>(
        &mut self,
        layout: &Layout<U>,
        color_for: impl Fn(&fontdue::layout::GlyphPosition<U>, &U) -> RenderColor,
    ) -> Result<()> {
        let font = default_font();
        let transform = self.current_device_transform();
        let clip = self.current_clip().cloned();
        let surface = self.current_surface_mut();

        for glyph in layout.glyphs() {
            if glyph.width == 0 || glyph.height == 0 {
                continue;
            }
            let color = color_for(glyph, &glyph.user_data);
            let (draw_x, draw_y, px, draw_transform) = if transform.is_scale_translate()
                && transform.sx > 0.0
                && transform.sy > 0.0
                && (transform.sx - transform.sy).abs() < 0.01
            {
                (
                    (glyph.x * transform.sx + transform.tx).round() as i32,
                    (glyph.y * transform.sy + transform.ty).round() as i32,
                    (glyph.key.px * transform.sx).max(1.0),
                    Transform::identity(),
                )
            } else {
                (
                    glyph.x.round() as i32,
                    glyph.y.round() as i32,
                    glyph.key.px,
                    transform,
                )
            };
            let (metrics, bitmap) = font.rasterize_indexed(glyph.key.glyph_index, px);
            if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
                continue;
            }

            let mut rgba = Vec::with_capacity(metrics.width * metrics.height * 4);
            for coverage in bitmap {
                let premul = rgba_to_premul(color, coverage);
                rgba.extend_from_slice(&[
                    premul.red(),
                    premul.green(),
                    premul.blue(),
                    premul.alpha(),
                ]);
            }
            let size = tiny_skia::IntSize::from_wh(metrics.width as u32, metrics.height as u32)
                .ok_or_else(|| anyhow!("invalid glyph pixmap size"))?;
            let pixmap = Pixmap::from_vec(rgba, size)
                .ok_or_else(|| anyhow!("failed to create glyph pixmap"))?;
            surface.draw_pixmap(
                draw_x,
                draw_y,
                pixmap.as_ref(),
                &PixmapPaint::default(),
                draw_transform,
                clip.as_ref(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_layout::{LineMetric, TextMeasurer};
    use fission_render::{DisplayList, DisplayOp, RenderScene, TextRun};
    use std::sync::Mutex;

    const NARROW_SPANISH_COPY: &str = "El comité de vecinos revisó con calma el sitio disponible y ordenó los materiales para la jornada común.";
    const TEXT_WIDTH: f32 = 342.0;
    const TEXT_HEIGHT: f32 = 36.4;
    const FONT_SIZE: f32 = 13.0;
    const LINE_HEIGHT: f32 = 18.2;
    const SURFACE_HEIGHT: u32 = 37;

    fn resolved_ascii_layout(
        text: &str,
        line_ranges: &[Range<usize>],
        baselines: &[f32],
        glyph_advance: f32,
    ) -> ResolvedParagraphLayout {
        assert_eq!(line_ranges.len(), baselines.len());
        let font = default_font();
        let mut clusters = Vec::new();
        let mut glyphs = Vec::new();
        let mut lines = Vec::new();

        for (line_index, (range, baseline)) in line_ranges.iter().zip(baselines.iter()).enumerate()
        {
            let mut x = 0.0;
            for (offset, character) in text[range.clone()].char_indices() {
                if character == '\n' {
                    continue;
                }
                let start = range.start + offset;
                let end = start + character.len_utf8();
                let cluster_index = clusters.len();
                clusters.push(fission_layout::ParagraphCluster {
                    start_index: start,
                    end_index: end,
                    line_index,
                    rect: LayoutRect::new(x, *baseline - 12.0, glyph_advance, 16.0),
                    is_rtl: false,
                });
                glyphs.push(fission_layout::ParagraphGlyph {
                    id: u32::from(font.lookup_glyph_index(character)),
                    style_index: 0,
                    cluster_index,
                    position: LayoutPoint::new(x, *baseline),
                    advance: glyph_advance,
                });
                x += glyph_advance;
            }
            lines.push(LineMetric {
                start_index: range.start,
                end_index: range.end,
                baseline: *baseline,
                height: 18.0,
                width: x,
            });
        }

        ResolvedParagraphLayout {
            constraint_width: Some(40.0),
            size: fission_render::LayoutSize::new(40.0, 54.0),
            lines,
            inline_boxes: Vec::new(),
            clusters,
            glyphs,
            caret_stops: Vec::new(),
            selection_boxes: Vec::new(),
        }
    }

    fn transparent() -> RenderColor {
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }

    fn text_color() -> RenderColor {
        RenderColor {
            r: 20,
            g: 40,
            b: 60,
            a: 255,
        }
    }

    fn region_has_ink(
        pixels: &[u8],
        surface_width: usize,
        x: Range<usize>,
        mut y: Range<usize>,
    ) -> bool {
        y.any(|row| {
            x.clone()
                .any(|column| pixels[(row * surface_width + column) * 4 + 3] > 0)
        })
    }

    struct SingleLineMeasurer;

    impl TextMeasurer for SingleLineMeasurer {
        fn measure(&self, text: &str, font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
            (text.len() as f32 * font_size, font_size * 1.2)
        }

        fn get_line_metrics(
            &self,
            text: &str,
            font_size: f32,
            _available_width: Option<f32>,
        ) -> Vec<LineMetric> {
            vec![LineMetric {
                start_index: 0,
                end_index: text.len(),
                baseline: font_size,
                height: font_size * 1.2,
                width: text.len() as f32 * font_size,
            }]
        }
    }

    #[test]
    fn software_text_does_not_rewrap_pipeline_single_line_layouts() {
        let bounds = LayoutRect::new(0.0, 0.0, 24.0, 24.0);
        let mut display_list = DisplayList::new(LayoutRect::new(0.0, 0.0, 260.0, 80.0));
        display_list.push(DisplayOp::DrawText {
            text: "Secure local storage required".into(),
            position: bounds.origin,
            size: 20.0,
            color: text_color(),
            bounds,
            node_id: None,
            underline: false,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
            resolved_layout: None,
        });

        let pixels = SoftwareRenderer::render_with_text_measurer(
            &RenderScene::from_display_list(display_list),
            260,
            80,
            transparent(),
            1.0,
            Arc::new(SingleLineMeasurer),
        )
        .expect("render pipeline-shaped single line");
        let row_has_ink = |y: usize| {
            pixels[y * 260 * 4..(y + 1) * 260 * 4]
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0)
        };

        assert!((0..24).any(|y| row_has_ink(y)));
        assert!(
            !(30..80).any(|y| row_has_ink(y)),
            "fontdue must not add lines below the pipeline's one-line bounds"
        );
    }

    #[test]
    fn software_rich_text_does_not_rewrap_pipeline_single_line_layouts() {
        let bounds = LayoutRect::new(0.0, 0.0, 24.0, 20.0);
        let mut display_list = DisplayList::new(LayoutRect::new(0.0, 0.0, 220.0, 70.0));
        let mut run = render_run("Enable secure storage", 14.0, 16.8);
        run.style.font_weight = 600;
        run.style.line_height = None;
        display_list.push(DisplayOp::DrawRichText {
            runs: vec![run],
            position: bounds.origin,
            bounds,
            node_id: None,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
            annotations: Vec::new(),
            resolved_layout: None,
        });

        let pixels = SoftwareRenderer::render_with_text_measurer(
            &RenderScene::from_display_list(display_list),
            220,
            70,
            transparent(),
            1.0,
            Arc::new(SingleLineMeasurer),
        )
        .expect("render pipeline-shaped rich-text line");
        let row_has_ink = |y: usize| {
            pixels[y * 220 * 4..(y + 1) * 220 * 4]
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0)
        };

        assert!((0..20).any(|y| row_has_ink(y)));
        assert!(
            !(26..70).any(|y| row_has_ink(y)),
            "fontdue must not rewrap a pipeline-shaped rich-text label"
        );
    }

    #[test]
    fn software_rich_text_paints_at_authoritative_resolved_glyph_positions() {
        let text = "ii";
        let mut resolved = resolved_ascii_layout(text, &[0..text.len()], &[15.0], 24.0);
        resolved.constraint_width = Some(48.0);
        resolved.size = fission_render::LayoutSize::new(48.0, 20.0);
        let bounds = LayoutRect::new(0.0, 0.0, 48.0, 20.0);
        let mut display_list = DisplayList::new(bounds);
        display_list.push(DisplayOp::DrawRichText {
            runs: vec![render_run(text, 14.0, 18.0)],
            position: bounds.origin,
            bounds,
            node_id: None,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
            annotations: Vec::new(),
            resolved_layout: Some(resolved),
        });

        let pixels = SoftwareRenderer::render(
            &RenderScene::from_display_list(display_list),
            48,
            20,
            transparent(),
            1.0,
        )
        .expect("render authoritative rich text positions");

        assert!(
            region_has_ink(&pixels, 48, 23..32, 0..20),
            "the second glyph must paint at the resolved x position instead of fontdue's independent advance"
        );
        assert!(
            !region_has_ink(&pixels, 48, 8..20, 0..20),
            "no independently laid-out glyph may appear between the resolved positions"
        );
    }

    #[test]
    fn nowrap_text_does_not_paint_a_bounded_resolved_wrap() {
        let text = "aaaa aaaa";
        let resolved = resolved_ascii_layout(text, &[0..5, 5..9], &[14.0, 32.0], 8.0);
        let bounds = LayoutRect::new(0.0, 0.0, 40.0, 60.0);
        let mut display_list = DisplayList::new(bounds);
        display_list.push(DisplayOp::DrawText {
            text: text.into(),
            position: bounds.origin,
            size: 14.0,
            color: text_color(),
            bounds,
            node_id: None,
            underline: false,
            wrap: false,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
            resolved_layout: Some(resolved),
        });

        let pixels = SoftwareRenderer::render(
            &RenderScene::from_display_list(display_list),
            80,
            60,
            transparent(),
            1.0,
        )
        .expect("render nowrap text with a bounded snapshot");

        assert!(region_has_ink(&pixels, 80, 0..80, 0..20));
        assert!(
            !region_has_ink(&pixels, 80, 0..80, 22..60),
            "nowrap paint must keep the legacy unwrapped line instead of consuming a bounded resolved wrap"
        );
    }

    #[test]
    fn zero_max_lines_paints_nothing_before_resolved_or_legacy_layout() {
        let bounds = LayoutRect::new(0.0, 0.0, 100.0, 30.0);
        let paragraph = Some(TextParagraphStyle {
            max_lines: Some(0),
            overflow: TextOverflow::Ellipsis,
            ..Default::default()
        });

        let mut plain = DisplayList::new(bounds);
        plain.push(DisplayOp::DrawText {
            text: "plain source".into(),
            position: bounds.origin,
            size: 14.0,
            color: text_color(),
            bounds,
            node_id: None,
            underline: false,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: paragraph,
            resolved_layout: None,
        });

        let mut rejected_rich = DisplayList::new(bounds);
        let mut run = render_run("rich source", 14.0, 18.0);
        run.style.font_weight = 600;
        rejected_rich.push(DisplayOp::DrawRichText {
            runs: vec![run],
            position: bounds.origin,
            bounds,
            node_id: None,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: paragraph,
            annotations: Vec::new(),
            resolved_layout: None,
        });

        for display_list in [plain, rejected_rich] {
            let pixels = SoftwareRenderer::render(
                &RenderScene::from_display_list(display_list),
                100,
                30,
                transparent(),
                1.0,
            )
            .expect("render zero-line paragraph");
            assert!(
                pixels.chunks_exact(4).all(|pixel| pixel[3] == 0),
                "max_lines=0 must suppress both resolved and fallback text paint"
            );
        }
    }

    fn render_three_line_paragraph_in_bounds(
        overflow: TextOverflow,
        max_lines: Option<usize>,
        bounds_height: f32,
    ) -> Vec<u8> {
        let text = "aaaa\naaaa\naaaa";
        let resolved =
            resolved_ascii_layout(text, &[0..5, 5..10, 10..14], &[14.0, 32.0, 50.0], 8.0);
        let bounds = LayoutRect::new(0.0, 0.0, 40.0, bounds_height);
        let mut display_list = DisplayList::new(bounds);
        display_list.push(DisplayOp::DrawText {
            text: text.into(),
            position: bounds.origin,
            size: 14.0,
            color: text_color(),
            bounds,
            node_id: None,
            underline: false,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: Some(TextParagraphStyle {
                max_lines,
                overflow,
                ..Default::default()
            }),
            resolved_layout: Some(resolved),
        });
        SoftwareRenderer::render(
            &RenderScene::from_display_list(display_list),
            40,
            60,
            transparent(),
            1.0,
        )
        .expect("render resolved multiline paragraph")
    }

    fn render_three_line_paragraph(overflow: TextOverflow, max_lines: Option<usize>) -> Vec<u8> {
        render_three_line_paragraph_in_bounds(overflow, max_lines, 60.0)
    }

    #[test]
    fn software_text_honors_resolved_max_lines_and_last_line_ellipsis() {
        let uncapped = render_three_line_paragraph(TextOverflow::Clip, None);
        let clipped = render_three_line_paragraph(TextOverflow::Clip, Some(2));
        let ellipsized = render_three_line_paragraph(TextOverflow::Ellipsis, Some(2));

        assert!(
            region_has_ink(&uncapped, 40, 0..40, 38..58),
            "the resolved third line must be paintable without a line cap"
        );
        assert!(
            !region_has_ink(&clipped, 40, 0..40, 38..58)
                && !region_has_ink(&ellipsized, 40, 0..40, 38..58),
            "max_lines must prevent hidden resolved lines from painting even without an ancestor clip"
        );
        assert!(
            clipped[18 * 40 * 4..38 * 40 * 4] != ellipsized[18 * 40 * 4..38 * 40 * 4],
            "Ellipsis must replace the tail of only the last visible line when more lines exist"
        );

        let short_bounds =
            render_three_line_paragraph_in_bounds(TextOverflow::Ellipsis, Some(2), 18.0);
        assert!(
            !region_has_ink(&short_bounds, 40, 0..40, 18..60),
            "the ellipsis must remain clipped to the paragraph bounds"
        );
    }

    #[test]
    fn resolved_text_fast_path_rejects_unresolved_geometry_and_font_identity() {
        let text = "plain text";
        let layout = resolved_ascii_layout(text, &[0..text.len()], &[14.0], 7.0);
        let bounds = LayoutRect::new(0.0, 0.0, 100.0, 20.0);
        let eligible_run = SoftwareTextPaintRun {
            range: 0..text.len(),
            font: None,
            font_size: 14.0,
            color: text_color(),
            underline: false,
            resolved_font_eligible: true,
        };

        assert!(resolved_paint_is_eligible(
            text,
            &layout,
            bounds,
            true,
            TextParagraphStyle::default(),
            std::slice::from_ref(&eligible_run),
        ));
        assert!(!resolved_paint_is_eligible(
            text,
            &layout,
            bounds,
            true,
            TextParagraphStyle {
                text_align: TextAlign::Center,
                ..Default::default()
            },
            std::slice::from_ref(&eligible_run),
        ));
        assert!(!resolved_paint_is_eligible(
            text,
            &layout,
            bounds,
            false,
            TextParagraphStyle::default(),
            std::slice::from_ref(&eligible_run),
        ));

        let mut unknown_font_run = eligible_run.clone();
        unknown_font_run.resolved_font_eligible = false;
        assert!(!resolved_paint_is_eligible(
            text,
            &layout,
            bounds,
            true,
            TextParagraphStyle::default(),
            &[unknown_font_run],
        ));

        let mut rtl_layout = layout;
        rtl_layout.clusters[0].is_rtl = true;
        assert!(!resolved_paint_is_eligible(
            text,
            &rtl_layout,
            bounds,
            true,
            TextParagraphStyle::default(),
            &[eligible_run],
        ));
    }

    fn production_text_measurer() -> Arc<fission_render_vello::VelloTextMeasurer> {
        const DEFAULT_FONT_FAMILY: &str = "Fission Default";
        let font_cx = Arc::new(Mutex::new(fission_render_vello::parley::FontContext {
            collection: fontique::Collection::new(fontique::CollectionOptions {
                shared: false,
                system_fonts: false,
            }),
            source_cache: fontique::SourceCache::default(),
        }));
        font_cx.lock().unwrap().collection.register_fonts(
            fontique::Blob::from(fission_theme::fonts::default_font_bytes().to_vec()),
            Some(fontique::FontInfoOverride {
                family_name: Some(DEFAULT_FONT_FAMILY),
                ..Default::default()
            }),
        );
        Arc::new(
            fission_render_vello::VelloTextMeasurer::new_with_default_family(
                font_cx,
                DEFAULT_FONT_FAMILY,
            ),
        )
    }

    fn ir_run(text: &str, font_size: f32, line_height: f32) -> fission_ir::op::TextRun {
        fission_ir::op::TextRun {
            text: text.into(),
            style: fission_ir::op::TextStyle {
                font_size,
                color: fission_ir::op::Color::BLACK,
                underline: false,
                font_family: None,
                locale: None,
                font_weight: 400,
                font_style: fission_ir::op::FontStyle::Normal,
                line_height: Some(line_height),
                letter_spacing: 0.0,
                background_color: None,
                typography: Default::default(),
            },
        }
    }

    fn render_run(text: &str, font_size: f32, line_height: f32) -> TextRun {
        TextRun {
            text: text.into(),
            style: fission_render::TextStyle {
                font_size,
                color: text_color(),
                underline: false,
                font_family: None,
                locale: None,
                font_weight: 400,
                font_style: fission_ir::op::FontStyle::Normal,
                line_height: Some(line_height),
                letter_spacing: 0.0,
                background_color: None,
                typography: Default::default(),
            },
        }
    }

    fn render_narrow_spanish_copy(
        measurer: Arc<dyn TextMeasurer>,
        resolved: ResolvedParagraphLayout,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    ) -> Vec<u8> {
        render_narrow_spanish_copy_at_scale(measurer, resolved, max_lines, overflow, 1.0)
    }

    fn render_narrow_spanish_copy_at_scale(
        measurer: Arc<dyn TextMeasurer>,
        resolved: ResolvedParagraphLayout,
        max_lines: Option<usize>,
        overflow: TextOverflow,
        scale: f32,
    ) -> Vec<u8> {
        let bounds = LayoutRect::new(0.0, 0.0, TEXT_WIDTH, TEXT_HEIGHT);
        let mut display_list = DisplayList::new(bounds);
        display_list.push(DisplayOp::DrawRichText {
            runs: vec![render_run(NARROW_SPANISH_COPY, FONT_SIZE, LINE_HEIGHT)],
            position: bounds.origin,
            bounds,
            node_id: None,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: Some(TextParagraphStyle {
                max_lines,
                overflow,
                ..Default::default()
            }),
            annotations: Vec::new(),
            resolved_layout: Some(resolved),
        });
        SoftwareRenderer::render_with_text_measurer(
            &RenderScene::from_display_list(display_list),
            (TEXT_WIDTH * scale) as u32,
            (SURFACE_HEIGHT as f32 * scale) as u32,
            transparent(),
            scale,
            measurer,
        )
        .expect("render the narrow Spanish paragraph")
    }

    fn reference_resolved_pixels(
        layout: &ResolvedParagraphLayout,
        visible_lines: usize,
        only_source_range: Option<Range<usize>>,
    ) -> Vec<u8> {
        reference_resolved_pixels_at_scale(layout, visible_lines, only_source_range, 1.0)
    }

    fn reference_resolved_pixels_at_scale(
        layout: &ResolvedParagraphLayout,
        visible_lines: usize,
        only_source_range: Option<Range<usize>>,
        scale: f32,
    ) -> Vec<u8> {
        let mut pixmap = Pixmap::new(
            (TEXT_WIDTH * scale) as u32,
            (SURFACE_HEIGHT as f32 * scale) as u32,
        )
        .unwrap();
        let font = default_font();
        for glyph in &layout.glyphs {
            let cluster = &layout.clusters[glyph.cluster_index];
            if cluster.line_index >= visible_lines
                || only_source_range.as_ref().is_some_and(|range| {
                    range.start >= cluster.end_index || range.end <= cluster.start_index
                })
            {
                continue;
            }
            let glyph_index = u16::try_from(glyph.id).unwrap();
            let (metrics, bitmap) = font.rasterize_indexed(glyph_index, FONT_SIZE * scale);
            if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
                continue;
            }
            let mut rgba = Vec::with_capacity(metrics.width * metrics.height * 4);
            for coverage in bitmap {
                let premul = rgba_to_premul(text_color(), coverage);
                rgba.extend_from_slice(&[
                    premul.red(),
                    premul.green(),
                    premul.blue(),
                    premul.alpha(),
                ]);
            }
            let size =
                tiny_skia::IntSize::from_wh(metrics.width as u32, metrics.height as u32).unwrap();
            let glyph_pixmap = Pixmap::from_vec(rgba, size).unwrap();
            pixmap.draw_pixmap(
                (glyph.position.x * scale).round() as i32 + metrics.xmin,
                (glyph.position.y * scale).round() as i32 - metrics.ymin - metrics.height as i32,
                glyph_pixmap.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        pixmap.take()
    }

    #[test]
    fn narrow_spanish_paragraph_keeps_resolved_text_continuous_and_ellipsis_correct() {
        let measurer = production_text_measurer();
        let resolved = measurer.resolve_rich_text(
            &[ir_run(NARROW_SPANISH_COPY, FONT_SIZE, LINE_HEIGHT)],
            Some(TEXT_WIDTH),
        );

        assert_eq!(resolved.constraint_width, Some(TEXT_WIDTH));
        assert_eq!(
            resolved.lines.len(),
            2,
            "the width-matched fixture must resolve to two lines"
        );
        let resolved_line_step = resolved.lines[1].baseline - resolved.lines[0].baseline;
        assert!(
            (resolved_line_step - LINE_HEIGHT).abs() < 0.01,
            "the resolved paragraph must retain the authored 18.2-point line step; resolved step was {resolved_line_step}",
        );
        assert!(
            (resolved.size.height - TEXT_HEIGHT).abs() < 0.5,
            "font ascent and descent may extend fractionally beyond the 342x36.4 allocation, but not by a whole pixel; resolved height was {}",
            resolved.size.height,
        );
        let first_line = &resolved.lines[0];
        let second_line = &resolved.lines[1];
        assert_eq!(first_line.end_index, second_line.start_index);
        assert!(
            NARROW_SPANISH_COPY[..first_line.end_index].ends_with("sitio disponible "),
            "the authoritative first line must contain the substring clipped by the regression"
        );
        assert!(
            NARROW_SPANISH_COPY[second_line.start_index..].starts_with("y ordenó"),
            "the second line must start after the complete word disponible"
        );

        let missing_start = NARROW_SPANISH_COPY.find("nible").unwrap();
        let missing_range = missing_start..missing_start + "nible".len();
        let missing_reference =
            reference_resolved_pixels(&resolved, 2, Some(missing_range.clone()));
        assert!(
            missing_reference.chunks_exact(4).any(|pixel| pixel[3] > 0),
            "the reference must include painted glyphs for nible"
        );

        let app_pixels = render_narrow_spanish_copy(
            measurer.clone(),
            resolved.clone(),
            Some(2),
            TextOverflow::Ellipsis,
        );
        let expected = reference_resolved_pixels(&resolved, 2, None);
        assert_eq!(
            app_pixels, expected,
            "software paint must consume every authoritative glyph, including nible, without independently shifting the line through its clip"
        );

        let uncapped = render_narrow_spanish_copy(
            measurer.clone(),
            resolved.clone(),
            None,
            TextOverflow::Clip,
        );
        assert_eq!(
            app_pixels, uncapped,
            "a two-line paragraph capped at two lines must not gain an ellipsis"
        );

        let one_line_clip = render_narrow_spanish_copy(
            measurer.clone(),
            resolved.clone(),
            Some(1),
            TextOverflow::Clip,
        );
        let one_line_ellipsis =
            render_narrow_spanish_copy(measurer, resolved, Some(1), TextOverflow::Ellipsis);
        assert_ne!(
            one_line_clip, one_line_ellipsis,
            "the same paragraph must paint an ellipsis when its line cap actually hides content"
        );
    }

    #[test]
    fn narrow_spanish_paragraph_uses_resolved_positions_at_retina_scale() {
        let measurer = production_text_measurer();
        let resolved = measurer.resolve_rich_text(
            &[ir_run(NARROW_SPANISH_COPY, FONT_SIZE, LINE_HEIGHT)],
            Some(TEXT_WIDTH),
        );
        let actual = render_narrow_spanish_copy_at_scale(
            measurer,
            resolved.clone(),
            Some(2),
            TextOverflow::Ellipsis,
            2.0,
        );
        let expected = reference_resolved_pixels_at_scale(&resolved, 2, None, 2.0);

        assert_eq!(
            actual, expected,
            "high-DPI software paint must scale authoritative glyph positions and rasterization together"
        );
    }
}
