use super::*;

pub(super) fn count_bar_groups(series: &[ResolvedSeries]) -> usize {
    series
        .iter()
        .filter(|series| matches!(series, ResolvedSeries::Bar(bar) if bar.source.stack.is_none()))
        .count()
        .max(1)
}

pub(super) fn stack_base(
    stacks: &HashMap<(String, usize), f32>,
    stack: Option<&String>,
    idx: usize,
) -> f32 {
    stack
        .and_then(|name| stacks.get(&(name.clone(), idx)).copied())
        .unwrap_or(0.0)
}

pub(super) fn path_for_line(points: &[(f32, f32)], smooth: bool, step: Option<&str>) -> String {
    if points.is_empty() {
        return String::new();
    }
    if smooth {
        return catmull_rom_to_bezier(points);
    }
    let mut path = format!("M {} {}", points[0].0, points[0].1);
    for pair in points.windows(2) {
        let (px, py) = pair[0];
        let (x, y) = pair[1];
        match step {
            Some("start") => path.push_str(&format!(" L {} {} L {} {}", px, y, x, y)),
            Some("end") => path.push_str(&format!(" L {} {} L {} {}", x, py, x, y)),
            Some("middle") => {
                let mx = px + (x - px) / 2.0;
                path.push_str(&format!(" L {} {} L {} {} L {} {}", mx, py, mx, y, x, y));
            }
            _ => path.push_str(&format!(" L {} {}", x, y)),
        }
    }
    path
}

pub(super) fn reveal_points(points: &[(f32, f32)], progress: f32) -> Vec<(f32, f32)> {
    if points.is_empty() || progress <= f32::EPSILON {
        return Vec::new();
    }
    if progress >= 1.0 || points.len() == 1 {
        return points.to_vec();
    }

    let span = progress.clamp(0.0, 1.0) * (points.len() - 1) as f32;
    let last_full = span.floor() as usize;
    let mut out = points[..=last_full].to_vec();
    if last_full + 1 < points.len() {
        let t = span - last_full as f32;
        let (ax, ay) = points[last_full];
        let (bx, by) = points[last_full + 1];
        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
    }
    out
}

pub(super) fn path_for_points(points: &[(f32, f32)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut path = format!("M {} {}", points[0].0, points[0].1);
    for (x, y) in points.iter().skip(1) {
        path.push_str(&format!(" L {} {}", x, y));
    }
    path
}

pub(super) fn circle_path(cx: f32, cy: f32, r: f32) -> String {
    format!(
        "M {} {} A {} {} 0 1 0 {} {} A {} {} 0 1 0 {} {}",
        cx + r,
        cy,
        r,
        r,
        cx - r,
        cy,
        r,
        r,
        cx + r,
        cy
    )
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

pub(super) fn path_bounds(path: &str) -> Option<(f32, f32, f32, f32)> {
    let tokens: Vec<&str> = path.split_whitespace().collect();
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut idx = 0usize;
    while idx < tokens.len() {
        let token = tokens[idx];
        idx += 1;
        let coord_count = match token {
            "M" | "L" => 2,
            "C" => 6,
            "Z" => 0,
            _ => continue,
        };
        let mut coords = Vec::with_capacity(coord_count);
        for _ in 0..coord_count {
            if let Some(raw) = tokens.get(idx) {
                coords.push(raw.parse::<f32>().ok()?);
                idx += 1;
            }
        }
        for pair in coords.chunks(2) {
            if let [x, y] = pair {
                min_x = min_x.min(*x);
                max_x = max_x.max(*x);
                min_y = min_y.min(*y);
                max_y = max_y.max(*y);
            }
        }
    }
    if min_x == f32::MAX {
        None
    } else {
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    }
}

pub(super) fn band_width(model: &ChartModel, area: &ChartArea) -> f32 {
    let count = model.x_categories.len().max(1) as f32;
    area.plot.width() / count
}

pub(super) fn category_band_width(count: usize, extent: f32) -> f32 {
    extent / count.max(1) as f32
}

pub(super) fn map_category_x(idx: usize, model: &ChartModel, area: &ChartArea) -> f32 {
    area.plot.x() + band_width(model, area) * (idx as f32 + 0.5)
}

pub(super) fn map_category_y(idx: usize, model: &ChartModel, area: &ChartArea) -> f32 {
    let count = model.y_categories.len().max(1);
    area.plot.y() + category_band_width(count, area.plot.height()) * (idx as f32 + 0.5)
}

pub(super) fn map_x(value: f32, area: &ChartArea, scale: &LinearScale) -> f32 {
    scale.map(value, area.plot.x(), area.plot.right())
}

pub(super) fn map_y(value: f32, area: &ChartArea, scale: &LinearScale) -> f32 {
    scale.map(value, area.plot.bottom(), area.plot.y())
}

pub(super) fn series_names(model: &ChartModel) -> Vec<String> {
    model
        .series
        .iter()
        .map(|series| match series {
            ResolvedSeries::Line(s) => s.source.name.clone(),
            ResolvedSeries::Bar(s) => s.source.name.clone(),
            ResolvedSeries::Scatter(s) => s.name.clone(),
            ResolvedSeries::Pie(s) => s.name.clone(),
            ResolvedSeries::Bubble(s) => s.name.clone(),
            ResolvedSeries::Boxplot(s) => s.name.clone(),
            ResolvedSeries::Candlestick(s) => s.name.clone(),
            ResolvedSeries::Heatmap(s) => s.name.clone(),
            ResolvedSeries::CalendarHeatmap(s) => s.name.clone(),
            ResolvedSeries::Lines(s) => s.name.clone(),
            ResolvedSeries::Graph(s) => s.name.clone(),
            ResolvedSeries::Tree(s) => s.name.clone(),
            ResolvedSeries::Treemap(s) => s.name.clone(),
            ResolvedSeries::Radar(s) => s.name.clone(),
            ResolvedSeries::Funnel(s) => s.name.clone(),
            ResolvedSeries::Gauge(s) => s.name.clone(),
            ResolvedSeries::Map(s) => s.name.clone(),
            ResolvedSeries::Sankey(s) => s.name.clone(),
            ResolvedSeries::Parallel(s) => s.name.clone(),
            ResolvedSeries::Sunburst(s) => s.name.clone(),
            ResolvedSeries::ThemeRiver(s) => s.name.clone(),
            ResolvedSeries::PictorialBar(s) => s.name.clone(),
            ResolvedSeries::EffectScatter(s) => s.name.clone(),
            ResolvedSeries::Liquidfill(s) => s.name.clone(),
            ResolvedSeries::Wordcloud(s) => s.name.clone(),
            ResolvedSeries::PolarBar(s) => s.name.clone(),
            ResolvedSeries::PolarLine(s) => s.name.clone(),
            ResolvedSeries::SingleAxis(s) => s.name.clone(),
        })
        .collect()
}

pub(super) fn render_edges(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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

pub(super) fn radar_angle(axis: usize, axes: usize) -> f32 {
    axis as f32 / axes as f32 * std::f32::consts::TAU - std::f32::consts::PI / 2.0
}

pub(super) fn visual_color(map: &VisualMap, value: f32) -> Color {
    let denom = (map.max - map.min).max(f32::EPSILON);
    visual_color_at(map, ((value - map.min) / denom).clamp(0.0, 1.0))
}

pub(super) fn visual_color_at(map: &VisualMap, t: f32) -> Color {
    let colors = if map.in_range_colors.is_empty() {
        vec![
            color(49, 130, 206, 255),
            color(252, 211, 77, 255),
            color(220, 38, 38, 255),
        ]
    } else {
        map.in_range_colors.clone()
    };
    if colors.len() == 1 {
        return colors[0];
    }
    let scaled = t.clamp(0.0, 1.0) * (colors.len() - 1) as f32;
    let idx = scaled.floor() as usize;
    let next = (idx + 1).min(colors.len() - 1);
    let local = scaled - idx as f32;
    mix_color(colors[idx], colors[next], local)
}

pub(super) fn heat_color(t: f32) -> Color {
    mix_color(
        color(59, 130, 246, 255),
        color(239, 68, 68, 255),
        t.clamp(0.0, 1.0),
    )
}

pub(super) fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let mix = |x: u8, y: u8| x as f32 + (y as f32 - x as f32) * t;
    color(
        mix(a.r, b.r) as u8,
        mix(a.g, b.g) as u8,
        mix(a.b, b.b) as u8,
        mix(a.a, b.a) as u8,
    )
}

pub(super) fn fade_color(color: Color, progress: f32) -> Color {
    color.with_alpha(((color.a as f32) * progress.clamp(0.0, 1.0)).round() as u8)
}

pub(super) fn fade_fill(fill: Fill, progress: f32) -> Fill {
    match fill {
        Fill::Solid(color) => Fill::Solid(fade_color(color, progress)),
        Fill::LinearGradient { start, end, stops } => Fill::LinearGradient {
            start,
            end,
            stops: stops
                .into_iter()
                .map(|(offset, color)| (offset, fade_color(color, progress)))
                .collect(),
        },
        Fill::RadialGradient {
            center,
            radius,
            stops,
        } => Fill::RadialGradient {
            center,
            radius,
            stops: stops
                .into_iter()
                .map(|(offset, color)| (offset, fade_color(color, progress)))
                .collect(),
        },
    }
}

pub(super) fn fade_stroke(mut stroke: Stroke, progress: f32) -> Stroke {
    stroke.fill = fade_fill(stroke.fill, progress);
    stroke
}

pub(super) fn interpolate(a: f32, b: f32, progress: f32) -> f32 {
    a + (b - a) * progress.clamp(0.0, 1.0)
}

pub(super) fn interpolate_point(from: (f32, f32), to: (f32, f32), progress: f32) -> (f32, f32) {
    (
        interpolate(from.0, to.0, progress),
        interpolate(from.1, to.1, progress),
    )
}

pub(super) fn scale_rect_from_center(rect: LayoutRect, progress: f32) -> LayoutRect {
    let progress = progress.clamp(0.0, 1.0);
    let width = (rect.width() * progress).max(1.0);
    let height = (rect.height() * progress).max(1.0);
    LayoutRect::new(
        rect.x() + (rect.width() - width) / 2.0,
        rect.y() + (rect.height() - height) / 2.0,
        width,
        height,
    )
}

pub(super) fn color_luma(color: Color) -> f32 {
    color.r as f32 * 0.2126 + color.g as f32 * 0.7152 + color.b as f32 * 0.0722
}

pub(super) fn translate_path(path: &str, dx: f32, dy: f32) -> String {
    if dx == 0.0 && dy == 0.0 {
        path.to_string()
    } else {
        // Sankey paths are relative to the plot origin and use M/C/L/Z commands.
        // Rebuild the coordinates with a simple command-aware parser.
        let tokens: Vec<&str> = path.split_whitespace().collect();
        let mut result = String::new();
        let mut idx = 0;
        while idx < tokens.len() {
            let cmd = tokens[idx];
            result.push_str(cmd);
            idx += 1;
            let coord_count = match cmd {
                "M" | "L" => 2,
                "C" => 6,
                "Z" => 0,
                _ => 0,
            };
            for coord_idx in 0..coord_count {
                if let Some(raw) = tokens.get(idx) {
                    let offset = if coord_idx % 2 == 0 { dx } else { dy };
                    let value = raw.parse::<f32>().unwrap_or(0.0) + offset;
                    result.push_str(&format!(" {}", value));
                    idx += 1;
                }
            }
            result.push(' ');
        }
        result
    }
}

#[derive(Debug, Clone)]
pub(super) struct TreeRenderNode {
    pub(super) name: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) depth: usize,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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

pub(super) fn normalize_bounds(min: f32, max: f32) -> (f32, f32) {
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if (max - min).abs() < f32::EPSILON {
        (min - 1.0, max + 1.0)
    } else {
        (min, max)
    }
}

pub(super) fn add_rect(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    rect: LayoutRect,
    fill: Color,
    stroke_value: Option<Stroke>,
    radius: f32,
) {
    add_positioned_paint(
        cx,
        root,
        rect,
        fission_ir::Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(fill)),
            stroke: stroke_value,
            corner_radius: radius,
            shadow: None,
        }),
    );
}

pub(super) fn add_text(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    text: &str,
    size: f32,
    color: Color,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
) {
    add_positioned_paint(
        cx,
        root,
        LayoutRect::new(left, top, width.max(1.0), height.max(1.0)),
        fission_ir::Op::Paint(PaintOp::DrawText {
            text: text.to_string(),
            size,
            color,
            underline: false,
            wrap: false,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
    );
}

pub(super) fn add_positioned_paint(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    rect: LayoutRect,
    op: fission_ir::Op,
) {
    let paint_id = cx.next_node_id();
    let mut pos = fission_core::internal::InternalIrBuilder::new(
        cx.next_node_id(),
        fission_ir::Op::Layout(LayoutOp::Positioned {
            left: Some(rect.x()),
            top: Some(rect.y()),
            right: None,
            bottom: None,
            width: Some(rect.width()),
            height: Some(rect.height()),
        }),
    );
    pos.add_child(cx.insert_node(paint_id, op, vec![]));
    root.add_child(pos.build(cx));
}

pub(super) fn add_path(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    path: &str,
    fill: Option<Fill>,
    stroke_value: Option<Stroke>,
) {
    let id = cx.next_node_id();
    root.add_child(cx.insert_node(
        id,
        fission_ir::Op::Paint(PaintOp::DrawPath {
            path: path.to_string(),
            fill,
            stroke: stroke_value,
        }),
        vec![],
    ));
}

pub(super) fn stroke(color: Color, width: f32) -> Stroke {
    Stroke {
        fill: Fill::Solid(color),
        width,
        dash_array: None,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
    }
}

pub(super) fn format_tick(value: f32) -> String {
    if value.abs() >= 1000.0 {
        format!("{:.1}k", value / 1000.0)
    } else if value.fract().abs() < 0.001 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    }
}

pub(super) fn color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}
