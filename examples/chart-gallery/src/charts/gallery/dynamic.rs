use super::GalleryBuildExt;
use fission::charts::{
    Axis, Chart, EffectScatterSeries, GaugeSeries, LiquidfillSeries, PictorialBarSeries,
    WordcloudSeries,
};
use fission::core::ui::Widget;

pub(super) struct DynamicChart {
    pub chart: usize,
    pub scale: f32,
}

impl From<DynamicChart> for Widget {
    fn from(selection: DynamicChart) -> Self {
        let chart = selection.chart;
        let s = selection.scale;

        match chart {
            0 => Chart::new()
                .title("Dynamic: Gauge")
                .series(vec![GaugeSeries::new("Speed")
                    .data(vec![("km/h", 50.0 * s)])
                    .into()])
                .in_gallery()
                .into(),
            1 => Chart::new()
                .title("Dynamic: PictorialBar")
                .x_axis(Axis::category(vec!["January", "February", "March"]))
                .y_axis(Axis::value())
                .series(vec![PictorialBarSeries::new("Spirits")
                    .data(vec![120.0 * s, 200.0 * s, 150.0 * s])
                    .into()])
                .in_gallery()
                .into(),
            2 => Chart::new()
                .title("Dynamic: EffectScatter")
                .x_axis(Axis::value())
                .y_axis(Axis::value())
                .series(vec![EffectScatterSeries::new("Effects")
                    .data(vec![
                        (10.0 * s, 8.0 * s),
                        (8.0 * s, 7.0 * s),
                        (13.0 * s, 7.5 * s),
                    ])
                    .into()])
                .in_gallery()
                .into(),
            3 => Chart::new()
                .title("Dynamic: Liquidfill")
                .series(vec![LiquidfillSeries::new("Water Level")
                    .data(vec![0.6 * s, 0.5 * s, 0.4 * s])
                    .into()])
                .in_gallery()
                .into(),
            4 => Chart::new()
                .title("Dynamic: Wordcloud")
                .series(vec![WordcloudSeries::new("Words")
                    .data(vec![
                        ("Rust", 100.0 * s),
                        ("Fission", 80.0 * s),
                        ("GPU", 60.0 * s),
                        ("Vello", 40.0 * s),
                    ])
                    .into()])
                .in_gallery()
                .into(),
            _ => unreachable!("chart catalog and builder are out of sync"),
        }
    }
}
