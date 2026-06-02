use super::GalleryBuildExt;
use crate::state::GalleryState;
use fission::charts::{
    Axis, BarSeries, Chart, ChartBrush, ChartGraphic, ChartInteraction, ChartTimeline,
    ChartToolAction, DataZoom, Legend, LineSeries, MarkArea, MarkLine, MarkPoint, ScatterSeries,
    Tooltip,
};
use fission::core::op::Color;
use fission::core::ui::Widget;
use fission::core::{BuildCtxHandle, ViewHandle};

pub(super) fn build_chart(
    chart: usize,
    ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    content_width: f32,
    s: f32,
) -> Widget {
    match chart {
        0 => mark_line_point(s).build_in_gallery(ctx, view, content_width),
        1 => data_zoom(s).build_in_gallery(ctx, view, content_width),
        2 => tooltip_axis(s).build_in_gallery(ctx, view, content_width),
        3 => timeline_events(s).build_in_gallery(ctx, view, content_width),
        4 => toolbox_actions(s).build_in_gallery(ctx, view, content_width),
        5 => brush_select(s).build_in_gallery(ctx, view, content_width),
        6 => graphic_overlay(s).build_in_gallery(ctx, view, content_width),
        _ => unreachable!("chart catalog and component builder are out of sync"),
    }
}

pub(crate) fn mark_line_point(s: f32) -> Chart {
    Chart::new()
        .title("Markers and target band")
        .x_axis(Axis::category(vec![
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        ]))
        .y_axis(Axis::value())
        .mark_area(MarkArea::y_range("Target band", 120.0 * s, 190.0 * s))
        .mark_line(MarkLine::y("Target", 160.0 * s))
        .mark_point(MarkPoint::xy("Peak", 4.0, 230.0 * s))
        .series(vec![LineSeries::new("Revenue")
            .smooth(true)
            .data(vec![
                80.0 * s,
                132.0 * s,
                101.0 * s,
                184.0 * s,
                230.0 * s,
                210.0 * s,
            ])
            .into()])
}

pub(crate) fn data_zoom(s: f32) -> Chart {
    Chart::new()
        .title("Data zoom")
        .x_axis(Axis::category(vec![
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
        ]))
        .y_axis(Axis::value())
        .data_zoom(DataZoom::new().start_percent(18.0).end_percent(82.0))
        .series(vec![LineSeries::new("Requests")
            .smooth(true)
            .data(
                vec![
                    120.0, 142.0, 118.0, 190.0, 240.0, 220.0, 260.0, 310.0, 280.0, 360.0,
                ]
                .into_iter()
                .map(|v| v * s)
                .collect(),
            )
            .into()])
}

pub(crate) fn tooltip_axis(s: f32) -> Chart {
    Chart::new()
        .title("Axis tooltip")
        .x_axis(Axis::category(vec![
            "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
        ]))
        .y_axis(Axis::value())
        .legend(Legend::top_right())
        .tooltip(Tooltip::axis_trigger())
        .interaction(ChartInteraction::new().emit_events(true))
        .series(vec![
            BarSeries::new("Orders")
                .data(
                    vec![320.0, 332.0, 301.0, 334.0, 390.0, 330.0, 320.0]
                        .into_iter()
                        .map(|v| v * s)
                        .collect(),
                )
                .color(Color {
                    r: 84,
                    g: 112,
                    b: 198,
                    a: 255,
                })
                .into(),
            LineSeries::new("Revenue")
                .smooth(true)
                .data(
                    vec![120.0, 132.0, 101.0, 134.0, 90.0, 230.0, 210.0]
                        .into_iter()
                        .map(|v| v * s)
                        .collect(),
                )
                .color(Color {
                    r: 20,
                    g: 184,
                    b: 166,
                    a: 255,
                })
                .into(),
        ])
}

pub(crate) fn timeline_events(s: f32) -> Chart {
    Chart::new()
        .title("Timeline")
        .x_axis(Axis::category(vec!["North", "South", "East", "West"]))
        .y_axis(Axis::value())
        .timeline(ChartTimeline::new(vec!["2024", "2025", "2026"]).current_index(2))
        .series(vec![BarSeries::new("Deployments")
            .data(vec![180.0 * s, 240.0 * s, 210.0 * s, 280.0 * s])
            .into()])
}

pub(crate) fn toolbox_actions(s: f32) -> Chart {
    Chart::new()
        .title("Tool actions")
        .x_axis(Axis::category(vec!["Q1", "Q2", "Q3", "Q4"]))
        .y_axis(Axis::value())
        .interaction(ChartInteraction::new().toolbox_actions(vec![
            ChartToolAction::DataZoom,
            ChartToolAction::Brush,
            ChartToolAction::Restore,
            ChartToolAction::SaveImage,
        ]))
        .series(vec![LineSeries::new("Usage")
            .smooth(true)
            .area_style(Color {
                r: 37,
                g: 99,
                b: 235,
                a: 90,
            })
            .data(vec![140.0 * s, 220.0 * s, 180.0 * s, 310.0 * s])
            .into()])
}

pub(crate) fn brush_select(s: f32) -> Chart {
    Chart::new()
        .title("Brush selection")
        .x_axis(Axis::value())
        .y_axis(Axis::value())
        .interaction(
            ChartInteraction::new().brush(ChartBrush::rect().preview_rect(0.28, 0.20, 0.38, 0.52)),
        )
        .series(vec![ScatterSeries::new("Samples")
            .data(vec![
                (10.0 * s, 8.0 * s),
                (15.0 * s, 12.0 * s),
                (18.0 * s, 16.0 * s),
                (24.0 * s, 18.0 * s),
                (30.0 * s, 28.0 * s),
                (40.0 * s, 34.0 * s),
            ])
            .color(Color {
                r: 20,
                g: 184,
                b: 166,
                a: 255,
            })
            .into()])
}

pub(crate) fn graphic_overlay(s: f32) -> Chart {
    Chart::new()
        .title("Graphic overlay")
        .x_axis(Axis::category(vec!["Mon", "Tue", "Wed", "Thu", "Fri"]))
        .y_axis(Axis::value())
        .graphic(
            ChartGraphic::rect(
                0.18,
                0.08,
                0.26,
                0.14,
                Color {
                    r: 239,
                    g: 246,
                    b: 255,
                    a: 215,
                },
            )
            .stroke(Color {
                r: 96,
                g: 165,
                b: 250,
                a: 255,
            }),
        )
        .graphic(ChartGraphic::text(
            0.20,
            0.12,
            "release window",
            Color {
                r: 37,
                g: 99,
                b: 235,
                a: 255,
            },
        ))
        .graphic(ChartGraphic::line(
            0.30,
            0.22,
            0.22,
            0.30,
            Color {
                r: 37,
                g: 99,
                b: 235,
                a: 255,
            },
        ))
        .series(vec![LineSeries::new("Latency")
            .smooth(true)
            .data(vec![80.0 * s, 132.0 * s, 101.0 * s, 184.0 * s, 140.0 * s])
            .color(Color {
                r: 20,
                g: 184,
                b: 166,
                a: 255,
            })
            .into()])
}
