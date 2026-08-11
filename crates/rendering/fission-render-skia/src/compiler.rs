use std::fmt;

use fission_render::capabilities::{is_2d_affine_transform, DisplayOpKind};
use fission_render::{
    BoxShadow, Color, DisplayList, DisplayOp, Fill, LayerClip, LayoutPoint, LayoutRect, LineCap,
    LineJoin, RenderNode, RenderScene, Stroke,
};
use kurbo::{BezPath, PathEl};

use crate::api::{
    RasterAffine, RasterBoxShadow, RasterColor, RasterCommand, RasterFillRule, RasterFrame,
    RasterGradientStop, RasterLineCap, RasterLineJoin, RasterPaint, RasterPath, RasterPathCommand,
    RasterPoint, RasterRect, RasterStroke,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompiledRasterFrame {
    pub frame: RasterFrame,
    pub source_operations: u64,
}

pub(crate) fn compile_scene(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
) -> Result<CompiledRasterFrame, CompileError> {
    let scale_factor = scale_factor as f32;
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return Err(CompileError::new(
            CompileErrorKind::InvalidScaleFactor,
            CompileProvenance::default(),
        ));
    }
    let mut compiler = Compiler {
        scale_factor,
        commands: vec![RasterCommand::Clear(srgb_color(clear_color))],
        source_operations: 0,
        save_depth: 0,
    };
    for (root_index, root) in scene.roots.iter().enumerate() {
        compiler.compile_node(root, root_index, &mut Vec::new(), None)?;
    }
    if compiler.save_depth != 0 {
        return Err(CompileError::new(
            CompileErrorKind::UnbalancedSaveRestore {
                remaining_saves: compiler.save_depth,
            },
            CompileProvenance::default(),
        ));
    }
    Ok(CompiledRasterFrame {
        frame: RasterFrame {
            commands: compiler.commands,
        },
        source_operations: compiler.source_operations,
    })
}

struct Compiler {
    scale_factor: f32,
    commands: Vec<RasterCommand>,
    source_operations: u64,
    save_depth: usize,
}

impl Compiler {
    fn compile_node(
        &mut self,
        node: &RenderNode,
        root_index: usize,
        node_path: &mut Vec<usize>,
        inherited_node_id: Option<fission_ir::WidgetId>,
    ) -> Result<(), CompileError> {
        match node {
            RenderNode::Paint(list) => self.compile_list(
                list,
                root_index,
                node_path,
                &mut Vec::new(),
                inherited_node_id,
            ),
            RenderNode::Layer(layer) => {
                let node_id = layer.node_id.or(inherited_node_id);
                if layer.style.opacity.to_bits() != 1.0_f32.to_bits() {
                    return Err(CompileError::new(
                        CompileErrorKind::UnsupportedOperation(DisplayOpKind::OpacityLayer),
                        CompileProvenance::layer(root_index, node_path, node_id, "opacity"),
                    ));
                }
                let needs_save = layer.style.clip.is_some() || layer.style.transform.is_some();
                if needs_save {
                    self.push_save();
                }
                if let Some(clip) = layer.style.clip.as_ref() {
                    let provenance =
                        CompileProvenance::layer(root_index, node_path, node_id, "clip");
                    self.compile_layer_clip(clip, provenance)?;
                }
                if let Some(matrix) = layer.style.transform.as_ref() {
                    let provenance =
                        CompileProvenance::layer(root_index, node_path, node_id, "transform");
                    self.commands.push(RasterCommand::ConcatAffine(
                        self.affine(*matrix, &provenance)?,
                    ));
                }
                for (child_index, child) in layer.children.iter().enumerate() {
                    node_path.push(child_index);
                    self.compile_node(child, root_index, node_path, node_id)?;
                    node_path.pop();
                }
                if needs_save {
                    self.push_restore(CompileProvenance::layer(
                        root_index, node_path, node_id, "restore",
                    ))?;
                }
                Ok(())
            }
        }
    }

    fn compile_layer_clip(
        &mut self,
        clip: &LayerClip,
        provenance: CompileProvenance,
    ) -> Result<(), CompileError> {
        match clip {
            LayerClip::Rect(rect) => self.commands.push(RasterCommand::ClipRect {
                rect: self.rect(*rect, &provenance)?,
            }),
            LayerClip::RoundedRect { rect, radius } => {
                self.commands.push(RasterCommand::ClipRoundedRect {
                    rect: self.rect(*rect, &provenance)?,
                    radius: self.scaled(*radius, &provenance, "clip radius")?,
                })
            }
        }
        Ok(())
    }

    fn compile_list(
        &mut self,
        list: &DisplayList,
        root_index: usize,
        node_path: &[usize],
        operation_path: &mut Vec<usize>,
        inherited_node_id: Option<fission_ir::WidgetId>,
    ) -> Result<(), CompileError> {
        for (operation_index, operation) in list.ops.iter().enumerate() {
            operation_path.push(operation_index);
            self.source_operations = self.source_operations.saturating_add(1);
            let provenance = CompileProvenance::display_list(
                root_index,
                node_path,
                operation_path,
                operation_node_id(operation).or(inherited_node_id),
            );
            self.compile_operation(
                operation,
                root_index,
                node_path,
                operation_path,
                inherited_node_id,
                provenance,
            )?;
            operation_path.pop();
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_operation(
        &mut self,
        operation: &DisplayOp,
        root_index: usize,
        node_path: &[usize],
        operation_path: &mut Vec<usize>,
        inherited_node_id: Option<fission_ir::WidgetId>,
        provenance: CompileProvenance,
    ) -> Result<(), CompileError> {
        match operation {
            DisplayOp::Save => self.push_save(),
            DisplayOp::Restore => self.push_restore(provenance)?,
            DisplayOp::ClipRect(rect) => self.commands.push(RasterCommand::ClipRect {
                rect: self.rect(*rect, &provenance)?,
            }),
            DisplayOp::ClipRoundedRect { rect, radius } => {
                self.commands.push(RasterCommand::ClipRoundedRect {
                    rect: self.rect(*rect, &provenance)?,
                    radius: self.scaled(*radius, &provenance, "clip radius")?,
                })
            }
            DisplayOp::Translate(point) => {
                self.commands.push(RasterCommand::ConcatAffine(
                    self.translation(*point, &provenance)?,
                ));
            }
            DisplayOp::Transform(matrix) => {
                self.commands.push(RasterCommand::ConcatAffine(
                    self.affine(*matrix, &provenance)?,
                ));
            }
            DisplayOp::CachedScene { list, .. } => self.compile_list(
                list,
                root_index,
                node_path,
                operation_path,
                inherited_node_id,
            )?,
            DisplayOp::DrawRect {
                rect,
                fill,
                stroke,
                corner_radius,
                shadow,
                ..
            } => self.draw_rect(
                *rect,
                fill.as_ref(),
                stroke.as_ref(),
                *corner_radius,
                shadow.as_ref(),
                &provenance,
            )?,
            DisplayOp::DrawPath {
                path,
                fill,
                stroke,
                bounds,
                ..
            } => self.draw_path(path, fill.as_ref(), stroke.as_ref(), *bounds, &provenance)?,
            other => {
                return Err(CompileError::new(
                    CompileErrorKind::UnsupportedOperation(other.kind()),
                    provenance,
                ));
            }
        }
        Ok(())
    }

    fn push_save(&mut self) {
        self.save_depth = self.save_depth.saturating_add(1);
        self.commands.push(RasterCommand::Save);
    }

    fn push_restore(&mut self, provenance: CompileProvenance) -> Result<(), CompileError> {
        self.save_depth = self
            .save_depth
            .checked_sub(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::RestoreWithoutSave, provenance))?;
        self.commands.push(RasterCommand::Restore);
        Ok(())
    }

    fn draw_rect(
        &mut self,
        rect: LayoutRect,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        corner_radius: f32,
        shadow: Option<&BoxShadow>,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        let physical_rect = self.rect(rect, provenance)?;
        let physical_radius = self.scaled(corner_radius, provenance, "corner radius")?;
        if let Some(shadow) = shadow {
            self.commands.push(RasterCommand::BoxShadow {
                rect: physical_rect,
                radius: physical_radius,
                shadow: self.shadow(*shadow, provenance)?,
            });
        }
        if let Some(fill) = fill {
            self.commands.push(RasterCommand::FillRect {
                rect: physical_rect,
                radius: physical_radius,
                paint: self.paint(fill, rect, provenance)?,
            });
        }
        if let Some(stroke) = stroke {
            self.commands.push(RasterCommand::StrokeRect {
                rect: physical_rect,
                radius: physical_radius,
                stroke: self.stroke(stroke, rect, provenance)?,
            });
        }
        Ok(())
    }

    fn draw_path(
        &mut self,
        source: &str,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        bounds: LayoutRect,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        let parsed = BezPath::from_svg(source).map_err(|error| {
            CompileError::new(
                CompileErrorKind::InvalidPath(error.to_string()),
                provenance.clone(),
            )
        })?;
        if parsed.elements().is_empty() {
            return Ok(());
        }
        let path = self.path(&parsed, bounds.origin, provenance)?;
        if let Some(fill) = fill {
            self.commands.push(RasterCommand::FillPath {
                path: path.clone(),
                paint: self.paint(fill, bounds, provenance)?,
            });
        }
        if let Some(stroke) = stroke {
            self.commands.push(RasterCommand::StrokePath {
                path,
                stroke: self.stroke(stroke, bounds, provenance)?,
            });
        }
        Ok(())
    }

    fn paint(
        &self,
        fill: &Fill,
        bounds: LayoutRect,
        provenance: &CompileProvenance,
    ) -> Result<RasterPaint, CompileError> {
        match fill {
            Fill::Solid(color) => Ok(RasterPaint::Solid(srgb_color(*color))),
            Fill::LinearGradient { start, end, stops } => Ok(RasterPaint::LinearGradient {
                start: self.normalized_point(bounds, *start, provenance)?,
                end: self.normalized_point(bounds, *end, provenance)?,
                stops: self.gradient_stops(stops, provenance)?,
            }),
            Fill::RadialGradient {
                center,
                radius,
                stops,
            } => Ok(RasterPaint::RadialGradient {
                center: self.normalized_point(bounds, *center, provenance)?,
                radius: self.scaled(
                    *radius * bounds.width().max(bounds.height()),
                    provenance,
                    "radial gradient radius",
                )?,
                stops: self.gradient_stops(stops, provenance)?,
            }),
        }
    }

    fn gradient_stops(
        &self,
        stops: &[(f32, Color)],
        provenance: &CompileProvenance,
    ) -> Result<Vec<RasterGradientStop>, CompileError> {
        stops
            .iter()
            .map(|(offset, color)| {
                if !offset.is_finite() || !(0.0..=1.0).contains(offset) {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidPaint(
                            "gradient stop offsets must be in 0..=1".into(),
                        ),
                        provenance.clone(),
                    ));
                }
                Ok(RasterGradientStop {
                    offset: *offset,
                    color: srgb_color(*color),
                })
            })
            .collect()
    }

    fn stroke(
        &self,
        stroke: &Stroke,
        bounds: LayoutRect,
        provenance: &CompileProvenance,
    ) -> Result<RasterStroke, CompileError> {
        let dash_array = stroke
            .dash_array
            .as_ref()
            .map(|dashes| {
                dashes
                    .iter()
                    .map(|dash| self.scaled(*dash, provenance, "stroke dash interval"))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(RasterStroke {
            paint: self.paint(&stroke.fill, bounds, provenance)?,
            width: self.scaled(stroke.width, provenance, "stroke width")?,
            dash_array,
            line_cap: match stroke.line_cap {
                LineCap::Butt => RasterLineCap::Butt,
                LineCap::Round => RasterLineCap::Round,
                LineCap::Square => RasterLineCap::Square,
            },
            line_join: match stroke.line_join {
                LineJoin::Miter => RasterLineJoin::Miter,
                LineJoin::Round => RasterLineJoin::Round,
                LineJoin::Bevel => RasterLineJoin::Bevel,
            },
        })
    }

    fn shadow(
        &self,
        shadow: BoxShadow,
        provenance: &CompileProvenance,
    ) -> Result<RasterBoxShadow, CompileError> {
        Ok(RasterBoxShadow {
            color: srgb_color(shadow.color),
            blur_radius: self.scaled(shadow.blur_radius, provenance, "shadow blur radius")?,
            spread_radius: self.scaled(shadow.spread_radius, provenance, "shadow spread radius")?,
            offset: RasterPoint {
                x: self.scaled(shadow.offset.0, provenance, "shadow x offset")?,
                y: self.scaled(shadow.offset.1, provenance, "shadow y offset")?,
            },
            inset: shadow.inset,
        })
    }

    fn path(
        &self,
        path: &BezPath,
        origin: LayoutPoint,
        provenance: &CompileProvenance,
    ) -> Result<RasterPath, CompileError> {
        let point = |x: f64, y: f64| -> Result<(f32, f32), CompileError> {
            Ok((
                self.scaled(origin.x + x as f32, provenance, "path x coordinate")?,
                self.scaled(origin.y + y as f32, provenance, "path y coordinate")?,
            ))
        };
        let commands = path
            .elements()
            .iter()
            .map(|element| match element {
                PathEl::MoveTo(value) => {
                    let (x, y) = point(value.x, value.y)?;
                    Ok(RasterPathCommand::MoveTo { x, y })
                }
                PathEl::LineTo(value) => {
                    let (x, y) = point(value.x, value.y)?;
                    Ok(RasterPathCommand::LineTo { x, y })
                }
                PathEl::QuadTo(control, value) => {
                    let (cx, cy) = point(control.x, control.y)?;
                    let (x, y) = point(value.x, value.y)?;
                    Ok(RasterPathCommand::QuadTo { cx, cy, x, y })
                }
                PathEl::CurveTo(first, second, value) => {
                    let (c1x, c1y) = point(first.x, first.y)?;
                    let (c2x, c2y) = point(second.x, second.y)?;
                    let (x, y) = point(value.x, value.y)?;
                    Ok(RasterPathCommand::CubicTo {
                        c1x,
                        c1y,
                        c2x,
                        c2y,
                        x,
                        y,
                    })
                }
                PathEl::ClosePath => Ok(RasterPathCommand::Close),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RasterPath {
            fill_rule: RasterFillRule::NonZero,
            commands,
        })
    }

    fn normalized_point(
        &self,
        bounds: LayoutRect,
        normalized: (f32, f32),
        provenance: &CompileProvenance,
    ) -> Result<RasterPoint, CompileError> {
        Ok(RasterPoint {
            x: self.scaled(
                bounds.x() + bounds.width() * normalized.0,
                provenance,
                "gradient x coordinate",
            )?,
            y: self.scaled(
                bounds.y() + bounds.height() * normalized.1,
                provenance,
                "gradient y coordinate",
            )?,
        })
    }

    fn rect(
        &self,
        rect: LayoutRect,
        provenance: &CompileProvenance,
    ) -> Result<RasterRect, CompileError> {
        Ok(RasterRect {
            left: self.scaled(rect.x(), provenance, "rectangle left")?,
            top: self.scaled(rect.y(), provenance, "rectangle top")?,
            right: self.scaled(rect.right(), provenance, "rectangle right")?,
            bottom: self.scaled(rect.bottom(), provenance, "rectangle bottom")?,
        })
    }

    fn translation(
        &self,
        point: LayoutPoint,
        provenance: &CompileProvenance,
    ) -> Result<RasterAffine, CompileError> {
        Ok(RasterAffine::translation(
            self.scaled(point.x, provenance, "translation x")?,
            self.scaled(point.y, provenance, "translation y")?,
        ))
    }

    fn affine(
        &self,
        matrix: [f32; 16],
        provenance: &CompileProvenance,
    ) -> Result<RasterAffine, CompileError> {
        if !is_2d_affine_transform(&matrix) {
            return Err(CompileError::new(
                CompileErrorKind::UnsupportedTransform,
                provenance.clone(),
            ));
        }
        Ok(RasterAffine {
            scale_x: matrix[0],
            skew_x: matrix[4],
            translate_x: self.scaled(matrix[12], provenance, "transform x translation")?,
            skew_y: matrix[1],
            scale_y: matrix[5],
            translate_y: self.scaled(matrix[13], provenance, "transform y translation")?,
        })
    }

    fn scaled(
        &self,
        value: f32,
        provenance: &CompileProvenance,
        label: &'static str,
    ) -> Result<f32, CompileError> {
        let scaled = value * self.scale_factor;
        if scaled.is_finite() {
            Ok(scaled)
        } else {
            Err(CompileError::new(
                CompileErrorKind::PhysicalGeometryOverflow(label),
                provenance.clone(),
            ))
        }
    }
}

fn srgb_color(color: Color) -> RasterColor {
    RasterColor {
        red: f32::from(color.r) / 255.0,
        green: f32::from(color.g) / 255.0,
        blue: f32::from(color.b) / 255.0,
        alpha: f32::from(color.a) / 255.0,
    }
}

fn operation_node_id(operation: &DisplayOp) -> Option<fission_ir::WidgetId> {
    match operation {
        DisplayOp::BackdropFilter { node_id, .. }
        | DisplayOp::DrawRect { node_id, .. }
        | DisplayOp::DrawText { node_id, .. }
        | DisplayOp::DrawRichText { node_id, .. }
        | DisplayOp::DrawImage { node_id, .. }
        | DisplayOp::DrawPath { node_id, .. }
        | DisplayOp::DrawSvg { node_id, .. }
        | DisplayOp::DrawSurface { node_id, .. } => *node_id,
        _ => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CompileProvenance {
    pub root_index: Option<usize>,
    pub node_path: Vec<usize>,
    pub operation_path: Vec<usize>,
    pub node_id: Option<fission_ir::WidgetId>,
    pub layer_property: Option<&'static str>,
}

impl CompileProvenance {
    fn display_list(
        root_index: usize,
        node_path: &[usize],
        operation_path: &[usize],
        node_id: Option<fission_ir::WidgetId>,
    ) -> Self {
        Self {
            root_index: Some(root_index),
            node_path: node_path.to_vec(),
            operation_path: operation_path.to_vec(),
            node_id,
            layer_property: None,
        }
    }

    fn layer(
        root_index: usize,
        node_path: &[usize],
        node_id: Option<fission_ir::WidgetId>,
        layer_property: &'static str,
    ) -> Self {
        Self {
            root_index: Some(root_index),
            node_path: node_path.to_vec(),
            operation_path: Vec::new(),
            node_id,
            layer_property: Some(layer_property),
        }
    }

    pub(crate) fn operation_index(&self) -> Option<usize> {
        self.operation_path.last().copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompileError {
    pub kind: CompileErrorKind,
    pub provenance: CompileProvenance,
}

impl CompileError {
    fn new(kind: CompileErrorKind, provenance: CompileProvenance) -> Self {
        Self { kind, provenance }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompileErrorKind {
    InvalidScaleFactor,
    UnsupportedOperation(DisplayOpKind),
    UnsupportedTransform,
    InvalidPaint(String),
    InvalidPath(String),
    PhysicalGeometryOverflow(&'static str),
    RestoreWithoutSave,
    UnbalancedSaveRestore { remaining_saves: usize },
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            CompileErrorKind::InvalidScaleFactor => {
                formatter.write_str("the frame scale factor is not finite and positive")?
            }
            CompileErrorKind::UnsupportedOperation(operation) => {
                write!(formatter, "the Skia ABI cannot yet execute {operation:?}")?
            }
            CompileErrorKind::UnsupportedTransform => formatter.write_str(
                "the Skia raster profile supports only finite two-dimensional affine transforms",
            )?,
            CompileErrorKind::InvalidPaint(message) => {
                write!(formatter, "the Skia paint is invalid: {message}")?
            }
            CompileErrorKind::InvalidPath(message) => {
                write!(formatter, "the Skia path is invalid: {message}")?
            }
            CompileErrorKind::PhysicalGeometryOverflow(field) => {
                write!(formatter, "{field} overflows after device scaling")?
            }
            CompileErrorKind::RestoreWithoutSave => {
                formatter.write_str("the display list restores without a matching save")?
            }
            CompileErrorKind::UnbalancedSaveRestore { remaining_saves } => write!(
                formatter,
                "the display list leaves {remaining_saves} save operation(s) unrestored"
            )?,
        }
        if let Some(root_index) = self.provenance.root_index {
            write!(
                formatter,
                " at root {root_index}, node path {:?}",
                self.provenance.node_path
            )?;
        }
        if !self.provenance.operation_path.is_empty() {
            write!(
                formatter,
                ", operation path {:?}",
                self.provenance.operation_path
            )?;
        }
        if let Some(property) = self.provenance.layer_property {
            write!(formatter, ", layer {property}")?;
        }
        if let Some(node_id) = self.provenance.node_id {
            write!(formatter, ", node {node_id}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_render::{DisplayList, LayoutRect};

    fn red() -> Color {
        Color {
            r: 255,
            g: 0,
            b: 0,
            a: 128,
        }
    }

    #[test]
    fn paint_state_and_complete_rectangle_style_are_batched() {
        let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 20.0, 10.0));
        list.push(DisplayOp::Save);
        list.push(DisplayOp::ClipRoundedRect {
            rect: LayoutRect::new(1.0, 1.0, 18.0, 8.0),
            radius: 2.0,
        });
        list.push(DisplayOp::Translate(LayoutPoint::new(2.0, 3.0)));
        list.push(DisplayOp::DrawRect {
            rect: LayoutRect::new(2.0, 3.0, 4.0, 5.0),
            fill: Some(Fill::LinearGradient {
                start: (0.0, 0.0),
                end: (1.0, 0.0),
                stops: vec![(1.0, red()), (0.0, Color { a: 255, ..red() })],
            }),
            stroke: Some(Stroke {
                fill: Fill::Solid(red()),
                width: 1.0,
                dash_array: Some(vec![1.0, 2.0, 3.0]),
                line_cap: LineCap::Round,
                line_join: LineJoin::Bevel,
            }),
            corner_radius: 1.5,
            shadow: Some(BoxShadow {
                color: red(),
                blur_radius: 4.0,
                spread_radius: -1.0,
                offset: (2.0, 1.0),
                inset: true,
            }),
            bounds: LayoutRect::new(2.0, 3.0, 4.0, 5.0),
            node_id: None,
        });
        list.push(DisplayOp::Restore);

        let compiled = compile_scene(
            &RenderScene::from_display_list(list),
            2.0,
            Color {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            },
        )
        .unwrap();

        assert_eq!(compiled.source_operations, 5);
        assert_eq!(
            compiled.frame.commands[0],
            RasterCommand::Clear(RasterColor {
                red: 1.0 / 255.0,
                green: 2.0 / 255.0,
                blue: 3.0 / 255.0,
                alpha: 1.0,
            })
        );
        assert!(matches!(compiled.frame.commands[1], RasterCommand::Save));
        assert!(matches!(
            compiled.frame.commands[2],
            RasterCommand::ClipRoundedRect { radius: 4.0, .. }
        ));
        assert!(matches!(
            compiled.frame.commands[3],
            RasterCommand::ConcatAffine(RasterAffine {
                translate_x: 4.0,
                translate_y: 6.0,
                ..
            })
        ));
        assert!(matches!(
            compiled.frame.commands[4],
            RasterCommand::BoxShadow { .. }
        ));
        let RasterCommand::FillRect { rect, paint, .. } = &compiled.frame.commands[5] else {
            panic!("expected a filled rectangle")
        };
        assert_eq!(
            *rect,
            RasterRect {
                left: 4.0,
                top: 6.0,
                right: 12.0,
                bottom: 16.0,
            }
        );
        assert!(matches!(
            paint,
            RasterPaint::LinearGradient {
                start: RasterPoint { x: 4.0, y: 6.0 },
                end: RasterPoint { x: 12.0, y: 6.0 },
                ..
            }
        ));
        assert!(matches!(
            compiled.frame.commands[6],
            RasterCommand::StrokeRect { .. }
        ));
        assert!(matches!(compiled.frame.commands[7], RasterCommand::Restore));
    }

    #[test]
    fn svg_path_data_is_lowered_with_bounds_origin_and_device_scale() {
        let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 20.0, 10.0));
        list.push(DisplayOp::DrawPath {
            path: "M 0 0 L 4 0 Q 5 1 4 2 C 3 3 1 3 0 2 Z".into(),
            fill: Some(Fill::Solid(red())),
            stroke: None,
            bounds: LayoutRect::new(3.0, 5.0, 6.0, 4.0),
            node_id: None,
        });

        let compiled = compile_scene(&RenderScene::from_display_list(list), 2.0, red()).unwrap();

        let RasterCommand::FillPath { path, .. } = &compiled.frame.commands[1] else {
            panic!("expected a filled path")
        };
        assert_eq!(
            path.commands[0],
            RasterPathCommand::MoveTo { x: 6.0, y: 10.0 }
        );
        assert!(matches!(
            path.commands.last(),
            Some(RasterPathCommand::Close)
        ));
    }

    #[test]
    fn malformed_paths_and_out_of_contract_gradients_keep_provenance() {
        let node_id = fission_ir::WidgetId::explicit("bad-paint");
        let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 10.0, 10.0));
        list.push(DisplayOp::DrawPath {
            path: "not path data".into(),
            fill: Some(Fill::Solid(red())),
            stroke: None,
            bounds: LayoutRect::new(0.0, 0.0, 10.0, 10.0),
            node_id: Some(node_id),
        });

        let error = compile_scene(&RenderScene::from_display_list(list), 1.0, red()).unwrap_err();

        assert!(matches!(error.kind, CompileErrorKind::InvalidPath(_)));
        assert_eq!(error.provenance.node_id, Some(node_id));
        assert_eq!(error.provenance.operation_index(), Some(0));
    }

    #[test]
    fn perspective_transform_is_rejected_instead_of_flattened() {
        let mut matrix = [0.0; 16];
        matrix[0] = 1.0;
        matrix[5] = 1.0;
        matrix[10] = 1.0;
        matrix[15] = 1.0;
        matrix[3] = 0.25;
        let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 1.0, 1.0));
        list.push(DisplayOp::Transform(matrix));

        let error = compile_scene(&RenderScene::from_display_list(list), 1.0, red()).unwrap_err();

        assert_eq!(error.kind, CompileErrorKind::UnsupportedTransform);
    }

    #[test]
    fn gradient_edge_cases_are_encoded_with_explicit_geometry() {
        let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 8.0, 6.0));
        list.push(DisplayOp::DrawRect {
            rect: LayoutRect::new(0.0, 0.0, 8.0, 6.0),
            fill: Some(Fill::RadialGradient {
                center: (0.5, 0.5),
                radius: 0.0,
                stops: Vec::new(),
            }),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: LayoutRect::new(0.0, 0.0, 8.0, 6.0),
            node_id: None,
        });

        let compiled = compile_scene(&RenderScene::from_display_list(list), 2.0, red()).unwrap();

        assert!(matches!(
            &compiled.frame.commands[1],
            RasterCommand::FillRect {
                paint: RasterPaint::RadialGradient {
                    center: RasterPoint { x: 8.0, y: 6.0 },
                    radius: 0.0,
                    stops,
                },
                ..
            } if stops.is_empty()
        ));
    }

    #[test]
    fn near_opaque_layers_are_rejected_until_opacity_is_implemented() {
        let mut layer = fission_render::RenderLayer::new(LayoutRect::new(0.0, 0.0, 4.0, 4.0));
        layer.style.opacity = 0.9995;
        let mut scene = RenderScene::new(LayoutRect::new(0.0, 0.0, 4.0, 4.0));
        scene.roots.push(RenderNode::Layer(layer));

        let error = compile_scene(&scene, 1.0, red()).unwrap_err();

        assert_eq!(
            error.kind,
            CompileErrorKind::UnsupportedOperation(DisplayOpKind::OpacityLayer)
        );
        assert_eq!(error.provenance.layer_property, Some("opacity"));
    }
}
