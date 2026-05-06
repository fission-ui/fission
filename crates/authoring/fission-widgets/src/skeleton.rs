use fission_core::op::Color;
use fission_core::ui::{Composite, Container, Node};
use fission_core::{
    AnimationPropertyId, AnimationRequest, AnimationStartValue, BuildCtx, View, Widget,
    WidgetNodeId,
};
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
    pub id: WidgetNodeId,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub circle: bool,
    #[serde(default = "skeleton_default_animated")]
    pub animated: bool,
}

impl<S: fission_core::AppState> Widget<S> for Skeleton {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;

        let base = Container::new(fission_core::ui::widgets::Spacer::default().into_node())
            .width(self.width.unwrap_or(100.0))
            .height(self.height.unwrap_or(20.0))
            .bg(Color {
                r: 200,
                g: 200,
                b: 200,
                a: (0.8 * 255.0) as u8,
            })
            .border_radius(if self.circle {
                9999.0
            } else {
                tokens.radii.small
            })
            .into_node();
        let boundary = Composite::new(base).repaint_boundary(true).into_node();

        if self.animated {
            ctx.anim_for(self.id).request(AnimationRequest {
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
                .animated_opacity(self.id, 0.4)
                .into_node()
        } else {
            boundary
        }
    }
}

const fn skeleton_default_animated() -> bool {
    true
}
