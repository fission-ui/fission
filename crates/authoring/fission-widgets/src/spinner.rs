use crate::stack::HStack;
use fission_core::ui::{Composite, Container, Widget};
use fission_core::{AnimationPropertyId, AnimationRequest, AnimationStartValue, WidgetId};
use serde::{Deserialize, Serialize};

const LOW_PRIORITY_REPEAT_FRAME_MS: u64 = 166;

/// A three-dot animated loading indicator.
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
    #[serde(default = "spinner_default_animated")]
    pub animated: bool,
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

        for i in 0..3 {
            // Generate stable sub-ID for animation
            // Hacking a sub-ID by XORing? Or using a deterministic derivation if available.
            // WidgetId doesn't expose derivation.
            // But we can create a new explicit one if we assume `id` is unique.
            // Or hash it.
            // Let's assume we can construct one.
            let sub_id_u128 = this.id.as_u128() ^ (i as u128 + 1);
            let sub_id = WidgetId::from_u128(sub_id_u128);

            let dot: Widget = Container::new(fission_core::ui::Row::default())
                .size(dot_size, dot_size)
                .bg(color)
                .border_radius(dot_size / 2.0)
                .into();
            let boundary = Composite::new(dot).repaint_boundary(true).into();

            let node = if this.animated {
                ctx.anim_for(sub_id).request(AnimationRequest {
                    property: AnimationPropertyId::Opacity,
                    from: AnimationStartValue::Explicit(0.3),
                    to: 1.0,
                    duration_ms: 600,
                    repeat: true,
                    delay_ms: i as u64 * 200,
                    frame_interval_ms: Some(LOW_PRIORITY_REPEAT_FRAME_MS),
                    easing: Default::default(),
                });
                Composite::new(boundary)
                    .animated_opacity(sub_id, 0.3)
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

const fn spinner_default_animated() -> bool {
    true
}
