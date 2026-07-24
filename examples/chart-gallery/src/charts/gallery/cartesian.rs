use super::GalleryBuildExt;
use crate::state::GalleryState;
use fission::charts::{
    Axis, BarSeries, Chart, DataZoom, Legend, LineSeries, PieSeries, ScatterSeries, Tooltip,
    VisualMap,
};
use fission::core::op::Color;
use fission::core::ui::Widget;

pub(super) struct CartesianChart {
    pub chart: usize,
    pub scale: f32,
}

impl From<CartesianChart> for Widget {
    fn from(selection: CartesianChart) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let chart = selection.chart;
        let s = selection.scale;

        match chart {
            0 => Chart::new()
                .title("Foundational: Line & Bar")
                .x_axis(Axis::category(vec![
                    "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
                ]))
                .y_axis(Axis::value())
                .legend(Legend::top_right())
                .tooltip(Tooltip::axis_trigger())
                .series(vec![
                    BarSeries::new("Direct")
                        .data(vec![
                            320.0 * s,
                            332.0 * s,
                            301.0 * s,
                            334.0 * s,
                            390.0 * s,
                            330.0 * s,
                            320.0 * s,
                        ])
                        .color(Color {
                            r: 84,
                            g: 112,
                            b: 198,
                            a: 255,
                        })
                        .into(),
                    LineSeries::new("Email")
                        .data(vec![
                            120.0 * s,
                            132.0 * s,
                            101.0 * s,
                            134.0 * s,
                            90.0 * s,
                            230.0 * s,
                            210.0 * s,
                        ])
                        .color(Color {
                            r: 145,
                            g: 204,
                            b: 117,
                            a: 255,
                        })
                        .smooth(view.state().smooth)
                        .into(),
                ])
                .in_gallery()
                .into(),
            1 => Chart::new()
                .title("Foundational: Stacked Area")
                .x_axis(Axis::category(vec![
                    "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
                ]))
                .y_axis(Axis::value())
                .legend(Legend::top_right())
                .data_zoom(DataZoom::new().start_percent(10.0).end_percent(90.0))
                .series(vec![
                    LineSeries::new("Email")
                        .stack("total")
                        .area_style(Color {
                            r: 145,
                            g: 204,
                            b: 117,
                            a: 100,
                        })
                        .data(vec![
                            120.0 * s,
                            132.0 * s,
                            101.0 * s,
                            134.0 * s,
                            90.0 * s,
                            230.0 * s,
                            210.0 * s,
                        ])
                        .color(Color {
                            r: 145,
                            g: 204,
                            b: 117,
                            a: 255,
                        })
                        .smooth(view.state().smooth)
                        .into(),
                    LineSeries::new("Video Ads")
                        .stack("total")
                        .area_style(Color {
                            r: 84,
                            g: 112,
                            b: 198,
                            a: 100,
                        })
                        .data(vec![
                            150.0 * s,
                            232.0 * s,
                            201.0 * s,
                            154.0 * s,
                            190.0 * s,
                            330.0 * s,
                            410.0 * s,
                        ])
                        .color(Color {
                            r: 84,
                            g: 112,
                            b: 198,
                            a: 255,
                        })
                        .smooth(view.state().smooth)
                        .into(),
                ])
                .in_gallery()
                .into(),
            2 => Chart::new()
                .title("Foundational: Step Line")
                .x_axis(Axis::category(vec![
                    "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
                ]))
                .y_axis(Axis::value())
                .series(vec![LineSeries::new("Step Start")
                    .step("middle")
                    .data(vec![
                        120.0 * s,
                        132.0 * s,
                        101.0 * s,
                        134.0 * s,
                        90.0 * s,
                        230.0 * s,
                        210.0 * s,
                    ])
                    .color(Color {
                        r: 250,
                        g: 200,
                        b: 88,
                        a: 255,
                    })
                    .into()])
                .in_gallery()
                .into(),
            3 => Chart::new()
                .title("Foundational: Donut Pie")
                .legend(Legend::top_right())
                .tooltip(Tooltip::item_trigger())
                .series(vec![PieSeries::new("Access Source")
                    .inner_radius(52.0)
                    .data(vec![
                        ("Search Engine", 1048.0 * s),
                        ("Direct", 735.0 * s),
                        ("Email", 580.0 * s),
                        ("Union Ads", 484.0 * s),
                        ("Video Ads", 300.0 * s),
                    ])
                    .into()])
                .in_gallery()
                .into(),
            4 => Chart::new()
                .title("Foundational: Scatter Visual")
                .x_axis(Axis::value())
                .y_axis(Axis::value())
                .visual_map(
                    VisualMap::new()
                        .min(6.0 * s)
                        .max(11.0 * s)
                        .in_range_colors(vec![
                            Color {
                                r: 59,
                                g: 130,
                                b: 246,
                                a: 255,
                            },
                            Color {
                                r: 250,
                                g: 204,
                                b: 21,
                                a: 255,
                            },
                            Color {
                                r: 239,
                                g: 68,
                                b: 68,
                                a: 255,
                            },
                        ]),
                )
                .series(vec![ScatterSeries::new("Data")
                    .data(vec![
                        (10.0 * s, 8.04 * s),
                        (8.0 * s, 6.95 * s),
                        (13.0 * s, 7.58 * s),
                        (9.0 * s, 8.81 * s),
                        (11.0 * s, 8.33 * s),
                        (14.0 * s, 9.96 * s),
                    ])
                    .color(Color {
                        r: 250,
                        g: 200,
                        b: 88,
                        a: 255,
                    })
                    .into()])
                .in_gallery()
                .into(),
            _ => unreachable!("chart catalog and builder are out of sync"),
        }
    }
}
