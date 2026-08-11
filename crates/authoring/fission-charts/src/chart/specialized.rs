use super::helpers::*;
use super::*;

pub(super) fn render_graph(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    graph: &crate::series::graph::GraphSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let positions = crate::layout::force_graph::ForceGraphLayout::compute_positions(
        &graph.nodes,
        &graph.edges,
        area.plot.width(),
        area.plot.height(),
        80,
    );
    render_edges(
        cx,
        root,
        &graph.edges,
        &positions,
        area,
        theme,
        animation,
        series_progress,
    );
    for (idx, node) in graph.nodes.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx + graph.edges.len());
        if item_progress <= f32::EPSILON {
            continue;
        }
        if let Some((x, y)) = positions.get(&node.id) {
            let r = (7.0 + node.value.sqrt().min(24.0)) * item_progress.sqrt();
            let px = area.plot.x() + *x;
            let py = area.plot.y() + *y;
            add_rect(
                cx,
                root,
                LayoutRect::new(px - r, py - r, r * 2.0, r * 2.0),
                fade_color(theme.palette[idx % theme.palette.len()], item_progress),
                Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
                r,
            );
            if item_progress > 0.82 {
                add_text(
                    cx,
                    root,
                    &node.name,
                    10.0,
                    theme.label,
                    px + r + 4.0,
                    py - 7.0,
                    100.0,
                    14.0,
                );
            }
        }
    }
}

pub(super) fn render_lines(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    lines: &crate::series::lines::LinesSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if lines.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut max_value = 1.0_f32;
    for segment in &lines.data {
        for (x, y) in [segment.from, segment.to] {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        max_value = max_value.max(segment.value);
    }
    let (min_x, max_x) = normalize_bounds(min_x, max_x);
    let (min_y, max_y) = normalize_bounds(min_y, max_y);

    for (idx, segment) in lines.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let from = map_lines_point(segment.from, min_x, max_x, min_y, max_y, area);
        let full_to = map_lines_point(segment.to, min_x, max_x, min_y, max_y, area);
        let to = interpolate_point(from, full_to, item_progress);
        let intensity = (segment.value / max_value).clamp(0.0, 1.0);
        let stroke_color = fade_color(
            mix_color(lines.color.with_alpha(110), lines.color, intensity),
            item_progress,
        );
        let control_x = (from.0 + to.0) / 2.0;
        let control_y = (from.1 + to.1) / 2.0 - 36.0 * intensity;
        let path = format!(
            "M {} {} C {} {} {} {} {} {}",
            from.0, from.1, control_x, control_y, control_x, control_y, to.0, to.1
        );
        add_path(
            cx,
            root,
            &path,
            None,
            Some(stroke(stroke_color, 1.6 + 2.2 * intensity)),
        );
        if item_progress > 0.72 {
            draw_arrow_head(cx, root, from, to, stroke_color);
        }

        if lines.effect {
            let mid = quadratic_midpoint(from, (control_x, control_y), to);
            let radius = 4.0 + 5.0 * intensity;
            add_rect(
                cx,
                root,
                LayoutRect::new(mid.0 - radius, mid.1 - radius, radius * 2.0, radius * 2.0),
                stroke_color.with_alpha(130),
                Some(stroke(Color::WHITE.with_alpha(150), 1.0)),
                radius,
            );
        }
    }

    add_text(
        cx,
        root,
        "lines",
        10.0,
        theme.label,
        area.plot.x() + 8.0,
        area.plot.y() + 8.0,
        56.0,
        14.0,
    );
}

pub(super) fn render_tree(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    tree: &crate::series::tree::TreeSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if tree.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let leaf_count = tree.data.iter().map(tree_leaf_count).sum::<usize>().max(1);
    let depth = tree
        .data
        .iter()
        .map(treemap_depth)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut next_leaf = 0usize;
    let mut nodes = Vec::<TreeRenderNode>::new();
    let mut edges = Vec::<((f32, f32), (f32, f32))>::new();

    for root_node in &tree.data {
        if tree.radial {
            layout_radial_tree_node(
                root_node,
                0,
                depth,
                leaf_count,
                &mut next_leaf,
                area,
                &mut nodes,
                &mut edges,
            );
        } else {
            layout_tree_node(
                root_node,
                0,
                depth,
                leaf_count,
                &mut next_leaf,
                area,
                &mut nodes,
                &mut edges,
            );
        }
    }

    for (idx, (from, to)) in edges.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let to = interpolate_point(*from, *to, item_progress);
        let path = if tree.radial {
            format!("M {} {} L {} {}", from.0, from.1, to.0, to.1)
        } else {
            let mid_x = (from.0 + to.0) / 2.0;
            format!(
                "M {} {} C {} {} {} {} {} {}",
                from.0, from.1, mid_x, from.1, mid_x, to.1, to.0, to.1
            )
        };
        add_path(
            cx,
            root,
            &path,
            None,
            Some(fade_stroke(
                stroke(theme.axis_line.with_alpha(150), 1.3),
                item_progress,
            )),
        );
    }

    for (idx, node) in nodes.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx + edges.len());
        if item_progress <= f32::EPSILON {
            continue;
        }
        let radius = (if node.depth == 0 { 8.0 } else { 6.0 }) * item_progress.sqrt();
        let color = theme.palette[idx % theme.palette.len()];
        add_rect(
            cx,
            root,
            LayoutRect::new(node.x - radius, node.y - radius, radius * 2.0, radius * 2.0),
            fade_color(color, item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            radius,
        );
        if item_progress > 0.82 && (!tree.radial || node.depth > 0) {
            add_text(
                cx,
                root,
                &node.name,
                10.0,
                theme.label,
                node.x + radius + 5.0,
                node.y - 7.0,
                110.0,
                14.0,
            );
        }
    }
}

pub(super) fn render_treemap(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    treemap: &crate::series::treemap::TreemapSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let layout = crate::layout::treemap::TreemapLayout::squarify(&treemap.data, area.plot);
    for (idx, (node, rect)) in layout.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let rect = scale_rect_from_center(*rect, 0.86 + item_progress * 0.14);
        add_rect(
            cx,
            root,
            rect,
            fade_color(theme.palette[idx % theme.palette.len()], item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 2.0), item_progress)),
            3.0,
        );
        if item_progress > 0.82 && rect.width() > 58.0 && rect.height() > 24.0 {
            add_text(
                cx,
                root,
                &node.name,
                11.0,
                Color::WHITE,
                rect.x() + 6.0,
                rect.y() + 6.0,
                rect.width() - 12.0,
                16.0,
            );
        }
    }
}

pub(super) fn render_radar(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
            polar.color.with_alpha(150),
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
        Some(fade_stroke(stroke(polar.color, 2.4), series_progress)),
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
            fade_color(polar.color, item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            r,
        );
    }
}

pub(super) fn render_single_axis(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    single_axis: &crate::series::single_axis::SingleAxisSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if single_axis.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let min = single_axis
        .data
        .iter()
        .map(|(value, _)| *value)
        .fold(f32::MAX, f32::min);
    let max = single_axis
        .data
        .iter()
        .map(|(value, _)| *value)
        .fold(f32::MIN, f32::max);
    let scale = LinearScale::nice(min, max, 6);
    let axis_y = area.plot.y() + area.plot.height() * 0.55;
    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            axis_y,
            area.plot.right(),
            axis_y
        ),
        None,
        Some(stroke(theme.axis_line, 1.2)),
    );
    for tick in &scale.ticks {
        let x = map_x(*tick, area, &scale);
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", x, axis_y - 5.0, x, axis_y + 5.0),
            None,
            Some(stroke(theme.axis_line, 1.0)),
        );
        add_text(
            cx,
            root,
            &format_tick(*tick),
            10.0,
            theme.label,
            x - 20.0,
            axis_y + 10.0,
            40.0,
            14.0,
        );
    }
    let max_size = single_axis
        .data
        .iter()
        .map(|(_, size)| *size)
        .fold(1.0_f32, f32::max);
    for (idx, (value, size)) in single_axis.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_x(*value, area, &scale);
        let lane = idx % 5;
        let y = axis_y - 32.0 + lane as f32 * 16.0;
        let r = (4.0 + 12.0 * (*size / max_size).clamp(0.0, 1.0).sqrt()) * item_progress.sqrt();
        add_rect(
            cx,
            root,
            LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
            fade_color(single_axis.color.with_alpha(170), item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            r,
        );
    }
}

pub(super) fn render_funnel(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    funnel: &crate::series::funnel::FunnelSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if funnel.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let max = funnel.data.iter().map(|(_, v)| *v).fold(1.0_f32, f32::max);
    let step_h = area.plot.height() / funnel.data.len() as f32;
    let cx_mid = area.plot.x() + area.plot.width() / 2.0;
    for (idx, (label, value)) in funnel.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let y = area.plot.y() + idx as f32 * step_h;
        let top_w = if idx == 0 {
            area.plot.width()
        } else {
            area.plot.width() * funnel.data[idx - 1].1 / max
        } * item_progress;
        let bot_w = area.plot.width() * *value / max * item_progress;
        let path = format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            cx_mid - top_w / 2.0,
            y,
            cx_mid + top_w / 2.0,
            y,
            cx_mid + bot_w / 2.0,
            y + step_h,
            cx_mid - bot_w / 2.0,
            y + step_h
        );
        add_path(
            cx,
            root,
            &path,
            Some(Fill::Solid(fade_color(
                theme.palette[idx % theme.palette.len()],
                item_progress,
            ))),
            Some(fade_stroke(stroke(Color::WHITE, 1.5), item_progress)),
        );
        if item_progress > 0.82 {
            add_text(
                cx,
                root,
                label,
                12.0,
                Color::WHITE,
                cx_mid - 50.0,
                y + step_h / 2.0 - 8.0,
                100.0,
                16.0,
            );
        }
    }
}

pub(super) fn render_gauge(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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

pub(super) fn render_map(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    map: &crate::series::map::MapSeries,
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
    let regions =
        crate::layout::map::MapLayout::compute_geojson(map, area.plot.width(), area.plot.height());
    if regions.is_empty() {
        return;
    }
    let values: Vec<f32> = regions.iter().filter_map(|region| region.value).collect();
    let min = values.iter().copied().fold(f32::MAX, f32::min);
    let max = values.iter().copied().fold(f32::MIN, f32::max);
    let denom = (max - min).max(f32::EPSILON);

    for (idx, region) in regions.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let fill = if let Some(value) = region.value {
            visual_map
                .map(|map| visual_color(map, value))
                .unwrap_or_else(|| {
                    mix_color(
                        theme.palette[idx % theme.palette.len()].with_alpha(90),
                        theme.palette[idx % theme.palette.len()],
                        ((value - min) / denom).clamp(0.0, 1.0),
                    )
                })
        } else {
            color(226, 232, 240, 255)
        };
        let shifted = translate_path(&region.path, area.plot.x(), area.plot.y());
        add_path(
            cx,
            root,
            &shifted,
            Some(Fill::Solid(fade_color(fill, item_progress))),
            Some(fade_stroke(stroke(Color::WHITE, 1.4), item_progress)),
        );
        if let Some((x, y, width, height)) = path_bounds(&shifted) {
            if item_progress > 0.82 && width > 42.0 && height > 18.0 {
                add_text(
                    cx,
                    root,
                    &region.name,
                    10.0,
                    theme.title,
                    x + 4.0,
                    y + height / 2.0 - 7.0,
                    width - 8.0,
                    14.0,
                );
            }
        }
    }
}

pub(super) fn render_sankey(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    sankey: &crate::series::sankey::SankeySeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let (rects, paths) = crate::layout::sankey::SankeyLayout::compute(
        &sankey.nodes,
        &sankey.edges,
        area.plot.width(),
        area.plot.height(),
    );
    for (idx, (_, _, path)) in paths.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        add_path(
            cx,
            root,
            &translate_path(path, area.plot.x(), area.plot.y()),
            Some(Fill::Solid(fade_color(
                theme.palette[idx % theme.palette.len()].with_alpha(115),
                item_progress,
            ))),
            None,
        );
    }
    for (idx, node) in sankey.nodes.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx + paths.len());
        if item_progress <= f32::EPSILON {
            continue;
        }
        if let Some(rect) = rects.get(&node.id) {
            let shifted = scale_rect_from_center(
                LayoutRect::new(
                    area.plot.x() + rect.x(),
                    area.plot.y() + rect.y(),
                    rect.width(),
                    rect.height(),
                ),
                0.86 + item_progress * 0.14,
            );
            add_rect(
                cx,
                root,
                shifted,
                fade_color(theme.palette[idx % theme.palette.len()], item_progress),
                None,
                3.0,
            );
            if item_progress > 0.82 {
                add_text(
                    cx,
                    root,
                    &node.name,
                    11.0,
                    theme.label,
                    shifted.right() + 6.0,
                    shifted.y() + 4.0,
                    100.0,
                    14.0,
                );
            }
        }
    }
}

pub(super) fn render_sunburst(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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

pub(super) fn render_parallel(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    parallel: &crate::series::parallel::ParallelSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let axes = parallel.data.first().map(|row| row.len()).unwrap_or(0);
    if axes < 2 {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let step = area.plot.width() / (axes - 1) as f32;
    for axis in 0..axes {
        let x = area.plot.x() + axis as f32 * step;
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", x, area.plot.y(), x, area.plot.bottom()),
            None,
            Some(stroke(theme.axis_line, 1.0)),
        );
    }
    for (idx, row) in parallel.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let points: Vec<(f32, f32)> = row
            .iter()
            .enumerate()
            .map(|(axis, value)| {
                let x = area.plot.x() + axis as f32 * step;
                let y = area.plot.bottom() - (*value / 100.0).clamp(0.0, 1.0) * area.plot.height();
                (x, y)
            })
            .collect();
        let path = path_for_points(&reveal_points(&points, item_progress));
        add_path(
            cx,
            root,
            &path,
            None,
            Some(fade_stroke(
                stroke(
                    theme.palette[idx % theme.palette.len()].with_alpha(170),
                    2.0,
                ),
                item_progress,
            )),
        );
    }
}

pub(super) fn render_theme_river(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    river: &crate::series::theme_river::ThemeRiverSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if river.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let mut by_time: BTreeMap<String, HashMap<String, f32>> = BTreeMap::new();
    let mut categories = Vec::<String>::new();
    for (time, value, category) in &river.data {
        by_time
            .entry(time.clone())
            .or_default()
            .insert(category.clone(), *value);
        if !categories.iter().any(|existing| existing == category) {
            categories.push(category.clone());
        }
    }
    let times: Vec<String> = by_time.keys().cloned().collect();
    if times.len() < 2 || categories.is_empty() {
        return;
    }

    let totals: Vec<f32> = times
        .iter()
        .map(|time| by_time[time].values().sum::<f32>())
        .collect();
    let max_total = totals.iter().copied().fold(1.0_f32, f32::max);
    let scale = area.plot.height() * 0.72 / max_total.max(f32::EPSILON);
    let step = area.plot.width() / (times.len() - 1) as f32;
    let mut bases = vec![0.0_f32; times.len()];

    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            area.plot.y() + area.plot.height() / 2.0,
            area.plot.right(),
            area.plot.y() + area.plot.height() / 2.0
        ),
        None,
        Some(stroke(theme.grid_line, 1.0)),
    );

    for (cat_idx, category) in categories.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, cat_idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let mut top = Vec::new();
        let mut bottom = Vec::new();
        for (idx, time) in times.iter().enumerate() {
            let value = by_time[time].get(category).copied().unwrap_or(0.0).max(0.0);
            let total = totals[idx];
            let baseline = area.plot.y() + area.plot.height() / 2.0 + total * scale / 2.0;
            let x = area.plot.x() + idx as f32 * step;
            let y_top = baseline - (bases[idx] + value) * scale;
            let y_bottom = baseline - bases[idx] * scale;
            top.push((x, y_top));
            bottom.push((x, y_bottom));
            bases[idx] += value;
        }
        let top = reveal_points(&top, item_progress);
        let bottom = reveal_points(&bottom, item_progress);
        if top.len() < 2 || bottom.len() < 2 {
            continue;
        }
        let mut path = path_for_points(&top);
        for (x, y) in bottom.iter().rev() {
            path.push_str(&format!(" L {} {}", x, y));
        }
        path.push_str(" Z");
        let color = theme.palette[cat_idx % theme.palette.len()];
        add_path(
            cx,
            root,
            &path,
            Some(Fill::Solid(fade_color(
                color.with_alpha(150),
                item_progress,
            ))),
            Some(fade_stroke(stroke(color, 1.0), item_progress)),
        );
    }

    for (idx, time) in times.iter().enumerate() {
        if idx % ((times.len() / 4).max(1)) == 0 {
            add_text(
                cx,
                root,
                time,
                10.0,
                theme.label,
                area.plot.x() + idx as f32 * step - 30.0,
                area.plot.bottom() + 8.0,
                60.0,
                14.0,
            );
        }
    }
}

pub(super) fn render_pictorial_bar(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    pic: &crate::series::pictorial_bar::PictorialBarSeries,
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
    for (idx, value) in pic.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_category_x(idx, model, area);
        let y0 = map_y(0.0, area, y_scale);
        let y1 = map_y(*value, area, y_scale);
        let count = ((*value).abs() / 20.0).ceil().max(1.0) as usize;
        let visible_units = (count as f32 * item_progress).ceil() as usize;
        let step = (y0 - y1) / count as f32;
        for unit in 0..visible_units.min(count) {
            let unit_progress = ((item_progress * count as f32) - unit as f32).clamp(0.0, 1.0);
            if unit_progress <= f32::EPSILON {
                continue;
            }
            let y = y0 - (unit as f32 + 0.5) * step;
            let half = 7.0 * unit_progress.sqrt();
            let top = 9.0 * unit_progress.sqrt();
            let bottom = 8.0 * unit_progress.sqrt();
            let path = if pic.symbol == "rect" {
                format!(
                    "M {} {} L {} {} L {} {} L {} {} Z",
                    x - half,
                    y - half,
                    x + half,
                    y - half,
                    x + half,
                    y + half,
                    x - half,
                    y + half
                )
            } else {
                format!(
                    "M {} {} L {} {} L {} {} Z",
                    x,
                    y - top,
                    x + bottom,
                    y + bottom,
                    x - bottom,
                    y + bottom
                )
            };
            add_path(
                cx,
                root,
                &path,
                Some(Fill::Solid(fade_color(pic.color, unit_progress))),
                None,
            );
        }
    }
}

pub(super) fn render_liquidfill(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    liquid: &crate::series::liquidfill::LiquidfillSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    let value = liquid.data.first().copied().unwrap_or(0.0).clamp(0.0, 1.0) * series_progress;
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let r = area.plot.width().min(area.plot.height()) * 0.34;
    add_rect(
        cx,
        root,
        LayoutRect::new(center.0 - r, center.1 - r, r * 2.0, r * 2.0),
        color(232, 244, 255, 255),
        Some(stroke(liquid.color, 2.0)),
        r,
    );
    let water_y = center.1 + r - value * r * 2.0;
    let path = format!(
        "M {} {} C {} {} {} {} {} {} L {} {} L {} {} Z",
        center.0 - r,
        water_y,
        center.0 - r * 0.45,
        water_y - 16.0,
        center.0 + r * 0.45,
        water_y + 16.0,
        center.0 + r,
        water_y,
        center.0 + r,
        center.1 + r,
        center.0 - r,
        center.1 + r
    );
    add_path(
        cx,
        root,
        &path,
        Some(Fill::Solid(fade_color(
            liquid.color.with_alpha(190),
            series_progress,
        ))),
        None,
    );
    add_text(
        cx,
        root,
        &format!("{:.0}%", value * 100.0),
        24.0,
        theme.title,
        center.0 - 40.0,
        center.1 - 14.0,
        80.0,
        28.0,
    );
}

pub(super) fn render_wordcloud(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    wordcloud: &crate::series::wordcloud::WordcloudSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let layout = crate::layout::wordcloud::WordcloudLayout::compute(
        &wordcloud.data,
        area.plot.width(),
        area.plot.height(),
    );
    for (idx, (word, size, x, y)) in layout.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        add_text(
            cx,
            root,
            word,
            (*size * (0.78 + item_progress * 0.22)).max(1.0),
            fade_color(theme.palette[idx % theme.palette.len()], item_progress),
            area.plot.x() + x + (*size * (1.0 - item_progress) * 0.08),
            area.plot.y() + y,
            180.0,
            size + 8.0,
        );
    }
}
