use fission_core::ui::{Composite, Node};
use fission_core::{
    AnimationPropertyId, AnimationRequest, AnimationStartValue, BuildCtx, View, Widget,
    WidgetNodeId,
};

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
        ctx.request_animation_for(
            self.id,
            AnimationRequest {
                property: self.property.clone(),
                from: AnimationStartValue::Current, // Always animate from current visual state
                to: self.value,
                duration_ms: self.duration,
                delay_ms: self.delay,
                repeat: false,
                frame_interval_ms: None,
                easing: Default::default(),
            },
        );

        let composite = Composite::new(*self.child.clone()).repaint_boundary(true);

        match self.property {
            AnimationPropertyId::Opacity => {
                composite.animated_opacity(self.id, self.value).into_node()
            }
            AnimationPropertyId::TranslateX => composite
                .animated_translate_x(self.id, self.value)
                .into_node(),
            AnimationPropertyId::TranslateY => composite
                .animated_translate_y(self.id, self.value)
                .into_node(),
            AnimationPropertyId::Scale => composite.animated_scale(self.id, self.value).into_node(),
            AnimationPropertyId::Rotation => {
                composite.animated_rotation(self.id, self.value).into_node()
            }
            AnimationPropertyId::Custom(_) => *self.child.clone(),
        }
    }
}
