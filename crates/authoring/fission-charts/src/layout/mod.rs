pub mod scale;
pub mod math;
pub mod treemap;
pub mod force_graph;
pub mod sankey;
pub mod wordcloud;
pub mod map;
pub mod polar;
pub mod calendar;
pub mod time_scale;
pub mod log_scale;

use crate::Chart;
use scale::{CategoryScale, LinearScale, Scale};

pub fn calculate_scales(chart: &Chart) -> (Scale, Scale) {
    let mut y_min = f32::MAX;
    let mut y_max = f32::MIN;
    let mut has_data = false;

    for series in &chart.series {
        match series {
            crate::Series::Line(s) => {
                for &v in &s.data {
                    y_max = y_max.max(v);
                    y_min = y_min.min(v);
                    has_data = true;
                }
            }
            crate::Series::Bar(s) => {
                for &v in &s.data {
                    y_max = y_max.max(v);
                    y_min = y_min.min(v.min(0.0)); // bars usually start at 0
                    has_data = true;
                }
            }
            crate::Series::Scatter(s) => {
                for &(_, dy) in &s.data {
                    y_max = y_max.max(dy);
                    y_min = y_min.min(dy);
                    has_data = true;
                }
            }
            _ => {}
        }
    }

    if !has_data {
        y_min = 0.0;
        y_max = 1.0;
    }

    let x_scale = if let Some(ax) = &chart.x_axis {
        match ax.axis_type {
            crate::axis::AxisType::Category => {
                Scale::Category(CategoryScale::new(ax.data.clone()))
            }
            _ => Scale::Linear(LinearScale::nice(0.0, 1.0, 5)),
        }
    } else {
        Scale::Linear(LinearScale::nice(0.0, 1.0, 5))
    };

    let y_scale = Scale::Linear(LinearScale::nice(y_min, y_max, 5));

    (x_scale, y_scale)
}
