use fission_charts::{
    Axis, BarSeries, BoxplotSeries, CandlestickSeries, Chart, CustomSeries, EffectScatterSeries,
    FunnelSeries, GaugeSeries, GraphNode, GraphSeries, Grid, HeatmapSeries, Legend, LineSeries,
    LiquidfillSeries, MapSeries, ParallelSeries, PictorialBarSeries, PieSeries, RadarSeries,
    SankeySeries, ScatterSeries, Series, SunburstSeries, ThemeRiverSeries, Tooltip, TreemapNode,
    TreemapSeries, WordcloudSeries,
};
use fission_charts::chart::ChartLowerer;
use fission_core::{env::Env, lowering::LoweringContext, op::Color, ui::traits::LowerDyn, NodeId, WidgetNodeId};
use fission_ir::op::{LayoutOp, PaintOp};

#[test]
fn test_all_chart_builders() {
    let mut chart = Chart::new()
        .width(800.0)
        .height(600.0)
        .title("Full Parity Test Chart")
        .x_axis(Axis::category(vec!["A", "B", "C"]))
        .y_axis(Axis::value())
        .grid(Grid::new())
        .animate(true);

    let series_list = vec![
        LineSeries::new("Line").data(vec![10.0, 20.0]).into(),
        BarSeries::new("Bar").data(vec![15.0, 25.0]).into(),
        ScatterSeries::new("Scatter").data(vec![(1.0, 2.0), (3.0, 4.0)]).into(),
        PieSeries::new("Pie").data(vec![("A", 10.0), ("B", 20.0)]).into(),
        BoxplotSeries::new("Boxplot").data(vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]]).into(),
        CandlestickSeries::new("Candlestick").data(vec![vec![10.0, 20.0, 5.0, 25.0]]).into(),
        HeatmapSeries::new("Heatmap").data(vec![(0, 0, 10.0), (0, 1, 20.0)]).into(),
        GraphSeries::new("Graph").nodes(vec![GraphNode { id: "1".into(), name: "A".into(), value: 10.0 }]).into(),
        TreemapSeries::new("Treemap").data(vec![TreemapNode { name: "Root".into(), value: 100.0, children: vec![] }]).into(),
        RadarSeries::new("Radar").data(vec![vec![10.0, 20.0, 30.0]]).into(),
        FunnelSeries::new("Funnel").data(vec![("Stage 1", 100.0), ("Stage 2", 80.0)]).into(),
        GaugeSeries::new("Gauge").data(vec![("Speed", 65.0)]).into(),
        MapSeries::new("Map", "world").data(vec![("USA", 100.0)]).into(),
        SankeySeries::new("Sankey").nodes(vec![GraphNode { id: "1".into(), name: "A".into(), value: 10.0 }]).into(),
        ParallelSeries::new("Parallel").data(vec![vec![1.0, 2.0, 3.0]]).into(),
        SunburstSeries::new("Sunburst").data(vec![TreemapNode { name: "Root".into(), value: 100.0, children: vec![] }]).into(),
        ThemeRiverSeries::new("ThemeRiver").data(vec![("2021/01/01", 10.0, "Category A")]).into(),
        PictorialBarSeries::new("Pictorial").data(vec![10.0, 20.0]).symbol("rect").into(),
        EffectScatterSeries::new("EffectScatter").data(vec![(10.0, 20.0)]).into(),
        CustomSeries::new("Custom", "my_custom_renderer").data(vec![1.0, 2.0]).into(),
        LiquidfillSeries::new("Liquidfill").data(vec![0.6]).into(),
        WordcloudSeries::new("Wordcloud").data(vec![("Fission", 100.0), ("Rust", 80.0)]).into(),
    ];

    chart = chart.series(series_list);

    assert_eq!(chart.title.unwrap(), "Full Parity Test Chart");
    assert_eq!(chart.series.len(), 22);
    assert!(chart.animate);
    assert_eq!(chart.width, Some(800.0));
    assert_eq!(chart.height, Some(600.0));
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
        fission_ir::Op::Layout(LayoutOp::ZStack) => {},
        _ => panic!("Expected ZStack LayoutOp for Chart"),
    }
    
    assert!(ir.nodes.len() > 10, "Should generate grid, axes, and series nodes");
    
    // Verify that PaintOps were generated for the Bar chart
    let has_rects = ir.nodes.values().any(|n| matches!(n.op, fission_ir::Op::Paint(PaintOp::DrawRect { .. })));
    assert!(has_rects, "Bar chart should generate DrawRect PaintOps");
    
    // Verify that PaintOps were generated for the Line chart
    let has_paths = ir.nodes.values().any(|n| matches!(n.op, fission_ir::Op::Paint(PaintOp::DrawPath { .. })));
    assert!(has_paths, "Line chart should generate DrawPath PaintOps");
}
