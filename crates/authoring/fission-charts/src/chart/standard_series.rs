use super::helpers::*;
use super::*;

pub(super) fn render_bar(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    bar: &ResolvedBarSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    group_index: usize,
    group_count: usize,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if bar.source.orientation == crate::series::bar::BarOrientation::Horizontal {
        render_horizontal_bar(
            cx,
            root,
            bar,
            stacks,
            model,
            area,
            x_scale,
            group_index,
            group_count,
            animation,
            series_progress,
        );
        return;
    }

    let band = band_width(model, area);
    let group_count = group_count.max(1) as f32;
    let bar_w = if bar.source.stack.is_some() {
        band * 0.64
    } else {
        (band * 0.72 / group_count).max(2.0)
    };
    let group_offset = if bar.source.stack.is_some() {
        0.0
    } else {
        (group_index as f32 - (group_count - 1.0) / 2.0) * bar_w
    };

    for (idx, value) in bar.values.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let base = stack_base(stacks, bar.source.stack.as_ref(), idx);
        let total = base + *value * item_progress;
        if bar.source.stack.is_some() {
            stacks.insert((bar.source.stack.clone().unwrap(), idx), total);
        }
        let x = map_category_x(idx, model, area) + group_offset;
        let y0 = map_y(base, area, y_scale);
        let y1 = map_y(total, area, y_scale);
        let top = y0.min(y1);
        let height = (y0 - y1).abs().max(1.0);
        if let Some(background) = bar.source.background {
            add_rect(
                cx,
                root,
                LayoutRect::new(x - bar_w / 2.0, area.plot.y(), bar_w, area.plot.height()),
                background,
                None,
                bar.source.border_radius.unwrap_or(4.0),
            );
        }
        add_rect(
            cx,
            root,
            LayoutRect::new(x - bar_w / 2.0, top, bar_w, height),
            bar.source.color,
            None,
            bar.source.border_radius.unwrap_or(4.0),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_horizontal_bar(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    bar: &ResolvedBarSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    x_scale: &LinearScale,
    group_index: usize,
    group_count: usize,
    animation: ChartAnimationFrame,
    series_progress: f32,
) {
    let band = category_band_width(
        model.y_categories.len().max(bar.values.len()),
        area.plot.height(),
    );
    let group_count = group_count.max(1) as f32;
    let bar_h = if bar.source.stack.is_some() {
        band * 0.64
    } else {
        (band * 0.72 / group_count).max(2.0)
    };
    let group_offset = if bar.source.stack.is_some() {
        0.0
    } else {
        (group_index as f32 - (group_count - 1.0) / 2.0) * bar_h
    };

    for (idx, value) in bar.values.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let base = stack_base(stacks, bar.source.stack.as_ref(), idx);
        let total = base + *value * item_progress;
        if bar.source.stack.is_some() {
            stacks.insert((bar.source.stack.clone().unwrap(), idx), total);
        }
        let y = map_category_y(idx, model, area) + group_offset;
        let x0 = map_x(base, area, x_scale);
        let x1 = map_x(total, area, x_scale);
        let left = x0.min(x1);
        let width = (x1 - x0).abs().max(1.0);
        if let Some(background) = bar.source.background {
            add_rect(
                cx,
                root,
                LayoutRect::new(area.plot.x(), y - bar_h / 2.0, area.plot.width(), bar_h),
                background,
                None,
                bar.source.border_radius.unwrap_or(4.0),
            );
        }
        add_rect(
            cx,
            root,
            LayoutRect::new(left, y - bar_h / 2.0, width, bar_h),
            bar.source.color,
            None,
            bar.source.border_radius.unwrap_or(4.0),
        );
    }
}

pub(super) fn render_line(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    line: &ResolvedLineSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    _x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if line.values.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let mut points = Vec::new();
    let mut base_points = Vec::new();
    for (idx, value) in line.values.iter().enumerate() {
        let base = stack_base(stacks, line.source.stack.as_ref(), idx);
        let total = base + *value;
        if line.source.stack.is_some() {
            stacks.insert((line.source.stack.clone().unwrap(), idx), total);
        }
        let x = map_category_x(idx, model, area);
        points.push((x, map_y(total, area, y_scale)));
        base_points.push((x, map_y(base, area, y_scale)));
    }

    let revealed_points = reveal_points(&points, series_progress);
    let revealed_base_points = reveal_points(&base_points, series_progress);

    if let Some(area_color) = line.source.area_style {
        if revealed_points.len() > 1 && revealed_base_points.len() > 1 {
            let mut area_path = path_for_line(
                &revealed_points,
                line.source.smooth,
                line.source.step.as_deref(),
            );
            for (x, y) in revealed_base_points.iter().rev() {
                area_path.push_str(&format!(" L {} {}", x, y));
            }
            area_path.push_str(" Z");
            let fill = Fill::LinearGradient {
                start: (0.0, 0.0),
                end: (0.0, 1.0),
                stops: vec![(0.0, area_color), (1.0, area_color.with_alpha(16))],
            };
            add_path(cx, root, &area_path, Some(fill), None);
        }
    }

    if revealed_points.len() > 1 {
        add_path(
            cx,
            root,
            &path_for_line(
                &revealed_points,
                line.source.smooth,
                line.source.step.as_deref(),
            ),
            None,
            Some(stroke(line.source.color, 2.4)),
        );
    }
    for (idx, (x, y)) in revealed_points.into_iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let radius = 3.0 * item_progress.sqrt();
        add_rect(
            cx,
            root,
            LayoutRect::new(x - radius, y - radius, radius * 2.0, radius * 2.0),
            fade_color(line.source.color, item_progress),
            Some(stroke(Color::WHITE, 1.0)),
            radius,
        );
    }
}

pub(super) fn render_scatter(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    data: &[(f32, f32)],
    color: Color,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    effect: bool,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    for (idx, (xv, yv)) in data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_x(*xv, area, x_scale);
        let y = map_y(*yv, area, y_scale);
        let fill = visual_map
            .map(|map| visual_color(map, *yv))
            .unwrap_or(color);
        if effect {
            for (scale, alpha) in [(2.2, 45), (1.55, 72), (1.0, 220)] {
                let r = 7.0 * scale * item_progress.sqrt();
                add_rect(
                    cx,
                    root,
                    LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
                    fill.with_alpha(((alpha as f32) * item_progress).round() as u8),
                    None,
                    r,
                );
            }
        } else {
            let r = 5.5 * item_progress.sqrt();
            add_rect(
                cx,
                root,
                LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
                fade_color(fill, item_progress),
                Some(stroke(Color::WHITE, 1.0)),
                r,
            );
        }
    }
}

pub(super) fn render_bubble(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    bubble: &crate::series::bubble::BubbleSeries,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let max_size = bubble
        .data
        .iter()
        .map(|(_, _, size)| *size)
        .fold(1.0_f32, f32::max);
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    for (idx, (xv, yv, size)) in bubble.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_x(*xv, area, x_scale);
        let y = map_y(*yv, area, y_scale);
        let t = (*size / max_size).clamp(0.0, 1.0).sqrt();
        let radius = (bubble.min_radius + (bubble.max_radius - bubble.min_radius) * t)
            * item_progress.sqrt();
        let fill = visual_map
            .map(|map| visual_color(map, *size))
            .unwrap_or_else(|| bubble.color.with_alpha(185));
        add_rect(
            cx,
            root,
            LayoutRect::new(x - radius, y - radius, radius * 2.0, radius * 2.0),
            fade_color(fill, item_progress),
            Some(stroke(Color::WHITE, 1.2)),
            radius,
        );
        if radius > 14.0 {
            add_text(
                cx,
                root,
                &(idx + 1).to_string(),
                10.0,
                Color::WHITE,
                x - 10.0,
                y - 6.0,
                20.0,
                12.0,
            );
        }
    }
}

pub(super) fn render_pie(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    pie: &crate::series::pie::PieSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let total: f32 = pie.data.iter().map(|(_, value)| *value).sum();
    if total <= 0.0 {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let cx_pie = area.plot.x() + area.plot.width() * 0.45;
    let cy_pie = area.plot.y() + area.plot.height() * 0.52;
    let max_r = area.plot.width().min(area.plot.height()) * 0.38;
    let inner = pie.inner_radius.max(0.0).min(max_r * 0.85);
    let max_value = pie
        .data
        .iter()
        .map(|(_, value)| *value)
        .fold(1.0_f32, f32::max);
    let mut angle = -std::f32::consts::PI / 2.0;
    let mut remaining_reveal = std::f32::consts::TAU * series_progress;
    for (idx, (label, value)) in pie.data.iter().enumerate() {
        let sweep = (*value / total) * std::f32::consts::TAU;
        let revealed_sweep = sweep.min(remaining_reveal.max(0.0));
        if revealed_sweep <= f32::EPSILON {
            break;
        }
        let end = angle + revealed_sweep;
        let mut outer = max_r;
        if let Some(rose_type) = pie.rose_type.as_deref() {
            let normalized = (*value / max_value).clamp(0.0, 1.0);
            outer = match rose_type {
                "area" => max_r * (0.42 + 0.58 * normalized.sqrt()),
                "radius" => max_r * (0.42 + 0.58 * normalized),
                _ => max_r,
            };
        }
        add_path(
            cx,
            root,
            &pie_slice(cx_pie, cy_pie, inner, outer, angle, end),
            Some(Fill::Solid(theme.palette[idx % theme.palette.len()])),
            Some(stroke(Color::WHITE, 1.2)),
        );
        let mid = angle + revealed_sweep / 2.0;
        let lx = cx_pie + (outer + 20.0) * mid.cos();
        let ly = cy_pie + (outer + 20.0) * mid.sin();
        if series_progress > 0.92 || revealed_sweep >= sweep * 0.92 {
            add_text(
                cx,
                root,
                label,
                11.0,
                theme.label,
                lx - 36.0,
                ly - 7.0,
                72.0,
                14.0,
            );
        }
        angle += sweep;
        remaining_reveal -= sweep;
    }
}

pub(super) fn render_boxplot(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    boxplot: &crate::series::boxplot::BoxplotSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let band = band_width(model, area);
    let box_w = band * 0.46;
    for (idx, row) in boxplot.data.iter().enumerate() {
        if row.len() < 5 {
            continue;
        }
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_category_x(idx, model, area);
        let median_anchor = map_y(row[2], area, y_scale);
        let min_y = interpolate(median_anchor, map_y(row[0], area, y_scale), item_progress);
        let q1_y = interpolate(median_anchor, map_y(row[1], area, y_scale), item_progress);
        let med_y = map_y(row[2], area, y_scale);
        let q3_y = interpolate(median_anchor, map_y(row[3], area, y_scale), item_progress);
        let max_y = interpolate(median_anchor, map_y(row[4], area, y_scale), item_progress);
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x - box_w / 2.0,
                q3_y.min(q1_y),
                box_w,
                (q1_y - q3_y).abs().max(1.0),
            ),
            fade_color(boxplot.color.with_alpha(70), item_progress),
            Some(fade_stroke(stroke(boxplot.color, 1.5), item_progress)),
            1.0,
        );
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {} M {} {} L {} {} M {} {} L {} {} M {} {} L {} {}",
                x,
                min_y,
                x,
                q1_y.max(q3_y),
                x,
                max_y,
                x,
                q1_y.min(q3_y),
                x - box_w / 2.0,
                min_y,
                x + box_w / 2.0,
                min_y,
                x - box_w / 2.0,
                max_y,
                x + box_w / 2.0,
                max_y
            ),
            None,
            Some(fade_stroke(stroke(boxplot.color, 1.2), item_progress)),
        );
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                x - box_w / 2.0,
                med_y,
                x + box_w / 2.0,
                med_y
            ),
            None,
            Some(fade_stroke(stroke(boxplot.color, 2.0), item_progress)),
        );
    }
}

pub(super) fn render_candlestick(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    candle: &crate::series::candlestick::CandlestickSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let band = band_width(model, area);
    let box_w = band * 0.5;
    for (idx, row) in candle.data.iter().enumerate() {
        if row.len() < 4 {
            continue;
        }
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let open = row[0];
        let close = row[1];
        let low = row[2];
        let high = row[3];
        let up = close >= open;
        let color = if up {
            candle.color_up
        } else {
            candle.color_down
        };
        let x = map_category_x(idx, model, area);
        let center_y = map_y((open + close) / 2.0, area, y_scale);
        let open_y = interpolate(center_y, map_y(open, area, y_scale), item_progress);
        let close_y = interpolate(center_y, map_y(close, area, y_scale), item_progress);
        let high_y = interpolate(center_y, map_y(high, area, y_scale), item_progress);
        let low_y = interpolate(center_y, map_y(low, area, y_scale), item_progress);
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", x, high_y, x, low_y),
            None,
            Some(fade_stroke(stroke(color, 1.4), item_progress)),
        );
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x - box_w / 2.0,
                open_y.min(close_y),
                box_w,
                (open_y - close_y).abs().max(1.0),
            ),
            fade_color(if up { Color::WHITE } else { color }, item_progress),
            Some(fade_stroke(stroke(color, 1.4), item_progress)),
            0.0,
        );
    }
}

pub(super) fn render_heatmap(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    heatmap: &crate::series::heatmap::HeatmapSeries,
    model: &ChartModel,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let max_x = heatmap.data.iter().map(|d| d.0).max().unwrap_or(0) + 1;
    let max_y = heatmap.data.iter().map(|d| d.1).max().unwrap_or(0) + 1;
    let cell_w = area.plot.width() / max_x.max(1) as f32;
    let cell_h = area.plot.height() / max_y.max(1) as f32;
    let max_val = heatmap.data.iter().map(|d| d.2).fold(1.0_f32, f32::max);
    for (idx, (x_idx, y_idx, val)) in heatmap.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = area.plot.x() + *x_idx as f32 * cell_w;
        let y = area.plot.bottom() - (*y_idx as f32 + 1.0) * cell_h;
        let fill = visual_map
            .map(|map| visual_color(map, *val))
            .unwrap_or_else(|| heat_color(*val / max_val));
        let rect = scale_rect_from_center(
            LayoutRect::new(x, y, cell_w, cell_h),
            0.82 + item_progress * 0.18,
        );
        add_rect(
            cx,
            root,
            rect,
            fade_color(fill, item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            0.0,
        );
    }
    if model.x_axis.axis_type == AxisType::Category {
        for (idx, label) in model.x_axis.data.iter().enumerate() {
            add_text(
                cx,
                root,
                label,
                10.0,
                theme.label,
                area.plot.x() + idx as f32 * cell_w,
                area.plot.bottom() + 8.0,
                cell_w,
                14.0,
            );
        }
    }
}

pub(super) fn render_calendar_heatmap(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    calendar: &crate::series::calendar_heatmap::CalendarHeatmapSeries,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    use chrono::{Datelike, Duration, NaiveDate};

    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let parsed: Vec<(NaiveDate, f32)> = calendar
        .data
        .iter()
        .filter_map(|(date, value)| {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()
                .map(|date| (date, *value))
        })
        .collect();
    if parsed.is_empty() {
        return;
    }

    let min_date = parsed.iter().map(|(date, _)| *date).min().unwrap();
    let max_date = parsed.iter().map(|(date, _)| *date).max().unwrap();
    let start = calendar
        .start
        .as_ref()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .unwrap_or(min_date);
    let end = calendar
        .end
        .as_ref()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .unwrap_or(max_date)
        .max(start);

    let start_weekday = start.weekday().num_days_from_monday() as i64;
    let days = (end - start).num_days().max(0) + 1;
    let weeks = ((start_weekday + days + 6) / 7).max(1) as usize;
    let cell = (area.plot.width() / weeks as f32)
        .min(area.plot.height() / 7.0)
        .max(4.0);
    let x0 = area.plot.x();
    let y0 = area.plot.y() + (area.plot.height() - cell * 7.0) / 2.0;
    let values: HashMap<NaiveDate, f32> = parsed.into_iter().collect();
    let max_value = values.values().copied().fold(1.0_f32, f32::max);

    let mut date = start;
    let mut idx = 0usize;
    while date <= end {
        let offset = (date - start).num_days() + start_weekday;
        let week = (offset / 7) as f32;
        let day = date.weekday().num_days_from_monday() as f32;
        let value = values.get(&date).copied().unwrap_or(0.0);
        let fill = visual_map
            .map(|map| visual_color(map, value))
            .unwrap_or_else(|| heat_color(value / max_value));
        let item_progress = animation.item_progress(series_progress, idx);
        let rect = scale_rect_from_center(
            LayoutRect::new(x0 + week * cell, y0 + day * cell, cell - 2.0, cell - 2.0),
            0.82 + item_progress * 0.18,
        );
        add_rect(
            cx,
            root,
            rect,
            fade_color(
                fill.with_alpha(if value > 0.0 { 230 } else { 55 }),
                item_progress,
            ),
            Some(fade_stroke(stroke(Color::WHITE, 0.8), item_progress)),
            2.0,
        );
        date += Duration::days(1);
        idx += 1;
    }

    for (idx, label) in ["Mon", "Wed", "Fri", "Sun"].iter().enumerate() {
        let day = [0.0, 2.0, 4.0, 6.0][idx];
        add_text(
            cx,
            root,
            label,
            10.0,
            theme.label,
            x0 - 34.0,
            y0 + day * cell - 2.0,
            28.0,
            12.0,
        );
    }
    add_text(
        cx,
        root,
        &format!("{} to {}", start.format("%b %Y"), end.format("%b %Y")),
        11.0,
        theme.label,
        x0,
        y0 + cell * 7.0 + 8.0,
        area.plot.width(),
        16.0,
    );
}
