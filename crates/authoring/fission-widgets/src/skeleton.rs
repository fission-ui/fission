use fission_core::op::Color;
use fission_core::ui::{Container, Node};
use fission_core::{
    AnimationPropertyId, AnimationRequest, AnimationStartValue, BuildCtx, NodeId, View, Widget,
    WidgetNodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skeleton {
    pub id: WidgetNodeId,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub circle: bool,
}

impl<S: fission_core::AppState> Widget<S> for Skeleton {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let node_id = NodeId::derived(self.id.as_u128(), &[]);

        // Animation
        ctx.anim_for(self.id).request(AnimationRequest {
            property: AnimationPropertyId::Opacity,
            from: AnimationStartValue::Explicit(0.4),
            to: 0.8,
            duration_ms: 800,
            repeat: true,
            delay_ms: 0,
        });

        // Use animation value if available, otherwise start at the animation's
        // explicit start (0.4) to avoid a full-opacity flash on the first frame.
        let opacity = {
            let v = view.animation_value(self.id, &AnimationPropertyId::Opacity);
            if (v - AnimationPropertyId::Opacity.default_value()).abs() < 0.001 {
                0.4 // match AnimationStartValue::Explicit(0.4) above
            } else {
                v
            }
        };
        let color = Color {
            r: 200,
            g: 200,
            b: 200,
            a: (opacity * 255.0) as u8,
        };

        Container::new(fission_core::ui::widgets::Spacer::default().into_node())
            .width(self.width.unwrap_or(100.0))
            .height(self.height.unwrap_or(20.0))
            .bg(color)
            .border_radius(if self.circle {
                9999.0
            } else {
                tokens.radii.small
            })
            .into_node()
    }
}
