use fission_charts::chart::ChartLowerer;
use fission_charts::{
    Axis, BarSeries, BoxplotSeries, CandlestickSeries, Chart, ChartModel, EffectScatterSeries,
    FunnelSeries, GaugeSeries, GraphNode, GraphSeries, Grid, HeatmapSeries, LineSeries,
    LiquidfillSeries, MapSeries, ParallelSeries, PictorialBarSeries, PieSeries, RadarSeries,
    SankeySeries, ScatterSeries, SunburstSeries, ThemeRiverSeries, TreemapNode, TreemapSeries,
    WordcloudSeries,
};
use fission_core::{env::Env, lowering::LoweringContext, ui::traits::LowerDyn};
use fission_ir::op::{LayoutOp, PaintOp};

const SIMPLE_GEOJSON: &str = r#"
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "properties": { "name": "North" },
      "geometry": {
        "type": "Polygon",
        "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]]
      }
    },
    {
      "type": "Feature",
      "properties": { "name": "South" },
      "geometry": {
        "type": "Polygon",
        "coordinates": [[[0, -10], [10, -10], [10, 0], [0, 0], [0, -10]]]
      }
    }
  ]
}
"#;

#[test]
fn test_all_chart_builders() {
    let mut chart = Chart::new()
        .width(800.0)
        .height(600.0)
        .title("Full Supported Chart Test")
        .x_axis(Axis::category(vec!["A", "B", "C"]))
        .y_axis(Axis::value())
        .grid(Grid::new())
        .animate(true);

    let series_list = vec![
        LineSeries::new("Line").data(vec![10.0, 20.0]).into(),
        BarSeries::new("Bar").data(vec![15.0, 25.0]).into(),
        ScatterSeries::new("Scatter")
            .data(vec![(1.0, 2.0), (3.0, 4.0)])
            .into(),
        PieSeries::new("Pie")
            .data(vec![("A", 10.0), ("B", 20.0)])
            .into(),
        BoxplotSeries::new("Boxplot")
            .data(vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]])
            .into(),
        CandlestickSeries::new("Candlestick")
            .data(vec![vec![10.0, 20.0, 5.0, 25.0]])
            .into(),
        HeatmapSeries::new("Heatmap")
            .data(vec![(0, 0, 10.0), (0, 1, 20.0)])
            .into(),
        GraphSeries::new("Graph")
            .nodes(vec![GraphNode {
                id: "1".into(),
                name: "A".into(),
                value: 10.0,
            }])
            .into(),
        TreemapSeries::new("Treemap")
            .data(vec![TreemapNode {
                name: "Root".into(),
                value: 100.0,
                children: vec![],
            }])
            .into(),
        RadarSeries::new("Radar")
            .data(vec![vec![10.0, 20.0, 30.0]])
            .into(),
        FunnelSeries::new("Funnel")
            .data(vec![("Stage 1", 100.0), ("Stage 2", 80.0)])
            .into(),
        GaugeSeries::new("Gauge").data(vec![("Speed", 65.0)]).into(),
        MapSeries::new("Map", "demo")
            .geojson(SIMPLE_GEOJSON)
            .data(vec![("North", 10.0), ("South", 20.0)])
            .into(),
        SankeySeries::new("Sankey")
            .nodes(vec![GraphNode {
                id: "1".into(),
                name: "A".into(),
                value: 10.0,
            }])
            .into(),
        ParallelSeries::new("Parallel")
            .data(vec![vec![1.0, 2.0, 3.0]])
            .into(),
        SunburstSeries::new("Sunburst")
            .data(vec![TreemapNode {
                name: "Root".into(),
                value: 100.0,
                children: vec![TreemapNode {
                    name: "Child".into(),
                    value: 40.0,
                    children: vec![],
                }],
            }])
            .into(),
        ThemeRiverSeries::new("River")
            .data(vec![
                ("2026-01-01", 10.0, "A"),
                ("2026-01-01", 20.0, "B"),
                ("2026-01-02", 30.0, "A"),
                ("2026-01-02", 12.0, "B"),
            ])
            .into(),
        PictorialBarSeries::new("Pictorial")
            .data(vec![10.0, 20.0])
            .symbol("rect")
            .into(),
        EffectScatterSeries::new("EffectScatter")
            .data(vec![(10.0, 20.0)])
            .into(),
        LiquidfillSeries::new("Liquidfill").data(vec![0.6]).into(),
        WordcloudSeries::new("Wordcloud")
            .data(vec![("Fission", 100.0), ("Rust", 80.0)])
            .into(),
    ];

    chart = chart.series(series_list);

    assert_eq!(chart.title.unwrap(), "Full Supported Chart Test");
    assert_eq!(chart.series.len(), 21);
    assert!(chart.animate);
    assert_eq!(chart.width, Some(800.0));
    assert_eq!(chart.height, Some(600.0));
}

#[test]
fn unsupported_series_emit_diagnostics_instead_of_drawing() {
    let chart = Chart::new().series(vec![
        fission_charts::series::map::MapSeries::new("World", "world")
            .data(vec![("USA", 100.0)])
            .into(),
        fission_charts::series::custom::CustomSeries::new("Custom", "string-callback")
            .data(vec![1.0])
            .into(),
    ]);

    let model = ChartModel::from_chart(&chart);

    assert!(model.series.is_empty());
    assert_eq!(model.diagnostics.len(), 2);
    assert!(model.diagnostics[0].message.contains("GeoJSON"));
    assert!(model.diagnostics[1].message.contains("String-named"));
}

#[test]
fn map_sunburst_and_theme_river_lower_to_paths() {
    let chart = Chart::new().width(800.0).height(600.0).series(vec![
        MapSeries::new("Map", "demo")
            .geojson(SIMPLE_GEOJSON)
            .data(vec![("North", 10.0), ("South", 20.0)])
            .into(),
        SunburstSeries::new("Sunburst")
            .data(vec![TreemapNode {
                name: "Root".into(),
                value: 100.0,
                children: vec![TreemapNode {
                    name: "Child".into(),
                    value: 40.0,
                    children: vec![],
                }],
            }])
            .into(),
        ThemeRiverSeries::new("River")
            .data(vec![
                ("2026-01-01", 10.0, "A"),
                ("2026-01-01", 20.0, "B"),
                ("2026-01-02", 30.0, "A"),
                ("2026-01-02", 12.0, "B"),
                ("2026-01-03", 20.0, "A"),
                ("2026-01-03", 24.0, "B"),
            ])
            .into(),
    ]);

    let model = ChartModel::from_chart(&chart);
    assert!(model.diagnostics.is_empty());
    assert_eq!(model.series.len(), 3);

    let lowerer = ChartLowerer { chart };
    let env = Env::default();
    let runtime_state = fission_core::RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = cx.next_node_id();
    cx.push_scope(root_id);
    lowerer.lower_dyn(&mut cx);

    let path_count = cx
        .ir
        .nodes
        .values()
        .filter(|node| matches!(node.op, fission_ir::Op::Paint(PaintOp::DrawPath { .. })))
        .count();
    assert!(path_count >= 6);
}

#[test]
fn test_chart_lowering() {
    let chart = Chart::new()
        .width(800.0)
        .height(600.0)
        .x_axis(Axis::category(vec!["A", "B", "C"]))
        .y_axis(Axis::value())
        .series(vec![
            BarSeries::new("Bar").data(vec![15.0, 25.0, 35.0]).into(),
            LineSeries::new("Line").data(vec![10.0, 20.0, 30.0]).into(),
        ]);

    let lowerer = ChartLowerer { chart };

    let env = Env::default();
    let runtime_state = fission_core::RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);

    let root_id = cx.next_node_id();
    cx.push_scope(root_id);

    let generated_id = lowerer.lower_dyn(&mut cx);

    let ir = cx.ir;
    let root_node = ir.nodes.get(&generated_id).expect("Root node should exist");

    // Root should be a ZStack
    match &root_node.op {
        fission_ir::Op::Layout(LayoutOp::ZStack) => {}
        _ => panic!("Expected ZStack LayoutOp for Chart"),
    }

    assert!(
        ir.nodes.len() > 10,
        "Should generate grid, axes, and series nodes"
    );

    // Verify that PaintOps were generated for the Bar chart
    let has_rects = ir
        .nodes
        .values()
        .any(|n| matches!(n.op, fission_ir::Op::Paint(PaintOp::DrawRect { .. })));
    assert!(has_rects, "Bar chart should generate DrawRect PaintOps");

    // Verify that PaintOps were generated for the Line chart
    let has_paths = ir
        .nodes
        .values()
        .any(|n| matches!(n.op, fission_ir::Op::Paint(PaintOp::DrawPath { .. })));
    assert!(has_paths, "Line chart should generate DrawPath PaintOps");
}
