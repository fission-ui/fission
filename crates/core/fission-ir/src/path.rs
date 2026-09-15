//! Path motion data and the arc-length geometry every shell shares.
//!
//! A stroke can be trimmed to part of its path, and a composited node can be placed at a
//! distance along a path. Both are expressed as fractions of the path's arc length so a value
//! means the same thing in every shell; the geometry here is the single implementation shells use
//! to turn those fractions into points and path pieces.

use crate::op::CompositeScalar;
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg, Point};
use serde::{Deserialize, Serialize};

/// Arc-length accuracy, in path units, used when measuring and splitting paths.
const ACCURACY: f64 = 1e-3;

/// The part of a path a stroke draws, as fractions of its arc length.
///
/// `start: 0.0, end: 1.0` draws the whole path. Animating `end` from `0.0` to `1.0` draws the
/// path on progressively. Values are clamped to `0.0..=1.0`; when `end <= start` nothing is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StrokeTrim {
    pub start: f32,
    pub end: f32,
}

impl StrokeTrim {
    /// A trim drawing the path from `start` to `end`.
    pub fn new(start: f32, end: f32) -> Self {
        Self { start, end }
    }

    /// The trim with both ends clamped to `0.0..=1.0`.
    pub fn clamped(self) -> Self {
        Self {
            start: self.start.clamp(0.0, 1.0),
            end: self.end.clamp(0.0, 1.0),
        }
    }

    /// Whether the trim leaves the whole path drawn.
    pub fn is_full(self) -> bool {
        let trim = self.clamped();
        trim.start <= 0.0 && trim.end >= 1.0
    }
}

impl std::hash::Hash for StrokeTrim {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.start.to_bits().hash(state);
        self.end.to_bits().hash(state);
    }
}

/// Places a composited node along a path.
///
/// `path` is SVG path data in the node's own coordinate space: `(0, 0)` is the node's laid-out
/// top-left. The node's centre is moved onto the point `distance` of the way along the path's arc
/// length (`0.0` is the start, `1.0` the end), and when `rotate` is set the node also turns to
/// follow the path's direction. `distance` can be bound to a motion target to animate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash)]
pub struct PathOffset {
    pub path: String,
    pub distance: CompositeScalar,
    #[serde(default)]
    pub rotate: bool,
}

impl PathOffset {
    /// Translation and rotation that put the centre of a `width` by `height` node on the path at
    /// `distance`, a fraction of its arc length.
    pub fn delta(&self, distance: f32, width: f32, height: f32) -> (f32, f32, f32) {
        let Some(point) = point_along_svg_path(&self.path, distance) else {
            return (0.0, 0.0, 0.0);
        };
        let rotation = if self.rotate {
            point.angle_degrees
        } else {
            0.0
        };
        (point.x - width * 0.5, point.y - height * 0.5, rotation)
    }
}

/// A point on a path and the path's direction there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathPoint {
    pub x: f32,
    pub y: f32,
    /// Direction of travel in degrees, clockwise from the positive x axis.
    pub angle_degrees: f32,
}

/// A parsed path split into measured segments.
struct MeasuredPath {
    segments: Vec<(PathSeg, f64)>,
    length: f64,
}

fn measure(path: &str) -> Option<MeasuredPath> {
    let bez = BezPath::from_svg(path).ok()?;
    let segments: Vec<(PathSeg, f64)> = bez
        .segments()
        .map(|segment| {
            let length = segment.arclen(ACCURACY);
            (segment, length)
        })
        .collect();
    let length = segments.iter().map(|(_, length)| length).sum();
    Some(MeasuredPath { segments, length })
}

/// Arc length of an SVG path, or `None` if the path cannot be parsed.
pub fn svg_path_length(path: &str) -> Option<f32> {
    measure(path).map(|measured| measured.length as f32)
}

/// The point `fraction` of the way along an SVG path's arc length.
///
/// `fraction` is clamped to `0.0..=1.0`. Returns `None` if the path cannot be parsed or has no
/// segments.
pub fn point_along_svg_path(path: &str, fraction: f32) -> Option<PathPoint> {
    let measured = measure(path)?;
    let (last_segment, _) = *measured.segments.last()?;
    let target = measured.length * f64::from(fraction.clamp(0.0, 1.0));
    let mut travelled = 0.0;
    for (segment, length) in &measured.segments {
        if travelled + length >= target && *length > 0.0 {
            let t = segment.inv_arclen(target - travelled, ACCURACY);
            return Some(point_on_segment(*segment, t));
        }
        travelled += length;
    }
    Some(point_on_segment(last_segment, 1.0))
}

fn point_on_segment(segment: PathSeg, t: f64) -> PathPoint {
    let point = segment.eval(t);
    // A central difference is precise enough for orientation and avoids per-curve derivatives.
    let before = segment.eval((t - 1e-3).max(0.0));
    let after = segment.eval((t + 1e-3).min(1.0));
    let (dx, dy) = (after.x - before.x, after.y - before.y);
    let angle_degrees = if dx.abs() < f64::EPSILON && dy.abs() < f64::EPSILON {
        0.0
    } else {
        dy.atan2(dx).to_degrees() as f32
    };
    PathPoint {
        x: point.x as f32,
        y: point.y as f32,
        angle_degrees,
    }
}

/// The portion of an SVG path selected by `trim`, as a path ready to stroke.
///
/// Returns `None` if the path cannot be parsed. An empty path is returned when the trim selects
/// nothing.
pub fn trim_svg_path(path: &str, trim: StrokeTrim) -> Option<BezPath> {
    let measured = measure(path)?;
    let trim = trim.clamped();
    let mut trimmed = BezPath::new();
    if trim.end <= trim.start || measured.length <= 0.0 {
        return Some(trimmed);
    }
    let from = measured.length * f64::from(trim.start);
    let to = measured.length * f64::from(trim.end);
    let mut travelled = 0.0;
    let mut last_end: Option<Point> = None;
    for (segment, length) in &measured.segments {
        let segment_start = travelled;
        let segment_end = travelled + length;
        travelled = segment_end;
        if segment_end <= from || segment_start >= to || *length <= 0.0 {
            continue;
        }
        let t0 = if from > segment_start {
            segment.inv_arclen(from - segment_start, ACCURACY)
        } else {
            0.0
        };
        let t1 = if to < segment_end {
            segment.inv_arclen(to - segment_start, ACCURACY)
        } else {
            1.0
        };
        let piece = segment.subsegment(t0..t1);
        let start = piece.eval(0.0);
        if last_end.is_none_or(|end| (end - start).hypot() > ACCURACY) {
            trimmed.move_to(start);
        }
        match piece {
            PathSeg::Line(line) => trimmed.line_to(line.p1),
            PathSeg::Quad(quad) => trimmed.quad_to(quad.p1, quad.p2),
            PathSeg::Cubic(cubic) => trimmed.curve_to(cubic.p1, cubic.p2, cubic.p3),
        }
        last_end = Some(piece.eval(1.0));
    }
    Some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn measures_polylines_including_the_closing_segment() {
        assert!(close(
            svg_path_length("M0 0 L100 0 L100 50").unwrap(),
            150.0
        ));
        assert!(close(svg_path_length("M0 0 H10 V10 H0 Z").unwrap(), 40.0));
        assert!(svg_path_length("not a path").is_none());
    }

    #[test]
    fn finds_points_and_direction_by_arc_length() {
        let path = "M0 0 L100 0 L100 100";
        let quarter = point_along_svg_path(path, 0.25).unwrap();
        assert!(close(quarter.x, 50.0) && close(quarter.y, 0.0));
        assert!(close(quarter.angle_degrees, 0.0));
        let three_quarters = point_along_svg_path(path, 0.75).unwrap();
        assert!(close(three_quarters.x, 100.0) && close(three_quarters.y, 50.0));
        assert!(close(three_quarters.angle_degrees, 90.0));
        let end = point_along_svg_path(path, 2.0).unwrap();
        assert!(close(end.x, 100.0) && close(end.y, 100.0));
    }

    #[test]
    fn trims_to_the_selected_arc_length() {
        let path = "M0 0 L100 0 L100 100";
        let half = trim_svg_path(path, StrokeTrim::new(0.0, 0.5)).unwrap();
        let length: f64 = half.segments().map(|s| s.arclen(ACCURACY)).sum();
        assert!((length - 100.0).abs() < 0.1);
        let middle = trim_svg_path(path, StrokeTrim::new(0.25, 0.75)).unwrap();
        let length: f64 = middle.segments().map(|s| s.arclen(ACCURACY)).sum();
        assert!((length - 100.0).abs() < 0.1);
        let empty = trim_svg_path(path, StrokeTrim::new(0.6, 0.4)).unwrap();
        assert_eq!(empty.segments().count(), 0);
    }
}
