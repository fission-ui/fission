use std::fmt;

use fission_render::{Color, DisplayList, DisplayOp, Fill, RenderNode, RenderScene};

use crate::api::{RasterColor, RasterCommand, RasterFrame, RasterRect};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompiledRasterFrame {
    pub frame: RasterFrame,
    pub source_operations: u64,
}

pub(crate) fn compile_scene(
    scene: &RenderScene,
    scale_factor: f64,
) -> Result<CompiledRasterFrame, CompileError> {
    let mut compiler = Compiler {
        scale_factor: scale_factor as f32,
        commands: vec![RasterCommand::Clear(RasterColor::TRANSPARENT)],
        source_operations: 0,
        save_depth: 0,
    };
    for root in &scene.roots {
        compiler.compile_node(root)?;
    }
    if compiler.save_depth != 0 {
        return Err(CompileError::UnbalancedSaveRestore {
            remaining_saves: compiler.save_depth,
        });
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
    fn compile_node(&mut self, node: &RenderNode) -> Result<(), CompileError> {
        match node {
            RenderNode::Paint(list) => self.compile_list(list),
            RenderNode::Layer(layer) => {
                if layer.style.clip.is_some()
                    || layer.style.transform.is_some()
                    || (layer.style.opacity - 1.0).abs() > 0.001
                {
                    return Err(CompileError::UnsupportedLayerSemantics);
                }
                for child in &layer.children {
                    self.compile_node(child)?;
                }
                Ok(())
            }
        }
    }

    fn compile_list(&mut self, list: &DisplayList) -> Result<(), CompileError> {
        for operation in &list.ops {
            self.source_operations = self.source_operations.saturating_add(1);
            match operation {
                DisplayOp::Save => self.save_depth = self.save_depth.saturating_add(1),
                DisplayOp::Restore => {
                    self.save_depth = self
                        .save_depth
                        .checked_sub(1)
                        .ok_or(CompileError::RestoreWithoutSave)?;
                }
                DisplayOp::CachedScene { list, .. } => self.compile_list(list)?,
                DisplayOp::DrawRect {
                    rect,
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                    ..
                } => {
                    if stroke.is_some() || *corner_radius != 0.0 || shadow.is_some() {
                        return Err(CompileError::UnsupportedDrawRectStyle);
                    }
                    let Some(fill) = fill else {
                        continue;
                    };
                    let Fill::Solid(color) = fill else {
                        return Err(CompileError::UnsupportedDrawRectStyle);
                    };
                    self.commands.push(RasterCommand::FillRect {
                        rect: RasterRect {
                            left: rect.x() * self.scale_factor,
                            top: rect.y() * self.scale_factor,
                            right: rect.right() * self.scale_factor,
                            bottom: rect.bottom() * self.scale_factor,
                        },
                        color: srgb_color(*color),
                    });
                }
                other => return Err(CompileError::UnsupportedOperation(other.kind())),
            }
        }
        Ok(())
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CompileError {
    UnsupportedLayerSemantics,
    UnsupportedDrawRectStyle,
    UnsupportedOperation(fission_render::capabilities::DisplayOpKind),
    RestoreWithoutSave,
    UnbalancedSaveRestore { remaining_saves: usize },
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLayerSemantics => {
                formatter.write_str("the frame contains layer semantics absent from the Skia ABI")
            }
            Self::UnsupportedDrawRectStyle => formatter.write_str(
                "the frame contains a rectangle style not yet represented by the Skia ABI",
            ),
            Self::UnsupportedOperation(operation) => {
                write!(formatter, "the Skia ABI cannot yet execute {operation:?}")
            }
            Self::RestoreWithoutSave => {
                formatter.write_str("the display list restores without a matching save")
            }
            Self::UnbalancedSaveRestore { remaining_saves } => write!(
                formatter,
                "the display list leaves {remaining_saves} save operation(s) unrestored"
            ),
        }
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_render::{DisplayList, LayoutRect};

    #[test]
    fn simple_solid_rect_is_batched_in_physical_coordinates() {
        let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 20.0, 10.0));
        list.push(DisplayOp::DrawRect {
            rect: LayoutRect::new(2.0, 3.0, 4.0, 5.0),
            fill: Some(Fill::Solid(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 128,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: LayoutRect::new(2.0, 3.0, 4.0, 5.0),
            node_id: None,
        });

        let compiled = compile_scene(&RenderScene::from_display_list(list), 2.0).unwrap();

        assert_eq!(compiled.source_operations, 1);
        assert_eq!(compiled.frame.commands.len(), 2);
        assert_eq!(
            compiled.frame.commands[1],
            RasterCommand::FillRect {
                rect: RasterRect {
                    left: 4.0,
                    top: 6.0,
                    right: 12.0,
                    bottom: 16.0,
                },
                color: RasterColor {
                    red: 1.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 128.0 / 255.0,
                },
            }
        );
    }

    #[test]
    fn cached_scene_is_lowered_without_becoming_a_second_scene_authority() {
        let nested = DisplayList::new(LayoutRect::new(0.0, 0.0, 1.0, 1.0));
        let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 1.0, 1.0));
        list.push(DisplayOp::CachedScene {
            cache_key: 9,
            bounds: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
            list: Box::new(nested),
        });

        let compiled = compile_scene(&RenderScene::from_display_list(list), 1.0).unwrap();

        assert_eq!(compiled.source_operations, 1);
        assert_eq!(
            compiled.frame.commands,
            vec![RasterCommand::Clear(RasterColor::TRANSPARENT)]
        );
    }
}
