use fission_core::motion::{
    scalar, MotionDeclaration, MotionDeclarationKind, MotionEasing, MotionPhase, MotionPropertyId,
    MotionStartValue, MotionTrack, MotionTransition,
};
use fission_core::op::Color;
use fission_core::ui::{Composite, Container, Widget};
use fission_core::WidgetId;
use serde::{Deserialize, Serialize};

const LOW_PRIORITY_REPEAT_FRAME_MS: u64 = 166;

/// A placeholder shimmer rectangle used as a loading indicator.
///
/// Animates opacity between 0.4 and 0.8 in an 800ms repeating loop, creating
/// a subtle pulsing effect. Use `circle: true` for a fully rounded skeleton
/// (e.g., avatar placeholder).
///
/// # Fields
///
/// * `id` - Stable widget identity (required for animation state).
/// * `width` - Rectangle width (default 100).
/// * `height` - Rectangle height (default 20).
/// * `circle` - If `true`, uses `border_radius: 9999` for a circular shape.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skeleton {
    pub id: WidgetId,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub circle: bool,
    /// Optional explicit skeleton motion. `None` emits no skeleton-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<SkeletonMotion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Skeleton`].
///
/// Skeletons are static unless [`Skeleton::motion`] is set. Use
/// [`SkeletonMotion::Default`] or [`SkeletonMotion::Pulse`] for the standard
/// loading pulse.
pub enum SkeletonMotion {
    /// Curated default skeleton motion.
    Default,
    /// Repeating opacity pulse.
    Pulse,
    /// Ordered composition of skeleton motion atoms.
    Composition(Vec<SkeletonMotion>),
    /// Caller-provided tracks for the skeleton surface.
    Custom {
        /// Tracks applied to the skeleton root.
        tracks: Vec<MotionTrack>,
    },
}

impl SkeletonMotion {
    /// Creates an ordered skeleton-motion composition.
    pub fn compose(items: impl IntoIterator<Item = Self>) -> Self {
        Self::Composition(items.into_iter().collect())
    }

    fn tracks(&self) -> Vec<MotionTrack> {
        match self {
            Self::Default | Self::Pulse => vec![skeleton_pulse_track()],
            Self::Composition(items) => items.iter().flat_map(Self::tracks).collect(),
            Self::Custom { tracks } => tracks.clone(),
        }
    }
}

impl From<Skeleton> for Widget {
    fn from(component: Skeleton) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let base: Widget = Container::new(fission_core::ui::widgets::Spacer::default())
            .width(this.width.unwrap_or(100.0))
            .height(this.height.unwrap_or(20.0))
            .bg(Color {
                r: 200,
                g: 200,
                b: 200,
                a: (0.8 * 255.0) as u8,
            })
            .border_radius(if this.circle {
                9999.0
            } else {
                tokens.radii.small
            })
            .into();
        let boundary = Composite::new(base).repaint_boundary(true).into();

        if let Some(motion) = &this.motion {
            ctx.register_motion(MotionDeclaration {
                id: this.id,
                kind: MotionDeclarationKind::Tracks {
                    tracks: motion.tracks(),
                },
            });
            Composite::new(boundary).motion_opacity(this.id, 0.4).into()
        } else {
            boundary
        }
    }
}

fn skeleton_pulse_track() -> MotionTrack {
    MotionTrack {
        property: MotionPropertyId::Opacity,
        phase: MotionPhase::Composite,
        from: MotionStartValue::Explicit(scalar(0.4)),
        to: scalar(0.8),
        transition: MotionTransition::tween(800, MotionEasing::EaseInOut)
            .repeat(true)
            .frame_interval_ms(Some(LOW_PRIORITY_REPEAT_FRAME_MS)),
    }
}
