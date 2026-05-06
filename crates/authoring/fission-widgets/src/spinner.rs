use crate::stack::HStack;
use fission_core::ui::{Composite, Container, Node};
use fission_core::{
    AnimationPropertyId, AnimationRequest, AnimationStartValue, BuildCtx, View, Widget,
    WidgetNodeId,
};
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
    pub id: WidgetNodeId,
    pub color: Option<fission_core::op::Color>,
    #[serde(default = "spinner_default_animated")]
    pub animated: bool,
}

impl<S: fission_core::AppState> Widget<S> for Spinner {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let color = self.color.unwrap_or(tokens.colors.primary);
        let dot_size = 10.0;

        let mut dots = Vec::new();

        for i in 0..3 {
            // Generate stable sub-ID for animation
            // Hacking a sub-ID by XORing? Or using a deterministic derivation if available.
            // WidgetNodeId doesn't expose derivation.
            // But we can create a new explicit one if we assume `id` is unique.
            // Or hash it.
            // Let's assume we can construct one.
            let sub_id_u128 = self.id.as_u128() ^ (i as u128 + 1);
            let sub_id = WidgetNodeId::from_u128(sub_id_u128);

            let dot = Container::new(fission_core::ui::Row::default().into())
                .size(dot_size, dot_size)
                .bg(color)
                .border_radius(dot_size / 2.0)
                .into_node();
            let boundary = Composite::new(dot).repaint_boundary(true).into_node();

            let node = if self.animated {
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
                    .into_node()
            } else {
                boundary
            };
            dots.push(node);
        }

        HStack {
            spacing: Some(6.0),
            children: dots,
        }
        .build(ctx, view)
    }
}

const fn spinner_default_animated() -> bool {
    true
}
