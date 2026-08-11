use super::helpers::*;
use super::*;

pub(super) fn draw_legend(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.legend.is_none() {
        return;
    }
    if area.outer_w < 420.0 {
        let mut x = 20.0;
        let mut y = if chart.title.is_some() { 54.0 } else { 24.0 };
        for (idx, name) in series_names(model).iter().enumerate() {
            let item_width = 28.0 + name.chars().count() as f32 * 6.5;
            if x > 20.0 && x + item_width > area.outer_w - 20.0 {
                x = 20.0;
                y += 20.0;
            }
            add_rect(
                cx,
                root,
                LayoutRect::new(x, y + 3.0, 10.0, 10.0),
                theme.palette[idx % theme.palette.len()],
                None,
                2.0,
            );
            add_text(
                cx,
                root,
                name,
                11.0,
                theme.label,
                x + 16.0,
                y,
                item_width,
                16.0,
            );
            x += item_width;
        }
        return;
    }
    let mut y = area.plot.y();
    let x = area.plot.right() + 18.0;
    for (idx, name) in series_names(model).iter().enumerate() {
        add_rect(
            cx,
            root,
            LayoutRect::new(x, y + 3.0, 10.0, 10.0),
            theme.palette[idx % theme.palette.len()],
            None,
            2.0,
        );
        add_text(cx, root, name, 11.0, theme.label, x + 16.0, y, 110.0, 16.0);
        y += 20.0;
    }
}

pub(super) fn draw_mark_areas(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(zoom) = chart.data_zoom.as_ref() else {
        return;
    };
    let x = area.plot.x();
    let y = area.plot.bottom() + 36.0;
    let w = area.plot.width();
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
}

pub(super) fn draw_brush(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
