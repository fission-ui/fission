//! A stroked or filled SVG path in its own box, with an animatable dash offset and trim.

use crate::authoring::{custom_widget, IrBuilder, LowerWidget, LoweringContext};
use crate::motion::{
    MotionDeclaration, MotionDeclarationKind, MotionPropertyId, MotionTrack, MotionValue,
};
use crate::ui::Widget;
use fission_ir::op::{BoxStyle, Fill, LayoutOp, Length, Op, PaintOp, Stroke, StrokeTrim};
use fission_ir::WidgetId;
use std::hash::{Hash, Hasher};

/// Draws SVG path data, filled, stroked or both, in a box of a fixed size.
///
/// The path is in the box's logical pixels unless a view box is set, in which case the view box is
/// scaled to fill the box. Connectors, arrows and diagram edges are ordinary `VectorPath`s.
///
/// Animate the stroke with [`MotionTrack`]s on [`MotionPropertyId::StrokeDashOffset`] (dashes flow
/// along the path), [`MotionPropertyId::PathTrimStart`] and [`MotionPropertyId::PathTrimEnd`] (the
/// path draws on or off). Trim values are fractions of the path's arc length.
///
/// ```rust,ignore
/// VectorPath::new("M0 0 H120 V80")
///     .id(WidgetId::explicit("edge"))
///     .size(120.0, 80.0)
///     .stroke(Stroke { dash_array: Some(vec![6.0, 4.0]), ..stroke })
///     .motion(vec![MotionTrack::paint(
///         MotionPropertyId::StrokeDashOffset,
///         MotionStartValue::Explicit(px(10.0)),
///         px(0.0),
///     )
///     .transition(MotionTransition::tween(600, MotionEasing::Linear).repeat(true))])
/// ```
#[derive(Debug, Clone)]
pub struct VectorPath {
    /// Identity motion values are keyed by. Required for motion to take effect.
    pub id: Option<WidgetId>,
    /// SVG path data.
    pub path: String,
    /// Box width in logical pixels.
    pub width: f32,
    /// Box height in logical pixels.
    pub height: f32,
    /// Size of the coordinate space the path is drawn in; `None` uses the box's pixels.
    pub view_box: Option<[f32; 2]>,
    /// Interior fill.
    pub fill: Option<Fill>,
    /// Outline, including its dash pattern, dash offset and trim.
    pub stroke: Option<Stroke>,
    /// Motion tracks for the stroke's dash offset and trim.
    pub motion: Vec<MotionTrack>,
}

impl VectorPath {
    /// A path with no fill, no stroke and a zero-sized box; set a size and a fill or stroke.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            id: None,
            path: path.into(),
            width: 0.0,
            height: 0.0,
            view_box: None,
            fill: None,
            stroke: None,
            motion: Vec::new(),
        }
    }

    /// Sets the identity motion values are keyed by.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the box size in logical pixels.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Draws the path in a `width` by `height` coordinate space scaled to the box.
    pub fn view_box(mut self, width: f32, height: f32) -> Self {
        self.view_box = Some([width, height]);
        self
    }

    /// Fills the path's interior.
    pub fn fill(mut self, fill: Fill) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Strokes the path's outline.
    pub fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// Animates the stroke's dash offset or trim. Requires an [`id`](Self::id).
    pub fn motion(mut self, tracks: Vec<MotionTrack>) -> Self {
        self.motion = tracks;
        self
    }
}

/// Applies the current dash offset and trim motion values for `id` to `stroke`.
fn animated_stroke(id: WidgetId, mut stroke: Stroke) -> Stroke {
    let Some(runtime) = crate::build::try_current_runtime_state() else {
        return stroke;
    };
    let value = |property: MotionPropertyId| {
        runtime
            .motion
            .values
            .get(&(id, property))
            .and_then(MotionValue::as_scalar_like)
    };
    if let Some(offset) = value(MotionPropertyId::StrokeDashOffset) {
        stroke.dash_offset = offset;
    }
    let start = value(MotionPropertyId::PathTrimStart);
    let end = value(MotionPropertyId::PathTrimEnd);
    if start.is_some() || end.is_some() {
        let base = stroke.trim.unwrap_or(StrokeTrim::new(0.0, 1.0));
        stroke.trim = Some(StrokeTrim::new(
            start.unwrap_or(base.start),
            end.unwrap_or(base.end),
        ));
    }
    stroke
}

impl From<VectorPath> for Widget {
    fn from(component: VectorPath) -> Self {
        let mut stroke = component.stroke;
        if let Some(id) = component.id {
            if !component.motion.is_empty() {
                crate::build::try_register_motion(MotionDeclaration {
                    id,
                    kind: MotionDeclarationKind::Tracks {
                        tracks: component.motion,
                    },
                });
            }
            stroke = stroke.map(|stroke| animated_stroke(id, stroke));
        }
        custom_widget(
            "VectorPath",
            VectorPathLowerer {
                id: component.id,
                path: component.path,
                width: component.width,
                height: component.height,
                view_box: component.view_box,
                fill: component.fill,
                stroke,
            },
        )
    }
}

/// The resolved path, with motion values already applied to its stroke.
#[derive(Debug, Clone)]
struct VectorPathLowerer {
    id: Option<WidgetId>,
    path: String,
    width: f32,
    height: f32,
    view_box: Option<[f32; 2]>,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
}

impl LowerWidget for VectorPathLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let box_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let paint = IrBuilder::new(
            WidgetId::derived(box_id.as_u128(), &[0x7A7_0001]),
            Op::Paint(PaintOp::DrawPath {
                path: self.path.clone(),
                fill: self.fill.clone(),
                stroke: self.stroke.clone(),
                // The site shell needs the coordinate space explicitly; the box size is the default.
                view_box: Some(self.view_box.unwrap_or([self.width, self.height])),
            }),
        )
        .build(cx);
        let mut layout = IrBuilder::new(
            box_id,
            Op::Layout(LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::points(self.width.max(0.0))),
                    height: Some(Length::points(self.height.max(0.0))),
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
        );
        layout.add_child(paint);
        layout.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        self.id
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.path.hash(&mut hasher);
        self.width.to_bits().hash(&mut hasher);
        self.height.to_bits().hash(&mut hasher);
        self.stroke.hash(&mut hasher);
        hasher.finish()
    }
}
