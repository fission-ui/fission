use std::collections::BTreeMap;
use std::fmt;

use fission_ir::WidgetId;
use fission_render::capabilities::DisplayOpKind;
use fission_render::external_surface::ExternalSurfaceSlotId;
use fission_render::{
    DisplayList, DisplayOp, LayerClip, LayoutPoint, LayoutRect, RenderNode, RenderScene,
};

use super::FrameSubmissionError;

#[derive(Debug)]
pub(super) struct CollectedSurfacePlacements {
    pub(super) placements: BTreeMap<ExternalSurfaceSlotId, ResolvedSurfacePlacement>,
    pub(super) paint_sequence: Vec<ScenePaintItem>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ScenePaintItem {
    Paint {
        operation: DisplayOpKind,
        bounds: ScenePaintBounds,
    },
    Surface(ExternalSurfaceSlotId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ScenePaintBounds {
    Visible(LayoutRect),
    Invisible,
    Unknown,
}

pub(super) fn collect_surface_placements(scene: &RenderScene) -> CollectedSurfacePlacements {
    let mut traversal = SurfacePlacementTraversal::default();
    for root in &scene.roots {
        traversal.visit_node(root);
    }
    CollectedSurfacePlacements {
        placements: traversal.placements,
        paint_sequence: traversal.paint_sequence,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NativeViewGeometry {
    pub(super) rect: LayoutRect,
    pub(super) visible_rect: LayoutRect,
    pub(super) transform: Option<[f32; 16]>,
    pub(super) opacity: f32,
    pub(super) paint_order: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedSurfacePlacement {
    pub(super) viewport: LayoutRect,
    pub(super) clip: ClipRegion,
    pub(super) transform: Affine2d,
    pub(super) opacity: f32,
    pub(super) paint_order: u64,
    pub(super) issue: Option<SurfacePlacementIssue>,
}

impl ResolvedSurfacePlacement {
    pub(super) fn native_view_geometry(
        self,
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
    ) -> Result<Option<NativeViewGeometry>, FrameSubmissionError> {
        let visible_rect = match self.visible_rect() {
            Some(visible_rect) => visible_rect,
            None => return Ok(None),
        };
        if let Some(issue) = self.issue {
            return Err(FrameSubmissionError::UnsupportedNativeViewPlacement {
                widget_id,
                slot_id,
                issue,
            });
        }
        if !is_positive_rect(self.viewport)
            || !is_positive_rect(visible_rect)
            || self.opacity <= 0.001
        {
            return Ok(None);
        }
        let paint_order = u32::try_from(self.paint_order).map_err(|_| {
            FrameSubmissionError::SurfacePaintOrderExhausted {
                widget_id,
                slot_id,
                paint_order: self.paint_order,
            }
        })?;
        Ok(Some(NativeViewGeometry {
            rect: self.viewport,
            visible_rect,
            transform: (!self.transform.is_identity()).then(|| self.transform.to_mat4()),
            opacity: self.opacity,
            paint_order,
        }))
    }

    #[cfg(feature = "three-d")]
    pub(super) fn direct_target_geometry(
        self,
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
    ) -> Result<Option<(LayoutRect, LayoutRect, f32, u64)>, FrameSubmissionError> {
        let Some(visible) = self.visible_rect() else {
            return Ok(None);
        };
        if let Some(issue) = self.issue.or_else(|| self.transform.direct_target_issue()) {
            return Err(FrameSubmissionError::UnsupportedDirectTargetPlacement {
                widget_id,
                slot_id,
                issue,
            });
        }
        if !is_positive_rect(self.viewport) || !is_positive_rect(visible) || self.opacity <= 0.001 {
            return Ok(None);
        }
        Ok(Some((
            self.viewport,
            visible,
            self.opacity,
            self.paint_order,
        )))
    }

    pub(super) fn visible_rect(self) -> Option<LayoutRect> {
        match self.clip {
            ClipRegion::Unbounded => Some(self.viewport),
            ClipRegion::Rect(clip) => intersect_layout_rects(self.viewport, clip),
            ClipRegion::Empty => None,
        }
    }

    pub(super) fn requires_affine_transform(self) -> bool {
        self.transform.direct_target_issue().is_some()
    }

    pub(super) fn is_visible(self) -> bool {
        self.visible_rect().is_some_and(is_positive_rect)
            && is_positive_rect(self.viewport)
            && self.opacity > 0.001
    }

    fn opacity_isolation_bounds(self) -> ScenePaintBounds {
        if self.opacity.is_finite() && self.opacity <= 0.001 {
            return ScenePaintBounds::Invisible;
        }
        if matches!(
            self.issue,
            Some(
                SurfacePlacementIssue::NonAffineTransform
                    | SurfacePlacementIssue::InvalidOpacity
                    | SurfacePlacementIssue::InvalidGeometry
            )
        ) {
            return ScenePaintBounds::Unknown;
        }
        self.visible_rect()
            .filter(|rect| is_positive_rect(*rect))
            .map(ScenePaintBounds::Visible)
            .unwrap_or(ScenePaintBounds::Invisible)
    }
}

fn is_positive_rect(rect: LayoutRect) -> bool {
    rect.origin.x.is_finite()
        && rect.origin.y.is_finite()
        && rect.size.width.is_finite()
        && rect.size.height.is_finite()
        && rect.size.width > 0.0
        && rect.size.height > 0.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SurfacePlacementIssue {
    NonAffineTransform,
    NonAxisAlignedTransform,
    ReflectedTransform,
    RoundedClip,
    InvalidOpacity,
    InvalidGeometry,
    IsolatedOpacityPaintOverlap {
        operation: DisplayOpKind,
    },
    IsolatedOpacitySurfaceOverlap {
        other_slot_id: ExternalSurfaceSlotId,
    },
}

impl fmt::Display for SurfacePlacementIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAffineTransform => {
                write!(formatter, "a perspective or non-2D affine transform")
            }
            Self::NonAxisAlignedTransform => write!(
                formatter,
                "rotation or shear, which the transitional direct-target 3D adapter cannot honor"
            ),
            Self::ReflectedTransform => write!(
                formatter,
                "a reflected or collapsed transform, which the transitional direct-target 3D adapter cannot honor"
            ),
            Self::RoundedClip => write!(
                formatter,
                "a rounded clip, which the external platform surface contract cannot honor"
            ),
            Self::InvalidOpacity => write!(formatter, "a non-finite opacity"),
            Self::InvalidGeometry => write!(formatter, "non-finite transformed geometry"),
            Self::IsolatedOpacityPaintOverlap { operation } => write!(
                formatter,
                "an isolated opacity group with overlapping {operation:?} paint; flattening the group opacity onto the external surface would change source-over compositing"
            ),
            Self::IsolatedOpacitySurfaceOverlap { other_slot_id } => write!(
                formatter,
                "an isolated opacity group with overlapping external surface slot {}; flattening the group opacity onto separate external surfaces would change source-over compositing",
                other_slot_id.0
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Affine2d {
    m00: f32,
    m10: f32,
    m01: f32,
    m11: f32,
    dx: f32,
    dy: f32,
}

impl Affine2d {
    pub(super) const IDENTITY: Self = Self {
        m00: 1.0,
        m10: 0.0,
        m01: 0.0,
        m11: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    fn translation(point: LayoutPoint) -> Self {
        Self {
            dx: point.x,
            dy: point.y,
            ..Self::IDENTITY
        }
    }

    fn from_mat4(matrix: &[f32; 16]) -> Result<Self, SurfacePlacementIssue> {
        const EPSILON: f32 = 0.000_01;
        if !matrix.iter().all(|value| value.is_finite())
            || matrix[2].abs() > EPSILON
            || matrix[3].abs() > EPSILON
            || matrix[6].abs() > EPSILON
            || matrix[7].abs() > EPSILON
            || matrix[8].abs() > EPSILON
            || matrix[9].abs() > EPSILON
            || (matrix[10] - 1.0).abs() > EPSILON
            || matrix[11].abs() > EPSILON
            || matrix[14].abs() > EPSILON
            || (matrix[15] - 1.0).abs() > EPSILON
        {
            return Err(SurfacePlacementIssue::NonAffineTransform);
        }
        Ok(Self {
            m00: matrix[0],
            m10: matrix[1],
            m01: matrix[4],
            m11: matrix[5],
            dx: matrix[12],
            dy: matrix[13],
        })
    }

    fn then(self, next: Self) -> Self {
        Self {
            m00: self.m00 * next.m00 + self.m01 * next.m10,
            m10: self.m10 * next.m00 + self.m11 * next.m10,
            m01: self.m00 * next.m01 + self.m01 * next.m11,
            m11: self.m10 * next.m01 + self.m11 * next.m11,
            dx: self.m00 * next.dx + self.m01 * next.dy + self.dx,
            dy: self.m10 * next.dx + self.m11 * next.dy + self.dy,
        }
    }

    fn transform_point(self, point: LayoutPoint) -> LayoutPoint {
        LayoutPoint::new(
            self.m00 * point.x + self.m01 * point.y + self.dx,
            self.m10 * point.x + self.m11 * point.y + self.dy,
        )
    }

    fn transform_rect_bounds(self, rect: LayoutRect) -> Result<LayoutRect, SurfacePlacementIssue> {
        let points = [
            self.transform_point(rect.origin),
            self.transform_point(LayoutPoint::new(rect.right(), rect.y())),
            self.transform_point(LayoutPoint::new(rect.x(), rect.bottom())),
            self.transform_point(LayoutPoint::new(rect.right(), rect.bottom())),
        ];
        if !points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
        {
            return Err(SurfacePlacementIssue::InvalidGeometry);
        }
        let min_x = points
            .iter()
            .map(|point| point.x)
            .fold(f32::INFINITY, f32::min);
        let min_y = points
            .iter()
            .map(|point| point.y)
            .fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = points
            .iter()
            .map(|point| point.y)
            .fold(f32::NEG_INFINITY, f32::max);
        Ok(LayoutRect::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }

    fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    fn to_mat4(self) -> [f32; 16] {
        [
            self.m00, self.m10, 0.0, 0.0, self.m01, self.m11, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
            self.dx, self.dy, 0.0, 1.0,
        ]
    }

    fn direct_target_issue(self) -> Option<SurfacePlacementIssue> {
        const EPSILON: f32 = 0.000_01;
        if self.m01.abs() > EPSILON || self.m10.abs() > EPSILON {
            Some(SurfacePlacementIssue::NonAxisAlignedTransform)
        } else if self.m00 <= EPSILON || self.m11 <= EPSILON {
            Some(SurfacePlacementIssue::ReflectedTransform)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlacementState {
    transform: Affine2d,
    clip: ClipRegion,
    opacity: f32,
    issue: Option<SurfacePlacementIssue>,
}

impl Default for PlacementState {
    fn default() -> Self {
        Self {
            transform: Affine2d::IDENTITY,
            clip: ClipRegion::Unbounded,
            opacity: 1.0,
            issue: None,
        }
    }
}

impl PlacementState {
    fn remember_issue(&mut self, issue: SurfacePlacementIssue) {
        if self.issue.is_none() {
            self.issue = Some(issue);
        }
    }

    fn apply_transform(&mut self, matrix: &[f32; 16]) {
        match Affine2d::from_mat4(matrix) {
            Ok(transform) => self.transform = self.transform.then(transform),
            Err(issue) => self.remember_issue(issue),
        }
    }

    fn apply_clip(&mut self, rect: LayoutRect, rounded: bool) {
        match self.transform.transform_rect_bounds(rect) {
            Ok(rect) => {
                self.clip = match self.clip {
                    ClipRegion::Unbounded => ClipRegion::Rect(rect),
                    ClipRegion::Rect(current) => intersect_layout_rects(current, rect)
                        .map(ClipRegion::Rect)
                        .unwrap_or(ClipRegion::Empty),
                    ClipRegion::Empty => ClipRegion::Empty,
                };
            }
            Err(issue) => self.remember_issue(issue),
        }
        if rounded {
            self.remember_issue(SurfacePlacementIssue::RoundedClip);
        }
    }

    fn apply_opacity(&mut self, opacity: f32, bounds: LayoutRect) {
        self.apply_clip(bounds, false);
        if opacity.is_finite() && (0.0..=1.0).contains(&opacity) {
            self.opacity *= opacity;
        } else {
            self.remember_issue(SurfacePlacementIssue::InvalidOpacity);
        }
    }

    fn resolve_surface(self, rect: LayoutRect, paint_order: u64) -> ResolvedSurfacePlacement {
        let mut issue = self.issue;
        let viewport = match self.transform.transform_rect_bounds(rect) {
            Ok(viewport) => viewport,
            Err(error) => {
                if issue.is_none() {
                    issue = Some(error);
                }
                LayoutRect::new(0.0, 0.0, 0.0, 0.0)
            }
        };
        ResolvedSurfacePlacement {
            viewport,
            clip: self.clip,
            transform: self.transform,
            opacity: self.opacity,
            paint_order,
            issue,
        }
    }

    fn resolve_paint_bounds(self, bounds: LayoutRect) -> ScenePaintBounds {
        if self.opacity.is_finite() && self.opacity <= 0.001 {
            return ScenePaintBounds::Invisible;
        }
        if matches!(
            self.issue,
            Some(
                SurfacePlacementIssue::NonAffineTransform
                    | SurfacePlacementIssue::InvalidOpacity
                    | SurfacePlacementIssue::InvalidGeometry
            )
        ) {
            return ScenePaintBounds::Unknown;
        }
        let transformed = match self.transform.transform_rect_bounds(bounds) {
            Ok(transformed) => transformed,
            Err(_) => return ScenePaintBounds::Unknown,
        };
        match self.clip {
            ClipRegion::Unbounded => ScenePaintBounds::Visible(transformed),
            ClipRegion::Rect(clip) => intersect_layout_rects(transformed, clip)
                .map(ScenePaintBounds::Visible)
                .unwrap_or(ScenePaintBounds::Invisible),
            ClipRegion::Empty => ScenePaintBounds::Invisible,
        }
    }
}

#[derive(Default)]
struct SurfacePlacementTraversal {
    current: PlacementState,
    saved: Vec<SavedPlacementState>,
    opacity_groups: Vec<OpacityGroup>,
    next_surface_order: u64,
    placements: BTreeMap<ExternalSurfaceSlotId, ResolvedSurfacePlacement>,
    paint_sequence: Vec<ScenePaintItem>,
}

#[derive(Debug, Clone, Copy)]
struct SavedPlacementState {
    placement: PlacementState,
    opacity_group_count: usize,
}

#[derive(Debug, Clone, Copy)]
enum OpacityGroupContent {
    Paint {
        operation: DisplayOpKind,
        bounds: ScenePaintBounds,
    },
    Surface {
        slot_id: ExternalSurfaceSlotId,
        bounds: ScenePaintBounds,
    },
}

#[derive(Debug, Default)]
struct OpacityGroup {
    // Renderer opacity layers isolate their children before applying alpha.
    // A platform child can receive that alpha directly only when no other
    // content in the same group overlaps it. Each nested group remains on
    // this stack, so leaf content is checked against every isolating ancestor.
    content: Vec<OpacityGroupContent>,
}

impl SurfacePlacementTraversal {
    fn visit_node(&mut self, node: &RenderNode) {
        match node {
            RenderNode::Paint(list) => self.visit_list(list),
            RenderNode::Layer(layer) => {
                let parent = self.current;
                let saved_len = self.saved.len();
                let opacity_group_count = self.opacity_groups.len();
                if let Some(clip) = &layer.style.clip {
                    match clip {
                        LayerClip::Rect(rect) => self.current.apply_clip(*rect, false),
                        LayerClip::RoundedRect { rect, .. } => self.current.apply_clip(*rect, true),
                    }
                }
                if !layer.style.opacity.is_finite() || (layer.style.opacity - 1.0).abs() > 0.001 {
                    self.begin_opacity_group(layer.style.opacity);
                    self.current
                        .apply_opacity(layer.style.opacity, layer.bounds);
                }
                if let Some(transform) = &layer.style.transform {
                    self.current.apply_transform(transform);
                }
                for child in &layer.children {
                    self.visit_node(child);
                }
                self.current = parent;
                self.saved.truncate(saved_len);
                self.opacity_groups.truncate(opacity_group_count);
            }
        }
    }

    fn visit_list(&mut self, list: &DisplayList) {
        for operation in &list.ops {
            match operation {
                DisplayOp::Save => self.saved.push(SavedPlacementState {
                    placement: self.current,
                    opacity_group_count: self.opacity_groups.len(),
                }),
                DisplayOp::Restore => {
                    if let Some(saved) = self.saved.pop() {
                        self.current = saved.placement;
                        self.opacity_groups.truncate(saved.opacity_group_count);
                    }
                }
                DisplayOp::ClipRect(rect) => self.current.apply_clip(*rect, false),
                DisplayOp::ClipRoundedRect { rect, .. } => self.current.apply_clip(*rect, true),
                DisplayOp::OpacityLayer { alpha, bounds } => {
                    self.begin_opacity_group(*alpha);
                    self.current.apply_opacity(*alpha, *bounds)
                }
                DisplayOp::Translate(point) => {
                    self.current.transform =
                        self.current.transform.then(Affine2d::translation(*point));
                }
                DisplayOp::Transform(matrix) => self.current.apply_transform(matrix),
                DisplayOp::CachedScene { list, .. } => {
                    let parent = self.current;
                    let saved_len = self.saved.len();
                    let opacity_group_count = self.opacity_groups.len();
                    self.visit_list(list);
                    self.current = parent;
                    self.saved.truncate(saved_len);
                    self.opacity_groups.truncate(opacity_group_count);
                }
                DisplayOp::DrawSurface {
                    surface_id, rect, ..
                } => {
                    let slot_id = ExternalSurfaceSlotId(*surface_id);
                    self.paint_sequence.push(ScenePaintItem::Surface(slot_id));
                    // Duplicate placement is rejected by `InteractiveFrame`
                    // before this derived plan can execute. Retaining the
                    // first value keeps preparation deterministic until then.
                    let mut placement =
                        self.current.resolve_surface(*rect, self.next_surface_order);
                    self.next_surface_order = self.next_surface_order.saturating_add(1);
                    self.record_surface_in_opacity_groups(slot_id, &mut placement);
                    self.placements.entry(slot_id).or_insert(placement);
                }
                DisplayOp::BackdropFilter { bounds, .. }
                | DisplayOp::DrawRect { bounds, .. }
                | DisplayOp::DrawText { bounds, .. }
                | DisplayOp::DrawRichText { bounds, .. }
                | DisplayOp::DrawImage { bounds, .. }
                | DisplayOp::DrawPath { bounds, .. }
                | DisplayOp::DrawSvg { bounds, .. } => {
                    let operation = operation.kind();
                    let bounds = self.current.resolve_paint_bounds(*bounds);
                    self.paint_sequence
                        .push(ScenePaintItem::Paint { operation, bounds });
                    self.record_paint_in_opacity_groups(operation, bounds);
                }
            }
        }
    }

    fn begin_opacity_group(&mut self, opacity: f32) {
        if !opacity.is_finite()
            || !(0.0..=1.0).contains(&opacity)
            || opacity <= 0.001
            || opacity >= 1.0
        {
            return;
        }
        self.opacity_groups.push(OpacityGroup::default());
    }

    fn record_surface_in_opacity_groups(
        &mut self,
        slot_id: ExternalSurfaceSlotId,
        placement: &mut ResolvedSurfacePlacement,
    ) {
        let bounds = placement.opacity_isolation_bounds();
        if bounds == ScenePaintBounds::Invisible {
            return;
        }

        for group_id in (0..self.opacity_groups.len()).rev() {
            let mut first_conflict = None;
            let mut surfaces_to_mark = Vec::new();
            for content in &self.opacity_groups[group_id].content {
                match *content {
                    OpacityGroupContent::Paint {
                        operation,
                        bounds: paint_bounds,
                    } if scene_bounds_overlap(bounds, paint_bounds) => {
                        if first_conflict.is_none() {
                            first_conflict =
                                Some(SurfacePlacementIssue::IsolatedOpacityPaintOverlap {
                                    operation,
                                });
                        }
                    }
                    OpacityGroupContent::Surface {
                        slot_id: other_slot_id,
                        bounds: other_bounds,
                    } if other_slot_id != slot_id && scene_bounds_overlap(bounds, other_bounds) => {
                        if first_conflict.is_none() {
                            first_conflict =
                                Some(SurfacePlacementIssue::IsolatedOpacitySurfaceOverlap {
                                    other_slot_id,
                                });
                        }
                        surfaces_to_mark.push(other_slot_id);
                    }
                    OpacityGroupContent::Paint { .. } | OpacityGroupContent::Surface { .. } => {}
                }
            }
            for other_slot_id in surfaces_to_mark {
                self.remember_placement_issue(
                    other_slot_id,
                    SurfacePlacementIssue::IsolatedOpacitySurfaceOverlap {
                        other_slot_id: slot_id,
                    },
                );
            }
            if placement.issue.is_none() {
                placement.issue = first_conflict;
            }
            self.opacity_groups[group_id]
                .content
                .push(OpacityGroupContent::Surface { slot_id, bounds });
        }
    }

    fn record_paint_in_opacity_groups(
        &mut self,
        operation: DisplayOpKind,
        bounds: ScenePaintBounds,
    ) {
        if bounds == ScenePaintBounds::Invisible {
            return;
        }

        for group_id in (0..self.opacity_groups.len()).rev() {
            let conflicting_slots = self.opacity_groups[group_id]
                .content
                .iter()
                .filter_map(|content| match *content {
                    OpacityGroupContent::Surface {
                        slot_id,
                        bounds: surface_bounds,
                    } if scene_bounds_overlap(bounds, surface_bounds) => Some(slot_id),
                    OpacityGroupContent::Paint { .. } | OpacityGroupContent::Surface { .. } => None,
                })
                .collect::<Vec<_>>();
            for slot_id in conflicting_slots {
                self.remember_placement_issue(
                    slot_id,
                    SurfacePlacementIssue::IsolatedOpacityPaintOverlap { operation },
                );
            }
            self.opacity_groups[group_id]
                .content
                .push(OpacityGroupContent::Paint { operation, bounds });
        }
    }

    fn remember_placement_issue(
        &mut self,
        slot_id: ExternalSurfaceSlotId,
        issue: SurfacePlacementIssue,
    ) {
        if let Some(placement) = self.placements.get_mut(&slot_id) {
            if placement.issue.is_none() {
                placement.issue = Some(issue);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ClipRegion {
    Unbounded,
    Rect(LayoutRect),
    Empty,
}

fn intersect_layout_rects(first: LayoutRect, second: LayoutRect) -> Option<LayoutRect> {
    let left = first.x().max(second.x());
    let top = first.y().max(second.y());
    let right = first.right().min(second.right());
    let bottom = first.bottom().min(second.bottom());
    (right > left && bottom > top).then(|| LayoutRect::new(left, top, right - left, bottom - top))
}

fn scene_bounds_overlap(first: ScenePaintBounds, second: ScenePaintBounds) -> bool {
    match (first, second) {
        (ScenePaintBounds::Invisible, _) | (_, ScenePaintBounds::Invisible) => false,
        (ScenePaintBounds::Visible(first), ScenePaintBounds::Visible(second)) => {
            first.x() < second.right()
                && first.right() > second.x()
                && first.y() < second.bottom()
                && first.bottom() > second.y()
        }
        (ScenePaintBounds::Unknown, _) | (_, ScenePaintBounds::Unknown) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_render::{Color, Fill, RenderLayer};

    fn bounds() -> LayoutRect {
        LayoutRect::new(0.0, 0.0, 20.0, 20.0)
    }

    fn surface_op(slot_id: ExternalSurfaceSlotId, bounds: LayoutRect) -> DisplayOp {
        DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: slot_id.0,
            position: 0,
            bounds,
            node_id: None,
        }
    }

    fn surface(slot_id: ExternalSurfaceSlotId, bounds: LayoutRect) -> RenderNode {
        let mut list = DisplayList::new(bounds);
        list.push(surface_op(slot_id, bounds));
        RenderNode::Paint(list)
    }

    fn paint_op(bounds: LayoutRect) -> DisplayOp {
        DisplayOp::DrawRect {
            rect: bounds,
            fill: Some(Fill::Solid(Color {
                r: 20,
                g: 40,
                b: 60,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds,
            node_id: None,
        }
    }

    fn paint(bounds: LayoutRect) -> RenderNode {
        let mut list = DisplayList::new(bounds);
        list.push(paint_op(bounds));
        RenderNode::Paint(list)
    }

    fn opacity_layer(opacity: f32, children: Vec<RenderNode>) -> RenderNode {
        let mut layer = RenderLayer::new(LayoutRect::new(0.0, 0.0, 200.0, 200.0));
        layer.style.opacity = opacity;
        layer.children = children;
        RenderNode::Layer(layer)
    }

    fn scene(root: RenderNode) -> RenderScene {
        let mut scene = RenderScene::new(LayoutRect::new(0.0, 0.0, 200.0, 200.0));
        scene.roots.push(root);
        scene
    }

    #[test]
    fn fully_clipped_and_transparent_paint_is_definitely_invisible() {
        let clipped = PlacementState {
            clip: ClipRegion::Empty,
            ..PlacementState::default()
        };
        let transparent = PlacementState {
            opacity: 0.0,
            ..PlacementState::default()
        };

        assert_eq!(
            clipped.resolve_paint_bounds(bounds()),
            ScenePaintBounds::Invisible
        );
        assert_eq!(
            transparent.resolve_paint_bounds(bounds()),
            ScenePaintBounds::Invisible
        );
    }

    #[test]
    fn malformed_placement_remains_conservatively_unknown() {
        let malformed = PlacementState {
            issue: Some(SurfacePlacementIssue::NonAffineTransform),
            ..PlacementState::default()
        };

        assert_eq!(
            malformed.resolve_paint_bounds(bounds()),
            ScenePaintBounds::Unknown
        );
    }

    #[test]
    fn isolated_opacity_flattens_for_a_lone_surface() {
        let slot_id = ExternalSurfaceSlotId(10);
        let nested = opacity_layer(
            0.5,
            vec![opacity_layer(
                0.4,
                vec![surface(slot_id, LayoutRect::new(10.0, 10.0, 50.0, 50.0))],
            )],
        );

        let placement = collect_surface_placements(&scene(nested)).placements[&slot_id];

        assert_eq!(placement.issue, None);
        assert!((placement.opacity - 0.2).abs() <= f32::EPSILON);
        let geometry = placement
            .native_view_geometry(WidgetId::explicit("opacity.lone"), slot_id)
            .unwrap()
            .unwrap();
        assert!((geometry.opacity - 0.2).abs() <= f32::EPSILON);
    }

    #[test]
    fn isolated_opacity_rejects_overlapping_paint_in_either_order() {
        let slot_id = ExternalSurfaceSlotId(11);
        let overlap = LayoutRect::new(10.0, 10.0, 50.0, 50.0);

        for children in [
            vec![paint(overlap), surface(slot_id, overlap)],
            vec![surface(slot_id, overlap), paint(overlap)],
        ] {
            let placement = collect_surface_placements(&scene(opacity_layer(0.5, children)))
                .placements[&slot_id];
            assert_eq!(
                placement.issue,
                Some(SurfacePlacementIssue::IsolatedOpacityPaintOverlap {
                    operation: DisplayOpKind::DrawRect,
                })
            );

            let error = placement
                .native_view_geometry(WidgetId::explicit("opacity.overlap"), slot_id)
                .unwrap_err();
            assert!(matches!(
                &error,
                FrameSubmissionError::UnsupportedNativeViewPlacement {
                    widget_id,
                    slot_id: rejected_slot,
                    issue: SurfacePlacementIssue::IsolatedOpacityPaintOverlap {
                        operation: DisplayOpKind::DrawRect,
                    },
                } if *widget_id == WidgetId::explicit("opacity.overlap")
                    && *rejected_slot == slot_id
            ));
            assert!(error.to_string().contains("isolated opacity group"));
            assert!(error.to_string().contains("DrawRect"));
        }
    }

    #[test]
    fn ancestor_isolated_opacity_detects_overlap_across_nested_siblings() {
        let slot_id = ExternalSurfaceSlotId(12);
        let overlap = LayoutRect::new(10.0, 10.0, 50.0, 50.0);
        let root = opacity_layer(
            0.5,
            vec![
                opacity_layer(0.4, vec![surface(slot_id, overlap)]),
                paint(overlap),
            ],
        );

        let placement = collect_surface_placements(&scene(root)).placements[&slot_id];

        assert_eq!(
            placement.issue,
            Some(SurfacePlacementIssue::IsolatedOpacityPaintOverlap {
                operation: DisplayOpKind::DrawRect,
            })
        );
    }

    #[test]
    fn display_list_opacity_scope_detects_overlap_and_ends_at_restore() {
        let slot_id = ExternalSurfaceSlotId(16);
        let overlap = LayoutRect::new(10.0, 10.0, 50.0, 50.0);
        let mut overlapping = DisplayList::new(overlap);
        overlapping.push(DisplayOp::Save);
        overlapping.push(DisplayOp::OpacityLayer {
            alpha: 0.9995,
            bounds: overlap,
        });
        overlapping.push(surface_op(slot_id, overlap));
        overlapping.push(paint_op(overlap));
        overlapping.push(DisplayOp::Restore);

        let placement = collect_surface_placements(&RenderScene::from_display_list(overlapping))
            .placements[&slot_id];
        assert_eq!(
            placement.issue,
            Some(SurfacePlacementIssue::IsolatedOpacityPaintOverlap {
                operation: DisplayOpKind::DrawRect,
            })
        );

        let mut outside = DisplayList::new(overlap);
        outside.push(DisplayOp::Save);
        outside.push(DisplayOp::OpacityLayer {
            alpha: 0.5,
            bounds: overlap,
        });
        outside.push(surface_op(slot_id, overlap));
        outside.push(DisplayOp::Restore);
        outside.push(paint_op(overlap));

        let placement = collect_surface_placements(&RenderScene::from_display_list(outside))
            .placements[&slot_id];
        assert_eq!(placement.issue, None);
    }

    #[test]
    fn isolated_opacity_rejects_each_overlapping_surface() {
        let first_slot = ExternalSurfaceSlotId(13);
        let second_slot = ExternalSurfaceSlotId(14);
        let overlap = LayoutRect::new(10.0, 10.0, 50.0, 50.0);
        let collected = collect_surface_placements(&scene(opacity_layer(
            0.5,
            vec![surface(first_slot, overlap), surface(second_slot, overlap)],
        )));

        assert_eq!(
            collected.placements[&first_slot].issue,
            Some(SurfacePlacementIssue::IsolatedOpacitySurfaceOverlap {
                other_slot_id: second_slot,
            })
        );
        assert_eq!(
            collected.placements[&second_slot].issue,
            Some(SurfacePlacementIssue::IsolatedOpacitySurfaceOverlap {
                other_slot_id: first_slot,
            })
        );
    }

    #[test]
    fn nonoverlapping_clipped_and_transparent_siblings_do_not_block_opacity() {
        let slot_id = ExternalSurfaceSlotId(15);
        let surface_bounds = LayoutRect::new(10.0, 10.0, 50.0, 50.0);
        let nonoverlap = LayoutRect::new(100.0, 100.0, 40.0, 40.0);

        let mut clipped_paint = DisplayList::new(surface_bounds);
        clipped_paint.push(DisplayOp::Save);
        clipped_paint.push(DisplayOp::ClipRect(nonoverlap));
        clipped_paint.push(paint_op(surface_bounds));
        clipped_paint.push(DisplayOp::Restore);

        let root = opacity_layer(
            0.5,
            vec![
                surface(slot_id, surface_bounds),
                paint(nonoverlap),
                RenderNode::Paint(clipped_paint),
                opacity_layer(0.0, vec![paint(surface_bounds)]),
            ],
        );

        let placement = collect_surface_placements(&scene(root)).placements[&slot_id];

        assert_eq!(placement.issue, None);
        assert!((placement.opacity - 0.5).abs() <= f32::EPSILON);
    }
}
