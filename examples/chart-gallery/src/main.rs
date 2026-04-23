use fission_charts::{
    Axis, BarSeries, BoxplotSeries, CandlestickSeries, Chart, CustomSeries, EffectScatterSeries,
    FunnelSeries, GaugeSeries, GraphNode, GraphSeries, Grid, HeatmapSeries, Legend, LineSeries,
    LiquidfillSeries, MapSeries, ParallelSeries, PictorialBarSeries, PieSeries, RadarSeries,
    SankeySeries, ScatterSeries, Series, SunburstSeries, ThemeRiverSeries, Tooltip, TreemapNode,
    TreemapSeries, WordcloudSeries,
};
use fission_3d::{Point3D, Primitive3D, Scene3D};
use fission_core::{AppState, BuildCtx, View, Widget, ActionEnvelope};
use fission_core::op::Color;
use fission_core::ui::{Column, Container, Node, Row, Text, Button, ButtonVariant, Scroll};
use fission_shell_desktop::DesktopApp;
use serde::{Deserialize, Serialize};
use fission_macros::Action;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GalleryState {
    pub selected_category: usize,
    pub selected_chart: usize,
    pub smooth: bool,
    pub gap: f32,
    pub data_scale: f32,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            selected_category: 0,
            selected_chart: 0,
            smooth: true,
            gap: 10.0,
            data_scale: 1.0,
        }
    }
}

impl AppState for GalleryState {}

#[derive(Action, Serialize, Deserialize, Clone, Debug)]
pub struct SelectChart(pub usize, pub usize);

#[derive(Action, Serialize, Deserialize, Clone, Debug)]
pub struct ToggleSmooth(pub bool);

#[derive(Action, Serialize, Deserialize, Clone, Debug)]
pub struct UpdateGap(pub f32);

#[derive(Action, Serialize, Deserialize, Clone, Debug)]
pub struct UpdateScale(pub f32);

struct GalleryApp;

impl Widget<GalleryState> for GalleryApp {
    fn build(&self, ctx: &mut BuildCtx<GalleryState>, view: &View<GalleryState>) -> Node {
        let select_chart_id = ctx.bind(
            SelectChart(0, 0),
            (|s: &mut GalleryState, a: SelectChart, _| {
                s.selected_category = a.0;
                s.selected_chart = a.1;
            }) as fission_core::registry::Handler<GalleryState, SelectChart>
        ).id;

        let toggle_smooth_id = ctx.bind(
            ToggleSmooth(false),
            (|s: &mut GalleryState, a: ToggleSmooth, _| {
                s.smooth = !s.smooth; // toggle it
            }) as fission_core::registry::Handler<GalleryState, ToggleSmooth>
        ).id;

        let update_scale_id = ctx.bind(
            UpdateScale(0.0),
            (|s: &mut GalleryState, a: UpdateScale, _| {
                s.data_scale = a.0;
            }) as fission_core::registry::Handler<GalleryState, UpdateScale>
        ).id;

        let categories = vec![
            ("Foundational", vec!["Line & Bar", "Pie", "Scatter"]),
            ("Statistical", vec!["Boxplot", "Candlestick", "Heatmap", "Graph", "Treemap"]),
            ("Specialized", vec!["Radar", "Funnel", "Gauge", "Map", "Sankey", "Parallel", "Sunburst"]),
            ("Dynamic", vec!["ThemeRiver", "PictorialBar", "EffectScatter"]),
            ("Extensions & Custom", vec!["Custom", "Liquidfill", "Wordcloud"]),
            ("3D", vec!["Scene3D"]),
        ];

        let mut sidebar_items = vec![
            Text::new("Chart Gallery").size(24.0).color(Color::WHITE).into_node(),
            fission_widgets::Spacer { height: Some(16.0), ..Default::default() }.into_node(),
        ];

        for (cat_idx, (cat_name, charts)) in categories.iter().enumerate() {
            sidebar_items.push(
                Text::new(*cat_name).size(14.0).color(Color { r: 180, g: 180, b: 180, a: 255 }).into_node()
            );
            
            for (chart_idx, chart_name) in charts.iter().enumerate() {
                let is_selected = view.state.selected_category == cat_idx && view.state.selected_chart == chart_idx;
                
                sidebar_items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        on_press: Some(ActionEnvelope {
                            id: select_chart_id,
                            payload: serde_json::to_vec(&SelectChart(cat_idx, chart_idx)).unwrap(),
                        }),
                        child: Some(Box::new(
                            Text::new(*chart_name)
                                .size(13.0)
                                .color(if is_selected { Color::WHITE } else { Color { r: 160, g: 160, b: 160, a: 255 } })
                                .into_node()
                        )),
                        ..Default::default()
                    }.into_node()
                );
            }
            sidebar_items.push(fission_widgets::Spacer { height: Some(8.0), ..Default::default() }.into_node());
        }

        let sidebar = Container::new(
            Scroll {
                direction: fission_core::FlexDirection::Column,
                child: Some(Box::new(Column {
                    children: sidebar_items,
                    gap: Some(4.0),
                    ..Default::default()
                }.into_node())),
                show_scrollbar: true,
                ..Default::default()
            }.into_node()
        )
        .width(200.0)
        .padding_all(12.0)
        .bg(Color { r: 30, g: 30, b: 30, a: 255 })
        .flex_shrink(0.0)
        .into_node();

        // Main content area
        let s = view.state.data_scale;
        let chart_node = match (view.state.selected_category, view.state.selected_chart) {
            (0, 0) => {
                Chart::new(800.0, 500.0)
                    .title("Foundational: Line & Bar")
                    .x_axis(Axis::category(vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]))
                    .y_axis(Axis::value())
                    .series(vec![
                        BarSeries::new("Direct")
                            .data(vec![320.0 * s, 332.0 * s, 301.0 * s, 334.0 * s, 390.0 * s, 330.0 * s, 320.0 * s])
                            .color(Color { r: 84, g: 112, b: 198, a: 255 })
                            .into(),
                        LineSeries::new("Email")
                            .data(vec![120.0 * s, 132.0 * s, 101.0 * s, 134.0 * s, 90.0 * s, 230.0 * s, 210.0 * s])
                            .color(Color { r: 145, g: 204, b: 117, a: 255 })
                            .smooth(view.state.smooth)
                            .into(),
                    ])
                    .build(ctx, view)
            }
            (0, 1) => {
                Chart::new(800.0, 500.0)
                    .title("Foundational: Pie")
                    .series(vec![
                        PieSeries::new("Access Source")
                            .data(vec![
                                ("Search Engine", 1048.0),
                                ("Direct", 735.0),
                                ("Email", 580.0),
                                ("Union Ads", 484.0),
                                ("Video Ads", 300.0),
                            ])
                            .into(),
                    ])
                    .build(ctx, view)
            }
            (0, 2) => {
                Chart::new(800.0, 500.0)
                    .title("Foundational: Scatter")
                    .x_axis(Axis::value())
                    .y_axis(Axis::value())
                    .series(vec![
                        ScatterSeries::new("Data")
                            .data(vec![(10.0, 8.04), (8.0, 6.95), (13.0, 7.58), (9.0, 8.81), (11.0, 8.33), (14.0, 9.96)])
                            .color(Color { r: 250, g: 200, b: 88, a: 255 })
                            .into(),
                    ])
                    .build(ctx, view)
            }
            (1, 0) => {
                Chart::new(800.0, 500.0)
                    .title("Statistical: Boxplot")
                    .x_axis(Axis::category(vec!["expr 1", "expr 2", "expr 3"]))
                    .y_axis(Axis::value())
                    .series(vec![
                        BoxplotSeries::new("Boxplot")
                            .data(vec![
                                vec![850.0, 960.0, 1060.0, 1080.0, 1100.0],
                                vec![800.0, 850.0, 900.0, 930.0, 980.0],
                                vec![750.0, 800.0, 850.0, 900.0, 1000.0],
                            ])
                            .color(Color { r: 115, g: 192, b: 222, a: 255 })
                            .into()
                    ])
                    .build(ctx, view)
            }
            (5, 0) => {
                Scene3D::new(800.0, 500.0)
                    .add_primitive(Primitive3D::Cube {
                        center: Point3D::new(0.0, 0.0, 0.0),
                        size: 2.0,
                        color: Color::RED,
                    })
                    .add_primitive(Primitive3D::Sphere {
                        center: Point3D::new(3.0, 3.0, 3.0),
                        radius: 1.5,
                        color: Color::BLUE,
                    })
                    .build(ctx, view)
            }
            _ => {
                Container::new(
                    Text::new("Chart implementation rendered as placeholder")
                        .color(Color { r: 150, g: 150, b: 150, a: 255 })
                        .into_node()
                ).into_node()
            }
        };

        let controls = Row {
            children: vec![
                Text::new("Smooth Lines:").color(Color::WHITE).into_node(),
                fission_widgets::Switch {
                    checked: view.state.smooth,
                    on_toggle: Some(ActionEnvelope { id: toggle_smooth_id, payload: vec![] }),
                    ..Default::default()
                }.into_node(),
                fission_widgets::Spacer { width: Some(32.0), ..Default::default() }.into_node(),
                Text::new("Data Scale:").color(Color::WHITE).into_node(),
                fission_widgets::Slider {
                    value: view.state.data_scale,
                    min: 0.1,
                    max: 2.0,
                    on_change: Some(ActionEnvelope { id: update_scale_id, payload: vec![] }),
                    ..Default::default()
                }.into_node(),
            ],
            gap: Some(12.0),
            align_items: fission_core::op::AlignItems::Center,
            ..Default::default()
        }.into_node();

        let content = Container::new(
            Column {
                children: vec![
                    Row {
                        children: vec![
                            Text::new("Interactive Demo").size(24.0).color(Color::WHITE).into_node(),
                            fission_widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                        ],
                        ..Default::default()
                    }.into_node(),
                    fission_widgets::Spacer { height: Some(24.0), ..Default::default() }.into_node(),
                    chart_node,
                    fission_widgets::Spacer { height: Some(24.0), ..Default::default() }.into_node(),
                    controls,
                ],
                flex_grow: 1.0,
                ..Default::default()
            }.into_node()
        )
        .padding_all(32.0)
        .bg(Color { r: 20, g: 20, b: 20, a: 255 })
        .flex_grow(1.0)
        .into_node();

        Row {
            children: vec![sidebar, content],
            flex_grow: 1.0,
            ..Default::default()
        }.into_node()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(GalleryApp)
        .with_title("Fission Chart Gallery")
        .with_sync_env(|_state: &GalleryState, env: &mut fission_core::Env| {
            env.theme = fission_theme::Theme::dark();
        })
        .with_frame_hook(|_state| {
            true // Keep event loop active for QA testing script
        });

    app.run()
}
