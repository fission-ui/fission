use super::GalleryBuildExt;
use crate::state::GalleryState;
use crate::style::{amber, blue, teal};
use fission::charts::{Axis, BarSeries, BubbleSeries, Chart, Legend, LineSeries, VisualMap};
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
        0 => horizontal_bar(s).build_in_gallery(ctx, view, content_width),
        1 => rounded_background_bar(s).build_in_gallery(ctx, view, content_width),
        2 => negative_bar(s).build_in_gallery(ctx, view, content_width),
        3 => bubble_scatter(s).build_in_gallery(ctx, view, content_width),
        4 => large_line(s).build_in_gallery(ctx, view, content_width),
        _ => unreachable!("chart catalog and cartesian-variant builder are out of sync"),
    }
}

pub(crate) fn horizontal_bar(s: f32) -> Chart {
    Chart::new()
        .title("Horizontal bar")
        .x_axis(Axis::value())
        .y_axis(Axis::category(vec![
            "Brazil",
            "Indonesia",
            "USA",
            "India",
            "China",
        ]))
        .series(vec![BarSeries::new("Population")
            .horizontal()
            .border_radius(6.0)
            .data(vec![
                182.0 * s,
                234.0 * s,
                290.0 * s,
                1049.0 * s,
                1317.0 * s,
            ])
            .color(teal())
            .into()])
}

pub(crate) fn rounded_background_bar(s: f32) -> Chart {
    Chart::new()
        .title("Rounded bar with background")
        .x_axis(Axis::category(vec![
            "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
        ]))
        .y_axis(Axis::value().max(260.0 * s))
        .series(vec![BarSeries::new("Traffic")
            .border_radius(10.0)
            .background(Color {
                r: 226,
                g: 232,
                b: 240,
                a: 150,
            })
            .data(vec![
                120.0 * s,
                200.0 * s,
                150.0 * s,
                80.0 * s,
                70.0 * s,
                110.0 * s,
                130.0 * s,
            ])
            .color(blue())
            .into()])
}

pub(crate) fn negative_bar(s: f32) -> Chart {
    Chart::new()
        .title("Positive and negative bars")
        .x_axis(Axis::category(vec![
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        ]))
        .y_axis(Axis::value())
        .series(vec![BarSeries::new("Delta")
            .border_radius(5.0)
            .data(vec![
                120.0 * s,
                -90.0 * s,
                160.0 * s,
                -40.0 * s,
                88.0 * s,
                132.0 * s,
            ])
            .color(amber())
            .into()])
}

pub(crate) fn bubble_scatter(s: f32) -> Chart {
    Chart::new()
        .title("Bubble scatter")
        .x_axis(Axis::value())
        .y_axis(Axis::value())
        .legend(Legend::top_right())
        .visual_map(VisualMap::new().min(10.0 * s).max(80.0 * s))
        .series(vec![BubbleSeries::new("Markets")
            .data(vec![
                (10.0 * s, 8.0 * s, 22.0 * s),
                (18.0 * s, 12.0 * s, 46.0 * s),
                (25.0 * s, 28.0 * s, 30.0 * s),
                (32.0 * s, 22.0 * s, 74.0 * s),
                (42.0 * s, 36.0 * s, 58.0 * s),
                (54.0 * s, 30.0 * s, 36.0 * s),
            ])
            .color(blue())
            .radius_range(6.0, 24.0)
            .into()])
}

pub(crate) fn large_line(s: f32) -> Chart {
    let data = (0..96)
        .map(|idx| {
            let x = idx as f32 / 7.0;
            (80.0 + x.sin() * 22.0 + (x * 0.37).cos() * 14.0 + idx as f32 * 0.8) * s
        })
        .collect();
    Chart::new()
        .title("Large line")
        .x_axis(Axis::category(
            (0..96)
                .map(|idx| if idx % 12 == 0 { "|" } else { "" })
                .collect(),
        ))
        .y_axis(Axis::value())
        .series(vec![LineSeries::new("Telemetry")
            .data(data)
            .smooth(true)
            .color(teal())
            .into()])
}
