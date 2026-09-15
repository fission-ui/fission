//! Places a child along an SVG path, with a distance that can be animated.

use crate::motion::{MotionDeclaration, MotionDeclarationKind, MotionPropertyId, MotionTrack};
use crate::ui::widgets::composite::Composite;
use crate::ui::Widget;
use fission_ir::op::{CompositeScalar, CompositeStyle, PathOffset};
use fission_ir::WidgetId;

/// Places a child's centre on a point along an SVG path.
///
/// `path` is SVG path data in the child's own coordinate space: `(0, 0)` is the child's laid-out
/// top-left, so a child positioned at the origin of a stack follows the path exactly as drawn by a
/// [`VectorPath`](crate::ui::widgets::vector_path::VectorPath) at the same origin. `distance` is a
/// fraction of the path's arc length, from `0.0` at the start to `1.0` at the end.
///
/// Animate the distance with a composite [`MotionTrack`] on
/// [`MotionPropertyId::PathDistance`]; it runs on the compositor, so it does not rebuild the tree
/// each frame.
///
/// ```rust,ignore
/// FollowPath::new(WidgetId::explicit("pulse"), "M0 0 H120 V80", dot)
///     .rotate(true)
///     .motion(vec![MotionTrack::composite(
///         MotionPropertyId::PathDistance,
///         MotionStartValue::Explicit(scalar(0.0)),
///         scalar(1.0),
///     )
///     .transition(MotionTransition::tween(1_600, MotionEasing::Linear).repeat(true))])
/// ```
#[derive(Debug, Clone)]
pub struct FollowPath {
    /// Identity the distance animation is keyed by.
    pub id: WidgetId,
    /// SVG path data in the child's coordinate space.
    pub path: String,
    /// Fraction of the path's arc length where the child sits when not animated.
    pub distance: f32,
    /// Whether the child turns to follow the path's direction.
    pub rotate: bool,
    /// Motion tracks, typically one on [`MotionPropertyId::PathDistance`].
    pub motion: Vec<MotionTrack>,
    /// The widget moved along the path.
    pub child: Widget,
}

impl FollowPath {
    /// Places `child` at the start of `path`.
    pub fn new(id: WidgetId, path: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            id,
            path: path.into(),
            distance: 0.0,
            rotate: false,
            motion: Vec::new(),
            child: child.into(),
        }
    }

    /// Sets the resting distance along the path, as a fraction of its arc length.
    pub fn distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }

    /// Turns the child to follow the path's direction.
    pub fn rotate(mut self, rotate: bool) -> Self {
        self.rotate = rotate;
        self
    }

    /// Animates the placement, typically with a track on [`MotionPropertyId::PathDistance`].
    pub fn motion(mut self, tracks: Vec<MotionTrack>) -> Self {
        self.motion = tracks;
        self
    }
}

impl From<FollowPath> for Widget {
    fn from(component: FollowPath) -> Self {
        let animated = component
            .motion
            .iter()
            .any(|track| track.property == MotionPropertyId::PathDistance);
        if !component.motion.is_empty() {
            crate::build::try_register_motion(MotionDeclaration {
                id: component.id,
                kind: MotionDeclarationKind::Tracks {
                    tracks: component.motion,
                },
            });
        }
        let mut distance = CompositeScalar::new(component.distance);
        if animated {
            distance = distance.motion(component.id);
        }
        Composite {
            id: Some(component.id),
            style: CompositeStyle {
                path_offset: Some(PathOffset {
                    path: component.path,
                    distance,
                    rotate: component.rotate,
                }),
                ..Default::default()
            },
            child: component.child,
        }
        .into()
    }
}
