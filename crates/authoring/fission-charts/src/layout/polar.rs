use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarCoord {
    pub cx: f32,
    pub cy: f32,
    pub radius_min: f32,
    pub radius_max: f32,
}

impl PolarCoord {
    pub fn new(cx: f32, cy: f32, radius_min: f32, radius_max: f32) -> Self {
        Self {
            cx,
            cy,
            radius_min,
            radius_max,
        }
    }

    /// Maps a radius value and an angle (in radians) to Cartesian coordinates (x, y).
    pub fn map(&self, radius_val: f32, angle_val: f32) -> (f32, f32) {
        let r = radius_val.clamp(self.radius_min, self.radius_max);
        let x = self.cx + r * angle_val.cos();
        let y = self.cy + r * angle_val.sin();
        (x, y)
    }
}
