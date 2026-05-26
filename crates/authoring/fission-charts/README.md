# fission-charts

Chart widgets and data-visualization primitives for Fission applications.

`fission-charts` provides the chart families used by dashboards, reporting screens, and the checked-in chart gallery. It is designed to be used through the public `fission` facade with the `charts` feature enabled:

```toml
[dependencies]
fission = { version = "0.1.1", features = ["desktop", "charts"] }
```

Most application code should import from `fission::prelude::*` and `fission::charts::*` rather than depending on this crate directly. Depend on `fission-charts` only when you are extending the chart layer itself.

## What it contains

- Cartesian charts such as line, area, bar, scatter, candlestick, boxplot, heatmap, calendar, and statistical variants.
- Relationship and hierarchy charts such as graph, tree, treemap, sunburst, sankey, funnel, and parallel coordinates.
- Spatial and specialized charts such as map, globe, gauge, radar, polar, pie, pictorial bar, theme river, and custom visualizations.
- Shared data-set, series, axis, legend, tooltip, interaction, and animation configuration used by the chart gallery.

## Example

```rust,ignore
use fission::prelude::*;
use fission::charts::*;

struct Dashboard;

impl Widget<App> for Dashboard {
    fn build(&self, _ctx: &mut BuildCtx<App>, _view: &View<App>) -> Node {
        LineChart::new()
            .title("Revenue")
            .series(LineSeries::new("Actual", vec![120.0, 156.0, 182.0]))
            .into_node()
    }
}
```

## Documentation

The chart guide, gallery, and chart reference live at [fission.rs](https://fission.rs/docs/charts/overview/). The gallery screenshots are generated from Fission charts, not copied from another charting package.

## License

MIT
