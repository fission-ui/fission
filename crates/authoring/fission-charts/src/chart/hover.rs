//! Hover feedback: the axis pointer, point highlights and the tooltip.

use super::*;
use crate::components::AxisPointerType;
use crate::components::DataZoomType;
use crate::interaction::{
    ChartLegendSelectionMode, ChartLegendToggled, ChartTooltipTrigger, ChartZoomChanged,
};
use fission_core::event::ScrollDeltaMode;
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

        add_positioned_paint(
            cx,
            root,
            LayoutRect::new(left, top, width, height),
            fission_ir::Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(style.bg_color)),
                stroke: None,
                corner_radius: style.radius,
                shadow: style
                    .style
                    .shadows
                    .first()
                    .map(|layer| layer.to_box_shadow()),
                corner_radii: None,
                border_sides: None,
            }),
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

/// The data index of the hovered item in the series at `series_index`, when the
/// chart's emphasis is enabled.
pub(super) fn emphasised_item(chart: &Chart, series_index: usize) -> Option<usize> {
    if !chart.interaction.emphasis.enabled {
        return None;
    }
    let hit = chart.hover.as_ref()?.hit.as_ref()?;
    if hit.series_index == Some(series_index) {
        hit.data_index
    } else {
        None
    }
}

/// How much one wheel line changes the zoom window's width.
const WHEEL_ZOOM_STEP: f32 = 0.1;
/// Pixels of trackpad scroll that count as one wheel line.
const PIXELS_PER_LINE: f32 = 40.0;
/// The narrowest the zoom window gets, in percent of the range.
const MIN_ZOOM_SPAN: f32 = 5.0;
/// How far above and below the slider track a press still grabs it.
const ZOOM_GRAB_MARGIN: f32 = 6.0;

/// A drag of the zoom slider: where it started and the window at that moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ZoomDrag {
    pub(crate) anchor_x: f32,
    pub(crate) start: f32,
    pub(crate) end: f32,
}

/// A chart's zoom window and any drag of it, remembered between builds.
#[derive(Debug, Clone, Default)]
pub(super) struct ZoomMemory {
    pub(super) window: Option<(f32, f32)>,
    pub(super) drag: Option<ZoomDrag>,
}

impl fission_core::GlobalState for ZoomMemory {}

pub(super) fn tracks_zoom(chart: &Chart) -> bool {
    chart.data_zoom.is_some()
}

pub(super) fn on_zoom_changed(
    memory: &mut ZoomMemory,
    action: ChartZoomChanged,
    _: &mut ReducerContext<'_, '_, '_, ZoomMemory>,
) {
    match action {
        ChartZoomChanged::Window { start, end } => memory.window = Some((start, end)),
        ChartZoomChanged::DragStarted {
            anchor_x,
            start,
            end,
        } => {
            memory.drag = Some(ZoomDrag {
                anchor_x,
                start,
                end,
            });
        }
        ChartZoomChanged::DragEnded => memory.drag = None,
    }
}

/// The window after dragging it `shift_percent` of the track, keeping its width.
pub(super) fn dragged_window(drag: ZoomDrag, shift_percent: f32) -> (f32, f32) {
    let span = drag.end - drag.start;
    let start = (drag.start + shift_percent).clamp(0.0, 100.0 - span);
    (start, start + span)
}

/// The window after zooming by `lines` wheel lines around `focus`, the fraction
/// of the window under the pointer. Positive lines zoom out.
pub(super) fn zoomed_window(start: f32, end: f32, lines: f32, focus: f32) -> (f32, f32) {
    let span = end - start;
    let new_span = (span * (1.0 + lines * WHEEL_ZOOM_STEP)).clamp(MIN_ZOOM_SPAN, 100.0);
    let pivot = start + span * focus;
    let new_start = (pivot - new_span * focus).clamp(0.0, 100.0 - new_span);
    (new_start, new_start + new_span)
}

impl ChartRenderObject {
    /// The zoom change a pointer event makes, if any: a press on the slider
    /// starts a drag, moves during a drag shift the window, releasing ends it,
    /// and the wheel zooms around the pointer.
    pub(super) fn zoom_change(
        &self,
        event: &InputEvent,
        kind: ChartInteractionKind,
        local: LayoutPoint,
        node_rect: LayoutRect,
    ) -> Option<ChartZoomChanged> {
        let zoom = self.chart.data_zoom.as_ref()?;
        let area = chart_area_for_size(&self.chart, node_rect.width(), node_rect.height());
        let track = zoom_track(&area);
        let grab = LayoutRect::new(
            track.x(),
            track.y() - ZOOM_GRAB_MARGIN,
            track.width(),
            track.height() + ZOOM_GRAB_MARGIN * 2.0,
        );
        match kind {
            ChartInteractionKind::Press => {
                grab.contains(local)
                    .then_some(ChartZoomChanged::DragStarted {
                        anchor_x: local.x,
                        start: zoom.start_percent,
                        end: zoom.end_percent,
                    })
            }
            ChartInteractionKind::Hover => {
                let drag = self.chart.zoom_drag?;
                let shift = (local.x - drag.anchor_x) / track.width().max(1.0) * 100.0;
                let (start, end) = dragged_window(drag, shift);
                Some(ChartZoomChanged::Window { start, end })
            }
            ChartInteractionKind::Release => {
                self.chart.zoom_drag.map(|_| ChartZoomChanged::DragEnded)
            }
            ChartInteractionKind::Scroll => {
                let InputEvent::Pointer(PointerEvent::Scroll {
                    delta, delta_mode, ..
                }) = event
                else {
                    return None;
                };
                let over = match zoom.zoom_type {
                    DataZoomType::Inside => area.plot.contains(local),
                    DataZoomType::Slider => grab.contains(local),
                };
                if !over || delta.y == 0.0 {
                    return None;
                }
                let lines = match delta_mode {
                    ScrollDeltaMode::Line => delta.y,
                    ScrollDeltaMode::Pixel => delta.y / PIXELS_PER_LINE,
                };
                let focus =
                    ((local.x - area.plot.x()) / area.plot.width().max(1.0)).clamp(0.0, 1.0);
                let (start, end) =
                    zoomed_window(zoom.start_percent, zoom.end_percent, lines, focus);
                Some(ChartZoomChanged::Window { start, end })
            }
            ChartInteractionKind::Key => None,
        }
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;

    #[test]
    fn dragging_keeps_the_window_width_and_stops_at_the_ends() {
        let drag = ZoomDrag {
            anchor_x: 0.0,
            start: 20.0,
            end: 50.0,
        };
        assert_eq!(dragged_window(drag, 10.0), (30.0, 60.0));
        assert_eq!(dragged_window(drag, 90.0), (70.0, 100.0));
        assert_eq!(dragged_window(drag, -40.0), (0.0, 30.0));
    }

    #[test]
    fn wheel_zoom_keeps_the_point_under_the_pointer_in_place() {
        let (start, end) = zoomed_window(20.0, 60.0, -2.0, 0.5);
        assert!((end - start - 32.0).abs() < 0.01);
        assert!(((start + end) / 2.0 - 40.0).abs() < 0.01);
    }

    #[test]
    fn wheel_zoom_never_narrows_below_the_minimum_or_widens_past_everything() {
        let (start, end) = zoomed_window(40.0, 50.0, -50.0, 0.0);
        assert!((end - start - MIN_ZOOM_SPAN).abs() < 0.01);
        assert_eq!(zoomed_window(10.0, 90.0, 50.0, 0.5), (0.0, 100.0));
    }
}
