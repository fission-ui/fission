use super::GalleryBuildExt;
use crate::data::{sample_lines, sample_tree, SIMPLE_GEOJSON};
use crate::state::GalleryState;
use crate::style::teal;
use fission::charts::{
    Chart, GraphNode, GraphSeries, Legend, LinesSeries, MapSeries, ParallelSeries, SankeySeries,
    SunburstSeries, ThemeRiverSeries, TreeSeries, TreemapNode, TreemapSeries, VisualMap,
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
        0 => Chart::new()
            .title("Relationships: Graph")
            .series(vec![GraphSeries::new("Les Miserables")
                .nodes(vec![
                    GraphNode {
                        id: "0".into(),
                        name: "Myriel".into(),
                        value: 28.6 * s,
                    },
                    GraphNode {
                        id: "1".into(),
                        name: "Napoleon".into(),
                        value: 10.0 * s,
                    },
                    GraphNode {
                        id: "2".into(),
                        name: "Mlle.Baptistine".into(),
                        value: 15.0 * s,
                    },
                    GraphNode {
                        id: "3".into(),
                        name: "Valjean".into(),
                        value: 42.0 * s,
                    },
                ])
                .edges(vec![
                    fission::charts::series::graph::GraphEdge {
                        source: "1".into(),
                        target: "0".into(),
                    },
                    fission::charts::series::graph::GraphEdge {
                        source: "2".into(),
                        target: "0".into(),
                    },
                    fission::charts::series::graph::GraphEdge {
                        source: "3".into(),
                        target: "0".into(),
                    },
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        1 => Chart::new()
            .title("Relationships: Treemap")
            .series(vec![TreemapSeries::new("Disk Usage")
                .data(vec![
                    TreemapNode {
                        name: "System".into(),
                        value: 120.0 * s,
                        children: vec![],
                    },
                    TreemapNode {
                        name: "Users".into(),
                        value: 450.0 * s,
                        children: vec![],
                    },
                    TreemapNode {
                        name: "Applications".into(),
                        value: 310.0 * s,
                        children: vec![],
                    },
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        2 => Chart::new()
            .title("Relationships: Sunburst")
            .series(vec![SunburstSeries::new("Spend")
                .data(vec![
                    TreemapNode {
                        name: "Product".into(),
                        value: 0.0,
                        children: vec![
                            TreemapNode {
                                name: "Design".into(),
                                value: 32.0 * s,
                                children: vec![],
                            },
                            TreemapNode {
                                name: "Build".into(),
                                value: 54.0 * s,
                                children: vec![],
                            },
                        ],
                    },
                    TreemapNode {
                        name: "Growth".into(),
                        value: 0.0,
                        children: vec![
                            TreemapNode {
                                name: "Sales".into(),
                                value: 42.0 * s,
                                children: vec![],
                            },
                            TreemapNode {
                                name: "Success".into(),
                                value: 28.0 * s,
                                children: vec![],
                            },
                        ],
                    },
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        3 => Chart::new()
            .title("Relationships: Sankey")
            .series(vec![SankeySeries::new("Energy Flow")
                .nodes(vec![
                    GraphNode {
                        id: "a".into(),
                        name: "Solar".into(),
                        value: 0.0,
                    },
                    GraphNode {
                        id: "b".into(),
                        name: "Grid".into(),
                        value: 0.0,
                    },
                ])
                .edges(vec![fission::charts::series::graph::GraphEdge {
                    source: "a".into(),
                    target: "b".into(),
                }])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        4 => Chart::new()
            .title("Relationships: Theme River")
            .legend(Legend::top_right())
            .series(vec![ThemeRiverSeries::new("Demand")
                .data(vec![
                    ("2026-01", 18.0 * s, "Search"),
                    ("2026-01", 12.0 * s, "Direct"),
                    ("2026-01", 8.0 * s, "Partner"),
                    ("2026-02", 26.0 * s, "Search"),
                    ("2026-02", 14.0 * s, "Direct"),
                    ("2026-02", 14.0 * s, "Partner"),
                    ("2026-03", 20.0 * s, "Search"),
                    ("2026-03", 28.0 * s, "Direct"),
                    ("2026-03", 12.0 * s, "Partner"),
                    ("2026-04", 34.0 * s, "Search"),
                    ("2026-04", 20.0 * s, "Direct"),
                    ("2026-04", 18.0 * s, "Partner"),
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        5 => Chart::new()
            .title("Relationships: Parallel")
            .series(vec![ParallelSeries::new("Data")
                .data(vec![
                    vec![12.99 * s, 100.0 * s, 82.0 * s, 90.0 * s],
                    vec![9.99 * s, 150.0 * s, 56.0 * s, 80.0 * s],
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        6 => Chart::new()
            .title("Geographic: Choropleth Map")
            .visual_map(VisualMap::new().min(10.0 * s).max(44.0 * s))
            .series(vec![MapSeries::new("Regions", "demo")
                .geojson(SIMPLE_GEOJSON)
                .data(vec![
                    ("North", 44.0 * s),
                    ("West", 18.0 * s),
                    ("East", 30.0 * s),
                ])
                .into()])
            .build_in_gallery(ctx, view, content_width),
        7 => Chart::new()
            .title("Relationships: Tree")
            .series(vec![TreeSeries::new("Product tree")
                .data(sample_tree(s))
                .into()])
            .build_in_gallery(ctx, view, content_width),
        8 => Chart::new()
            .title("Relationships: Radial Tree")
            .series(vec![TreeSeries::new("Product tree")
                .data(sample_tree(s))
                .radial(true)
                .into()])
            .build_in_gallery(ctx, view, content_width),
        9 => Chart::new()
            .title("Geographic: Lines")
            .series(vec![LinesSeries::new("Routes")
                .data(sample_lines(s))
                .color(teal())
                .effect(true)
                .into()])
            .build_in_gallery(ctx, view, content_width),
        _ => unreachable!("chart catalog and builder are out of sync"),
    }
}
