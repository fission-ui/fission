use fission_core::ui::{Container, Widget};
use fission_core::WidgetId;
use fission_ir::op::{Fill, LineCap, LineJoin, Stroke};
use fission_layout::{LayoutPoint, LayoutRect};

use super::{CanvasGrid, CanvasGridPattern, CanvasVectorLayer};

#[derive(Debug, Clone)]
pub(crate) struct InfiniteCanvasGridLayer {
    pub canvas_id: WidgetId,
    pub grid: CanvasGrid,
    pub visible_world: LayoutRect,
}

impl From<InfiniteCanvasGridLayer> for Widget {
    fn from(layer: InfiniteCanvasGridLayer) -> Self {
        let spacing = layer.grid.spacing.max(1.0);
        let left = (layer.visible_world.x() / spacing).floor() * spacing;
        let top = (layer.visible_world.y() / spacing).floor() * spacing;
        let right = layer.visible_world.right() + spacing;
        let bottom = layer.visible_world.bottom() + spacing;
        let origin = LayoutPoint::new(left, top);

        let (mut minor, major) = match layer.grid.pattern {
            CanvasGridPattern::Lines { .. } => {
                grid_line_paths(left, top, right, bottom, spacing, layer.grid.major_every)
            }
            CanvasGridPattern::Dots {
                radius,
                major_radius,
            } => grid_dot_paths(
                left,
                top,
                right,
                bottom,
                spacing,
                layer.grid.major_every,
                radius.max(0.0),
                major_radius.unwrap_or(radius).max(0.0),
            ),
        };
        if layer.grid.major_color.is_none() {
            minor.push_str(&major);
        }
        let mut children = Vec::new();
        if !minor.is_empty() {
            children.push(positioned_path(
                layer.canvas_id,
                0x601D_0001,
                minor,
                origin,
                right - left,
                bottom - top,
                grid_fill(layer.grid.color, layer.grid.pattern),
                grid_stroke(layer.grid.color, layer.grid.pattern),
            ));
        }
        if let Some(color) = layer.grid.major_color {
            if !major.is_empty() {
                children.push(positioned_path(
                    layer.canvas_id,
                    0x601D_0002,
                    major,
                    origin,
                    right - left,
                    bottom - top,
                    grid_fill(color, layer.grid.pattern),
                    grid_stroke(color, layer.grid.pattern),
                ));
            }
        }
        fission_core::ui::ZStack { children, id: None }.into()
    }
}

fn positioned_path(
    canvas_id: WidgetId,
    discriminator: u32,
    path: String,
    origin: LayoutPoint,
    width: f32,
    height: f32,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
) -> Widget {
    Container::new(CanvasVectorLayer {
        id: WidgetId::derived(canvas_id.as_u128(), &[discriminator]),
        path,
        width,
        height,
        fill,
        stroke,
    })
    .positioned(Some(origin.x), Some(origin.y), None, None)
    .width(width)
    .height(height)
    .into()
}

fn grid_fill(color: fission_ir::op::Color, pattern: CanvasGridPattern) -> Option<Fill> {
    matches!(pattern, CanvasGridPattern::Dots { .. }).then_some(Fill::Solid(color))
}

fn grid_stroke(color: fission_ir::op::Color, pattern: CanvasGridPattern) -> Option<Stroke> {
    let CanvasGridPattern::Lines { width } = pattern else {
        return None;
    };
    Some(Stroke {
        fill: Fill::Solid(color),
        width: width.max(0.0),
        dash_array: None,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
    })
}

fn grid_line_paths(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    spacing: f32,
    major_every: u16,
) -> (String, String) {
    let mut minor = String::new();
    let mut major = String::new();
    let columns = ((right - left) / spacing).ceil().max(0.0) as usize;
    let rows = ((bottom - top) / spacing).ceil().max(0.0) as usize;
    let first_column = (left / spacing).round() as i64;
    let first_row = (top / spacing).round() as i64;
    for index in 0..=columns {
        let x = index as f32 * spacing;
        let world_index = first_column + index as i64;
        let target = if major_every > 0 && world_index.rem_euclid(major_every as i64) == 0 {
            &mut major
        } else {
            &mut minor
        };
        target.push_str(&format!("M{x} 0 L{x} {} ", bottom - top));
    }
    for index in 0..=rows {
        let y = index as f32 * spacing;
        let world_index = first_row + index as i64;
        let target = if major_every > 0 && world_index.rem_euclid(major_every as i64) == 0 {
            &mut major
        } else {
            &mut minor
        };
        target.push_str(&format!("M0 {y} L{} {y} ", right - left));
    }
    (minor, major)
}

#[allow(clippy::too_many_arguments)]
fn grid_dot_paths(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    spacing: f32,
    major_every: u16,
    minor_radius: f32,
    major_radius: f32,
) -> (String, String) {
    let mut minor = String::new();
    let mut major = String::new();
    let columns = ((right - left) / spacing).ceil().max(0.0) as usize;
    let rows = ((bottom - top) / spacing).ceil().max(0.0) as usize;
    let first_column = (left / spacing).round() as i64;
    let first_row = (top / spacing).round() as i64;
    for column in 0..=columns {
        for row in 0..=rows {
            let world_column = first_column + column as i64;
            let world_row = first_row + row as i64;
            let is_major = major_every > 0
                && world_column.rem_euclid(major_every as i64) == 0
                && world_row.rem_euclid(major_every as i64) == 0;
            let (target, radius) = if is_major {
                (&mut major, major_radius)
            } else {
                (&mut minor, minor_radius)
            };
            append_circle(
                target,
                column as f32 * spacing,
                row as f32 * spacing,
                radius,
            );
        }
    }
    (minor, major)
}

fn append_circle(path: &mut String, x: f32, y: f32, radius: f32) {
    if radius <= 0.0 {
        return;
    }
    let diameter = radius * 2.0;
    path.push_str(&format!(
        "M{} {y} a{radius} {radius} 0 1 0 {diameter} 0 a{radius} {radius} 0 1 0 -{diameter} 0 ",
        x - radius
    ));
}

#[cfg(test)]
mod tests {
    use super::{grid_dot_paths, grid_line_paths};

    #[test]
    fn grid_batches_minor_and_major_lines() {
        let (minor, major) = grid_line_paths(-20.0, -20.0, 60.0, 60.0, 20.0, 2);
        assert!(minor.contains("M0 0 L0 80"));
        assert!(major.contains("M20 0 L20 80"));
        assert!(major.contains("M0 20 L80 20"));
    }

    #[test]
    fn dot_grid_batches_major_intersections_across_negative_world_coordinates() {
        let (minor, major) = grid_dot_paths(-20.0, -20.0, 60.0, 60.0, 20.0, 2, 1.0, 2.0);
        assert!(minor.contains("M19 0 a1 1"));
        assert!(major.contains("M18 20 a2 2"));
        assert!(major.contains("M18 60 a2 2"));
    }
}
