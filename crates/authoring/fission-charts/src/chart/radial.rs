//! Series drawn around a centre: pie, radar, polar, gauge and sunburst.

use super::*;

pub(super) fn render_pie(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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

pub(super) fn render_radar(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    radar: &crate::series::radar::RadarSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let axes = radar.data.first().map(|data| data.len()).unwrap_or(0);
    if axes == 0 {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let r = area.plot.width().min(area.plot.height()) * 0.38;
    for ring in 1..=5 {
        let rr = r * ring as f32 / 5.0;
        let mut path = String::new();
        for axis in 0..axes {
            let angle = radar_angle(axis, axes);
            let x = center.0 + rr * angle.cos();
            let y = center.1 + rr * angle.sin();
            if axis == 0 {
                path.push_str(&format!("M {} {}", x, y));
            } else {
                path.push_str(&format!(" L {} {}", x, y));
            }
        }
        path.push_str(" Z");
        add_path(cx, root, &path, None, Some(stroke(theme.grid_line, 1.0)));
    }
    for axis in 0..axes {
        let angle = radar_angle(axis, axes);
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                center.0,
                center.1,
                center.0 + r * angle.cos(),
                center.1 + r * angle.sin()
            ),
            None,
            Some(stroke(theme.axis_line, 1.0)),
        );
    }
    for (idx, data) in radar.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let mut path = String::new();
        for (axis, value) in data.iter().enumerate() {
            let angle = radar_angle(axis, axes);
            let rr = r * (*value / 100.0).clamp(0.0, 1.0) * item_progress;
            let x = center.0 + rr * angle.cos();
            let y = center.1 + rr * angle.sin();
            if axis == 0 {
                path.push_str(&format!("M {} {}", x, y));
            } else {
                path.push_str(&format!(" L {} {}", x, y));
            }
        }
        path.push_str(" Z");
        let c = theme.palette[idx % theme.palette.len()];
        add_path(
            cx,
            root,
            &path,
            Some(Fill::Solid(fade_color(c.with_alpha(70), item_progress))),
            Some(fade_stroke(stroke(c, 2.0), item_progress)),
        );
    }
}

pub(super) fn render_polar_bar(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    polar: &crate::series::polar::PolarBarSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if polar.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let max_r = area.plot.width().min(area.plot.height()) * 0.43;
    let inner = polar.inner_radius.min(max_r * 0.72);
    let max_value = polar
        .data
        .iter()
        .map(|(_, value)| *value)
        .fold(1.0_f32, f32::max);
    let slot = std::f32::consts::TAU / polar.data.len() as f32;

    for ring in 1..=4 {
        let r = inner + (max_r - inner) * ring as f32 / 4.0;
        add_path(
            cx,
            root,
            &circle_path(center.0, center.1, r),
            None,
            Some(stroke(theme.grid_line, 1.0)),
        );
    }

    for (idx, (label, value)) in polar.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let start = -std::f32::consts::PI / 2.0 + idx as f32 * slot + slot * 0.10;
        let end = start + slot * 0.80 * item_progress;
        let outer = inner + (max_r - inner) * (*value / max_value).clamp(0.0, 1.0) * item_progress;
        let c = mix_color(
            filled(polar.color).with_alpha(150),
            theme.palette[idx % theme.palette.len()],
            0.35,
        );
        add_path(
            cx,
            root,
            &pie_slice(center.0, center.1, inner, outer, start, end),
            Some(Fill::Solid(fade_color(c, item_progress))),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
        );
        let mid = (start + end) / 2.0;
        if item_progress > 0.86 {
            add_text(
                cx,
                root,
                label,
                10.0,
                theme.label,
                center.0 + (max_r + 16.0) * mid.cos() - 28.0,
                center.1 + (max_r + 16.0) * mid.sin() - 7.0,
                56.0,
                14.0,
            );
        }
    }
}

pub(super) fn render_polar_line(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    polar: &crate::series::polar::PolarLineSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if polar.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let max_r = area.plot.width().min(area.plot.height()) * 0.42;
    let max_value = polar
        .data
        .iter()
        .map(|(_, radius)| *radius)
        .fold(1.0_f32, f32::max);
    for ring in 1..=4 {
        let r = max_r * ring as f32 / 4.0;
        add_path(
            cx,
            root,
            &circle_path(center.0, center.1, r),
            None,
            Some(stroke(theme.grid_line, 1.0)),
        );
    }
    for axis in 0..8 {
        let angle = -std::f32::consts::PI / 2.0 + axis as f32 / 8.0 * std::f32::consts::TAU;
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                center.0,
                center.1,
                center.0 + max_r * angle.cos(),
                center.1 + max_r * angle.sin()
            ),
            None,
            Some(stroke(theme.grid_line, 0.8)),
        );
    }

    let points: Vec<(f32, f32)> = polar
        .data
        .iter()
        .map(|(angle_degrees, radius)| {
            let angle = angle_degrees.to_radians() - std::f32::consts::PI / 2.0;
            let r = max_r * (*radius / max_value).clamp(0.0, 1.0);
            (center.0 + r * angle.cos(), center.1 + r * angle.sin())
        })
        .collect();
    let revealed_points = reveal_points(&points, series_progress);
    add_path(
        cx,
        root,
        &path_for_line(&revealed_points, polar.smooth, None),
        None,
        Some(fade_stroke(
            stroke(filled(polar.color), 2.4),
            series_progress,
        )),
    );
    for (idx, (x, y)) in revealed_points.into_iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let r = 4.0 * item_progress.sqrt();
        add_rect(
            cx,
            root,
            LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
            fade_color(filled(polar.color), item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            r,
        );
    }
}

pub(super) fn render_gauge(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    gauge: &crate::series::gauge::GaugeSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() * 0.68,
    );
    let r = area.plot.width().min(area.plot.height()) * 0.42;
    add_path(
        cx,
        root,
        &arc(
            center.0,
            center.1,
            r,
            std::f32::consts::PI,
            std::f32::consts::TAU,
        ),
        None,
        Some(stroke(theme.grid_line, 18.0)),
    );
    if let Some((label, value)) = gauge.data.first() {
        let series_progress = animation.series_progress(series_index);
        if series_progress <= f32::EPSILON {
            return;
        }
        let pct = (*value / 100.0).clamp(0.0, 1.0);
        let angle = std::f32::consts::PI + pct * std::f32::consts::PI * series_progress;
        add_path(
            cx,
            root,
            &arc(center.0, center.1, r, std::f32::consts::PI, angle),
            None,
            Some(stroke(theme.palette[0], 18.0)),
        );
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                center.0,
                center.1,
                center.0 + r * 0.78 * angle.cos(),
                center.1 + r * 0.78 * angle.sin()
            ),
            None,
            Some(stroke(theme.title, 3.5)),
        );
        add_rect(
            cx,
            root,
            LayoutRect::new(center.0 - 7.0, center.1 - 7.0, 14.0, 14.0),
            theme.title,
            None,
            7.0,
        );
        add_text(
            cx,
            root,
            &format!("{} {:.0}", label, value),
            18.0,
            theme.title,
            center.0 - 70.0,
            center.1 + 20.0,
            140.0,
            24.0,
        );
    }
}

pub(super) fn render_sunburst(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    sunburst: &crate::series::sunburst::SunburstSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if sunburst.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let depth = sunburst
        .data
        .iter()
        .map(treemap_depth)
        .max()
        .unwrap_or(1)
        .max(1);
    let radius = area.plot.width().min(area.plot.height()) * 0.44;
    let ring = radius / depth as f32;
    let total: f32 = sunburst.data.iter().map(treemap_weight).sum();
    if total <= 0.0 {
        return;
    }
    let mut angle = -std::f32::consts::PI / 2.0;
    let mut index = 0usize;
    for node in &sunburst.data {
        let sweep = treemap_weight(node) / total * std::f32::consts::TAU * series_progress;
        render_sunburst_node(
            cx,
            root,
            node,
            center,
            ring,
            0,
            angle,
            angle + sweep,
            theme,
            &mut index,
            animation,
            series_progress,
        );
        angle += sweep;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_sunburst_node(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    node: &crate::series::treemap::TreemapNode,
    center: (f32, f32),
    ring: f32,
    depth: usize,
    start: f32,
    end: f32,
    theme: &ChartTheme,
    index: &mut usize,
    animation: ChartAnimationFrame,
    series_progress: f32,
) {
    if end <= start {
        return;
    }
    let item_index = *index;
    let item_progress = animation.item_progress(series_progress, item_index);
    if item_progress <= f32::EPSILON {
        *index += 1;
        return;
    }
    let inner = depth as f32 * ring;
    let outer = inner + ring * 0.94;
    let color = theme.palette[item_index % theme.palette.len()];
    *index += 1;
    add_path(
        cx,
        root,
        &pie_slice(center.0, center.1, inner, outer, start, end),
        Some(Fill::Solid(fade_color(
            color.with_alpha(215),
            item_progress,
        ))),
        Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
    );
    if item_progress > 0.82 && end - start > 0.22 && outer > 28.0 {
        let mid = (start + end) / 2.0;
        let label_r = inner + (outer - inner) * 0.52;
        add_text(
            cx,
            root,
            &node.name,
            10.0,
            Color::WHITE,
            center.0 + label_r * mid.cos() - 30.0,
            center.1 + label_r * mid.sin() - 7.0,
            60.0,
            14.0,
        );
    }
    let child_total: f32 = node.children.iter().map(treemap_weight).sum();
    if child_total <= 0.0 {
        return;
    }
    let mut child_start = start;
    for child in &node.children {
        let child_sweep = treemap_weight(child) / child_total * (end - start);
        render_sunburst_node(
            cx,
            root,
            child,
            center,
            ring,
            depth + 1,
            child_start,
            child_start + child_sweep,
            theme,
            index,
            animation,
            series_progress,
        );
        child_start += child_sweep;
    }
}

pub(super) fn radar_angle(axis: usize, axes: usize) -> f32 {
    axis as f32 / axes as f32 * std::f32::consts::TAU - std::f32::consts::PI / 2.0
}
