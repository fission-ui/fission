//! Shared path, scale, colour and IR emission helpers.

use super::*;

pub(super) fn count_bar_groups(model: &ChartModel) -> usize {
    model
        .series
        .iter()
        .enumerate()
        .filter(|(index, series)| {
            !model.is_hidden(*index)
                && matches!(series, ResolvedSeries::Bar(bar) if bar.source.stack.is_none())
        })
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

/// The indices of at most `threshold` points that keep the line's visual shape,
/// by largest-triangle-three-buckets: the first and last points always, and
/// from each bucket between them the point enclosing the largest triangle with
/// the previously kept point and the next bucket's average. Lines with no more
/// than `threshold` points, or thresholds below three, keep every index.
pub(super) fn lttb_indices(points: &[(f32, f32)], threshold: usize) -> Vec<usize> {
    let len = points.len();
    if threshold >= len || threshold < 3 {
        return (0..len).collect();
    }
    let every = (len - 2) as f32 / (threshold - 2) as f32;
    let bucket_start = |bucket: usize| ((bucket as f32 * every) as usize + 1).min(len - 1);
    let mut kept = Vec::with_capacity(threshold);
    kept.push(0);
    let mut previous = points[0];
    for bucket in 0..threshold - 2 {
        let start = bucket_start(bucket);
        let end = bucket_start(bucket + 1).max(start + 1);
        let next = &points[end.min(len - 1)..bucket_start(bucket + 2).max(end + 1).min(len)];
        let (sum_x, sum_y) = next
            .iter()
            .fold((0.0, 0.0), |(x, y), point| (x + point.0, y + point.1));
        let average = (sum_x / next.len() as f32, sum_y / next.len() as f32);
        let mut best = start;
        let mut best_area = -1.0_f32;
        for (index, point) in points.iter().enumerate().take(end).skip(start) {
            let area = ((previous.0 - average.0) * (point.1 - previous.1)
                - (previous.0 - point.0) * (average.1 - previous.1))
                .abs();
            if area > best_area {
                best = index;
                best_area = area;
            }
        }
        kept.push(best);
        previous = points[best];
    }
    kept.push(len - 1);
    kept
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
    fn faded(stops: Vec<(f32, Color)>, progress: f32) -> Vec<(f32, Color)> {
        stops
            .into_iter()
            .map(|(offset, color)| (offset, fade_color(color, progress)))
            .collect()
    }
    match fill {
        Fill::Solid(color) => Fill::Solid(fade_color(color, progress)),
        Fill::LinearGradient {
            start,
            end,
            stops,
            extend,
        } => Fill::LinearGradient {
            start,
            end,
            stops: faded(stops, progress),
            extend,
        },
        Fill::RadialGradient {
            center,
            radius,
            stops,
            extend,
        } => Fill::RadialGradient {
            center,
            radius,
            stops: faded(stops, progress),
            extend,
        },
        Fill::SweepGradient {
            center,
            start_angle,
            end_angle,
            stops,
            extend,
        } => Fill::SweepGradient {
            center,
            start_angle,
            end_angle,
            stops: faded(stops, progress),
            extend,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
            corner_radii: None,
            border_sides: None,
        }),
    );
}

pub(super) fn add_text(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
            locale: None,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    rect: LayoutRect,
    op: fission_ir::Op,
) {
    let paint_id = cx.next_node_id();
    let mut pos = fission_core::internal::IrBuilder::new(
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
    pos.add_child(fission_core::internal::IrBuilder::new(paint_id, op).build(cx));
    root.add_child(pos.build(cx));
}

pub(super) fn add_path(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    path: &str,
    fill: Option<Fill>,
    stroke_value: Option<Stroke>,
) {
    let id = cx.next_node_id();
    root.add_child(
        fission_core::internal::IrBuilder::new(
            id,
            fission_ir::Op::Paint(PaintOp::DrawPath {
                path: path.to_string(),
                fill,
                stroke: stroke_value,
            }),
        )
        .build(cx),
    );
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

/// The palette colour for the series at `index`.
pub(super) fn palette_color(theme: &ChartTheme, index: usize) -> Color {
    theme.palette[index % theme.palette.len()]
}

/// Gives every series without a colour the palette colour for its position,
/// the same colour the legend and tooltip show for it.
pub(super) fn apply_series_palette(model: &mut ChartModel, theme: &ChartTheme) {
    for (index, series) in model.series.iter_mut().enumerate() {
        if let Some(slot) = series_color_slot(series) {
            slot.get_or_insert(palette_color(theme, index));
        }
    }
}

/// The colour the series at `index` draws with.
pub(super) fn series_color(model: &ChartModel, theme: &ChartTheme, index: usize) -> Color {
    model
        .series
        .get(index)
        .and_then(|series| match series {
            ResolvedSeries::Line(s) => s.source.color,
            ResolvedSeries::Bar(s) => s.source.color,
            ResolvedSeries::Scatter(s) => s.color,
            ResolvedSeries::Bubble(s) => s.color,
            ResolvedSeries::Boxplot(s) => s.color,
            ResolvedSeries::EffectScatter(s) => s.color,
            ResolvedSeries::PolarBar(s) => s.color,
            ResolvedSeries::PolarLine(s) => s.color,
            ResolvedSeries::Liquidfill(s) => s.color,
            ResolvedSeries::SingleAxis(s) => s.color,
            ResolvedSeries::PictorialBar(s) => s.color,
            ResolvedSeries::Lines(s) => s.color,
            _ => None,
        })
        .unwrap_or_else(|| palette_color(theme, index))
}

fn series_color_slot(series: &mut ResolvedSeries) -> Option<&mut Option<Color>> {
    Some(match series {
        ResolvedSeries::Line(s) => &mut s.source.color,
        ResolvedSeries::Bar(s) => &mut s.source.color,
        ResolvedSeries::Scatter(s) => &mut s.color,
        ResolvedSeries::Bubble(s) => &mut s.color,
        ResolvedSeries::Boxplot(s) => &mut s.color,
        ResolvedSeries::EffectScatter(s) => &mut s.color,
        ResolvedSeries::PolarBar(s) => &mut s.color,
        ResolvedSeries::PolarLine(s) => &mut s.color,
        ResolvedSeries::Liquidfill(s) => &mut s.color,
        ResolvedSeries::SingleAxis(s) => &mut s.color,
        ResolvedSeries::PictorialBar(s) => &mut s.color,
        ResolvedSeries::Lines(s) => &mut s.color,
        _ => return None,
    })
}

/// A series colour after [`apply_series_palette`] has filled it in.
pub(super) fn filled(color: Option<Color>) -> Color {
    color.unwrap_or(Color::TRANSPARENT)
}

/// How much white an emphasised bar mixes into its colour.
const EMPHASIS_LIGHTEN: f32 = 0.18;
const EMPHASIS_SHADOW_ALPHA: u8 = 110;
const EMPHASIS_SHADOW_BLUR: f32 = 8.0;
const EMPHASIS_SHADOW_OFFSET: f32 = 2.0;

/// A bar, lightened and lifted on a soft shadow of its own colour when it is
/// the emphasised item.
pub(super) fn add_bar_rect(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    rect: LayoutRect,
    color: Color,
    radius: f32,
    emphasised: bool,
) {
    if !emphasised {
        add_rect(cx, root, rect, color, None, radius);
        return;
    }
    add_positioned_paint(
        cx,
        root,
        rect,
        fission_ir::Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(mix_color(
                color,
                Color::WHITE,
                EMPHASIS_LIGHTEN,
            ))),
            stroke: None,
            corner_radius: radius,
            shadow: Some(fission_ir::op::BoxShadow {
                color: color.with_alpha(EMPHASIS_SHADOW_ALPHA),
                blur_radius: EMPHASIS_SHADOW_BLUR,
                spread_radius: 0.0,
                offset: (0.0, EMPHASIS_SHADOW_OFFSET),
                inset: false,
            }),
            corner_radii: None,
            border_sides: None,
        }),
    );
}

#[cfg(test)]
mod sampling_tests {
    use super::lttb_indices;

    #[test]
    fn sparse_lines_keep_every_point() {
        let points: Vec<(f32, f32)> = (0..10).map(|i| (i as f32, 0.0)).collect();
        assert_eq!(lttb_indices(&points, 10), (0..10).collect::<Vec<_>>());
        assert_eq!(lttb_indices(&points, 2), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn dense_lines_keep_their_ends_and_spikes_in_order() {
        let mut points: Vec<(f32, f32)> = (0..1000).map(|i| (i as f32, 100.0)).collect();
        points[437].1 = 0.0;
        let kept = lttb_indices(&points, 50);

        assert_eq!(kept.len(), 50);
        assert_eq!(kept[0], 0);
        assert_eq!(kept[49], 999);
        assert!(kept.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(kept.contains(&437), "the spike must survive sampling");
    }
}
