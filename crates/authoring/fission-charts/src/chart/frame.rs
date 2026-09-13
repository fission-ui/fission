//! The chart's background, title and cartesian axes.

use super::*;

pub(super) fn draw_background(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
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
        let widest = model
            .x_categories
            .iter()
            .map(|label| label.chars().count())
            .max()
            .unwrap_or(0) as f32
            * AXIS_LABEL_SIZE
            * AXIS_LABEL_ADVANCE;
        let rotation = model
            .x_axis
            .label_rotate
            .filter(|degrees| degrees.abs() > MIN_LABEL_ROTATION);
        let needed = match rotation {
            // A slanted label only needs its line height of room across the band.
            Some(degrees) => {
                (AXIS_LABEL_HEIGHT / degrees.to_radians().sin().abs()).min(widest + AXIS_LABEL_GAP)
            }
            None => widest + AXIS_LABEL_GAP,
        };
        let step = label_step(needed, band);
        for (idx, label) in model.x_categories.iter().enumerate().step_by(step) {
            let x = map_category_x(idx, model, area);
            if let Some(degrees) = rotation {
                add_rotated_label(
                    cx,
                    root,
                    label,
                    theme.label,
                    (x, area.plot.bottom() + 8.0),
                    degrees,
                );
                continue;
            }
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
        let step = label_step(AXIS_LABEL_HEIGHT, band);
        for (idx, label) in model.y_categories.iter().enumerate().step_by(step) {
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

/// The size category axis labels are drawn at.
const AXIS_LABEL_SIZE: f32 = 11.0;
/// The approximate advance of one label character, as a fraction of its size.
const AXIS_LABEL_ADVANCE: f32 = 0.6;
/// The space kept between neighbouring horizontal labels.
const AXIS_LABEL_GAP: f32 = 8.0;
/// The height a vertical category label needs.
const AXIS_LABEL_HEIGHT: f32 = 16.0;
/// Rotations smaller than this, in degrees, draw labels upright.
const MIN_LABEL_ROTATION: f32 = 0.5;

/// Draws a category label turned `degrees` counter-clockwise, with the end
/// nearest the axis at `anchor`: a positive angle slants the label up to the
/// right so it ends at its tick, and a negative one slants it down to the right
/// from its tick. Label width is estimated from its character count.
fn add_rotated_label(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    text: &str,
    color: Color,
    anchor: (f32, f32),
    degrees: f32,
) {
    let width = text.chars().count() as f32 * AXIS_LABEL_SIZE * AXIS_LABEL_ADVANCE + 2.0;
    let height = AXIS_LABEL_HEIGHT;
    let radians = degrees.to_radians();
    // Screen y grows downwards, so the label's reading direction is
    // (cos, -sin), and the box centre sits half a label back from the anchor
    // along it for positive angles, or forward for negative ones.
    let side = if degrees >= 0.0 { -1.0 } else { 1.0 };
    let centre = (
        anchor.0 + side * width / 2.0 * radians.cos(),
        anchor.1 - side * width / 2.0 * radians.sin(),
    );
    let paint_id = cx.next_node_id();
    let mut label = fission_core::internal::IrBuilder::new(
        cx.next_node_id(),
        fission_ir::Op::Layout(LayoutOp::Positioned {
            left: Some(centre.0 - width / 2.0),
            top: Some(centre.1 - height / 2.0),
            right: None,
            bottom: None,
            width: Some(width),
            height: Some(height),
        }),
    )
    // Composite rotation turns about the box centre, clockwise for positive values.
    .composite(fission_ir::op::CompositeStyle {
        rotation: Some(fission_ir::op::CompositeScalar::new(-radians)),
        ..Default::default()
    });
    label.add_child(
        fission_core::internal::IrBuilder::new(
            paint_id,
            fission_ir::Op::Paint(PaintOp::DrawText {
                text: text.to_string(),
                size: AXIS_LABEL_SIZE,
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
        )
        .build(cx),
    );
    root.add_child(label.build(cx));
}

/// The extra room below the plot that slanted category labels need beyond an
/// upright label's.
pub(super) fn rotated_label_room(chart: &Chart) -> f32 {
    let Some(axis) = chart
        .x_axis
        .as_ref()
        .filter(|axis| axis.axis_type == AxisType::Category)
    else {
        return 0.0;
    };
    let Some(degrees) = axis
        .label_rotate
        .filter(|degrees| degrees.abs() > MIN_LABEL_ROTATION)
    else {
        return 0.0;
    };
    let widest = axis
        .data
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0) as f32
        * AXIS_LABEL_SIZE
        * AXIS_LABEL_ADVANCE;
    let radians = degrees.to_radians();
    let drop = widest * radians.sin().abs() + AXIS_LABEL_HEIGHT * radians.cos().abs();
    (drop - AXIS_LABEL_HEIGHT).max(0.0)
}

/// How many categories to advance between drawn labels so a label needing
/// `needed` points never overlaps its neighbour in bands `band` points wide:
/// every label when they fit, otherwise every second, third and so on,
/// starting with the first.
fn label_step(needed: f32, band: f32) -> usize {
    if band <= 0.0 {
        return 1;
    }
    (needed / band).ceil().max(1.0) as usize
}

#[cfg(test)]
mod label_tests {
    use super::label_step;

    #[test]
    fn labels_that_fit_are_all_drawn_and_crowded_ones_are_thinned() {
        assert_eq!(label_step(40.0, 60.0), 1);
        assert_eq!(label_step(40.0, 20.0), 2);
        assert_eq!(label_step(41.0, 20.0), 3);
        assert_eq!(label_step(40.0, 0.0), 1);
    }
}
