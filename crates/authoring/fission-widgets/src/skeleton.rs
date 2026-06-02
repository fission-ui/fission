use fission_core::op::Color;
use fission_core::ui::{Composite, Container, Widget};
use fission_core::{AnimationPropertyId, AnimationRequest, AnimationStartValue, WidgetId};
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
    #[serde(default = "skeleton_default_animated")]
    pub animated: bool,
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

        if this.animated {
            ctx.anim_for(this.id).request(AnimationRequest {
                property: AnimationPropertyId::Opacity,
                from: AnimationStartValue::Explicit(0.4),
                to: 0.8,
                duration_ms: 800,
                repeat: true,
                delay_ms: 0,
                frame_interval_ms: Some(LOW_PRIORITY_REPEAT_FRAME_MS),
                easing: Default::default(),
            });
            Composite::new(boundary)
                .animated_opacity(this.id, 0.4)
                .into()
        } else {
            boundary
        }
    }
}

const fn skeleton_default_animated() -> bool {
    true
}
