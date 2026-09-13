//! Components drawn over the plot: legend, marks, visual map, zoom, brush, graphics, timeline, toolbox and diagnostics.

use super::*;

/// Legends narrower than this flow their entries in rows under the title.
const COMPACT_LEGEND_WIDTH: f32 = 420.0;
const LEGEND_ROW_HEIGHT: f32 = 20.0;
const LEGEND_ENTRY_HEIGHT: f32 = 16.0;
const LEGEND_SWATCH: f32 = 10.0;
const LEGEND_LABEL_OFFSET: f32 = 16.0;
const LEGEND_FONT_SIZE: f32 = 11.0;

/// One legend entry and where it sits in the chart.
pub(super) struct LegendItem {
    pub(super) index: usize,
    pub(super) name: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
}

impl LegendItem {
    /// The area a press toggles this entry from.
    pub(super) fn bounds(&self) -> LayoutRect {
        LayoutRect::new(self.x, self.y, self.width, LEGEND_ENTRY_HEIGHT)
    }
}

/// Where each legend entry goes: flowed in rows on narrow charts, otherwise in
/// a column beside the plot.
pub(super) fn legend_items(model: &ChartModel, chart: &Chart, area: &ChartArea) -> Vec<LegendItem> {
    if chart.legend.is_none() {
        return Vec::new();
    }
    let names = series_names(model);
    if area.outer_w < COMPACT_LEGEND_WIDTH {
        let mut x = 20.0;
        let mut y = if chart.title.is_some() { 54.0 } else { 24.0 };
        names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let width = 28.0 + name.chars().count() as f32 * 6.5;
                if x > 20.0 && x + width > area.outer_w - 20.0 {
                    x = 20.0;
                    y += LEGEND_ROW_HEIGHT;
                }
                let item = LegendItem {
                    index,
                    name,
                    x,
                    y,
                    width,
                };
                x += width;
                item
            })
            .collect()
    } else {
        let x = area.plot.right() + 18.0;
        names
            .into_iter()
            .enumerate()
            .map(|(index, name)| LegendItem {
                index,
                name,
                x,
                y: area.plot.y() + index as f32 * LEGEND_ROW_HEIGHT,
                width: LEGEND_LABEL_OFFSET + 110.0,
            })
            .collect()
    }
}

pub(super) fn draw_legend(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    for item in legend_items(model, chart, area) {
        // Hidden entries stay in place, muted, so they can be turned back on.
        let (swatch, label) = if model.is_hidden(item.index) {
            (theme.grid_line, theme.grid_line)
        } else {
            (series_color(model, theme, item.index), theme.label)
        };
        add_rect(
            cx,
            root,
            LayoutRect::new(item.x, item.y + 3.0, LEGEND_SWATCH, LEGEND_SWATCH),
            swatch,
            None,
            2.0,
        );
        add_text(
            cx,
            root,
            &item.name,
            LEGEND_FONT_SIZE,
            label,
            item.x + LEGEND_LABEL_OFFSET,
            item.y,
            item.width - LEGEND_LABEL_OFFSET,
            LEGEND_ENTRY_HEIGHT,
        );
    }
}

pub(super) fn draw_mark_areas(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
) {
    if chart.mark_areas.is_empty() || !model.has_cartesian_series() {
        return;
    }
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for mark in &chart.mark_areas {
        let y0 = map_y(mark.y_min, area, &y_scale);
        let y1 = map_y(mark.y_max, area, &y_scale);
        add_rect(
            cx,
            root,
            LayoutRect::new(
                area.plot.x(),
                y0.min(y1),
                area.plot.width(),
                (y0 - y1).abs().max(1.0),
            ),
            mark.color,
            None,
            0.0,
        );
    }
}

pub(super) fn draw_mark_lines(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.mark_lines.is_empty() || !model.has_cartesian_series() {
        return;
    }
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for mark in &chart.mark_lines {
        let y = map_y(mark.y, area, &y_scale);
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", area.plot.x(), y, area.plot.right(), y),
            None,
            Some(stroke(mark.color, mark.width)),
        );
        add_text(
            cx,
            root,
            &mark.name,
            10.0,
            theme.label,
            area.plot.right() - 90.0,
            y - 16.0,
            86.0,
            14.0,
        );
    }
}

pub(super) fn draw_mark_points(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.mark_points.is_empty() || !model.has_cartesian_series() {
        return;
    }
    let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for mark in &chart.mark_points {
        let x = if model.x_axis.axis_type == AxisType::Category {
            mark.x
                .map(|x| map_category_x(x.round().max(0.0) as usize, model, area))
                .unwrap_or(area.plot.x() + area.plot.width() / 2.0)
        } else {
            map_x(mark.x.unwrap_or(model.x_domain.0), area, &x_scale)
        };
        let y = map_y(mark.y, area, &y_scale);
        add_rect(
            cx,
            root,
            LayoutRect::new(x - 5.0, y - 5.0, 10.0, 10.0),
            mark.color,
            Some(stroke(Color::WHITE, 1.0)),
            5.0,
        );
        add_text(
            cx,
            root,
            &mark.name,
            10.0,
            theme.label,
            x + 8.0,
            y - 8.0,
            90.0,
            14.0,
        );
    }
}

pub(super) fn draw_visual_map(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(map) = chart.visual_map.as_ref() else {
        return;
    };
    let x = area.plot.right() + 24.0;
    let y = area.plot.bottom() - 110.0;
    let h = 90.0;
    add_rect(
        cx,
        root,
        LayoutRect::new(x, y, 12.0, h),
        color(255, 255, 255, 255),
        Some(stroke(theme.grid_line, 1.0)),
        2.0,
    );
    for i in 0..18 {
        let t = i as f32 / 17.0;
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x + 1.0,
                y + h - (i as f32 + 1.0) * h / 18.0,
                10.0,
                h / 18.0 + 0.5,
            ),
            visual_color_at(map, t),
            None,
            0.0,
        );
    }
    add_text(
        cx,
        root,
        &format_tick(map.max),
        10.0,
        theme.label,
        x + 18.0,
        y - 2.0,
        70.0,
        14.0,
    );
    add_text(
        cx,
        root,
        &format_tick(map.min),
        10.0,
        theme.label,
        x + 18.0,
        y + h - 12.0,
        70.0,
        14.0,
    );
}

pub(super) fn draw_data_zoom(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(zoom) = chart.data_zoom.as_ref() else {
        return;
    };
    let track = zoom_track(area);
    let (x, y, w) = (track.x(), track.y(), track.width());
    add_rect(
        cx,
        root,
        LayoutRect::new(x, y, w, 8.0),
        theme.grid_line,
        None,
        4.0,
    );
    let start = (zoom.start_percent / 100.0).clamp(0.0, 1.0);
    let end = (zoom.end_percent / 100.0).clamp(start, 1.0);
    add_rect(
        cx,
        root,
        LayoutRect::new(x + w * start, y - 2.0, w * (end - start), 12.0),
        theme.palette[0].with_alpha(180),
        None,
        6.0,
    );
    for edge in [start, end] {
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x + w * edge - ZOOM_HANDLE_WIDTH / 2.0,
                y - ZOOM_HANDLE_OVERHANG,
                ZOOM_HANDLE_WIDTH,
                track.height() + ZOOM_HANDLE_OVERHANG * 2.0,
            ),
            theme.axis_line,
            None,
            ZOOM_HANDLE_WIDTH / 2.0,
        );
    }
}

const ZOOM_HANDLE_WIDTH: f32 = 6.0;
const ZOOM_HANDLE_OVERHANG: f32 = 4.0;

/// The data-zoom slider's track, below the x axis.
pub(super) fn zoom_track(area: &ChartArea) -> LayoutRect {
    LayoutRect::new(
        area.plot.x(),
        area.plot.bottom() + 36.0,
        area.plot.width(),
        8.0,
    )
}

pub(super) fn draw_brush(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(brush) = chart.interaction.brush.as_ref() else {
        return;
    };
    let Some((x, y, width, height)) = brush.preview_rect else {
        return;
    };
    let rect = LayoutRect::new(
        area.plot.x() + x * area.plot.width(),
        area.plot.y() + y * area.plot.height(),
        width * area.plot.width(),
        height * area.plot.height(),
    );
    add_rect(
        cx,
        root,
        rect,
        theme.palette[0].with_alpha(42),
        Some(stroke(theme.palette[0].with_alpha(190), 1.4)),
        3.0,
    );
}

pub(super) fn draw_graphics(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    for graphic in &chart.graphics {
        let x = area.plot.x() + graphic.x * area.plot.width();
        let y = area.plot.y() + graphic.y * area.plot.height();
        let width = graphic.width * area.plot.width();
        let height = graphic.height * area.plot.height();
        match graphic.kind {
            ChartGraphicKind::Rect => add_rect(
                cx,
                root,
                LayoutRect::new(x, y, width, height),
                graphic.color,
                graphic.stroke.map(|color| stroke(color, 1.0)),
                4.0,
            ),
            ChartGraphicKind::Circle => {
                let r = width.min(height) / 2.0;
                add_rect(
                    cx,
                    root,
                    LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
                    graphic.color,
                    graphic.stroke.map(|color| stroke(color, 1.0)),
                    r,
                );
            }
            ChartGraphicKind::Text => {
                if let Some(text) = graphic.text.as_ref() {
                    add_text(cx, root, text, 12.0, graphic.color, x, y, width, height);
                }
            }
            ChartGraphicKind::Line => add_path(
                cx,
                root,
                &format!("M {} {} L {} {}", x, y, x + width, y + height),
                None,
                Some(stroke(graphic.color, 1.8)),
            ),
        }
    }
    if !chart.graphics.is_empty() {
        add_text(
            cx,
            root,
            "graphic layer",
            10.0,
            theme.label,
            area.plot.x() + 8.0,
            area.plot.y() + 8.0,
            110.0,
            14.0,
        );
    }
}

pub(super) fn draw_timeline(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(timeline) = chart.timeline.as_ref() else {
        return;
    };
    if timeline.labels.is_empty() {
        return;
    }

    let x = area.plot.x();
    let y = area.outer_h - 30.0;
    let w = area.plot.width();
    add_path(
        cx,
        root,
        &format!("M {} {} L {} {}", x, y, x + w, y),
        None,
        Some(stroke(theme.grid_line, 2.0)),
    );
    let denom = timeline.labels.len().saturating_sub(1).max(1) as f32;
    for (idx, label) in timeline.labels.iter().enumerate() {
        let px = x + idx as f32 / denom * w;
        let active = idx == timeline.current_index.min(timeline.labels.len() - 1);
        let r = if active { 6.0 } else { 4.0 };
        add_rect(
            cx,
            root,
            LayoutRect::new(px - r, y - r, r * 2.0, r * 2.0),
            if active {
                theme.palette[0]
            } else {
                theme.axis_line
            },
            Some(stroke(Color::WHITE, 1.0)),
            r,
        );
        add_text(
            cx,
            root,
            label,
            10.0,
            theme.label,
            px - 28.0,
            y + 8.0,
            56.0,
            14.0,
        );
    }
}

pub(super) fn draw_toolbox(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.interaction.toolbox_actions.is_empty() {
        return;
    }

    let mut x = area.plot.right() - chart.interaction.toolbox_actions.len() as f32 * 54.0;
    let y = 18.0;
    for action in &chart.interaction.toolbox_actions {
        let label = match action {
            crate::interaction::ChartToolAction::Restore => "reset",
            crate::interaction::ChartToolAction::SaveImage => "save",
            crate::interaction::ChartToolAction::DataZoom => "zoom",
            crate::interaction::ChartToolAction::Brush => "brush",
        };
        add_rect(
            cx,
            root,
            LayoutRect::new(x, y, 48.0, 22.0),
            theme.plot_background,
            Some(stroke(theme.grid_line, 1.0)),
            5.0,
        );
        add_text(
            cx,
            root,
            label,
            10.0,
            theme.label,
            x + 5.0,
            y + 4.0,
            38.0,
            14.0,
        );
        x += 54.0;
    }
}

pub(super) fn draw_diagnostics(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    model: &ChartModel,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    for (idx, diagnostic) in model.diagnostics.iter().enumerate() {
        let text = if let Some(name) = diagnostic.series_name.as_ref() {
            format!("{}: {}", name, diagnostic.message)
        } else {
            diagnostic.message.clone()
        };
        add_text(
            cx,
            root,
            &text,
            12.0,
            theme.diagnostic,
            area.plot.x() + 12.0,
            area.plot.y() + 16.0 + idx as f32 * 18.0,
            area.plot.width() - 24.0,
            16.0,
        );
    }
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
