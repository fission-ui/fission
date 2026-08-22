use fission_ir::op::{BackdropFilter, ImageAlignment, ImageRequest};
use fission_render::capabilities::{is_2d_affine_transform, DisplayOpKind};
use fission_render::diagnostics::DiagnosticCategory;
use fission_render::paragraph::ParagraphFrameBindings;
use fission_render::resource::ResourceSnapshot;
use fission_render::{
    BoxShadow, Color, DisplayList, DisplayOp, Fill, ImageFit, LayerClip, LayoutPoint, LayoutRect,
    LineCap, LineJoin, RenderNode, RenderScene, Stroke,
};
use kurbo::{BezPath, PathEl};
use std::fmt;

use crate::api::{
    RasterAffine, RasterBoxShadow, RasterColor, RasterCommand, RasterFillRule, RasterFrame,
    RasterGradientStop, RasterLineCap, RasterLineJoin, RasterPaint, RasterPath, RasterPathCommand,
    RasterPoint, RasterRect, RasterStroke, SkiaPictureRecorder,
};
use crate::compiler_paragraph::{CompiledParagraphDrawData, ParagraphCompilation};
use crate::image::{place_image, resolve_image_resource, ImageError, SkiaImageCache};
use crate::paragraph_caret::{paragraph_caret_paint, ParagraphCaretPaint, ParagraphCaretStyle};
use crate::paragraph_draw_data::ParagraphDrawDataError;
use crate::paragraph_engine::CanvasKitParagraphDrawDataRegistry;
use crate::picture::SkiaPictureCache;
use crate::profile::SkiaParagraphDrawDataRegistry;
use crate::svg::{
    parse_svg_geometry, place_svg_geometry, resolve_svg_resource, validate_svg_bounds,
    SkiaSvgCache, SvgError,
};

#[path = "compiler_picture.rs"]
mod picture_cache;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompiledRasterFrame {
    pub frame: RasterFrame,
    pub source_operations: u64,
    pub reused_layers: u64,
}

#[cfg(test)]
pub(crate) fn compile_scene(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
) -> Result<CompiledRasterFrame, CompileError> {
    let svg_cache = SkiaSvgCache::new();
    compile_scene_inner(
        scene,
        scale_factor,
        clear_color,
        None,
        None,
        SvgCompilation::Native(&svg_cache),
        None,
    )
}

pub(crate) fn compile_scene_with_paragraphs(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
    resources: &ResourceSnapshot,
    image_cache: &SkiaImageCache,
    svg_cache: &SkiaSvgCache,
    paragraph_bindings: Option<&ParagraphFrameBindings>,
    paragraph_draw_data: &SkiaParagraphDrawDataRegistry,
    picture_cache: &SkiaPictureCache,
    picture_recorder: &dyn SkiaPictureRecorder,
) -> Result<CompiledRasterFrame, CompileError> {
    let paragraphs = ParagraphCompilation::native(paragraph_bindings, paragraph_draw_data)
        .map_err(paragraph_registry_error)?;
    compile_scene_inner(
        scene,
        scale_factor,
        clear_color,
        paragraphs,
        Some(ImageCompilation::Native {
            resources,
            cache: image_cache,
        }),
        SvgCompilation::Native(svg_cache),
        Some(PictureCompilation {
            cache: picture_cache,
            recorder: picture_recorder,
        }),
    )
}

/// Compiles the Web resource path without constructing native Skia objects.
///
/// Images retain their authoritative logical resource identity for later
/// resolution against the transactional CanvasKit resource plan. SVGs use the
/// backend-neutral geometry path, and picture caching is deliberately disabled
/// so cached subtrees expand into ordinary commands on Web.
pub(crate) fn compile_scene_for_web(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
    resources: &ResourceSnapshot,
    paragraph_bindings: Option<&ParagraphFrameBindings>,
    paragraph_draw_data: Option<&CanvasKitParagraphDrawDataRegistry>,
) -> Result<CompiledRasterFrame, CompileError> {
    let paragraphs = ParagraphCompilation::canvaskit(paragraph_bindings, paragraph_draw_data)
        .map_err(paragraph_registry_error)?;
    compile_scene_inner(
        scene,
        scale_factor,
        clear_color,
        paragraphs,
        Some(ImageCompilation::Web { resources }),
        SvgCompilation::Web { resources },
        None,
    )
}

fn compile_scene_inner<'a>(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
    paragraphs: Option<ParagraphCompilation>,
    images: Option<ImageCompilation<'a>>,
    svg: SvgCompilation<'a>,
    pictures: Option<PictureCompilation<'a>>,
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
        reused_layers: 0,
        save_scopes: Vec::new(),
        root_opacity_layers: 0,
        paragraphs: paragraphs.as_ref(),
        images,
        svg,
        pictures,
    };
    for (root_index, root) in scene.roots.iter().enumerate() {
        compiler.compile_node(root, root_index, &mut Vec::new(), None)?;
    }
    let remaining_saves = compiler.remaining_native_saves();
    if remaining_saves != 0 {
        return Err(CompileError::new(
            CompileErrorKind::UnbalancedSaveRestore { remaining_saves },
            CompileProvenance::default(),
        ));
    }
    Ok(CompiledRasterFrame {
        frame: RasterFrame {
            commands: compiler.commands,
        },
        source_operations: compiler.source_operations,
        reused_layers: compiler.reused_layers,
    })
}

#[derive(Clone, Copy)]
enum ImageCompilation<'a> {
    Native {
        resources: &'a ResourceSnapshot,
        cache: &'a SkiaImageCache,
    },
    Web {
        resources: &'a ResourceSnapshot,
    },
}

impl<'a> ImageCompilation<'a> {
    fn resources(self) -> &'a ResourceSnapshot {
        match self {
            Self::Native { resources, .. } | Self::Web { resources } => resources,
        }
    }
}

#[derive(Clone, Copy)]
enum SvgCompilation<'a> {
    Native(&'a SkiaSvgCache),
    Web { resources: &'a ResourceSnapshot },
}

#[derive(Clone, Copy)]
struct PictureCompilation<'a> {
    cache: &'a SkiaPictureCache,
    recorder: &'a dyn SkiaPictureRecorder,
}

struct Compiler<'a> {
    scale_factor: f32,
    commands: Vec<RasterCommand>,
    source_operations: u64,
    reused_layers: u64,
    /// Each logical `Save` owns every isolated opacity group opened before its
    /// matching `Restore`, mirroring Fission's display-list semantics.
    save_scopes: Vec<usize>,
    /// Explicit opacity groups are valid at the display-list root and are
    /// closed together by the next `Restore`, as in the existing renderers.
    root_opacity_layers: usize,
    paragraphs: Option<&'a ParagraphCompilation>,
    images: Option<ImageCompilation<'a>>,
    svg: SvgCompilation<'a>,
    pictures: Option<PictureCompilation<'a>>,
}

impl Compiler<'_> {
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
                if self.compile_cached_layer(layer, root_index, node_path, inherited_node_id)? {
                    Ok(())
                } else {
                    self.compile_layer_uncached(layer, root_index, node_path, inherited_node_id)
                }
            }
        }
    }

    fn compile_layer_uncached(
        &mut self,
        layer: &fission_render::RenderLayer,
        root_index: usize,
        node_path: &mut Vec<usize>,
        inherited_node_id: Option<fission_ir::WidgetId>,
    ) -> Result<(), CompileError> {
        let node_id = layer.node_id.or(inherited_node_id);
        let has_opacity = layer.style.opacity.to_bits() != 1.0_f32.to_bits();
        let needs_save =
            layer.style.clip.is_some() || layer.style.transform.is_some() || has_opacity;
        if needs_save {
            self.push_save();
        }
        if let Some(clip) = layer.style.clip.as_ref() {
            let provenance = CompileProvenance::layer(root_index, node_path, node_id, "clip");
            self.compile_layer_clip(clip, provenance)?;
        }
        if has_opacity {
            let provenance = CompileProvenance::layer(root_index, node_path, node_id, "opacity");
            self.push_opacity_layer(layer.bounds, layer.style.opacity, &provenance)?;
        }
        if let Some(matrix) = layer.style.transform.as_ref() {
            let provenance = CompileProvenance::layer(root_index, node_path, node_id, "transform");
            let affine = self.affine(*matrix, &provenance)?;
            self.commands.push(RasterCommand::ConcatAffine(affine));
        }
        if !self.compile_cached_layer_contents(layer, root_index, node_path, node_id)? {
            self.compile_layer_children(layer, root_index, node_path, node_id)?;
        }
        if needs_save {
            self.push_restore(CompileProvenance::layer(
                root_index, node_path, node_id, "restore",
            ))?;
        }
        Ok(())
    }

    fn compile_layer_children(
        &mut self,
        layer: &fission_render::RenderLayer,
        root_index: usize,
        node_path: &mut Vec<usize>,
        node_id: Option<fission_ir::WidgetId>,
    ) -> Result<(), CompileError> {
        for (child_index, child) in layer.children.iter().enumerate() {
            node_path.push(child_index);
            self.compile_node(child, root_index, node_path, node_id)?;
            node_path.pop();
        }
        Ok(())
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
            DisplayOp::OpacityLayer { alpha, bounds } => {
                self.push_opacity_layer(*bounds, *alpha, &provenance)?
            }
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
            DisplayOp::CachedScene {
                cache_key, list, ..
            } => {
                if !self.compile_cached_display_list(
                    *cache_key,
                    list,
                    root_index,
                    node_path,
                    operation_path,
                    inherited_node_id,
                )? {
                    self.compile_list(
                        list,
                        root_index,
                        node_path,
                        operation_path,
                        inherited_node_id,
                    )?;
                }
            }
            DisplayOp::BackdropFilter {
                rect,
                filter,
                corner_radius,
                ..
            } => self.draw_backdrop_filter(*rect, *filter, *corner_radius, &provenance)?,
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
            DisplayOp::DrawText {
                position,
                color,
                caret_index,
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
                ..
            } => self.draw_paragraph(
                DisplayOpKind::DrawText,
                provenance.node_id,
                *position,
                *caret_index,
                ParagraphCaretStyle {
                    color: caret_color.unwrap_or(*color),
                    width: *caret_width,
                    height: *caret_height,
                    radius: *caret_radius,
                },
                &provenance,
            )?,
            DisplayOp::DrawRichText {
                runs,
                position,
                caret_index,
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
                ..
            } => self.draw_paragraph(
                DisplayOpKind::DrawRichText,
                provenance.node_id,
                *position,
                *caret_index,
                ParagraphCaretStyle {
                    color: caret_color.unwrap_or_else(|| {
                        runs.first().map(|run| run.style.color).unwrap_or(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        })
                    }),
                    width: *caret_width,
                    height: *caret_height,
                    radius: *caret_radius,
                },
                &provenance,
            )?,
            DisplayOp::DrawImage {
                rect,
                request,
                fit,
                alignment,
                ..
            } => self.draw_image(
                *rect,
                request,
                *fit,
                *alignment,
                provenance.node_id,
                &provenance,
            )?,
            DisplayOp::DrawSvg {
                content,
                fill,
                stroke,
                bounds,
                ..
            } => self.draw_svg(
                content,
                fill.as_ref(),
                stroke.as_ref(),
                *bounds,
                &provenance,
            )?,
            other => {
                return Err(CompileError::new(
                    CompileErrorKind::UnsupportedOperation(other.kind()),
                    provenance,
                ));
            }
        }
        Ok(())
    }

    fn draw_svg(
        &mut self,
        content: &str,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        bounds: LayoutRect,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        validate_svg_bounds(bounds)
            .map_err(|error| CompileError::new(CompileErrorKind::Svg(error), provenance.clone()))?;
        if bounds.width() == 0.0 || bounds.height() == 0.0 {
            return Ok(());
        }

        let content = match self.svg {
            SvgCompilation::Native(cache) if fill.is_none() && stroke.is_none() => {
                let document = cache.get_or_parse(content).map_err(|error| {
                    CompileError::new(CompileErrorKind::Svg(error), provenance.clone())
                })?;
                self.commands.push(RasterCommand::DrawSvg {
                    document,
                    destination: self.rect(bounds, provenance)?,
                });
                return Ok(());
            }
            SvgCompilation::Native(_) => content,
            SvgCompilation::Web { resources } => {
                if fill.is_none() && stroke.is_none() {
                    return Err(CompileError::new(
                        CompileErrorKind::Svg(SvgError::NeutralDocumentPaintUnsupported),
                        provenance.clone(),
                    ));
                }
                let node_id = provenance.node_id.ok_or_else(|| {
                    CompileError::new(CompileErrorKind::MissingSvgNodeId, provenance.clone())
                })?;
                resolve_svg_resource(resources, content, node_id)
                    .map_err(|error| {
                        CompileError::new(CompileErrorKind::Svg(error), provenance.clone())
                    })?
                    .content
            }
        };

        let geometry = parse_svg_geometry(content)
            .map_err(|error| CompileError::new(CompileErrorKind::Svg(error), provenance.clone()))?;
        let placement = place_svg_geometry(&geometry, bounds)
            .map_err(|error| CompileError::new(CompileErrorKind::Svg(error), provenance.clone()))?;
        let paths = geometry
            .paths
            .iter()
            .map(|path| self.path(path, LayoutPoint::new(0.0, 0.0), provenance))
            .collect::<Result<Vec<_>, _>>()?;
        // The current neutral SVG profile deliberately requires Fission paint.
        // Document-owned paint remains on native SkSVGDOM until the neutral
        // parser can represent it without silently changing colors or strokes.
        let fill = fill
            .map(|fill| self.paint(fill, placement.source_bounds, provenance))
            .transpose()?;
        let stroke = stroke
            .map(|stroke| self.stroke(stroke, placement.source_bounds, provenance))
            .transpose()?;
        let transform = RasterAffine {
            scale_x: placement.scale,
            skew_x: 0.0,
            translate_x: self.scaled(placement.translation.x, provenance, "SVG x translation")?,
            skew_y: 0.0,
            scale_y: placement.scale,
            translate_y: self.scaled(placement.translation.y, provenance, "SVG y translation")?,
        };

        self.commands.push(RasterCommand::Save);
        self.commands.push(RasterCommand::ConcatAffine(transform));
        for path in paths {
            if let Some(paint) = fill.as_ref() {
                self.commands.push(RasterCommand::FillPath {
                    path: path.clone(),
                    paint: paint.clone(),
                });
            }
            if let Some(stroke) = stroke.as_ref() {
                self.commands.push(RasterCommand::StrokePath {
                    path,
                    stroke: stroke.clone(),
                });
            }
        }
        self.commands.push(RasterCommand::Restore);
        Ok(())
    }

    fn draw_backdrop_filter(
        &mut self,
        rect: LayoutRect,
        filter: BackdropFilter,
        corner_radius: f32,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        let sigma = match filter {
            BackdropFilter::Blur(sigma) => sigma,
        };
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidBackdropBlurSigma,
                provenance.clone(),
            ));
        }
        if !corner_radius.is_finite() || corner_radius < 0.0 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidBackdropCornerRadius,
                provenance.clone(),
            ));
        }

        // Validate every encoded field before recognizing the semantic no-op.
        // This prevents zero blur from hiding malformed retained geometry.
        let bounds = self.backdrop_bounds(rect, provenance)?;
        let corner_radius =
            self.scaled(corner_radius, provenance, "backdrop filter corner radius")?;
        let sigma = self.scaled(sigma, provenance, "backdrop blur sigma")?;
        if sigma == 0.0 {
            return Ok(());
        }

        self.commands.push(RasterCommand::BackdropBlur {
            bounds,
            corner_radius,
            sigma,
        });
        Ok(())
    }

    fn backdrop_bounds(
        &self,
        rect: LayoutRect,
        provenance: &CompileProvenance,
    ) -> Result<RasterRect, CompileError> {
        if ![rect.x(), rect.y(), rect.width(), rect.height()]
            .iter()
            .all(|value| value.is_finite())
            || rect.width() < 0.0
            || rect.height() < 0.0
        {
            return Err(CompileError::new(
                CompileErrorKind::InvalidBackdropBounds,
                provenance.clone(),
            ));
        }
        self.rect(rect, provenance)
    }

    fn draw_image(
        &mut self,
        rect: LayoutRect,
        request: &ImageRequest,
        fit: ImageFit,
        alignment: ImageAlignment,
        node_id: Option<fission_ir::WidgetId>,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return Ok(());
        }
        let node_id = node_id.ok_or_else(|| {
            CompileError::new(CompileErrorKind::MissingImageNodeId, provenance.clone())
        })?;
        let images = self.images.as_ref().ok_or_else(|| {
            CompileError::new(CompileErrorKind::MissingImageResources, provenance.clone())
        })?;
        let resource =
            resolve_image_resource(images.resources(), request, node_id).map_err(|error| {
                CompileError::new(CompileErrorKind::Image(error), provenance.clone())
            })?;
        match images {
            ImageCompilation::Native { cache, .. } => {
                let image = cache
                    .get_or_decode(&resource.cache_key, resource.encoded)
                    .map_err(|error| {
                        CompileError::new(CompileErrorKind::Image(error), provenance.clone())
                    })?;
                let Some(placement) =
                    place_image(rect, image.width(), image.height(), fit, alignment)
                else {
                    return Ok(());
                };

                self.commands.push(RasterCommand::Save);
                self.commands.push(RasterCommand::ClipRect {
                    rect: self.rect(placement.clip, provenance)?,
                });
                self.commands.push(RasterCommand::DrawImage {
                    source: RasterRect {
                        left: 0.0,
                        top: 0.0,
                        right: image.width() as f32,
                        bottom: image.height() as f32,
                    },
                    destination: self.rect(placement.destination, provenance)?,
                    image,
                });
                self.commands.push(RasterCommand::Restore);
            }
            ImageCompilation::Web { .. } => {
                self.commands.push(RasterCommand::DrawImageResource {
                    resource_id: resource.entry.id(),
                    target: self.rect(rect, provenance)?,
                    fit,
                    alignment,
                });
            }
        }
        Ok(())
    }

    fn draw_paragraph(
        &mut self,
        operation: DisplayOpKind,
        node_id: Option<fission_ir::WidgetId>,
        position: LayoutPoint,
        caret_index: Option<usize>,
        caret_style: ParagraphCaretStyle,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        let node_id = node_id.ok_or_else(|| {
            CompileError::new(
                CompileErrorKind::MissingParagraphNodeId(operation),
                provenance.clone(),
            )
        })?;
        let paragraphs = self.paragraphs.as_ref().ok_or_else(|| {
            CompileError::new(
                CompileErrorKind::MissingParagraphBindings,
                provenance.clone(),
            )
        })?;
        let bound = paragraphs.get(node_id).ok_or_else(|| {
            CompileError::new(
                CompileErrorKind::MissingParagraphBinding { node_id },
                provenance.clone(),
            )
        })?;
        let result = bound.result;
        let origin = RasterPoint {
            x: self.scaled(position.x, provenance, "paragraph origin x")?,
            y: self.scaled(position.y, provenance, "paragraph origin y")?,
        };
        self.commands.push(match bound.draw_data {
            CompiledParagraphDrawData::Native(data) => RasterCommand::DrawParagraph {
                data,
                origin,
                scale_factor: self.scale_factor,
            },
            CompiledParagraphDrawData::CanvasKit(paragraph) => {
                RasterCommand::DrawParagraphResource {
                    paragraph,
                    origin,
                    scale_factor: self.scale_factor,
                }
            }
        });

        let caret = paragraph_caret_paint(result.as_ref(), caret_index, position, caret_style)
            .map_err(|error| {
                CompileError::new(
                    CompileErrorKind::InvalidParagraphCaret(error.to_string()),
                    provenance.clone(),
                )
            })?;
        if let Some(caret) = caret {
            self.draw_paragraph_caret(caret, provenance)?;
        }
        Ok(())
    }

    fn draw_paragraph_caret(
        &mut self,
        caret: ParagraphCaretPaint,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        self.commands.push(RasterCommand::FillRect {
            rect: self.rect(caret.rect, provenance)?,
            radius: self.scaled(caret.radius, provenance, "caret radius")?,
            paint: RasterPaint::Solid(srgb_color(caret.color)),
        });
        Ok(())
    }

    fn push_save(&mut self) {
        self.save_scopes.push(0);
        self.commands.push(RasterCommand::Save);
    }

    fn push_restore(&mut self, provenance: CompileProvenance) -> Result<(), CompileError> {
        if let Some(opacity_layers) = self.save_scopes.pop() {
            self.commands.extend(std::iter::repeat_n(
                RasterCommand::Restore,
                opacity_layers.saturating_add(1),
            ));
            return Ok(());
        }
        if self.root_opacity_layers != 0 {
            self.commands.extend(std::iter::repeat_n(
                RasterCommand::Restore,
                self.root_opacity_layers,
            ));
            self.root_opacity_layers = 0;
            return Ok(());
        }
        Err(CompileError::new(
            CompileErrorKind::RestoreWithoutSave,
            provenance,
        ))
    }

    fn push_opacity_layer(
        &mut self,
        bounds: LayoutRect,
        alpha: f32,
        provenance: &CompileProvenance,
    ) -> Result<(), CompileError> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidOpacity,
                provenance.clone(),
            ));
        }
        let bounds = self.rect(bounds, provenance)?;
        self.commands
            .push(RasterCommand::OpacityLayer { bounds, alpha });
        if let Some(opacity_layers) = self.save_scopes.last_mut() {
            *opacity_layers = opacity_layers.saturating_add(1);
        } else {
            self.root_opacity_layers = self.root_opacity_layers.saturating_add(1);
        }
        Ok(())
    }

    fn remaining_native_saves(&self) -> usize {
        self.root_opacity_layers.saturating_add(
            self.save_scopes
                .iter()
                .fold(self.save_scopes.len(), |total, opacity_layers| {
                    total.saturating_add(*opacity_layers)
                }),
        )
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
    MissingParagraphNodeId(DisplayOpKind),
    MissingParagraphBindings,
    MissingParagraphBinding { node_id: fission_ir::WidgetId },
    InvalidParagraphDrawData(String),
    InvalidParagraphCaret(String),
    MissingImageNodeId,
    MissingImageResources,
    Image(ImageError),
    MissingSvgNodeId,
    Svg(SvgError),
    InvalidOpacity,
    InvalidBackdropBounds,
    InvalidBackdropBlurSigma,
    InvalidBackdropCornerRadius,
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
            CompileErrorKind::MissingParagraphNodeId(operation) => write!(
                formatter,
                "the Skia {operation:?} operation has no stable paragraph node identity"
            )?,
            CompileErrorKind::MissingParagraphBindings => formatter
                .write_str("the Skia text frame has no authoritative paragraph-result bindings")?,
            CompileErrorKind::MissingParagraphBinding { node_id } => write!(
                formatter,
                "the Skia text frame has no paragraph result for node {node_id}"
            )?,
            CompileErrorKind::InvalidParagraphDrawData(message) => write!(
                formatter,
                "the Skia paragraph paint resource is invalid: {message}"
            )?,
            CompileErrorKind::InvalidParagraphCaret(message) => {
                write!(formatter, "the Skia paragraph caret is invalid: {message}")?
            }
            CompileErrorKind::MissingImageNodeId => formatter
                .write_str("the Skia image operation has no stable requesting-node identity")?,
            CompileErrorKind::MissingImageResources => formatter
                .write_str("the Skia image frame has no authoritative resource snapshot")?,
            CompileErrorKind::Image(error) => error.fmt(formatter)?,
            CompileErrorKind::MissingSvgNodeId => formatter
                .write_str("the Skia SVG operation has no stable requesting-node identity")?,
            CompileErrorKind::Svg(error) => error.fmt(formatter)?,
            CompileErrorKind::InvalidOpacity => {
                formatter.write_str("the Skia opacity must be finite and in 0..=1")?
            }
            CompileErrorKind::InvalidBackdropBounds => formatter.write_str(
                "the Skia backdrop bounds must be finite with non-negative dimensions",
            )?,
            CompileErrorKind::InvalidBackdropBlurSigma => formatter
                .write_str("the Skia backdrop blur sigma must be finite and non-negative")?,
            CompileErrorKind::InvalidBackdropCornerRadius => formatter.write_str(
                "the Skia backdrop filter corner radius must be finite and non-negative",
            )?,
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
                "the display list leaves {remaining_saves} save/layer operation(s) unrestored"
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

impl CompileError {
    pub(crate) fn diagnostic_code(&self) -> &'static str {
        match &self.kind {
            CompileErrorKind::MissingImageNodeId => "skia-image-node-id-missing",
            CompileErrorKind::MissingImageResources => "skia-image-resources-missing",
            CompileErrorKind::Image(error) => error.diagnostic_code(),
            CompileErrorKind::MissingSvgNodeId => "skia-svg-node-id-missing",
            CompileErrorKind::Svg(error) => error.diagnostic_code(),
            CompileErrorKind::InvalidBackdropBounds => "skia-backdrop-bounds-invalid",
            CompileErrorKind::InvalidBackdropBlurSigma => "skia-backdrop-blur-sigma-invalid",
            CompileErrorKind::InvalidBackdropCornerRadius => "skia-backdrop-corner-radius-invalid",
            _ => "skia-frame-lowering-unsupported",
        }
    }

    pub(crate) fn diagnostic_category(&self) -> DiagnosticCategory {
        match &self.kind {
            CompileErrorKind::MissingImageNodeId
            | CompileErrorKind::MissingImageResources
            | CompileErrorKind::MissingSvgNodeId => DiagnosticCategory::Resource,
            CompileErrorKind::Image(error) => error.diagnostic_category(),
            CompileErrorKind::Svg(error) => error.diagnostic_category(),
            _ => DiagnosticCategory::Capability,
        }
    }
}

fn paragraph_registry_error(error: ParagraphDrawDataError) -> CompileError {
    let node_id = match &error {
        ParagraphDrawDataError::MissingNodeDrawData { node_id }
        | ParagraphDrawDataError::UnknownNodeIdentifier { node_id, .. }
        | ParagraphDrawDataError::NodeCacheKeyMismatch { node_id, .. }
        | ParagraphDrawDataError::DuplicateNode { node_id } => Some(*node_id),
        _ => None,
    };
    CompileError::new(
        CompileErrorKind::InvalidParagraphDrawData(error.to_string()),
        CompileProvenance {
            node_id,
            ..CompileProvenance::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_ir::op::ImageSource;
    use fission_render::frame::ResourceEpoch;
    use fission_render::resource::{
        resolved_resource_content_identity, ResourceEntry, ResourceId, ResourceKind,
        ResourcePayload, ResourceProvenance, ResourceSource,
    };
    use fission_render::{DisplayList, LayoutRect};

    struct NoPictureRecorder;

    impl crate::api::SkiaPictureRecorder for NoPictureRecorder {
        fn record_picture(
            &self,
            _bounds: RasterRect,
            _frame: &RasterFrame,
        ) -> Result<Option<fission_skia_sys::RecordedPicture>, crate::api::ApiError> {
            Ok(None)
        }
    }

    fn red() -> Color {
        Color {
            r: 255,
            g: 0,
            b: 0,
            a: 128,
        }
    }

    fn memory_image_request(bytes: &[u8]) -> ImageRequest {
        ImageRequest {
            source: ImageSource::Memory {
                bytes: bytes.to_vec(),
                mime_type: Some("image/x-fission-test".into()),
            },
            ..ImageRequest::default()
        }
    }

    fn image_scene(request: ImageRequest, node_id: fission_ir::WidgetId) -> RenderScene {
        let rect = LayoutRect::new(10.0, 20.0, 100.0, 100.0);
        let mut list = DisplayList::new(rect);
        list.push(DisplayOp::DrawImage {
            rect,
            request,
            fit: ImageFit::Cover,
            alignment: ImageAlignment::TopEnd,
            bounds: rect,
            node_id: Some(node_id),
        });
        RenderScene::from_display_list(list)
    }

    fn backdrop_scene(
        rect: LayoutRect,
        sigma: f32,
        corner_radius: f32,
        node_id: fission_ir::WidgetId,
    ) -> RenderScene {
        let mut list = DisplayList::new(rect);
        list.push(DisplayOp::BackdropFilter {
            rect,
            filter: BackdropFilter::Blur(sigma),
            corner_radius,
            bounds: rect,
            node_id: Some(node_id),
        });
        RenderScene::from_display_list(list)
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
    fn render_layer_opacity_is_isolated_after_clip_and_before_transform() {
        let bounds = LayoutRect::new(1.0, 2.0, 4.0, 5.0);
        let mut layer = fission_render::RenderLayer::new(bounds);
        layer.style.clip = Some(LayerClip::Rect(LayoutRect::new(0.0, 1.0, 8.0, 7.0)));
        layer.style.opacity = 0.9995;
        let mut transform = [0.0; 16];
        transform[0] = 1.0;
        transform[5] = 1.0;
        transform[10] = 1.0;
        transform[12] = 3.0;
        transform[13] = 4.0;
        transform[15] = 1.0;
        layer.style.transform = Some(transform);
        let mut child = DisplayList::new(bounds);
        child.push(DisplayOp::DrawRect {
            rect: bounds,
            fill: Some(Fill::Solid(red())),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds,
            node_id: None,
        });
        layer.children.push(RenderNode::Paint(child));
        let mut scene = RenderScene::new(bounds);
        scene.roots.push(RenderNode::Layer(layer));

        let compiled = compile_scene(&scene, 2.0, red()).unwrap();

        assert!(matches!(compiled.frame.commands[1], RasterCommand::Save));
        assert!(matches!(
            compiled.frame.commands[2],
            RasterCommand::ClipRect { .. }
        ));
        assert!(matches!(
            compiled.frame.commands[3],
            RasterCommand::OpacityLayer {
                bounds: RasterRect {
                    left: 2.0,
                    top: 4.0,
                    right: 10.0,
                    bottom: 14.0,
                },
                alpha: 0.9995,
            }
        ));
        assert!(matches!(
            compiled.frame.commands[4],
            RasterCommand::ConcatAffine(RasterAffine {
                translate_x: 6.0,
                translate_y: 8.0,
                ..
            })
        ));
        assert!(matches!(
            compiled.frame.commands[5],
            RasterCommand::FillRect { .. }
        ));
        assert!(matches!(compiled.frame.commands[6], RasterCommand::Restore));
        assert!(matches!(compiled.frame.commands[7], RasterCommand::Restore));
    }

    #[test]
    fn display_list_opacity_closes_before_its_saved_clip_scope() {
        let bounds = LayoutRect::new(2.0, 3.0, 4.0, 5.0);
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::Save);
        list.push(DisplayOp::ClipRect(LayoutRect::new(1.0, 1.0, 8.0, 8.0)));
        list.push(DisplayOp::OpacityLayer { alpha: 0.5, bounds });
        list.push(DisplayOp::Restore);

        let compiled = compile_scene(&RenderScene::from_display_list(list), 2.0, red()).unwrap();

        assert!(matches!(compiled.frame.commands[1], RasterCommand::Save));
        assert!(matches!(
            compiled.frame.commands[2],
            RasterCommand::ClipRect { .. }
        ));
        assert!(matches!(
            compiled.frame.commands[3],
            RasterCommand::OpacityLayer { alpha: 0.5, .. }
        ));
        assert!(matches!(compiled.frame.commands[4], RasterCommand::Restore));
        assert!(matches!(compiled.frame.commands[5], RasterCommand::Restore));
    }

    #[test]
    fn backdrop_blur_is_one_atomic_device_scaled_command() {
        let node_id = fission_ir::WidgetId::explicit("backdrop.scaled");
        let scene = backdrop_scene(LayoutRect::new(1.0, 2.0, 10.0, 16.0), 4.0, 3.0, node_id);

        let compiled = compile_scene(&scene, 2.0, red()).unwrap();

        assert_eq!(compiled.source_operations, 1);
        assert_eq!(compiled.frame.commands.len(), 2);
        assert_eq!(
            compiled.frame.commands[1],
            RasterCommand::BackdropBlur {
                bounds: RasterRect {
                    left: 2.0,
                    top: 4.0,
                    right: 22.0,
                    bottom: 36.0,
                },
                corner_radius: 6.0,
                sigma: 8.0,
            }
        );
    }

    #[test]
    fn zero_backdrop_sigma_is_a_noop_only_after_field_validation() {
        let node_id = fission_ir::WidgetId::explicit("backdrop.zero");
        let scene = backdrop_scene(LayoutRect::new(1.0, 2.0, 3.0, 4.0), 0.0, 2.0, node_id);

        let compiled = compile_scene(&scene, 2.0, red()).unwrap();

        assert_eq!(compiled.source_operations, 1);
        assert_eq!(compiled.frame.commands.len(), 1);

        let malformed = backdrop_scene(LayoutRect::new(f32::NAN, 2.0, 3.0, 4.0), 0.0, 2.0, node_id);
        let error = compile_scene(&malformed, 2.0, red()).unwrap_err();
        assert_eq!(error.kind, CompileErrorKind::InvalidBackdropBounds);
        assert_eq!(error.diagnostic_code(), "skia-backdrop-bounds-invalid");
        assert_eq!(error.diagnostic_category(), DiagnosticCategory::Capability);
        assert_eq!(error.provenance.node_id, Some(node_id));
        assert_eq!(error.provenance.operation_path, vec![0]);

        let negative = backdrop_scene(LayoutRect::new(1.0, 2.0, -3.0, 4.0), 0.0, 2.0, node_id);
        let error = compile_scene(&negative, 2.0, red()).unwrap_err();
        assert_eq!(error.kind, CompileErrorKind::InvalidBackdropBounds);
        assert_eq!(error.provenance.node_id, Some(node_id));
        assert_eq!(error.provenance.operation_path, vec![0]);
    }

    #[test]
    fn invalid_backdrop_parameters_have_stable_diagnostics_and_provenance() {
        let node_id = fission_ir::WidgetId::explicit("backdrop.invalid");
        for sigma in [-1.0, f32::NAN, f32::INFINITY] {
            let scene = backdrop_scene(LayoutRect::new(1.0, 2.0, 3.0, 4.0), sigma, 2.0, node_id);
            let error = compile_scene(&scene, 1.0, red()).unwrap_err();
            assert_eq!(error.kind, CompileErrorKind::InvalidBackdropBlurSigma);
            assert_eq!(error.diagnostic_code(), "skia-backdrop-blur-sigma-invalid");
            assert_eq!(error.diagnostic_category(), DiagnosticCategory::Capability);
            assert_eq!(error.provenance.root_index, Some(0));
            assert!(error.provenance.node_path.is_empty());
            assert_eq!(error.provenance.operation_path, vec![0]);
            assert_eq!(error.provenance.node_id, Some(node_id));
        }

        for corner_radius in [-1.0, f32::NAN, f32::INFINITY] {
            let scene = backdrop_scene(
                LayoutRect::new(1.0, 2.0, 3.0, 4.0),
                0.0,
                corner_radius,
                node_id,
            );
            let error = compile_scene(&scene, 1.0, red()).unwrap_err();
            assert_eq!(error.kind, CompileErrorKind::InvalidBackdropCornerRadius);
            assert_eq!(
                error.diagnostic_code(),
                "skia-backdrop-corner-radius-invalid"
            );
            assert_eq!(error.diagnostic_category(), DiagnosticCategory::Capability);
            assert_eq!(error.provenance.root_index, Some(0));
            assert!(error.provenance.node_path.is_empty());
            assert_eq!(error.provenance.operation_path, vec![0]);
            assert_eq!(error.provenance.node_id, Some(node_id));
        }
    }

    #[test]
    fn image_lowering_reports_a_missing_authoritative_resource_distinctly() {
        let node_id = fission_ir::WidgetId::explicit("image.missing-resource");
        let request = memory_image_request(&[1, 2, 3]);
        let scene = image_scene(request, node_id);
        let resources = ResourceSnapshot::empty(ResourceEpoch(1));
        let cache = SkiaImageCache::with_budget_bytes(1_024);
        let svg_cache = SkiaSvgCache::with_budget_bytes(8_192);
        let picture_cache = SkiaPictureCache::with_limits(8_192, 8);
        let paragraphs = crate::profile::new_paragraph_draw_data_registry();
        let no_picture = NoPictureRecorder;

        let error = compile_scene_with_paragraphs(
            &scene,
            2.0,
            red(),
            &resources,
            &cache,
            &svg_cache,
            None,
            paragraphs.as_ref(),
            &picture_cache,
            &no_picture,
        )
        .unwrap_err();

        assert!(matches!(
            error.kind,
            CompileErrorKind::Image(ImageError::MissingResource { .. })
        ));
        assert_eq!(error.diagnostic_code(), "skia-image-resource-missing");
        assert_eq!(error.diagnostic_category(), DiagnosticCategory::Resource);
        assert_eq!(error.provenance.node_id, Some(node_id));
        assert_eq!(error.provenance.operation_index(), Some(0));
    }

    #[cfg(feature = "test-shim")]
    #[test]
    fn image_lowering_clips_scales_and_reuses_the_driver_cache() {
        let mut encoded = b"FSIM".to_vec();
        encoded.extend_from_slice(&2_u32.to_le_bytes());
        encoded.extend_from_slice(&1_u32.to_le_bytes());
        encoded.extend_from_slice(&[255, 0, 0, 255, 0, 255, 0, 255]);
        let node_id = fission_ir::WidgetId::explicit("image.ready");
        let request = memory_image_request(&encoded);
        let identity =
            resolved_resource_content_identity(&ResourceKind::Image, &request.source, &encoded);
        let scene = image_scene(request, node_id);
        let resources = ResourceSnapshot::try_new(
            ResourceEpoch(1),
            [ResourceEntry::ready(
                ResourceId(7),
                identity,
                ResourceKind::Image,
                ResourceProvenance {
                    source: ResourceSource::Memory,
                    locator: Some("image/x-fission-test".into()),
                    requested_by: Some(node_id),
                },
                ResourcePayload::Bytes(encoded),
            )],
        )
        .unwrap();
        let cache = SkiaImageCache::with_budget_bytes(1_024);
        let svg_cache = SkiaSvgCache::with_budget_bytes(8_192);
        let picture_cache = SkiaPictureCache::with_limits(8_192, 8);
        let paragraphs = crate::profile::new_paragraph_draw_data_registry();
        let no_picture = NoPictureRecorder;

        let compile = || {
            compile_scene_with_paragraphs(
                &scene,
                2.0,
                red(),
                &resources,
                &cache,
                &svg_cache,
                None,
                paragraphs.as_ref(),
                &picture_cache,
                &no_picture,
            )
            .unwrap()
        };
        let first = compile();
        let second = compile();

        assert!(matches!(first.frame.commands[1], RasterCommand::Save));
        assert_eq!(
            first.frame.commands[2],
            RasterCommand::ClipRect {
                rect: RasterRect {
                    left: 20.0,
                    top: 40.0,
                    right: 220.0,
                    bottom: 240.0,
                }
            }
        );
        let RasterCommand::DrawImage {
            image: first_image,
            source,
            destination,
        } = &first.frame.commands[3]
        else {
            panic!("expected a decoded image command")
        };
        assert_eq!(
            *source,
            RasterRect {
                left: 0.0,
                top: 0.0,
                right: 2.0,
                bottom: 1.0,
            }
        );
        assert_eq!(
            *destination,
            RasterRect {
                left: -180.0,
                top: 40.0,
                right: 220.0,
                bottom: 240.0,
            }
        );
        let RasterCommand::DrawImage {
            image: second_image,
            ..
        } = &second.frame.commands[3]
        else {
            panic!("expected a cached decoded image command")
        };
        assert_eq!(first_image, second_image);
        assert!(matches!(first.frame.commands[4], RasterCommand::Restore));
    }
}

#[cfg(test)]
#[path = "compiler_svg_tests.rs"]
mod svg_tests;
