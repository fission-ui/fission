use std::fmt;

use fission_ir::op::{BackdropFilter, TextParagraphStyle};
use fission_ir::WidgetId;

use crate::capabilities::DisplayOpKind;
use crate::frame::{DamageRegion, FrameMetadata};
use crate::{
    BoxShadow, DisplayList, DisplayOp, Fill, LayerClip, LayoutPoint, LayoutRect, LayoutSize,
    RenderLayer, RenderNode, RenderScene, Stroke, TextStyle,
};

/// The reason an encoder-facing numeric value is not valid frame geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameGeometryProblem {
    NonFinite,
    Negative,
    NonPositive,
    OutsideUnitInterval,
}

/// Coordinate or collection element within a numeric frame field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameGeometryElement {
    X,
    Y,
    Width,
    Height,
    Right,
    Bottom,
    Index(usize),
}

impl fmt::Display for FrameGeometryProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => formatter.write_str("must be finite"),
            Self::Negative => formatter.write_str("must be non-negative"),
            Self::NonPositive => formatter.write_str("must be greater than zero"),
            Self::OutsideUnitInterval => {
                formatter.write_str("must be between zero and one inclusive")
            }
        }
    }
}

/// Exact retained-frame location of invalid numeric geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameGeometrySource {
    Viewport,
    Damage {
        rect_index: usize,
    },
    Scene,
    Layer {
        root_index: usize,
        node_path: Vec<usize>,
    },
    DisplayList {
        root_index: usize,
        node_path: Vec<usize>,
        operation_path: Vec<usize>,
        operation: Option<DisplayOpKind>,
    },
}

impl FrameGeometrySource {
    pub fn operation_index(&self) -> Option<usize> {
        match self {
            Self::DisplayList { operation_path, .. } => operation_path.last().copied(),
            Self::Viewport | Self::Damage { .. } | Self::Scene | Self::Layer { .. } => None,
        }
    }
}

impl fmt::Display for FrameGeometrySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Viewport => formatter.write_str("frame viewport"),
            Self::Damage { rect_index } => write!(formatter, "damage rectangle {rect_index}"),
            Self::Scene => formatter.write_str("render scene"),
            Self::Layer {
                root_index,
                node_path,
            } => write!(
                formatter,
                "render layer at root {root_index}, node path {node_path:?}"
            ),
            Self::DisplayList {
                root_index,
                node_path,
                operation_path,
                operation,
            } => {
                write!(
                    formatter,
                    "display list at root {root_index}, node path {node_path:?}"
                )?;
                if operation_path.is_empty() {
                    return Ok(());
                }
                write!(formatter, ", operation path {operation_path:?}")?;
                if let Some(operation) = operation {
                    write!(formatter, " ({operation:?})")?;
                }
                Ok(())
            }
        }
    }
}

/// First invalid numeric value encountered during deterministic frame traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameGeometryError {
    pub field: &'static str,
    pub element: Option<FrameGeometryElement>,
    pub problem: FrameGeometryProblem,
    pub node_id: Option<WidgetId>,
    pub source: FrameGeometrySource,
}

impl fmt::Display for FrameGeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid frame geometry at {}: {}",
            self.source, self.field
        )?;
        if let Some(element) = self.element {
            match element {
                FrameGeometryElement::X => formatter.write_str(".x")?,
                FrameGeometryElement::Y => formatter.write_str(".y")?,
                FrameGeometryElement::Width => formatter.write_str(".width")?,
                FrameGeometryElement::Height => formatter.write_str(".height")?,
                FrameGeometryElement::Right => formatter.write_str(".right")?,
                FrameGeometryElement::Bottom => formatter.write_str(".bottom")?,
                FrameGeometryElement::Index(index) => write!(formatter, "[{index}]")?,
            }
        }
        write!(formatter, " {}", self.problem)
    }
}

impl std::error::Error for FrameGeometryError {}

pub(crate) fn validate_frame_geometry(
    metadata: &FrameMetadata,
    scene: &RenderScene,
) -> Result<(), FrameGeometryError> {
    let viewport = GeometryContext::new(FrameGeometrySource::Viewport, None);
    viewport.size(
        metadata.viewport.logical_size,
        "logical_size.width",
        "logical_size.height",
    )?;
    viewport.positive_f64(metadata.viewport.scale_factor.get(), "scale_factor", None)?;

    if let DamageRegion::Rects(rects) = &metadata.damage {
        for (rect_index, rect) in rects.iter().enumerate() {
            GeometryContext::new(FrameGeometrySource::Damage { rect_index }, None)
                .rect(*rect, "rect")?;
        }
    }

    GeometryContext::new(FrameGeometrySource::Scene, None).rect(scene.bounds, "bounds")?;
    let mut visitor = GeometryVisitor;
    for (root_index, root) in scene.roots.iter().enumerate() {
        visitor.visit_node(root, root_index, &mut Vec::new(), None)?;
    }
    Ok(())
}

struct GeometryVisitor;

impl GeometryVisitor {
    fn visit_node(
        &mut self,
        node: &RenderNode,
        root_index: usize,
        node_path: &mut Vec<usize>,
        inherited_node_id: Option<WidgetId>,
    ) -> Result<(), FrameGeometryError> {
        match node {
            RenderNode::Layer(layer) => {
                self.visit_layer(layer, root_index, node_path, inherited_node_id)
            }
            RenderNode::Paint(list) => self.visit_display_list(
                list,
                root_index,
                node_path,
                &mut Vec::new(),
                inherited_node_id,
            ),
        }
    }

    fn visit_layer(
        &mut self,
        layer: &RenderLayer,
        root_index: usize,
        node_path: &mut Vec<usize>,
        inherited_node_id: Option<WidgetId>,
    ) -> Result<(), FrameGeometryError> {
        let node_id = layer.node_id.or(inherited_node_id);
        let context = GeometryContext::new(
            FrameGeometrySource::Layer {
                root_index,
                node_path: node_path.clone(),
            },
            node_id,
        );
        context.rect(layer.bounds, "bounds")?;
        match &layer.style.clip {
            Some(LayerClip::Rect(rect)) => context.rect(*rect, "clip.rect")?,
            Some(LayerClip::RoundedRect { rect, radius }) => {
                context.rect(*rect, "clip.rect")?;
                context.non_negative(*radius, "clip.radius", None)?;
            }
            None => {}
        }
        context.unit_interval(layer.style.opacity, "opacity", None)?;
        if let Some(matrix) = layer.style.transform {
            context.matrix(&matrix, "transform")?;
        }

        for (child_index, child) in layer.children.iter().enumerate() {
            node_path.push(child_index);
            self.visit_node(child, root_index, node_path, node_id)?;
            node_path.pop();
        }
        Ok(())
    }

    fn visit_display_list(
        &mut self,
        list: &DisplayList,
        root_index: usize,
        node_path: &[usize],
        operation_path: &mut Vec<usize>,
        inherited_node_id: Option<WidgetId>,
    ) -> Result<(), FrameGeometryError> {
        GeometryContext::new(
            FrameGeometrySource::DisplayList {
                root_index,
                node_path: node_path.to_vec(),
                operation_path: operation_path.clone(),
                operation: None,
            },
            inherited_node_id,
        )
        .rect(list.bounds, "bounds")?;

        for (operation_index, operation) in list.ops.iter().enumerate() {
            operation_path.push(operation_index);
            let context = GeometryContext::new(
                FrameGeometrySource::DisplayList {
                    root_index,
                    node_path: node_path.to_vec(),
                    operation_path: operation_path.clone(),
                    operation: Some(operation.kind()),
                },
                operation_node_id(operation).or(inherited_node_id),
            );
            self.visit_operation(operation, &context)?;
            if let DisplayOp::CachedScene { list, .. } = operation {
                self.visit_display_list(
                    list,
                    root_index,
                    node_path,
                    operation_path,
                    inherited_node_id,
                )?;
            }
            operation_path.pop();
        }
        Ok(())
    }

    fn visit_operation(
        &self,
        operation: &DisplayOp,
        context: &GeometryContext,
    ) -> Result<(), FrameGeometryError> {
        match operation {
            DisplayOp::Save | DisplayOp::Restore => {}
            DisplayOp::ClipRect(rect) => context.rect(*rect, "rect")?,
            DisplayOp::ClipRoundedRect { rect, radius } => {
                context.rect(*rect, "rect")?;
                context.non_negative(*radius, "radius", None)?;
            }
            DisplayOp::OpacityLayer { alpha, bounds } => {
                context.unit_interval(*alpha, "alpha", None)?;
                context.rect(*bounds, "bounds")?;
            }
            DisplayOp::Translate(point) => context.point(*point, "offset")?,
            DisplayOp::Transform(matrix) => context.matrix(matrix, "matrix")?,
            DisplayOp::CachedScene { bounds, .. } => context.rect(*bounds, "bounds")?,
            DisplayOp::BackdropFilter {
                rect,
                filter,
                corner_radius,
                bounds,
                ..
            } => {
                context.rect(*rect, "rect")?;
                match filter {
                    BackdropFilter::Blur(sigma) => {
                        context.non_negative(*sigma, "filter.blur_sigma", None)?;
                    }
                }
                context.non_negative(*corner_radius, "corner_radius", None)?;
                context.rect(*bounds, "bounds")?;
            }
            DisplayOp::DrawRect {
                rect,
                fill,
                stroke,
                corner_radius,
                shadow,
                bounds,
                ..
            } => {
                context.rect(*rect, "rect")?;
                context.optional_fill(fill.as_ref())?;
                context.optional_stroke(stroke.as_ref())?;
                context.non_negative(*corner_radius, "corner_radius", None)?;
                if let Some(shadow) = shadow {
                    context.shadow(shadow)?;
                }
                context.rect(*bounds, "bounds")?;
            }
            DisplayOp::DrawText {
                position,
                size,
                bounds,
                caret_width,
                caret_height,
                caret_radius,
                paragraph_style,
                ..
            } => {
                context.point(*position, "position")?;
                context.non_negative(*size, "size", None)?;
                context.rect(*bounds, "bounds")?;
                context.optional_non_negative(*caret_width, "caret_width", None)?;
                context.optional_non_negative(*caret_height, "caret_height", None)?;
                context.optional_non_negative(*caret_radius, "caret_radius", None)?;
                context.paragraph_style(paragraph_style.as_ref())?;
            }
            DisplayOp::DrawRichText {
                runs,
                position,
                bounds,
                caret_width,
                caret_height,
                caret_radius,
                paragraph_style,
                ..
            } => {
                for (run_index, run) in runs.iter().enumerate() {
                    context.text_style(&run.style, run_index)?;
                }
                context.point(*position, "position")?;
                context.rect(*bounds, "bounds")?;
                context.optional_non_negative(*caret_width, "caret_width", None)?;
                context.optional_non_negative(*caret_height, "caret_height", None)?;
                context.optional_non_negative(*caret_radius, "caret_radius", None)?;
                context.paragraph_style(paragraph_style.as_ref())?;
            }
            DisplayOp::DrawImage { rect, bounds, .. } => {
                context.rect(*rect, "rect")?;
                context.rect(*bounds, "bounds")?;
            }
            DisplayOp::DrawPath {
                fill,
                stroke,
                bounds,
                ..
            }
            | DisplayOp::DrawSvg {
                fill,
                stroke,
                bounds,
                ..
            } => {
                context.optional_fill(fill.as_ref())?;
                context.optional_stroke(stroke.as_ref())?;
                context.rect(*bounds, "bounds")?;
            }
            DisplayOp::DrawSurface { rect, bounds, .. } => {
                context.rect(*rect, "rect")?;
                context.rect(*bounds, "bounds")?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct GeometryContext {
    source: FrameGeometrySource,
    node_id: Option<WidgetId>,
}

impl GeometryContext {
    fn new(source: FrameGeometrySource, node_id: Option<WidgetId>) -> Self {
        Self { source, node_id }
    }

    fn error(
        &self,
        field: &'static str,
        element: Option<FrameGeometryElement>,
        problem: FrameGeometryProblem,
    ) -> FrameGeometryError {
        FrameGeometryError {
            field,
            element,
            problem,
            node_id: self.node_id,
            source: self.source.clone(),
        }
    }

    fn finite(
        &self,
        value: f32,
        field: &'static str,
        element: Option<FrameGeometryElement>,
    ) -> Result<(), FrameGeometryError> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(self.error(field, element, FrameGeometryProblem::NonFinite))
        }
    }

    fn positive_f64(
        &self,
        value: f64,
        field: &'static str,
        element: Option<FrameGeometryElement>,
    ) -> Result<(), FrameGeometryError> {
        if !value.is_finite() {
            Err(self.error(field, element, FrameGeometryProblem::NonFinite))
        } else if value <= 0.0 {
            Err(self.error(field, element, FrameGeometryProblem::NonPositive))
        } else {
            Ok(())
        }
    }

    fn non_negative(
        &self,
        value: f32,
        field: &'static str,
        element: Option<FrameGeometryElement>,
    ) -> Result<(), FrameGeometryError> {
        self.finite(value, field, element)?;
        if value < 0.0 {
            Err(self.error(field, element, FrameGeometryProblem::Negative))
        } else {
            Ok(())
        }
    }

    fn optional_non_negative(
        &self,
        value: Option<f32>,
        field: &'static str,
        element: Option<FrameGeometryElement>,
    ) -> Result<(), FrameGeometryError> {
        if let Some(value) = value {
            self.non_negative(value, field, element)?;
        }
        Ok(())
    }

    fn unit_interval(
        &self,
        value: f32,
        field: &'static str,
        element: Option<FrameGeometryElement>,
    ) -> Result<(), FrameGeometryError> {
        self.finite(value, field, element)?;
        if !(0.0..=1.0).contains(&value) {
            Err(self.error(field, element, FrameGeometryProblem::OutsideUnitInterval))
        } else {
            Ok(())
        }
    }

    fn point(&self, point: LayoutPoint, field: &'static str) -> Result<(), FrameGeometryError> {
        self.finite(point.x, field, Some(FrameGeometryElement::X))?;
        self.finite(point.y, field, Some(FrameGeometryElement::Y))
    }

    fn size(
        &self,
        size: LayoutSize,
        width_field: &'static str,
        height_field: &'static str,
    ) -> Result<(), FrameGeometryError> {
        self.non_negative(size.width, width_field, None)?;
        self.non_negative(size.height, height_field, None)
    }

    fn rect(&self, rect: LayoutRect, field: &'static str) -> Result<(), FrameGeometryError> {
        self.finite(rect.origin.x, field, Some(FrameGeometryElement::X))?;
        self.finite(rect.origin.y, field, Some(FrameGeometryElement::Y))?;
        self.non_negative(rect.size.width, field, Some(FrameGeometryElement::Width))?;
        self.non_negative(rect.size.height, field, Some(FrameGeometryElement::Height))?;
        self.finite(rect.right(), field, Some(FrameGeometryElement::Right))?;
        self.finite(rect.bottom(), field, Some(FrameGeometryElement::Bottom))
    }

    fn matrix(&self, matrix: &[f32; 16], field: &'static str) -> Result<(), FrameGeometryError> {
        for (element_index, value) in matrix.iter().copied().enumerate() {
            self.finite(
                value,
                field,
                Some(FrameGeometryElement::Index(element_index)),
            )?;
        }
        Ok(())
    }

    fn optional_fill(&self, fill: Option<&Fill>) -> Result<(), FrameGeometryError> {
        if let Some(fill) = fill {
            self.fill(fill)?;
        }
        Ok(())
    }

    fn fill(&self, fill: &Fill) -> Result<(), FrameGeometryError> {
        match fill {
            Fill::Solid(_) => {}
            Fill::LinearGradient { start, end, stops } => {
                self.finite(
                    start.0,
                    "fill.linear_gradient.start",
                    Some(FrameGeometryElement::X),
                )?;
                self.finite(
                    start.1,
                    "fill.linear_gradient.start",
                    Some(FrameGeometryElement::Y),
                )?;
                self.finite(
                    end.0,
                    "fill.linear_gradient.end",
                    Some(FrameGeometryElement::X),
                )?;
                self.finite(
                    end.1,
                    "fill.linear_gradient.end",
                    Some(FrameGeometryElement::Y),
                )?;
                for (stop_index, (offset, _)) in stops.iter().enumerate() {
                    self.finite(
                        *offset,
                        "fill.linear_gradient.stop",
                        Some(FrameGeometryElement::Index(stop_index)),
                    )?;
                }
            }
            Fill::RadialGradient {
                center,
                radius,
                stops,
            } => {
                self.finite(
                    center.0,
                    "fill.radial_gradient.center",
                    Some(FrameGeometryElement::X),
                )?;
                self.finite(
                    center.1,
                    "fill.radial_gradient.center",
                    Some(FrameGeometryElement::Y),
                )?;
                self.non_negative(*radius, "fill.radial_gradient.radius", None)?;
                for (stop_index, (offset, _)) in stops.iter().enumerate() {
                    self.finite(
                        *offset,
                        "fill.radial_gradient.stop",
                        Some(FrameGeometryElement::Index(stop_index)),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn optional_stroke(&self, stroke: Option<&Stroke>) -> Result<(), FrameGeometryError> {
        if let Some(stroke) = stroke {
            self.fill(&stroke.fill)?;
            self.non_negative(stroke.width, "stroke.width", None)?;
            if let Some(dash_array) = &stroke.dash_array {
                for (dash_index, dash) in dash_array.iter().copied().enumerate() {
                    self.non_negative(
                        dash,
                        "stroke.dash_array",
                        Some(FrameGeometryElement::Index(dash_index)),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn shadow(&self, shadow: &BoxShadow) -> Result<(), FrameGeometryError> {
        self.non_negative(shadow.blur_radius, "shadow.blur_radius", None)?;
        self.finite(shadow.spread_radius, "shadow.spread_radius", None)?;
        self.finite(
            shadow.offset.0,
            "shadow.offset",
            Some(FrameGeometryElement::X),
        )?;
        self.finite(
            shadow.offset.1,
            "shadow.offset",
            Some(FrameGeometryElement::Y),
        )
    }

    fn text_style(&self, style: &TextStyle, run_index: usize) -> Result<(), FrameGeometryError> {
        let run = Some(FrameGeometryElement::Index(run_index));
        self.non_negative(style.font_size, "run.font_size", run)?;
        self.optional_non_negative(style.line_height, "run.line_height", run)?;
        self.finite(style.letter_spacing, "run.letter_spacing", run)
    }

    fn paragraph_style(
        &self,
        style: Option<&TextParagraphStyle>,
    ) -> Result<(), FrameGeometryError> {
        if let Some(strut_line_height) = style.and_then(|style| style.strut_line_height) {
            self.non_negative(strut_line_height, "paragraph.strut_line_height", None)?;
        }
        Ok(())
    }
}

fn operation_node_id(operation: &DisplayOp) -> Option<WidgetId> {
    match operation {
        DisplayOp::BackdropFilter { node_id, .. }
        | DisplayOp::DrawRect { node_id, .. }
        | DisplayOp::DrawText { node_id, .. }
        | DisplayOp::DrawRichText { node_id, .. }
        | DisplayOp::DrawImage { node_id, .. }
        | DisplayOp::DrawPath { node_id, .. }
        | DisplayOp::DrawSvg { node_id, .. }
        | DisplayOp::DrawSurface { node_id, .. } => *node_id,
        DisplayOp::Save
        | DisplayOp::Restore
        | DisplayOp::ClipRect(_)
        | DisplayOp::ClipRoundedRect { .. }
        | DisplayOp::OpacityLayer { .. }
        | DisplayOp::Translate(_)
        | DisplayOp::Transform(_)
        | DisplayOp::CachedScene { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{FrameId, FrameViewport, ResourceEpoch, SemanticsEpoch};
    use crate::surface::{PhysicalSize, ScaleFactor};
    use crate::{Color, LayerStyle, LineCap, LineJoin, RenderNode, TextRun};

    fn bounds() -> LayoutRect {
        LayoutRect::new(0.0, 0.0, 100.0, 50.0)
    }

    fn metadata() -> FrameMetadata {
        FrameMetadata {
            frame_id: FrameId(1),
            viewport: FrameViewport {
                logical_size: bounds().size,
                physical_size: PhysicalSize::new(100, 50),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(1),
            semantics_epoch: SemanticsEpoch(1),
        }
    }

    fn black() -> Color {
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    #[test]
    fn rejects_invalid_viewport_and_damage_before_scene_traversal() {
        let scene = RenderScene::new(bounds());
        let mut invalid_viewport = metadata();
        invalid_viewport.viewport.logical_size.width = f32::NAN;
        let error = validate_frame_geometry(&invalid_viewport, &scene).unwrap_err();
        assert_eq!(error.field, "logical_size.width");
        assert_eq!(error.problem, FrameGeometryProblem::NonFinite);
        assert_eq!(error.source, FrameGeometrySource::Viewport);

        let mut invalid_damage = metadata();
        invalid_damage.damage =
            DamageRegion::Rects(vec![bounds(), LayoutRect::new(0.0, 0.0, -1.0, 2.0)]);
        let error = validate_frame_geometry(&invalid_damage, &scene).unwrap_err();
        assert_eq!(error.problem, FrameGeometryProblem::Negative);
        assert_eq!(error.element, Some(FrameGeometryElement::Width));
        assert_eq!(error.source, FrameGeometrySource::Damage { rect_index: 1 });
    }

    #[test]
    fn rejects_layer_geometry_with_retained_node_provenance() {
        let layer_id = WidgetId::explicit("invalid-layer");
        let mut child = RenderLayer::new(bounds());
        child.node_id = Some(layer_id);
        child.style = LayerStyle {
            clip: Some(LayerClip::RoundedRect {
                rect: bounds(),
                radius: 4.0,
            }),
            opacity: 1.2,
            transform: None,
            transform_clip: true,
            cache_key: None,
            content_cache_key: None,
        };
        let mut parent = RenderLayer::new(bounds());
        parent.children.push(RenderNode::Layer(child));
        let mut scene = RenderScene::new(bounds());
        scene.roots.push(RenderNode::Layer(parent));

        let error = validate_frame_geometry(&metadata(), &scene).unwrap_err();

        assert_eq!(error.field, "opacity");
        assert_eq!(error.problem, FrameGeometryProblem::OutsideUnitInterval);
        assert_eq!(error.node_id, Some(layer_id));
        assert_eq!(
            error.source,
            FrameGeometrySource::Layer {
                root_index: 0,
                node_path: vec![0],
            }
        );
    }

    #[test]
    fn rejects_nested_cached_surface_geometry_with_exact_operation_path() {
        let node_id = WidgetId::explicit("invalid-surface");
        let mut nested = DisplayList::new(bounds());
        nested.push(DisplayOp::DrawSurface {
            rect: LayoutRect::new(f32::INFINITY, 0.0, 10.0, 10.0),
            surface_id: 9,
            position: 0,
            bounds: bounds(),
            node_id: Some(node_id),
        });
        let mut outer = DisplayList::new(bounds());
        outer.push(DisplayOp::CachedScene {
            cache_key: 1,
            bounds: bounds(),
            list: Box::new(nested),
        });
        let mut layer = RenderLayer::new(bounds());
        layer.children.push(RenderNode::Paint(outer));
        let mut scene = RenderScene::new(bounds());
        scene.roots.push(RenderNode::Layer(layer));

        let error = validate_frame_geometry(&metadata(), &scene).unwrap_err();

        assert_eq!(error.field, "rect");
        assert_eq!(error.element, Some(FrameGeometryElement::X));
        assert_eq!(error.node_id, Some(node_id));
        assert_eq!(
            error.source,
            FrameGeometrySource::DisplayList {
                root_index: 0,
                node_path: vec![0],
                operation_path: vec![0, 0],
                operation: Some(DisplayOpKind::DrawSurface),
            }
        );
    }

    #[test]
    fn rejects_derived_non_finite_rectangle_edges() {
        let mut list = DisplayList::new(bounds());
        list.push(DisplayOp::ClipRect(LayoutRect::new(
            f32::MAX,
            0.0,
            f32::MAX,
            1.0,
        )));
        let scene = RenderScene::from_display_list(list);

        let error = validate_frame_geometry(&metadata(), &scene).unwrap_err();

        assert_eq!(error.field, "rect");
        assert_eq!(error.element, Some(FrameGeometryElement::Right));
        assert_eq!(error.problem, FrameGeometryProblem::NonFinite);
    }

    #[test]
    fn validates_encoder_facing_paint_and_text_numbers() {
        let mut list = DisplayList::new(bounds());
        list.push(DisplayOp::DrawRect {
            rect: bounds(),
            fill: Some(Fill::LinearGradient {
                start: (0.0, 0.0),
                end: (1.0, 1.0),
                stops: vec![(0.0, black()), (f32::NAN, black())],
            }),
            stroke: Some(Stroke {
                fill: Fill::Solid(black()),
                width: 1.0,
                dash_array: Some(vec![2.0, 1.0]),
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
            }),
            corner_radius: 0.0,
            shadow: None,
            bounds: bounds(),
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);
        let error = validate_frame_geometry(&metadata(), &scene).unwrap_err();
        assert_eq!(error.field, "fill.linear_gradient.stop");
        assert_eq!(error.element, Some(FrameGeometryElement::Index(1)));
        assert_eq!(error.problem, FrameGeometryProblem::NonFinite);

        let run = TextRun {
            text: "legal negative spacing".into(),
            style: TextStyle {
                font_size: 14.0,
                color: black(),
                underline: false,
                font_family: None,
                locale: None,
                font_weight: 400,
                font_style: Default::default(),
                line_height: Some(1.2),
                letter_spacing: -0.5,
                background_color: None,
            },
        };
        let mut list = DisplayList::new(bounds());
        list.push(DisplayOp::DrawRichText {
            runs: vec![run],
            position: LayoutPoint::ZERO,
            bounds: bounds(),
            node_id: None,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
            annotations: Vec::new(),
        });
        validate_frame_geometry(&metadata(), &RenderScene::from_display_list(list)).unwrap();
    }

    #[test]
    fn invalid_numeric_diagnostics_are_stable() {
        let mut list = DisplayList::new(bounds());
        list.push(DisplayOp::ClipRoundedRect {
            rect: bounds(),
            radius: -1.0,
        });
        let error = validate_frame_geometry(&metadata(), &RenderScene::from_display_list(list))
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "invalid frame geometry at display list at root 0, node path [], operation path [0] (ClipRoundedRect): radius must be non-negative"
        );
    }
}
