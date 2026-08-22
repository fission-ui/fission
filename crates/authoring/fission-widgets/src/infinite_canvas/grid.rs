use fission_core::ui::{Container, Widget};
use fission_core::WidgetId;
use fission_ir::op::{Fill, LineCap, LineJoin, Stroke};
use fission_layout::{LayoutPoint, LayoutRect};

use super::{CanvasGrid, CanvasVectorLayer};

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

        let (minor, major) = grid_paths(left, top, right, bottom, spacing, layer.grid.major_every);
        let mut children = Vec::new();
        if !minor.is_empty() {
            children.push(positioned_path(
                layer.canvas_id,
                0x601D_0001,
                minor,
                origin,
                right - left,
                bottom - top,
                Stroke {
                    fill: Fill::Solid(layer.grid.color),
                    width: layer.grid.line_width.max(0.0),
                    dash_array: None,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                },
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
                    Stroke {
                        fill: Fill::Solid(color),
                        width: layer.grid.line_width.max(0.0),
                        dash_array: None,
                        line_cap: LineCap::Butt,
                        line_join: LineJoin::Miter,
                    },
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
    stroke: Stroke,
) -> Widget {
    Container::new(CanvasVectorLayer {
        id: WidgetId::derived(canvas_id.as_u128(), &[discriminator]),
        path,
        width,
        height,
        fill: None,
        stroke: Some(stroke),
    })
    .positioned(Some(origin.x), Some(origin.y), None, None)
    .width(width)
    .height(height)
    .into()
}

fn grid_paths(
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

#[cfg(test)]
mod tests {
    use super::grid_paths;

    #[test]
    fn grid_batches_minor_and_major_lines() {
        let (minor, major) = grid_paths(-20.0, -20.0, 60.0, 60.0, 20.0, 2);
        assert!(minor.contains("M0 0 L0 80"));
        assert!(major.contains("M20 0 L20 80"));
        assert!(major.contains("M0 20 L80 20"));
    }
}
