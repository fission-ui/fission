use super::helpers::*;
use super::*;

pub(super) fn draw_background(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    add_rect(
        cx,
        root,
        LayoutRect::new(0.0, 0.0, area.outer_w, area.outer_h),
        theme.background,
        None,
        14.0,
    );
    add_rect(
        cx,
        root,
        area.plot,
        theme.plot_background,
        Some(stroke(theme.grid_line, 1.0)),
        8.0,
    );
}

pub(super) fn draw_title(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if let Some(title) = model.title.as_ref() {
        add_text(
            cx,
            root,
            title,
            18.0,
            theme.title,
            20.0,
            18.0,
            (area.outer_w - 40.0).max(1.0),
            28.0,
        );
    }
}

pub(super) fn draw_cartesian_axes(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for tick in &y_scale.ticks {
        let y = map_y(*tick, area, &y_scale);
        if model.y_axis.split_line {
            add_path(
                cx,
                root,
                &format!("M {} {} L {} {}", area.plot.x(), y, area.plot.right(), y),
                None,
                Some(stroke(theme.grid_line, 1.0)),
            );
        }
        add_text(
            cx,
            root,
            &format_tick(*tick),
            11.0,
            theme.label,
            8.0,
            y - 7.0,
            area.plot.x() - 14.0,
            14.0,
        );
    }

    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            area.plot.bottom(),
            area.plot.right(),
            area.plot.bottom()
        ),
        None,
        Some(stroke(theme.axis_line, 1.0)),
    );
    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            area.plot.y(),
            area.plot.x(),
            area.plot.bottom()
        ),
        None,
        Some(stroke(theme.axis_line, 1.0)),
    );

    if model.x_axis.axis_type == AxisType::Category && !model.x_categories.is_empty() {
        let band = band_width(model, area);
        for (idx, label) in model.x_categories.iter().enumerate() {
            let x = map_category_x(idx, model, area);
            add_text(
                cx,
                root,
                label,
                11.0,
                theme.label,
                x - band / 2.0,
                area.plot.bottom() + 8.0,
                band,
                18.0,
            );
        }
    } else if model.y_axis.axis_type == AxisType::Category && !model.y_categories.is_empty() {
        let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
        for tick in &x_scale.ticks {
            let x = map_x(*tick, area, &x_scale);
            add_text(
                cx,
                root,
                &format_tick(*tick),
                11.0,
                theme.label,
                x - 24.0,
                area.plot.bottom() + 8.0,
                48.0,
                18.0,
            );
        }
        let band = category_band_width(model.y_categories.len(), area.plot.height());
        for (idx, label) in model.y_categories.iter().enumerate() {
            let y = map_category_y(idx, model, area);
            add_text(
                cx,
                root,
                label,
                11.0,
                theme.label,
                8.0,
                y - band / 2.0,
                area.plot.x() - 14.0,
                band.max(16.0),
            );
        }
    } else {
        let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
        for tick in &x_scale.ticks {
            let x = map_x(*tick, area, &x_scale);
            add_text(
                cx,
                root,
                &format_tick(*tick),
                11.0,
                theme.label,
                x - 24.0,
                area.plot.bottom() + 8.0,
                48.0,
                18.0,
            );
        }
    }
}
