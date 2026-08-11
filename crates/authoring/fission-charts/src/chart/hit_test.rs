use super::helpers::*;
use super::*;

pub(super) fn hit_test_chart(
    model: &ChartModel,
    area: &ChartArea,
    point: LayoutPoint,
) -> Option<ChartHit> {
    if !area.plot.contains(point) {
        return None;
    }

    let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    let threshold = 10.0;
    let bar_groups = count_bar_groups(&model.series);
    let mut bar_group_index = 0usize;
    let mut bar_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut line_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut direct_hit = None;

    for (series_index, series) in model.series.iter().enumerate() {
        match series {
            ResolvedSeries::Bar(bar) => {
                let group_index = if bar.source.stack.is_none() {
                    let idx = bar_group_index;
                    bar_group_index += 1;
                    idx
                } else {
                    0
                };
                let band = band_width(model, area);
                let group_count = bar_groups.max(1) as f32;
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
                    let base = stack_base(&bar_stacks, bar.source.stack.as_ref(), idx);
                    let total = base + *value;
                    if let Some(stack) = bar.source.stack.as_ref() {
                        bar_stacks.insert((stack.clone(), idx), total);
                    }
                    let rect = if bar.source.orientation
                        == crate::series::bar::BarOrientation::Horizontal
                    {
                        let band = category_band_width(
                            model.y_categories.len().max(bar.values.len()),
                            area.plot.height(),
                        );
                        let bar_h = if bar.source.stack.is_some() {
                            band * 0.64
                        } else {
                            (band * 0.72 / group_count).max(2.0)
                        };
                        let group_offset_y = if bar.source.stack.is_some() {
                            0.0
                        } else {
                            (group_index as f32 - (group_count - 1.0) / 2.0) * bar_h
                        };
                        let y = map_category_y(idx, model, area) + group_offset_y;
                        let x0 = map_x(base, area, &x_scale);
                        let x1 = map_x(total, area, &x_scale);
                        LayoutRect::new(
                            x0.min(x1),
                            y - bar_h / 2.0,
                            (x1 - x0).abs().max(1.0),
                            bar_h,
                        )
                    } else {
                        let x = map_category_x(idx, model, area) + group_offset;
                        let y0 = map_y(base, area, &y_scale);
                        let y1 = map_y(total, area, &y_scale);
                        LayoutRect::new(
                            x - bar_w / 2.0,
                            y0.min(y1),
                            bar_w,
                            (y0 - y1).abs().max(1.0),
                        )
                    };
                    if rect.contains(point) {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            bar.source.name.clone(),
                            idx,
                            Some(idx as f32),
                            Some(total),
                        ));
                    }
                }
            }
            ResolvedSeries::Line(line) => {
                for (idx, value) in line.values.iter().enumerate() {
                    let base = stack_base(&line_stacks, line.source.stack.as_ref(), idx);
                    let total = base + *value;
                    if let Some(stack) = line.source.stack.as_ref() {
                        line_stacks.insert((stack.clone(), idx), total);
                    }
                    let x = map_category_x(idx, model, area);
                    let y = map_y(total, area, &y_scale);
                    if distance(point, (x, y)) <= threshold {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            line.source.name.clone(),
                            idx,
                            Some(idx as f32),
                            Some(total),
                        ));
                    }
                }
            }
            ResolvedSeries::Scatter(scatter) => {
                if let Some(hit) = hit_test_points(
                    series_index,
                    &scatter.name,
                    &scatter.data,
                    area,
                    &x_scale,
                    &y_scale,
                    point,
                    threshold,
                ) {
                    direct_hit = Some(hit);
                }
            }
            ResolvedSeries::Bubble(bubble) => {
                let max_size = bubble
                    .data
                    .iter()
                    .map(|(_, _, size)| *size)
                    .fold(1.0_f32, f32::max);
                for (idx, (xv, yv, size)) in bubble.data.iter().enumerate() {
                    let x = map_x(*xv, area, &x_scale);
                    let y = map_y(*yv, area, &y_scale);
                    let t = (*size / max_size).clamp(0.0, 1.0).sqrt();
                    let radius = bubble.min_radius + (bubble.max_radius - bubble.min_radius) * t;
                    if distance(point, (x, y)) <= radius.max(threshold) {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            bubble.name.clone(),
                            idx,
                            Some(*xv),
                            Some(*yv),
                        ));
                    }
                }
            }
            ResolvedSeries::EffectScatter(scatter) => {
                if let Some(hit) = hit_test_points(
                    series_index,
                    &scatter.name,
                    &scatter.data,
                    area,
                    &x_scale,
                    &y_scale,
                    point,
                    threshold * 1.6,
                ) {
                    direct_hit = Some(hit);
                }
            }
            ResolvedSeries::Pie(pie) => {
                if let Some(hit) = hit_test_pie(series_index, pie, area, point) {
                    direct_hit = Some(hit);
                }
            }
            ResolvedSeries::Heatmap(heatmap) => {
                let max_x = heatmap.data.iter().map(|d| d.0).max().unwrap_or(0) + 1;
                let max_y = heatmap.data.iter().map(|d| d.1).max().unwrap_or(0) + 1;
                let cell_w = area.plot.width() / max_x.max(1) as f32;
                let cell_h = area.plot.height() / max_y.max(1) as f32;
                for (idx, (x_idx, y_idx, value)) in heatmap.data.iter().enumerate() {
                    let rect = LayoutRect::new(
                        area.plot.x() + *x_idx as f32 * cell_w,
                        area.plot.bottom() - (*y_idx as f32 + 1.0) * cell_h,
                        cell_w,
                        cell_h,
                    );
                    if rect.contains(point) {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            heatmap.name.clone(),
                            idx,
                            Some(*x_idx as f32),
                            Some(*value),
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    direct_hit
        .or_else(|| nearest_cartesian_hit(model, area, point))
        .or_else(|| Some(ChartHit::plot_area()))
}
pub(super) fn hit_test_points(
    series_index: usize,
    series_name: &str,
    data: &[(f32, f32)],
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    point: LayoutPoint,
    threshold: f32,
) -> Option<ChartHit> {
    for (idx, (xv, yv)) in data.iter().enumerate() {
        let x = map_x(*xv, area, x_scale);
        let y = map_y(*yv, area, y_scale);
        if distance(point, (x, y)) <= threshold {
            return Some(ChartHit::series_item(
                series_index,
                series_name.to_string(),
                idx,
                Some(*xv),
                Some(*yv),
            ));
        }
    }
    None
}

pub(super) fn nearest_cartesian_hit(
    model: &ChartModel,
    area: &ChartArea,
    point: LayoutPoint,
) -> Option<ChartHit> {
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    let mut best: Option<(f32, ChartHit)> = None;

    for (series_index, series) in model.series.iter().enumerate() {
        match series {
            ResolvedSeries::Line(line) => {
                for (idx, value) in line.values.iter().enumerate() {
                    let x = map_category_x(idx, model, area);
                    let y = map_y(*value, area, &y_scale);
                    let dx = (point.x - x).abs();
                    let dy = (point.y - y).abs() * 0.25;
                    let score = dx + dy;
                    let hit = ChartHit::series_item(
                        series_index,
                        line.source.name.clone(),
                        idx,
                        Some(idx as f32),
                        Some(*value),
                    );
                    if best
                        .as_ref()
                        .map_or(true, |(best_score, _)| score < *best_score)
                    {
                        best = Some((score, hit));
                    }
                }
            }
            ResolvedSeries::Bar(bar) => {
                for (idx, value) in bar.values.iter().enumerate() {
                    let x = map_category_x(idx, model, area);
                    let score = (point.x - x).abs();
                    let hit = ChartHit::series_item(
                        series_index,
                        bar.source.name.clone(),
                        idx,
                        Some(idx as f32),
                        Some(*value),
                    );
                    if best
                        .as_ref()
                        .map_or(true, |(best_score, _)| score < *best_score)
                    {
                        best = Some((score, hit));
                    }
                }
            }
            _ => {}
        }
    }

    best.and_then(|(score, hit)| {
        let max_distance = (band_width(model, area) * 0.55).max(16.0);
        if score <= max_distance {
            Some(hit)
        } else {
            None
        }
    })
}

pub(super) fn hit_test_pie(
    series_index: usize,
    pie: &crate::series::pie::PieSeries,
    area: &ChartArea,
    point: LayoutPoint,
) -> Option<ChartHit> {
    let total: f32 = pie.data.iter().map(|(_, value)| *value).sum();
    if total <= 0.0 {
        return None;
    }

    let center = (
        area.plot.x() + area.plot.width() * 0.45,
        area.plot.y() + area.plot.height() * 0.52,
    );
    let max_r = area.plot.width().min(area.plot.height()) * 0.38;
    let dx = point.x - center.0;
    let dy = point.y - center.1;
    let radius = (dx * dx + dy * dy).sqrt();
    if radius > max_r {
        return None;
    }
    let inner = pie.inner_radius.max(0.0).min(max_r * 0.85);
    if radius < inner {
        return None;
    }

    let mut angle = dy.atan2(dx);
    if angle < -std::f32::consts::PI / 2.0 {
        angle += std::f32::consts::TAU;
    }
    let mut start = -std::f32::consts::PI / 2.0;
    for (idx, (label, value)) in pie.data.iter().enumerate() {
        let sweep = (*value / total) * std::f32::consts::TAU;
        let end = start + sweep;
        if angle >= start && angle <= end {
            let _ = label;
            return Some(ChartHit::series_item(
                series_index,
                pie.name.clone(),
                idx,
                None,
                Some(*value),
            ));
        }
        start = end;
    }
    None
}

pub(super) fn distance(point: LayoutPoint, other: (f32, f32)) -> f32 {
    let dx = point.x - other.0;
    let dy = point.y - other.1;
    (dx * dx + dy * dy).sqrt()
}
