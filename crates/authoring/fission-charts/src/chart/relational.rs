//! Series that draw relationships: graphs, lines, trees, treemaps, flows and maps.

use super::*;

pub(super) fn render_graph(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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

pub(super) fn render_sankey(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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

pub(super) fn render_map(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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

pub(super) fn render_parallel(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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

pub(super) fn render_edges(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    edges: &[GraphEdge],
    positions: &HashMap<String, (f32, f32)>,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_progress: f32,
) {
    for (idx, edge) in edges.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        if let (Some(a), Some(b)) = (positions.get(&edge.source), positions.get(&edge.target)) {
            let from = (area.plot.x() + a.0, area.plot.y() + a.1);
            let to = interpolate_point(
                from,
                (area.plot.x() + b.0, area.plot.y() + b.1),
                item_progress,
            );
            add_path(
                cx,
                root,
                &format!("M {} {} L {} {}", from.0, from.1, to.0, to.1),
                None,
                Some(fade_stroke(
                    stroke(theme.axis_line.with_alpha(140), 1.2),
                    item_progress,
                )),
            );
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct TreeRenderNode {
    name: String,
    x: f32,
    y: f32,
    depth: usize,
}

pub(super) fn tree_leaf_count(node: &crate::series::treemap::TreemapNode) -> usize {
    if node.children.is_empty() {
        1
    } else {
        node.children.iter().map(tree_leaf_count).sum()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_tree_node(
    node: &crate::series::treemap::TreemapNode,
    depth_index: usize,
    depth_count: usize,
    leaf_count: usize,
    next_leaf: &mut usize,
    area: &ChartArea,
    nodes: &mut Vec<TreeRenderNode>,
    edges: &mut Vec<((f32, f32), (f32, f32))>,
) -> (f32, f32) {
    let x_denom = depth_count.saturating_sub(1).max(1) as f32;
    let x = area.plot.x() + depth_index as f32 / x_denom * area.plot.width();
    let mut child_points = Vec::new();
    let y = if node.children.is_empty() {
        let y = area.plot.y() + (*next_leaf as f32 + 0.5) / leaf_count as f32 * area.plot.height();
        *next_leaf += 1;
        y
    } else {
        let mut sum = 0.0;
        for child in &node.children {
            let child_point = layout_tree_node(
                child,
                depth_index + 1,
                depth_count,
                leaf_count,
                next_leaf,
                area,
                nodes,
                edges,
            );
            child_points.push(child_point);
            let (_, child_y) = child_point;
            sum += child_y;
        }
        sum / node.children.len().max(1) as f32
    };

    let point = (x, y);
    for child_point in child_points {
        edges.push((point, child_point));
    }
    nodes.push(TreeRenderNode {
        name: node.name.clone(),
        x,
        y,
        depth: depth_index,
    });
    point
}

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_radial_tree_node(
    node: &crate::series::treemap::TreemapNode,
    depth_index: usize,
    depth_count: usize,
    leaf_count: usize,
    next_leaf: &mut usize,
    area: &ChartArea,
    nodes: &mut Vec<TreeRenderNode>,
    edges: &mut Vec<((f32, f32), (f32, f32))>,
) -> (f32, f32) {
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let radius = area.plot.width().min(area.plot.height()) * 0.44;
    let mut child_points = Vec::new();
    let point = if node.children.is_empty() {
        let angle = -std::f32::consts::PI / 2.0
            + (*next_leaf as f32 + 0.5) / leaf_count as f32 * std::f32::consts::TAU;
        *next_leaf += 1;
        let r = depth_index as f32 / depth_count.saturating_sub(1).max(1) as f32 * radius;
        (center.0 + r * angle.cos(), center.1 + r * angle.sin())
    } else {
        let mut points = Vec::new();
        for child in &node.children {
            let child_point = layout_radial_tree_node(
                child,
                depth_index + 1,
                depth_count,
                leaf_count,
                next_leaf,
                area,
                nodes,
                edges,
            );
            points.push(child_point);
            child_points.push(child_point);
        }
        if depth_index == 0 {
            center
        } else {
            let avg_x = points.iter().map(|point| point.0).sum::<f32>() / points.len() as f32;
            let avg_y = points.iter().map(|point| point.1).sum::<f32>() / points.len() as f32;
            let angle = (avg_y - center.1).atan2(avg_x - center.0);
            let r = depth_index as f32 / depth_count.saturating_sub(1).max(1) as f32 * radius;
            (center.0 + r * angle.cos(), center.1 + r * angle.sin())
        }
    };

    nodes.push(TreeRenderNode {
        name: node.name.clone(),
        x: point.0,
        y: point.1,
        depth: depth_index,
    });
    for child_point in child_points {
        edges.push((point, child_point));
    }
    point
}

pub(super) fn map_lines_point(
    point: (f32, f32),
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    area: &ChartArea,
) -> (f32, f32) {
    let x_t = ((point.0 - min_x) / (max_x - min_x).max(f32::EPSILON)).clamp(0.0, 1.0);
    let y_t = ((point.1 - min_y) / (max_y - min_y).max(f32::EPSILON)).clamp(0.0, 1.0);
    (
        area.plot.x() + x_t * area.plot.width(),
        area.plot.bottom() - y_t * area.plot.height(),
    )
}

pub(super) fn quadratic_midpoint(
    from: (f32, f32),
    control: (f32, f32),
    to: (f32, f32),
) -> (f32, f32) {
    (
        0.25 * from.0 + 0.5 * control.0 + 0.25 * to.0,
        0.25 * from.1 + 0.5 * control.1 + 0.25 * to.1,
    )
}

pub(super) fn draw_arrow_head(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    from: (f32, f32),
    to: (f32, f32),
    fill: Color,
) {
    let angle = (to.1 - from.1).atan2(to.0 - from.0);
    let size = 8.0;
    let left = (
        to.0 - size * (angle - 0.45).cos(),
        to.1 - size * (angle - 0.45).sin(),
    );
    let right = (
        to.0 - size * (angle + 0.45).cos(),
        to.1 - size * (angle + 0.45).sin(),
    );
    let path = format!(
        "M {} {} L {} {} L {} {} Z",
        to.0, to.1, left.0, left.1, right.0, right.1
    );
    add_path(cx, root, &path, Some(Fill::Solid(fill)), None);
}

pub(super) fn treemap_weight(node: &crate::series::treemap::TreemapNode) -> f32 {
    let child_total: f32 = node.children.iter().map(treemap_weight).sum();
    if child_total > 0.0 {
        child_total
    } else {
        node.value.max(0.0)
    }
}

pub(super) fn treemap_depth(node: &crate::series::treemap::TreemapNode) -> usize {
    1 + node.children.iter().map(treemap_depth).max().unwrap_or(0)
}
