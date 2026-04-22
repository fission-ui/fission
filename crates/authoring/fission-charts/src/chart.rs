use crate::axis::{Axis, AxisType};
use crate::grid::Grid;
use crate::legend::Legend;
use crate::series::Series;
use crate::tooltip::Tooltip;
use crate::layout::{calculate_scales, Scale};
use fission_core::op::Color;
use fission_core::{BuildCtx, View, Widget};
use fission_core::ui::{Node, Container, CustomNode};
use fission_ir::op::{PaintOp, LayoutOp, Stroke, Fill, LineCap, LineJoin};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chart {
    pub width: f32,
    pub height: f32,
    pub title: Option<String>,
    pub tooltip: Option<Tooltip>,
    pub legend: Option<Legend>,
    pub grid: Option<Grid>,
    pub x_axis: Option<Axis>,
    pub y_axis: Option<Axis>,
    pub series: Vec<Series>,
    pub animate: bool,
}

impl Chart {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            title: None,
            tooltip: None,
            legend: None,
            grid: None,
            x_axis: None,
            y_axis: None,
            series: Vec::new(),
            animate: false,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn x_axis(mut self, axis: Axis) -> Self {
        self.x_axis = Some(axis);
        self
    }

    pub fn y_axis(mut self, axis: Axis) -> Self {
        self.y_axis = Some(axis);
        self
    }

    pub fn series(mut self, series: Vec<Series>) -> Self {
        self.series = series;
        self
    }

    pub fn grid(mut self, grid: Grid) -> Self {
        self.grid = Some(grid);
        self
    }

    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }
}

impl<S: fission_core::AppState> Widget<S> for Chart {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Container::new(
            Node::Custom(CustomNode {
                debug_tag: "fission_charts::Chart".into(),
                lowerer: Some(std::sync::Arc::new(ChartLowerer {
                    chart: self.clone(),
                })),
            })
        ).width(self.width).height(self.height).into_node()
    }
}

#[derive(Debug)]
pub struct ChartLowerer {
    pub chart: Chart,
}

// Helper to generate SVG arc path
fn arc(cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32) -> String {
    let start_x = cx + r * start_angle.cos();
    let start_y = cy + r * start_angle.sin();
    let end_x = cx + r * end_angle.cos();
    let end_y = cy + r * end_angle.sin();
    let large_arc = if end_angle - start_angle > std::f32::consts::PI { 1 } else { 0 };
    format!("M {} {} A {} {} 0 {} 1 {} {}", start_x, start_y, r, r, large_arc, end_x, end_y)
}

fn pie_slice(cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32) -> String {
    let arc_str = arc(cx, cy, r, start_angle, end_angle);
    format!("{} L {} {} Z", arc_str, cx, cy)
}

impl fission_core::ui::traits::LowerDyn for ChartLowerer {
    fn lower_dyn(&self, cx: &mut fission_core::lowering::LoweringContext) -> fission_ir::NodeId {
        let node_id = cx.next_node_id();
        let mut root = fission_core::lowering::NodeBuilder::new(node_id, fission_ir::Op::Layout(fission_ir::op::LayoutOp::ZStack));
        
        let w = self.chart.width;
        let h = self.chart.height;
        let pad_left = 60.0;
        let pad_bottom = 40.0;
        let pad_top = 40.0;
        let pad_right = 40.0;
        
        let inner_w = (w - pad_left - pad_right).max(0.0);
        let inner_h = (h - pad_top - pad_bottom).max(0.0);

        let (x_scale, y_scale) = calculate_scales(&self.chart);

        // 1. Grid Background
        let grid_id = cx.next_node_id();
        let grid_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color { r: 250, g: 250, b: 250, a: 255 })),
            stroke: Some(Stroke {
                fill: Fill::Solid(Color { r: 230, g: 230, b: 230, a: 255 }),
                width: 1.0,
                dash_array: None,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
            }),
            corner_radius: 0.0,
            shadow: None,
        });
        let mut grid_pos = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
            left: Some(pad_left), top: Some(pad_top), width: Some(inner_w), height: Some(inner_h),
            right: None, bottom: None,
        }));
        grid_pos.add_child(cx.insert_node(grid_id, grid_paint, vec![]));
        root.add_child(grid_pos.build(cx));

        // 2. Axes
        if let Scale::Linear { min, max } = &y_scale {
            let steps = 5;
            for i in 0..=steps {
                let val = min + (max - min) * (i as f32 / steps as f32);
                let y = pad_top + inner_h - (i as f32 / steps as f32) * inner_h;
                
                let label_id = cx.next_node_id();
                let label_paint = fission_ir::Op::Paint(PaintOp::DrawText {
                    text: format!("{:.0}", val),
                    size: 11.0,
                    color: Color { r: 100, g: 100, b: 100, a: 255 },
                    underline: false,
                    caret_index: None,
                });
                let mut label_pos = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                    left: Some(5.0), top: Some(y - 6.0), width: Some(pad_left - 10.0), height: Some(12.0),
                    right: None, bottom: None,
                }));
                label_pos.add_child(cx.insert_node(label_id, label_paint, vec![]));
                root.add_child(label_pos.build(cx));
            }
        }

        if let Scale::Category { labels } = &x_scale {
            let step = inner_w / labels.len().max(1) as f32;
            for (i, label_str) in labels.iter().enumerate() {
                let label_text: String = label_str.clone();
                let x = pad_left + (i as f32) * step;
                
                let label_id = cx.next_node_id();
                let label_paint = fission_ir::Op::Paint(PaintOp::DrawText {
                    text: label_text,
                    size: 11.0,
                    color: Color { r: 100, g: 100, b: 100, a: 255 },
                    underline: false,
                    caret_index: None,
                });
                let mut label_pos = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                    left: Some(x), top: Some(pad_top + inner_h + 5.0), width: Some(step), height: Some(20.0),
                    right: None, bottom: None,
                }));
                label_pos.add_child(cx.insert_node(label_id, label_paint, vec![]));
                root.add_child(label_pos.build(cx));
            }
        }

        // 3. Series
        for series in &self.chart.series {
            match series {
                Series::Bar(bar) => {
                    if let Scale::Category { labels: _ } = &x_scale {
                        let step = inner_w / self.chart.x_axis.as_ref().map(|a| a.data.len()).unwrap_or(1).max(1) as f32;
                        let bar_w = step * 0.7;
                        for (i, val_ref) in bar.data.iter().enumerate() {
                            let val: f32 = *val_ref;
                            let x = pad_left + (i as f32) * step + (step - bar_w) / 2.0;
                            let mapped_h = y_scale.map(val, 0.0, inner_h);
                            let y = pad_top + inner_h - mapped_h;
                            
                            let bar_id = cx.next_node_id();
                            let bar_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
                                fill: Some(Fill::Solid(bar.color)),
                                stroke: None,
                                corner_radius: 2.0,
                                shadow: None,
                            });
                            let mut bar_pos = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                                left: Some(x), top: Some(y), width: Some(bar_w), height: Some(mapped_h),
                                right: None, bottom: None,
                            }));
                            bar_pos.add_child(cx.insert_node(bar_id, bar_paint, vec![]));
                            root.add_child(bar_pos.build(cx));
                        }
                    }
                }
                Series::Line(line) => {
                    if line.data.is_empty() { continue; }
                    let step = inner_w / (line.data.len().max(2) - 1) as f32;
                    let mut path = String::new();
                    
                    for (i, &val) in line.data.iter().enumerate() {
                        let x = pad_left + (i as f32) * step;
                        let mapped_h = y_scale.map(val, 0.0, inner_h);
                        let y = pad_top + inner_h - mapped_h;
                        
                        if i == 0 {
                            path.push_str(&format!("M {} {}", x, y));
                        } else {
                            if line.smooth {
                                // Simple smooth curve approximation (Bezier)
                                let px = pad_left + ((i - 1) as f32) * step;
                                let pval = line.data[i - 1];
                                let pmapped_h = y_scale.map(pval, 0.0, inner_h);
                                let py = pad_top + inner_h - pmapped_h;
                                let cp1x = px + step / 2.0;
                                let cp2x = x - step / 2.0;
                                path.push_str(&format!(" C {} {} {} {} {} {}", cp1x, py, cp2x, y, x, y));
                            } else {
                                path.push_str(&format!(" L {} {}", x, y));
                            }
                        }
                    }

                    let line_id = cx.next_node_id();
                    let line_paint = fission_ir::Op::Paint(PaintOp::DrawPath {
                        path,
                        fill: None,
                        stroke: Some(Stroke {
                            fill: Fill::Solid(line.color),
                            width: 2.0,
                            dash_array: None,
                            line_cap: LineCap::Round,
                            line_join: LineJoin::Round,
                        }),
                    });
                    root.add_child(cx.insert_node(line_id, line_paint, vec![]));
                }
                Series::Scatter(scatter) => {
                    for &(dx, dy) in &scatter.data {
                        let x = pad_left + (dx / 100.0) * inner_w;
                        let y = pad_top + inner_h - (dy / 100.0) * inner_h;
                        
                        let r = 5.0;
                        let rect_id = cx.next_node_id();
                        let rect_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
                            fill: Some(Fill::Solid(scatter.color)),
                            stroke: None,
                            corner_radius: r,
                            shadow: None,
                        });
                        
                        let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                            left: Some(x - r), top: Some(y - r), width: Some(r * 2.0), height: Some(r * 2.0), right: None, bottom: None,
                        }));
                        pos_b.add_child(cx.insert_node(rect_id, rect_paint, vec![]));
                        root.add_child(pos_b.build(cx));
                    }
                }
                Series::Pie(pie) => {
                    let total: f32 = pie.data.iter().map(|(_, v)| v).sum();
                    if total == 0.0 { continue; }
                    
                    let cx_pie = pad_left + inner_w / 2.0;
                    let cy_pie = pad_top + inner_h / 2.0;
                    let r = inner_h.min(inner_w) * 0.4;
                    let mut current_angle = -std::f32::consts::PI / 2.0;
                    
                    let colors = [
                        Color { r: 84, g: 112, b: 198, a: 255 },
                        Color { r: 145, g: 204, b: 117, a: 255 },
                        Color { r: 250, g: 204, b: 20, a: 255 },
                        Color { r: 238, g: 102, b: 102, a: 255 },
                        Color { r: 115, g: 192, b: 222, a: 255 },
                    ];

                    for (i, (_, val)) in pie.data.iter().enumerate() {
                        let sweep_angle = (val / total) * 2.0 * std::f32::consts::PI;
                        let end_angle = current_angle + sweep_angle;
                        
                        let path = pie_slice(cx_pie, cy_pie, r, current_angle, end_angle);
                        let color = colors[i % colors.len()];
                        
                        let slice_id = cx.next_node_id();
                        let slice_paint = fission_ir::Op::Paint(PaintOp::DrawPath {
                            path,
                            fill: Some(Fill::Solid(color)),
                            stroke: Some(Stroke {
                                fill: Fill::Solid(Color { r: 255, g: 255, b: 255, a: 255 }),
                                width: 1.0,
                                dash_array: None,
                                line_cap: LineCap::Round,
                                line_join: LineJoin::Round,
                            }),
                        });
                        root.add_child(cx.insert_node(slice_id, slice_paint, vec![]));
                        
                        current_angle = end_angle;
                    }
                }
                Series::Boxplot(boxplot) => {
                    let step = inner_w / boxplot.data.len().max(1) as f32;
                    let box_w = step * 0.5;
                    for (i, points) in boxplot.data.iter().enumerate() {
                        if points.len() < 5 { continue; }
                        let x = pad_left + (i as f32) * step + (step - box_w) / 2.0;
                        let min_y = pad_top + inner_h - (points[0] / 100.0) * inner_h;
                        let q1_y = pad_top + inner_h - (points[1] / 100.0) * inner_h;
                        let med_y = pad_top + inner_h - (points[2] / 100.0) * inner_h;
                        let q3_y = pad_top + inner_h - (points[3] / 100.0) * inner_h;
                        let max_y = pad_top + inner_h - (points[4] / 100.0) * inner_h;
                        
                        // Main Box
                        let rect_id = cx.next_node_id();
                        let rect_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
                            fill: Some(Fill::Solid(Color { r: boxplot.color.r, g: boxplot.color.g, b: boxplot.color.b, a: 100 })),
                            stroke: Some(Stroke { fill: Fill::Solid(boxplot.color), width: 1.5, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }),
                            corner_radius: 0.0,
                            shadow: None,
                        });
                        let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                            left: Some(x), top: Some(q3_y.min(q1_y)), width: Some(box_w), height: Some((q1_y - q3_y).abs().max(1.0)), right: None, bottom: None,
                        }));
                        pos_b.add_child(cx.insert_node(rect_id, rect_paint, vec![]));
                        root.add_child(pos_b.build(cx));

                        // Median Line
                        let med_path = format!("M {} {} L {} {}", x, med_y, x + box_w, med_y);
                        let med_id = cx.next_node_id();
                        let med_paint = fission_ir::Op::Paint(PaintOp::DrawPath { path: med_path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(boxplot.color), width: 2.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                        root.add_child(cx.insert_node(med_id, med_paint, vec![]));

                        // Whiskers
                        let center_x = x + box_w / 2.0;
                        let whisk_path = format!("M {} {} L {} {} M {} {} L {} {} M {} {} L {} {} M {} {} L {} {}",
                            center_x, min_y, center_x, q1_y.max(q3_y),
                            x, min_y, x + box_w, min_y,
                            center_x, max_y, center_x, q1_y.min(q3_y),
                            x, max_y, x + box_w, max_y
                        );
                        let whisk_id = cx.next_node_id();
                        let whisk_paint = fission_ir::Op::Paint(PaintOp::DrawPath { path: whisk_path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(boxplot.color), width: 1.5, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                        root.add_child(cx.insert_node(whisk_id, whisk_paint, vec![]));
                    }
                }
                Series::Candlestick(candle) => {
                    let step = inner_w / candle.data.len().max(1) as f32;
                    let box_w = step * 0.6;
                    for (i, points) in candle.data.iter().enumerate() {
                        if points.len() < 4 { continue; } // [open, close, low, high]
                        let open = points[0];
                        let close = points[1];
                        let low = points[2];
                        let high = points[3];
                        
                        let color = if close > open { candle.color_up } else { candle.color_down };
                        
                        let x = pad_left + (i as f32) * step + (step - box_w) / 2.0;
                        let center_x = x + box_w / 2.0;
                        let top_y = pad_top + inner_h - (open.max(close) / 100.0) * inner_h;
                        let bottom_y = pad_top + inner_h - (open.min(close) / 100.0) * inner_h;
                        let high_y = pad_top + inner_h - (high / 100.0) * inner_h;
                        let low_y = pad_top + inner_h - (low / 100.0) * inner_h;
                        
                        // Body
                        let rect_id = cx.next_node_id();
                        let rect_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
                            fill: if close > open { None } else { Some(Fill::Solid(color)) },
                            stroke: Some(Stroke { fill: Fill::Solid(color), width: 1.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }),
                            corner_radius: 0.0,
                            shadow: None,
                        });
                        let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                            left: Some(x), top: Some(top_y), width: Some(box_w), height: Some((bottom_y - top_y).max(1.0)), right: None, bottom: None,
                        }));
                        pos_b.add_child(cx.insert_node(rect_id, rect_paint, vec![]));
                        root.add_child(pos_b.build(cx));

                        // Wick
                        let wick_path = format!("M {} {} L {} {} M {} {} L {} {}", center_x, high_y, center_x, top_y, center_x, bottom_y, center_x, low_y);
                        let wick_id = cx.next_node_id();
                        let wick_paint = fission_ir::Op::Paint(PaintOp::DrawPath { path: wick_path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(color), width: 1.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                        root.add_child(cx.insert_node(wick_id, wick_paint, vec![]));
                    }
                }
                Series::Heatmap(heatmap) => {
                    let max_x = heatmap.data.iter().map(|d| d.0).max().unwrap_or(1).max(1) as f32 + 1.0;
                    let max_y = heatmap.data.iter().map(|d| d.1).max().unwrap_or(1).max(1) as f32 + 1.0;
                    let max_val = heatmap.data.iter().map(|d| d.2).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(1.0);
                    let cell_w = inner_w / max_x;
                    let cell_h = inner_h / max_y;
                    
                    for (x_idx, y_idx, val) in &heatmap.data {
                        let intensity = (*val / max_val).clamp(0.0, 1.0) * 255.0;
                        let color = Color { r: intensity as u8, g: 0, b: (255.0 - intensity) as u8, a: 255 };
                        let px = pad_left + (*x_idx as f32) * cell_w;
                        let py = pad_top + inner_h - (*y_idx as f32 + 1.0) * cell_h;
                        
                        let rect_id = cx.next_node_id();
                        let rect_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
                            fill: Some(Fill::Solid(color)),
                            stroke: Some(Stroke { fill: Fill::Solid(Color { r: 255, g: 255, b: 255, a: 255 }), width: 1.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }),
                            corner_radius: 0.0,
                            shadow: None,
                        });
                        let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                            left: Some(px), top: Some(py), width: Some(cell_w), height: Some(cell_h), right: None, bottom: None,
                        }));
                        pos_b.add_child(cx.insert_node(rect_id, rect_paint, vec![]));
                        root.add_child(pos_b.build(cx));
                    }
                }
                Series::Graph(graph) => {
                    let cx_graph = pad_left + inner_w / 2.0;
                    let cy_graph = pad_top + inner_h / 2.0;
                    let r = inner_h.min(inner_w) * 0.4;
                    let mut node_positions = std::collections::HashMap::new();
                    
                    for (i, node) in graph.nodes.iter().enumerate() {
                        let angle = (i as f32 / graph.nodes.len() as f32) * 2.0 * std::f32::consts::PI;
                        let px = cx_graph + r * angle.cos();
                        let py = cy_graph + r * angle.sin();
                        node_positions.insert(node.id.clone(), (px, py, node.value));
                    }
                    
                    for edge in &graph.edges {
                        if let (Some(&(x1, y1, _)), Some(&(x2, y2, _))) = (node_positions.get(&edge.source), node_positions.get(&edge.target)) {
                            let path = format!("M {} {} L {} {}", x1, y1, x2, y2);
                            let edge_id = cx.next_node_id();
                            let edge_paint = fission_ir::Op::Paint(PaintOp::DrawPath {
                                path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(Color { r: 150, g: 150, b: 150, a: 150 }), width: 1.5, dash_array: None, line_cap: LineCap::Round, line_join: LineJoin::Round }),
                            });
                            root.add_child(cx.insert_node(edge_id, edge_paint, vec![]));
                        }
                    }
                    
                    for (id, (px, py, val)) in node_positions {
                        let radius = 5.0 + (val / 100.0) * 15.0; // scale node by value
                        let rect_id = cx.next_node_id();
                        let rect_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
                            fill: Some(Fill::Solid(Color { r: 54, g: 162, b: 235, a: 255 })),
                            stroke: Some(Stroke { fill: Fill::Solid(Color { r: 255, g: 255, b: 255, a: 255 }), width: 1.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }),
                            corner_radius: radius,
                            shadow: None,
                        });
                        let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                            left: Some(px - radius), top: Some(py - radius), width: Some(radius * 2.0), height: Some(radius * 2.0), right: None, bottom: None,
                        }));
                        pos_b.add_child(cx.insert_node(rect_id, rect_paint, vec![]));
                        root.add_child(pos_b.build(cx));
                    }
                }
                Series::Treemap(treemap) => {
                    let total: f32 = treemap.data.iter().map(|n| n.value).sum();
                    let mut current_x = pad_left;
                    let colors = [
                        Color { r: 84, g: 112, b: 198, a: 255 },
                        Color { r: 145, g: 204, b: 117, a: 255 },
                        Color { r: 250, g: 204, b: 20, a: 255 },
                    ];
                    for (i, node) in treemap.data.iter().enumerate() {
                        let w = (node.value / total) * inner_w;
                        let rect_id = cx.next_node_id();
                        let color = colors[i % colors.len()];
                        let rect_paint = fission_ir::Op::Paint(PaintOp::DrawRect {
                            fill: Some(Fill::Solid(color)),
                            stroke: Some(Stroke { fill: Fill::Solid(Color { r: 255, g: 255, b: 255, a: 255 }), width: 2.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }),
                            corner_radius: 0.0, shadow: None,
                        });
                        let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned {
                            left: Some(current_x), top: Some(pad_top), width: Some(w), height: Some(inner_h), right: None, bottom: None,
                        }));
                        pos_b.add_child(cx.insert_node(rect_id, rect_paint, vec![]));
                        root.add_child(pos_b.build(cx));
                        current_x += w;
                    }
                }
                Series::Radar(radar) => {
                    let cx_radar = pad_left + inner_w / 2.0;
                    let cy_radar = pad_top + inner_h / 2.0;
                    let r = inner_h.min(inner_w) * 0.4;
                    let axes = radar.data.get(0).map(|d| d.len()).unwrap_or(5);
                    
                    // Radar axes
                    for i in 0..axes {
                        let angle = (i as f32 / axes as f32) * 2.0 * std::f32::consts::PI - std::f32::consts::PI / 2.0;
                        let px = cx_radar + r * angle.cos();
                        let py = cy_radar + r * angle.sin();
                        let path = format!("M {} {} L {} {}", cx_radar, cy_radar, px, py);
                        let id = cx.next_node_id();
                        let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(Color { r: 200, g: 200, b: 200, a: 255 }), width: 1.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                        root.add_child(cx.insert_node(id, paint, vec![]));
                    }

                    // Radar polygons
                    for (idx, series_data) in radar.data.iter().enumerate() {
                        let mut path = String::new();
                        for (i, &val) in series_data.iter().enumerate() {
                            let angle = (i as f32 / axes as f32) * 2.0 * std::f32::consts::PI - std::f32::consts::PI / 2.0;
                            let scaled_r = r * (val / 100.0);
                            let px = cx_radar + scaled_r * angle.cos();
                            let py = cy_radar + scaled_r * angle.sin();
                            if i == 0 { path.push_str(&format!("M {} {}", px, py)); } else { path.push_str(&format!(" L {} {}", px, py)); }
                        }
                        path.push_str(" Z");
                        let id = cx.next_node_id();
                        let color = if idx % 2 == 0 { Color { r: 54, g: 162, b: 235, a: 100 } } else { Color { r: 255, g: 99, b: 132, a: 100 } };
                        let border_color = if idx % 2 == 0 { Color { r: 54, g: 162, b: 235, a: 255 } } else { Color { r: 255, g: 99, b: 132, a: 255 } };
                        let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: Some(Fill::Solid(color)), stroke: Some(Stroke { fill: Fill::Solid(border_color), width: 2.0, dash_array: None, line_cap: LineCap::Round, line_join: LineJoin::Round }) });
                        root.add_child(cx.insert_node(id, paint, vec![]));
                    }
                }
                Series::Funnel(funnel) => {
                    let total_h = inner_h;
                    let step_h = total_h / funnel.data.len().max(1) as f32;
                    let max_val = funnel.data.iter().map(|(_, v)| *v).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(100.0);
                    let cx_funnel = pad_left + inner_w / 2.0;
                    
                    let mut current_y = pad_top;
                    for (i, (_, val)) in funnel.data.iter().enumerate() {
                        let w_top = if i == 0 { inner_w } else { (funnel.data[i - 1].1 / max_val) * inner_w };
                        let w_bot = (*val / max_val) * inner_w;
                        let path = format!("M {} {} L {} {} L {} {} L {} {} Z", cx_funnel - w_top / 2.0, current_y, cx_funnel + w_top / 2.0, current_y, cx_funnel + w_bot / 2.0, current_y + step_h, cx_funnel - w_bot / 2.0, current_y + step_h);
                        
                        let color = Color { r: (100 + i * 30) as u8, g: (150 + i * 20) as u8, b: 200, a: 255 };
                        let id = cx.next_node_id();
                        let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: Some(Fill::Solid(color)), stroke: Some(Stroke { fill: Fill::Solid(Color { r: 255, g: 255, b: 255, a: 255 }), width: 2.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                        root.add_child(cx.insert_node(id, paint, vec![]));
                        current_y += step_h;
                    }
                }
                Series::Gauge(gauge) => {
                    let cx_gauge = pad_left + inner_w / 2.0;
                    let cy_gauge = pad_top + inner_h / 1.5;
                    let r = inner_h.min(inner_w) * 0.5;
                    
                    // Background arc
                    let bg_arc = arc(cx_gauge, cy_gauge, r, std::f32::consts::PI, 2.0 * std::f32::consts::PI);
                    let bg_id = cx.next_node_id();
                    let bg_paint = fission_ir::Op::Paint(PaintOp::DrawPath { path: bg_arc, fill: None, stroke: Some(Stroke { fill: Fill::Solid(Color { r: 230, g: 230, b: 230, a: 255 }), width: 20.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                    root.add_child(cx.insert_node(bg_id, bg_paint, vec![]));

                    if let Some((_, val)) = gauge.data.first() {
                        let angle = std::f32::consts::PI + (*val / 100.0) * std::f32::consts::PI;
                        let val_arc = arc(cx_gauge, cy_gauge, r, std::f32::consts::PI, angle);
                        let val_id = cx.next_node_id();
                        let val_paint = fission_ir::Op::Paint(PaintOp::DrawPath { path: val_arc, fill: None, stroke: Some(Stroke { fill: Fill::Solid(Color { r: 54, g: 162, b: 235, a: 255 }), width: 20.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                        root.add_child(cx.insert_node(val_id, val_paint, vec![]));

                        // Needle
                        let nx = cx_gauge + (r * 0.8) * angle.cos();
                        let ny = cy_gauge + (r * 0.8) * angle.sin();
                        let needle_path = format!("M {} {} L {} {}", cx_gauge, cy_gauge, nx, ny);
                        let n_id = cx.next_node_id();
                        let n_paint = fission_ir::Op::Paint(PaintOp::DrawPath { path: needle_path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(Color { r: 50, g: 50, b: 50, a: 255 }), width: 4.0, dash_array: None, line_cap: LineCap::Round, line_join: LineJoin::Round }) });
                        root.add_child(cx.insert_node(n_id, n_paint, vec![]));
                        
                        // Pivot
                        let p_id = cx.next_node_id();
                        let p_paint = fission_ir::Op::Paint(PaintOp::DrawRect { fill: Some(Fill::Solid(Color { r: 50, g: 50, b: 50, a: 255 })), stroke: None, corner_radius: 8.0, shadow: None });
                        let mut p_pos = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned { left: Some(cx_gauge - 8.0), top: Some(cy_gauge - 8.0), width: Some(16.0), height: Some(16.0), right: None, bottom: None }));
                        p_pos.add_child(cx.insert_node(p_id, p_paint, vec![]));
                        root.add_child(p_pos.build(cx));
                    }
                }
                Series::Map(_) => {
                    // Draw a placeholder map silhouette
                    let cx_map = pad_left + inner_w / 2.0;
                    let cy_map = pad_top + inner_h / 2.0;
                    let path = format!("M {} {} L {} {} L {} {} Z", cx_map - 100.0, cy_map + 50.0, cx_map, cy_map - 100.0, cx_map + 100.0, cy_map + 50.0);
                    let id = cx.next_node_id();
                    let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: Some(Fill::Solid(Color { r: 230, g: 240, b: 230, a: 255 })), stroke: Some(Stroke { fill: Fill::Solid(Color { r: 150, g: 180, b: 150, a: 255 }), width: 2.0, dash_array: None, line_cap: LineCap::Round, line_join: LineJoin::Round }) });
                    root.add_child(cx.insert_node(id, paint, vec![]));
                }
                Series::Sankey(sankey) => {
                    if sankey.nodes.len() > 1 && !sankey.edges.is_empty() {
                        // Mock 2-layer sankey
                        let n1_y = pad_top + 50.0;
                        let n2_y = pad_top + 150.0;
                        
                        let path = format!("M {} {} C {} {} {} {} {} {} L {} {} C {} {} {} {} {} {} Z", 
                            pad_left, n1_y, 
                            pad_left + inner_w / 2.0, n1_y, pad_left + inner_w / 2.0, n2_y, pad_left + inner_w, n2_y,
                            pad_left + inner_w, n2_y + 40.0,
                            pad_left + inner_w / 2.0, n2_y + 40.0, pad_left + inner_w / 2.0, n1_y + 40.0, pad_left, n1_y + 40.0);
                        let id = cx.next_node_id();
                        let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: Some(Fill::Solid(Color { r: 84, g: 112, b: 198, a: 100 })), stroke: None });
                        root.add_child(cx.insert_node(id, paint, vec![]));
                    }
                }
                Series::Parallel(parallel) => {
                    let axes_count = parallel.data.get(0).map(|d| d.len()).unwrap_or(3);
                    let step = inner_w / (axes_count - 1).max(1) as f32;
                    
                    // Axes
                    for i in 0..axes_count {
                        let x = pad_left + i as f32 * step;
                        let path = format!("M {} {} L {} {}", x, pad_top, x, pad_top + inner_h);
                        let id = cx.next_node_id();
                        let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(Color { r: 200, g: 200, b: 200, a: 255 }), width: 2.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                        root.add_child(cx.insert_node(id, paint, vec![]));
                    }

                    // Lines
                    for line in &parallel.data {
                        let mut path = String::new();
                        for (i, &val) in line.iter().enumerate() {
                            let x = pad_left + i as f32 * step;
                            let y = pad_top + inner_h - (val / 100.0) * inner_h;
                            if i == 0 { path.push_str(&format!("M {} {}", x, y)); } else { path.push_str(&format!(" L {} {}", x, y)); }
                        }
                        let id = cx.next_node_id();
                        let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: None, stroke: Some(Stroke { fill: Fill::Solid(Color { r: 255, g: 99, b: 132, a: 150 }), width: 2.0, dash_array: None, line_cap: LineCap::Round, line_join: LineJoin::Round }) });
                        root.add_child(cx.insert_node(id, paint, vec![]));
                    }
                }
                Series::Sunburst(sunburst) => {
                    let cx_sun = pad_left + inner_w / 2.0;
                    let cy_sun = pad_top + inner_h / 2.0;
                    let r1 = inner_h.min(inner_w) * 0.2;
                    let r2 = inner_h.min(inner_w) * 0.4;
                    
                    let path1 = pie_slice(cx_sun, cy_sun, r1, 0.0, std::f32::consts::PI);
                    let id1 = cx.next_node_id();
                    let paint1 = fission_ir::Op::Paint(PaintOp::DrawPath { path: path1, fill: Some(Fill::Solid(Color { r: 250, g: 204, b: 20, a: 255 })), stroke: Some(Stroke { fill: Fill::Solid(Color { r: 255, g: 255, b: 255, a: 255 }), width: 1.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                    root.add_child(cx.insert_node(id1, paint1, vec![]));

                    let path2 = arc(cx_sun, cy_sun, r2, 0.0, std::f32::consts::PI);
                    let id2 = cx.next_node_id();
                    let paint2 = fission_ir::Op::Paint(PaintOp::DrawPath { path: format!("{} L {} {} A {} {} 0 0 0 {} {} Z", path2, cx_sun - r1, cy_sun, r1, r1, cx_sun + r1, cy_sun), fill: Some(Fill::Solid(Color { r: 250, g: 204, b: 20, a: 150 })), stroke: Some(Stroke { fill: Fill::Solid(Color { r: 255, g: 255, b: 255, a: 255 }), width: 1.0, dash_array: None, line_cap: LineCap::Butt, line_join: LineJoin::Miter }) });
                    root.add_child(cx.insert_node(id2, paint2, vec![]));
                }
                Series::ThemeRiver(_) => {
                    let path = format!("M {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} Z", 
                        pad_left, pad_top + inner_h / 2.0, 
                        pad_left + inner_w / 3.0, pad_top + 20.0, pad_left + 2.0 * inner_w / 3.0, pad_top + inner_h - 20.0, pad_left + inner_w, pad_top + inner_h / 2.0,
                        pad_left + 2.0 * inner_w / 3.0, pad_top + inner_h, pad_left + inner_w / 3.0, pad_top, pad_left, pad_top + inner_h / 2.0);
                    let id = cx.next_node_id();
                    let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: Some(Fill::Solid(Color { r: 145, g: 204, b: 117, a: 200 })), stroke: None });
                    root.add_child(cx.insert_node(id, paint, vec![]));
                }
                Series::PictorialBar(pic) => {
                    let step = inner_w / pic.data.len().max(1) as f32;
                    for (i, &val) in pic.data.iter().enumerate() {
                        let x = pad_left + (i as f32) * step + step / 2.0;
                        let count = (val / 20.0).floor() as i32;
                        for j in 0..count {
                            let y = pad_top + inner_h - (j as f32 * 20.0) - 10.0;
                            let id = cx.next_node_id();
                            let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path: format!("M {} {} L {} {} L {} {} Z", x, y - 8.0, x + 8.0, y + 8.0, x - 8.0, y + 8.0), fill: Some(Fill::Solid(pic.color)), stroke: None });
                            root.add_child(cx.insert_node(id, paint, vec![]));
                        }
                    }
                }
                Series::EffectScatter(effect) => {
                    for &(dx, dy) in &effect.data {
                        let x = pad_left + (dx / 100.0) * inner_w;
                        let y = pad_top + inner_h - (dy / 100.0) * inner_h;
                        for scale in [1.0, 1.5, 2.0] {
                            let r = 8.0 * scale;
                            let id = cx.next_node_id();
                            let paint = fission_ir::Op::Paint(PaintOp::DrawRect { fill: Some(Fill::Solid(Color { r: effect.color.r, g: effect.color.g, b: effect.color.b, a: (255.0 / scale) as u8 })), stroke: None, corner_radius: r, shadow: None });
                            let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned { left: Some(x - r), top: Some(y - r), width: Some(r * 2.0), height: Some(r * 2.0), right: None, bottom: None }));
                            pos_b.add_child(cx.insert_node(id, paint, vec![]));
                            root.add_child(pos_b.build(cx));
                        }
                    }
                }
                Series::Custom(_) => {
                    let id = cx.next_node_id();
                    let paint = fission_ir::Op::Paint(PaintOp::DrawRect { fill: Some(Fill::Solid(Color { r: 200, g: 100, b: 200, a: 150 })), stroke: Some(Stroke { fill: Fill::Solid(Color { r: 100, g: 50, b: 100, a: 255 }), width: 2.0, dash_array: Some(vec![5.0, 5.0]), line_cap: LineCap::Round, line_join: LineJoin::Round }), corner_radius: 8.0, shadow: None });
                    let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned { left: Some(pad_left + inner_w * 0.2), top: Some(pad_top + inner_h * 0.2), width: Some(inner_w * 0.6), height: Some(inner_h * 0.6), right: None, bottom: None }));
                    pos_b.add_child(cx.insert_node(id, paint, vec![]));
                    root.add_child(pos_b.build(cx));
                }
                Series::Liquidfill(_) => {
                    let cx_liq = pad_left + inner_w / 2.0;
                    let cy_liq = pad_top + inner_h / 2.0;
                    let r = inner_h.min(inner_w) * 0.4;
                    let id_bg = cx.next_node_id();
                    let paint_bg = fission_ir::Op::Paint(PaintOp::DrawRect { fill: Some(Fill::Solid(Color { r: 230, g: 240, b: 250, a: 255 })), stroke: None, corner_radius: r, shadow: None });
                    let mut pos_bg = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned { left: Some(cx_liq - r), top: Some(cy_liq - r), width: Some(r * 2.0), height: Some(r * 2.0), right: None, bottom: None }));
                    pos_bg.add_child(cx.insert_node(id_bg, paint_bg, vec![]));
                    root.add_child(pos_bg.build(cx));
                    
                    let path = format!("M {} {} Q {} {} {} {} T {} {} L {} {} L {} {} Z", 
                        cx_liq - r, cy_liq, cx_liq - r/2.0, cy_liq - 20.0, cx_liq, cy_liq, cx_liq + r, cy_liq, cx_liq + r, cy_liq + r, cx_liq - r, cy_liq + r);
                    let id = cx.next_node_id();
                    let paint = fission_ir::Op::Paint(PaintOp::DrawPath { path, fill: Some(Fill::Solid(Color { r: 84, g: 112, b: 198, a: 200 })), stroke: None });
                    root.add_child(cx.insert_node(id, paint, vec![]));
                }
                Series::Wordcloud(wordcloud) => {
                    let cx_word = pad_left + inner_w / 2.0;
                    let cy_word = pad_top + inner_h / 2.0;
                    for (i, (word, weight)) in wordcloud.data.iter().enumerate() {
                        let id = cx.next_node_id();
                        let paint = fission_ir::Op::Paint(PaintOp::DrawText { text: word.clone(), size: 10.0 + (weight / 100.0) * 30.0, color: Color { r: (100 + i * 20 % 150) as u8, g: (50 + i * 40 % 200) as u8, b: 150, a: 255 }, underline: false, caret_index: None });
                        let mut pos_b = fission_core::lowering::NodeBuilder::new(cx.next_node_id(), fission_ir::Op::Layout(LayoutOp::Positioned { left: Some(cx_word - 50.0 + (i as f32 * 40.0 % 100.0)), top: Some(cy_word - 50.0 + (i as f32 * 30.0 % 100.0)), width: Some(100.0), height: Some(40.0), right: None, bottom: None }));
                        pos_b.add_child(cx.insert_node(id, paint, vec![]));
                        root.add_child(pos_b.build(cx));
                    }
                }
            }
        }

        root.build(cx)
    }
}
