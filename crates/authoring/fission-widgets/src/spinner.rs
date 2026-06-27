use crate::motion_support::{slot_id, SLOT_INDICATOR};
use crate::stack::HStack;
use fission_core::motion::{
    scalar, MotionDeclaration, MotionDeclarationKind, MotionEasing, MotionPhase, MotionPropertyId,
    MotionStartValue, MotionTrack, MotionTransition,
};
use fission_core::ui::{Composite, Container, Widget};
use fission_core::WidgetId;
use serde::{Deserialize, Serialize};

const LOW_PRIORITY_REPEAT_FRAME_MS: u64 = 166;

/// A three-dot loading indicator.
///
/// Each dot pulses between 30% and 100% opacity in a 600ms cycle, with a 200ms
/// stagger between dots, creating a wave effect. The dot color defaults to the
/// theme's primary color.
///
/// # Fields
///
/// * `id` - Stable widget identity (required for animation state).
/// * `color` - Override dot color (defaults to `tokens.colors.primary`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spinner {
    pub id: WidgetId,
    pub color: Option<fission_core::op::Color>,
    /// Optional explicit spinner motion. `None` emits no spinner-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<SpinnerMotion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Spinner`].
///
/// Spinners are static unless [`Spinner::motion`] is set. Use
/// [`SpinnerMotion::Default`] or [`SpinnerMotion::Pulse`] for the standard
/// three-dot pulse.
pub enum SpinnerMotion {
    /// Curated default spinner motion.
    Default,
    /// Repeating opacity pulse for each dot.
    Pulse,
    /// Ordered composition of spinner motion atoms.
    Composition(Vec<SpinnerMotion>),
    /// Caller-provided tracks applied to each dot.
    Custom {
        /// Tracks applied to each spinner dot.
        tracks: Vec<MotionTrack>,
    },
}

impl SpinnerMotion {
    /// Creates an ordered spinner-motion composition.
    pub fn compose(items: impl IntoIterator<Item = Self>) -> Self {
        Self::Composition(items.into_iter().collect())
    }

    fn dot_tracks(&self, delay_ms: u64) -> Vec<MotionTrack> {
        match self {
            Self::Default | Self::Pulse => vec![pulse_track(delay_ms)],
            Self::Composition(items) => items
                .iter()
                .flat_map(|item| item.dot_tracks(delay_ms))
                .collect(),
            Self::Custom { tracks } => tracks.clone(),
        }
    }
}

impl From<Spinner> for Widget {
    fn from(component: Spinner) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let color = this.color.unwrap_or(tokens.colors.primary);
        let dot_size = 10.0;

        let mut dots = Vec::new();
        let dot_motion_root = slot_id(this.id, SLOT_INDICATOR);

        for i in 0..3 {
            let dot_motion_id = WidgetId::derived(dot_motion_root.as_u128(), &[i as u32]);

            let dot: Widget = Container::new(fission_core::ui::Row::default())
                .size(dot_size, dot_size)
                .bg(color)
                .border_radius(dot_size / 2.0)
                .into();
            let boundary = Composite::new(dot).repaint_boundary(true).into();

            let node = if let Some(motion) = &this.motion {
                ctx.register_motion(MotionDeclaration {
                    id: dot_motion_id,
                    kind: MotionDeclarationKind::Tracks {
                        tracks: motion.dot_tracks(i as u64 * 200),
                    },
                });
                Composite::new(boundary)
                    .motion_opacity(dot_motion_id, 0.3)
                    .into()
            } else {
                boundary
            };
            dots.push(node);
        }

        HStack {
            spacing: Some(6.0),
            children: dots,
        }
        .into()
    }
}

fn pulse_track(delay_ms: u64) -> MotionTrack {
    MotionTrack {
        property: MotionPropertyId::Opacity,
        phase: MotionPhase::Composite,
        from: MotionStartValue::Explicit(scalar(0.3)),
        to: scalar(1.0),
        transition: MotionTransition::tween(600, MotionEasing::EaseInOut)
            .repeat(true)
            .delay_ms(delay_ms)
            .frame_interval_ms(Some(LOW_PRIORITY_REPEAT_FRAME_MS)),
    }
}
