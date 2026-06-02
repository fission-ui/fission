use super::GalleryBuildExt;
use crate::state::GalleryState;
use fission::charts::{
    Axis, BoxplotSeries, CandlestickSeries, Chart, FunnelSeries, HeatmapSeries, RadarSeries,
    VisualMap,
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
        0 => Chart::new()
            .title("Statistical: Boxplot")
            .x_axis(Axis::category(vec!["expr 1", "expr 2", "expr 3"]))
            .y_axis(Axis::value())
            .series(vec![BoxplotSeries::new("Boxplot")
                .data(vec![
                    vec![850.0 * s, 960.0 * s, 1060.0 * s, 1080.0 * s, 1100.0 * s],
                    vec![800.0 * s, 850.0 * s, 900.0 * s, 930.0 * s, 980.0 * s],
                    vec![750.0 * s, 800.0 * s, 850.0 * s, 900.0 * s, 1000.0 * s],
                ])
                .color(Color {
                    r: 115,
                    g: 192,
                    b: 222,
                    a: 255,
                })
                .into()])
            .build_in_gallery(ctx, view, content_width),
        1 => {
            Chart::new()
                .title("Statistical: Candlestick")
                .x_axis(Axis::category(vec![
                    "2017-10-24",
                    "2017-10-25",
                    "2017-10-26",
                    "2017-10-27",
                ]))
                .y_axis(Axis::value())
                .series(vec![CandlestickSeries::new("Data")
                    .data(vec![
                        vec![20.0 * s, 34.0 * s, 10.0 * s, 38.0 * s], // open, close, lowest, highest
                        vec![40.0 * s, 35.0 * s, 30.0 * s, 50.0 * s],
                        vec![31.0 * s, 38.0 * s, 33.0 * s, 44.0 * s],
                        vec![38.0 * s, 15.0 * s, 5.0 * s, 42.0 * s],
                    ])
                    .into()])
                .build_in_gallery(ctx, view, content_width)
        }
        2 => Chart::new()
            .title("Statistical: Heatmap")
            .x_axis(Axis::category(vec!["12a", "1a", "2a", "3a"]))
            .y_axis(Axis::category(vec!["Sat", "Fri", "Thu"]))
            .visual_map(VisualMap::new().min(0.0).max(8.0 * s).in_range_colors(vec![
                Color {
                    r: 219,
                    g: 234,
                    b: 254,
                    a: 255,
                },
                Color {
                    r: 96,
                    g: 165,
                    b: 250,
                    a: 255,
                },
                Color {
                    r: 30,
                    g: 64,
                    b: 175,
                    a: 255,
                },
            ]))
            .series(vec![HeatmapSeries::new("Punch Card")
                .data(vec![
                    (0, 0, 5.0 * s),
                    (0, 1, 1.0 * s),
                    (0, 2, 0.0 * s),
                    (1, 0, 3.0 * s),
                    (1, 1, 0.0 * s),
                    (1, 2, 0.0 * s),
                    (2, 0, 4.0 * s),
                    (2, 1, 2.0 * s),
                    (2, 2, 0.0 * s),
                    (3, 0, 1.0 * s),
                    (3, 1, 0.0 * s),
                    (3, 2, 8.0 * s),
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        3 => Chart::new()
            .title("Statistical: Radar")
            .series(vec![RadarSeries::new("Budget vs spending")
                .data(vec![
                    vec![42.0 * s, 30.0 * s, 20.0 * s, 35.0 * s, 50.0 * s, 18.0 * s],
                    vec![50.0 * s, 14.0 * s, 28.0 * s, 26.0 * s, 42.0 * s, 21.0 * s],
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        4 => Chart::new()
            .title("Statistical: Funnel")
            .series(vec![FunnelSeries::new("Expected")
                .data(vec![
                    ("Visit", 100.0 * s),
                    ("Inquiry", 80.0 * s),
                    ("Order", 60.0 * s),
                    ("Click", 40.0 * s),
                    ("Return", 20.0 * s),
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        _ => unreachable!("chart catalog and builder are out of sync"),
    }
}
