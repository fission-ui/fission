use fission_core::ui::{Composite, Widget};
use fission_core::{AnimationPropertyId, AnimationRequest, AnimationStartValue, WidgetId};

#[derive(Clone, Debug)]
pub struct Transition {
    pub id: WidgetId,
    pub value: f32,
    pub property: AnimationPropertyId,
    pub duration: u64,
    pub delay: u64,
    pub child: Widget,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("transition"),
            value: 0.0,
            property: AnimationPropertyId::Opacity,
            duration: 300,
            delay: 0,
            child: fission_core::ui::widgets::Spacer::default().into(),
        }
    }
}

impl From<Transition> for Widget {
    fn from(component: Transition) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        ctx.request_animation_for(
            this.id,
            AnimationRequest {
                property: this.property.clone(),
                from: AnimationStartValue::Current, // Always animate from current visual state
                to: this.value,
                duration_ms: this.duration,
                delay_ms: this.delay,
                repeat: false,
                frame_interval_ms: None,
                easing: Default::default(),
            },
        );

        let composite = Composite::new(this.child.clone()).repaint_boundary(true);

        match this.property {
            AnimationPropertyId::Opacity => composite.animated_opacity(this.id, this.value).into(),
            AnimationPropertyId::TranslateX => {
                composite.animated_translate_x(this.id, this.value).into()
            }
            AnimationPropertyId::TranslateY => {
                composite.animated_translate_y(this.id, this.value).into()
            }
            AnimationPropertyId::Scale => composite.animated_scale(this.id, this.value).into(),
            AnimationPropertyId::Rotation => {
                composite.animated_rotation(this.id, this.value).into()
            }
            AnimationPropertyId::Custom(_) => this.child.clone(),
        }
    }
}
