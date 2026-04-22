use crate::layout::scale::{CategoryScale, LinearScale, Scale};
use std::f32::consts::PI;

pub trait CoordSystem {
    fn map(&self, x: f32, y: f32) -> (f32, f32);
}

#[derive(Debug, Clone)]
pub struct Cartesian2D {
    pub x_scale: Scale,
    pub y_scale: Scale,
    pub x_range: (f32, f32), // (left, right)
    pub y_range: (f32, f32), // (bottom, top)
}

impl Cartesian2D {
    pub fn new(x_scale: Scale, y_scale: Scale, x_range: (f32, f32), y_range: (f32, f32)) -> Self {
        Self { x_scale, y_scale, x_range, y_range }
    }

    pub fn map_val(&self, x_val: f32, y_val: f32) -> (f32, f32) {
        let px = match &self.x_scale {
            Scale::Linear(l) => l.map(x_val, self.x_range.0, self.x_range.1),
            Scale::Category(c) => c.map(x_val as usize, self.x_range.0, self.x_range.1),
        };
        let py = match &self.y_scale {
            Scale::Linear(l) => l.map(y_val, self.y_range.0, self.y_range.1),
            Scale::Category(c) => c.map(y_val as usize, self.y_range.0, self.y_range.1),
        };
        (px, py)
    }

    pub fn x_band_width(&self) -> f32 {
        match &self.x_scale {
            Scale::Category(c) => c.band_width(self.x_range.0, self.x_range.1),
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Polar {
    pub cx: f32,
    pub cy: f32,
    pub r_scale: Scale,
    pub angle_scale: Scale,
    pub r_range: (f32, f32),
    pub angle_range: (f32, f32),
}

impl Polar {
    pub fn map_val(&self, radius_val: f32, angle_val: f32) -> (f32, f32) {
        let r = match &self.r_scale {
            Scale::Linear(l) => l.map(radius_val, self.r_range.0, self.r_range.1),
            Scale::Category(c) => c.map(radius_val as usize, self.r_range.0, self.r_range.1),
        };
        let a = match &self.angle_scale {
            Scale::Linear(l) => l.map(angle_val, self.angle_range.0, self.angle_range.1),
            Scale::Category(c) => c.map(angle_val as usize, self.angle_range.0, self.angle_range.1),
        };
        (self.cx + r * a.cos(), self.cy + r * a.sin())
    }
}

#[derive(Debug, Clone)]
pub struct Geo {
    pub cx: f32,
    pub cy: f32,
    pub scale: f32,
}

impl Geo {
    pub fn map_val(&self, lon: f32, lat: f32) -> (f32, f32) {
        // Simple equirectangular projection for map parity
        let x = self.cx + (lon * PI / 180.0) * self.scale;
        let y = self.cy - (lat * PI / 180.0) * self.scale;
        (x, y)
    }
}
