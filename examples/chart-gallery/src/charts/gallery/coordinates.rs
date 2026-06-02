use super::GalleryBuildExt;
use crate::state::GalleryState;
use crate::style::{blue, teal};
use fission::charts::{
    CalendarHeatmapSeries, Chart, PolarBarSeries, PolarLineSeries, SingleAxisSeries, VisualMap,
};
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
        0 => polar_bar(s).build_in_gallery(ctx, view, content_width),
        1 => polar_line(s).build_in_gallery(ctx, view, content_width),
        2 => calendar_heatmap(s).build_in_gallery(ctx, view, content_width),
        3 => single_axis(s).build_in_gallery(ctx, view, content_width),
        _ => unreachable!("chart catalog and coordinate builder are out of sync"),
    }
}

pub(crate) fn polar_bar(s: f32) -> Chart {
    Chart::new()
        .title("Polar bar")
        .series(vec![PolarBarSeries::new("Radial spend")
            .data(vec![
                ("Search", 82.0 * s),
                ("Direct", 67.0 * s),
                ("Email", 43.0 * s),
                ("Ads", 58.0 * s),
                ("Partner", 76.0 * s),
            ])
            .color(teal())
            .inner_radius(34.0)
            .into()])
}

pub(crate) fn polar_line(s: f32) -> Chart {
    Chart::new()
        .title("Polar line")
        .series(vec![PolarLineSeries::new("Wind")
            .data(vec![
                (0.0, 18.0 * s),
                (45.0, 42.0 * s),
                (90.0, 32.0 * s),
                (135.0, 60.0 * s),
                (180.0, 44.0 * s),
                (225.0, 24.0 * s),
                (270.0, 36.0 * s),
                (315.0, 56.0 * s),
            ])
            .smooth(true)
            .color(blue())
            .into()])
}

pub(crate) fn calendar_heatmap(s: f32) -> Chart {
    Chart::new()
        .title("Calendar heatmap")
        .visual_map(VisualMap::new().min(0.0).max(18.0 * s))
        .series(vec![CalendarHeatmapSeries::new("Commits")
            .range("2026-01-01", "2026-03-31")
            .data(vec![
                ("2026-01-02", 3.0 * s),
                ("2026-01-05", 12.0 * s),
                ("2026-01-12", 7.0 * s),
                ("2026-01-23", 16.0 * s),
                ("2026-02-03", 8.0 * s),
                ("2026-02-14", 18.0 * s),
                ("2026-02-27", 6.0 * s),
                ("2026-03-04", 10.0 * s),
                ("2026-03-16", 15.0 * s),
                ("2026-03-24", 9.0 * s),
            ])
            .into()])
}

pub(crate) fn single_axis(s: f32) -> Chart {
    Chart::new()
        .title("Single axis")
        .series(vec![SingleAxisSeries::new("Events")
            .data(vec![
                (2.0 * s, 12.0 * s),
                (7.0 * s, 28.0 * s),
                (13.0 * s, 20.0 * s),
                (22.0 * s, 36.0 * s),
                (31.0 * s, 18.0 * s),
                (42.0 * s, 30.0 * s),
                (55.0 * s, 24.0 * s),
                (63.0 * s, 34.0 * s),
            ])
            .color(teal())
            .into()])
}
