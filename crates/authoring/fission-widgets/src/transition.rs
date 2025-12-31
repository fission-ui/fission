use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, AnimationPropertyId, AnimationRequest, AnimationStartValue};
use fission_core::ui::Node;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Transition {
    pub id: WidgetNodeId,
    pub value: f32,
    pub property: AnimationPropertyId,
    pub duration: u64,
    pub delay: u64,
    pub child: Box<Node>,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            id: WidgetNodeId::explicit("transition"),
            value: 0.0,
            property: AnimationPropertyId::Opacity,
            duration: 300,
            delay: 0,
            child: Box::new(fission_core::ui::widgets::Spacer::default().into_node()),
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for Transition {
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        ctx.request_animation_for(self.id, AnimationRequest {
            property: self.property.clone(),
            from: AnimationStartValue::Current, // Always animate from current visual state
            to: self.value,
            duration_ms: self.duration,
            delay_ms: self.delay,
            repeat: false,
        });

        // Pass-through child
        *self.child.clone()
    }
}
