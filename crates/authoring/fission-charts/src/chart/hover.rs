//! Hover feedback: the axis pointer, point highlights and the tooltip.

use super::*;
use crate::components::AxisPointerType;
use crate::interaction::{ChartLegendSelectionMode, ChartLegendToggled, ChartTooltipTrigger};
use fission_core::ReducerContext;

/// How far the tooltip sits from the pointer.
const TOOLTIP_OFFSET: f32 = 14.0;
/// The closest the tooltip comes to the chart's edges.
const EDGE_INSET: f32 = 4.0;
/// The approximate advance of one character, as a fraction of the font size.
const CHAR_ADVANCE: f32 = 0.6;
/// Tooltip rows are this many font sizes tall.
const LINE_HEIGHT: f32 = 1.5;
const MARKER_SIZE: f32 = 8.0;
const MARKER_GAP: f32 = 6.0;
const VALUE_GAP: f32 = 16.0;
const HALO_RADIUS: f32 = 5.0;
const HALO_STROKE: f32 = 2.0;
const POINTER_DASH: f32 = 4.0;
const SHADOW_ALPHA: u8 = 90;

/// The hover a chart remembers between builds.
#[derive(Debug, Clone, Default)]
pub(super) struct HoverMemory(pub(super) Option<ChartHover>);

impl fission_core::GlobalState for HoverMemory {}

pub(super) fn on_hover_changed(
    memory: &mut HoverMemory,
    _: ChartHoverChanged,
    cx: &mut ReducerContext<'_, '_, '_, HoverMemory>,
) {
    if let Some((_, event)) = ChartInteractionEvent::from_action_input(cx.input) {
        memory.0 = Some(ChartHover {
            x: event.local_x,
            y: event.local_y,
            hit: event.hit,
        });
    }
}

pub(super) fn on_hover_cleared(
    memory: &mut HoverMemory,
    _: ChartHoverCleared,
    _: &mut ReducerContext<'_, '_, '_, HoverMemory>,
) {
    memory.0 = None;
}

/// Whether the chart shows any hover feedback, and so needs to track the pointer.
pub(super) fn tracks_hover(chart: &Chart) -> bool {
    tooltip_trigger(chart) != ChartTooltipTrigger::None
        || chart.axis_pointer.is_some()
        || chart.interaction.emphasis.enabled
}

/// The interaction's trigger, or the one named by the chart's `Tooltip`.
fn tooltip_trigger(chart: &Chart) -> ChartTooltipTrigger {
    if chart.interaction.tooltip_trigger != ChartTooltipTrigger::None {
        return chart.interaction.tooltip_trigger;
    }
    match chart
        .tooltip
        .as_ref()
        .map(|tooltip| tooltip.trigger.as_str())
    {
        Some("axis") => ChartTooltipTrigger::Axis,
        Some("item") => ChartTooltipTrigger::Item,
        _ => ChartTooltipTrigger::None,
    }
}

pub(super) fn draw_hover(
    cx: &mut fission_core::internal::LoweringContext,
    root: &mut fission_core::internal::IrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(hover) = chart.hover.as_ref() else {
        return;
    };
    let pointer = LayoutPoint::new(hover.x, hover.y);
    if !area.plot.contains(pointer) {
        return;
    }
    let scene = HoverScene {
        model,
        area,
        theme,
        pointer,
        category: hovered_category(model, area, pointer),
    };
    let trigger = tooltip_trigger(chart);

    let pointer_type = chart
        .axis_pointer
        .as_ref()
        .map(|axis_pointer| axis_pointer.pointer_type)
        .or((trigger == ChartTooltipTrigger::Axis).then_some(AxisPointerType::Line));
    if let Some(pointer_type) = pointer_type {
        scene.draw_axis_pointer(cx, root, pointer_type);
    }

    let highlighted = match trigger {
        ChartTooltipTrigger::Axis => scene.category,
        _ if chart.interaction.emphasis.enabled => {
            hover.hit.as_ref().and_then(|hit| hit.data_index)
        }
        _ => None,
    };
    if let Some(index) = highlighted {
        scene.draw_line_halos(cx, root, index);
    }

    let (header, rows) = match trigger {
        ChartTooltipTrigger::Axis => match scene.category {
            Some(index) => (
                model.x_categories.get(index).cloned(),
                scene.axis_rows(index),
            ),
            None => (None, Vec::new()),
        },
        ChartTooltipTrigger::Item => match hover.hit.as_ref() {
            Some(hit) => (
                hit.data_index
                    .and_then(|index| model.x_categories.get(index).cloned()),
                scene.item_rows(hit),
            ),
            None => (None, Vec::new()),
        },
        ChartTooltipTrigger::None => (None, Vec::new()),
    };
    if !rows.is_empty() {
        scene.draw_tooltip(cx, root, header.as_deref(), &rows);
    }
}

/// The index of the category band under `pointer`, when the x axis has categories.
fn hovered_category(model: &ChartModel, area: &ChartArea, pointer: LayoutPoint) -> Option<usize> {
    let count = model.x_categories.len();
    if count == 0 || area.plot.width() <= 0.0 {
        return None;
    }
    let fraction = (pointer.x - area.plot.x()) / area.plot.width();
    Some(((fraction * count as f32).floor().max(0.0) as usize).min(count - 1))
}

/// One series' entry in the tooltip.
struct TooltipRow {
    color: Color,
    label: String,
    value: String,
}

/// What hover feedback is drawn against.
struct HoverScene<'a> {
    model: &'a ChartModel,
    area: &'a ChartArea,
    theme: &'a ChartTheme,
    pointer: LayoutPoint,
    category: Option<usize>,
}

impl HoverScene<'_> {
    fn series_color(&self, series_index: usize) -> Color {
        series_color(self.model, self.theme, series_index)
    }

    /// A line along the hovered category, a band shading it, or a cross through
    /// the pointer.
    fn draw_axis_pointer(
        &self,
        cx: &mut fission_core::internal::LoweringContext,
        root: &mut fission_core::internal::IrBuilder,
        pointer_type: AxisPointerType,
    ) {
        let plot = self.area.plot;
        let x = self
            .category
            .map(|index| map_category_x(index, self.model, self.area))
            .unwrap_or(self.pointer.x);
        let dashed = Stroke {
            dash_array: Some(vec![POINTER_DASH, POINTER_DASH]),
            ..stroke(self.theme.axis_line, 1.0)
        };
        if pointer_type == AxisPointerType::Shadow && self.category.is_some() {
            let band = band_width(self.model, self.area);
            add_rect(
                cx,
                root,
                LayoutRect::new(x - band / 2.0, plot.y(), band, plot.height()),
                self.theme.grid_line.with_alpha(SHADOW_ALPHA),
                None,
                0.0,
            );
            return;
        }
        add_path(
            cx,
            root,
            &format!("M {x} {} L {x} {}", plot.y(), plot.bottom()),
            None,
            Some(dashed.clone()),
        );
        if pointer_type == AxisPointerType::Cross {
            let y = self.pointer.y;
            add_path(
                cx,
                root,
                &format!("M {} {y} L {} {y}", plot.x(), plot.right()),
                None,
                Some(dashed),
            );
        }
    }

    /// A ring around each line series' point in the category.
    fn draw_line_halos(
        &self,
        cx: &mut fission_core::internal::LoweringContext,
        root: &mut fission_core::internal::IrBuilder,
        index: usize,
    ) {
        if self.model.x_categories.is_empty() {
            return;
        }
        let scale = LinearScale::nice(self.model.y_domain.0, self.model.y_domain.1, 6);
        for (series_index, series) in self.model.series.iter().enumerate() {
            if self.model.is_hidden(series_index) {
                continue;
            }
            let ResolvedSeries::Line(line) = series else {
                continue;
            };
            let Some(value) = line.values.get(index) else {
                continue;
            };
            let x = map_category_x(index, self.model, self.area);
            let y = map_y(*value, self.area, &scale);
            add_path(
                cx,
                root,
                &circle_path(x, y, HALO_RADIUS),
                Some(Fill::Solid(self.theme.background)),
                Some(stroke(self.series_color(series_index), HALO_STROKE)),
            );
        }
    }

    /// Every category series' value at `index`, in series order.
    fn axis_rows(&self, index: usize) -> Vec<TooltipRow> {
        let names = series_names(self.model);
        self.model
            .series
            .iter()
            .enumerate()
            .filter(|(series_index, _)| !self.model.is_hidden(*series_index))
            .filter_map(|(series_index, series)| {
                let values = match series {
                    ResolvedSeries::Line(line) => &line.values,
                    ResolvedSeries::Bar(bar) => &bar.values,
                    _ => return None,
                };
                Some(TooltipRow {
                    color: self.series_color(series_index),
                    label: names[series_index].clone(),
                    value: format_tick(*values.get(index)?),
                })
            })
            .collect()
    }

    /// The hit item's series and value.
    fn item_rows(&self, hit: &ChartHit) -> Vec<TooltipRow> {
        let Some(series_index) = hit.series_index else {
            return Vec::new();
        };
        let label = hit
            .series_name
            .clone()
            .or_else(|| series_names(self.model).get(series_index).cloned())
            .unwrap_or_default();
        let value = hit
            .value_y
            .or(hit.value_x)
            .map(format_tick)
            .unwrap_or_default();
        vec![TooltipRow {
            color: self.series_color(series_index),
            label,
            value,
        }]
    }

    /// A card beside the pointer, flipped to stay inside the chart, styled by
    /// the theme's tooltip component.
    fn draw_tooltip(
        &self,
        cx: &mut fission_core::internal::LoweringContext,
        root: &mut fission_core::internal::IrBuilder,
        header: Option<&str>,
        rows: &[TooltipRow],
    ) {
        let style = cx.env().theme.components.tooltip.clone();
        let font = style.font_size;
        let line = font * LINE_HEIGHT;
        let text_width = |text: &str| text.chars().count() as f32 * font * CHAR_ADVANCE;
        let content_width = rows
            .iter()
            .map(|row| {
                MARKER_SIZE
                    + MARKER_GAP
                    + text_width(&row.label)
                    + VALUE_GAP
                    + text_width(&row.value)
            })
            .fold(header.map_or(0.0, text_width), f32::max);
        let width = (content_width + style.padding_x * 2.0).min(style.max_width.max(content_width));
        let line_count = rows.len() + usize::from(header.is_some());
        let height = line_count as f32 * line + style.padding_y * 2.0;

        let mut left = self.pointer.x + TOOLTIP_OFFSET;
        if left + width > self.area.outer_w - EDGE_INSET {
            left = self.pointer.x - TOOLTIP_OFFSET - width;
        }
        let mut top = self.pointer.y + TOOLTIP_OFFSET;
        if top + height > self.area.outer_h - EDGE_INSET {
            top = self.pointer.y - TOOLTIP_OFFSET - height;
        }
        let left = left.max(EDGE_INSET);
        let top = top.max(EDGE_INSET);

        add_rect(
            cx,
            root,
            LayoutRect::new(left, top, width, height),
            style.bg_color,
            None,
            style.radius,
        );
        let inner_left = left + style.padding_x;
        let inner_right = left + width - style.padding_x;
        let mut y = top + style.padding_y;
        if let Some(header) = header {
            add_text(
                cx,
                root,
                header,
                font,
                style.text_color,
                inner_left,
                y,
                inner_right - inner_left,
                line,
            );
            y += line;
        }
        for row in rows {
            add_rect(
                cx,
                root,
                LayoutRect::new(
                    inner_left,
                    y + (line - MARKER_SIZE) / 2.0,
                    MARKER_SIZE,
                    MARKER_SIZE,
                ),
                row.color,
                None,
                MARKER_SIZE / 2.0,
            );
            let label_left = inner_left + MARKER_SIZE + MARKER_GAP;
            add_text(
                cx,
                root,
                &row.label,
                font,
                style.text_color,
                label_left,
                y,
                text_width(&row.label),
                line,
            );
            let value_width = text_width(&row.value);
            add_text(
                cx,
                root,
                &row.value,
                font,
                style.text_color,
                inner_right - value_width,
                y,
                value_width,
                line,
            );
            y += line;
        }
    }
}

/// The series a chart's legend has hidden, remembered between builds.
#[derive(Debug, Clone, Default)]
pub(super) struct LegendMemory(pub(super) Vec<String>);

impl fission_core::GlobalState for LegendMemory {}

/// Whether pressing legend entries changes which series show.
pub(super) fn tracks_legend(chart: &Chart) -> bool {
    chart.legend.is_some() && chart.interaction.legend_selection != ChartLegendSelectionMode::Static
}

/// Toggles the pressed series, or in single mode shows only it, and shows every
/// series again when the isolated one is pressed a second time.
pub(super) fn on_legend_toggled(
    memory: &mut LegendMemory,
    action: ChartLegendToggled,
    _: &mut ReducerContext<'_, '_, '_, LegendMemory>,
) {
    let hidden = &mut memory.0;
    match action.mode {
        ChartLegendSelectionMode::Static => {}
        ChartLegendSelectionMode::Toggle => {
            if let Some(position) = hidden.iter().position(|name| *name == action.series) {
                hidden.remove(position);
            } else {
                hidden.push(action.series);
            }
        }
        ChartLegendSelectionMode::Single => {
            let others: Vec<String> = action
                .all
                .into_iter()
                .filter(|name| *name != action.series)
                .collect();
            let isolated =
                hidden.len() == others.len() && others.iter().all(|name| hidden.contains(name));
            *hidden = if isolated { Vec::new() } else { others };
        }
    }
}
