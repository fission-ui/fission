//! Stroke styling for painted paths, rectangles and borders.

use crate::op::{Fill, LayoutUnit};
use crate::path::StrokeTrim;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub fill: Fill,
    pub width: LayoutUnit,
    pub dash_array: Option<Vec<f32>>,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    /// How far into the dash pattern the stroke starts, in logical pixels. Animating it makes a
    /// dashed stroke appear to flow along its path.
    #[serde(default)]
    pub dash_offset: f32,
    /// Draws only part of the path, as fractions of its arc length. `None` draws the whole path.
    #[serde(default)]
    pub trim: Option<StrokeTrim>,
}

impl std::hash::Hash for Stroke {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fill.hash(state);
        self.width.to_bits().hash(state);
        if let Some(da) = &self.dash_array {
            1.hash(state);
            for d in da {
                d.to_bits().hash(state);
            }
        } else {
            0.hash(state);
        }
        self.line_cap.hash(state);
        self.line_join.hash(state);
        self.dash_offset.to_bits().hash(state);
        self.trim.hash(state);
    }
}
