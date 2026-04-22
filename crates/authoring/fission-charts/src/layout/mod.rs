use crate::{Axis, Chart, Series};
use fission_ir::NodeId;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ChartGeometry {
    pub grid_rect: fission_layout::LayoutRect,
    pub x_scale: Scale,
    pub y_scale: Scale,
}

#[derive(Debug, Clone)]
pub enum Scale {
    Linear { min: f32, max: f32 },
    Category { labels: Vec<String> },
}

impl Scale {
    pub fn map(&self, value: f32, range_min: f32, range_max: f32) -> f32 {
        match self {
            Scale::Linear { min, max } => {
                let p = (value - *min) / (*max - *min).max(0.0001);
                range_min + p * (range_max - range_min)
            }
            Scale::Category { labels } => {
                let idx = value.floor() as usize;
                let step = (range_max - range_min) / labels.len().max(1) as f32;
                range_min + (idx as f32 + 0.5) * step
            }
        }
    }
}

pub fn calculate_scales(chart: &Chart) -> (Scale, Scale) {
    // Basic auto-scaling logic
    let mut y_min = 0.0f32;
    let mut y_max = 1.0f32;
    
    for series in &chart.series {
        match series {
            crate::Series::Line(s) => {
                for &v in &s.data {
                    y_max = y_max.max(v);
                    y_min = y_min.min(v);
                }
            }
            crate::Series::Bar(s) => {
                for &v in &s.data {
                    y_max = y_max.max(v);
                }
            }
            _ => {}
        }
    }
    
    let x_scale = if let Some(ax) = &chart.x_axis {
        match ax.axis_type {
            crate::axis::AxisType::Category => Scale::Category { labels: ax.data.clone() },
            _ => Scale::Linear { min: 0.0, max: 1.0 },
        }
    } else {
        Scale::Linear { min: 0.0, max: 1.0 }
    };

    (x_scale, Scale::Linear { min: y_min, max: y_max * 1.1 })
}
