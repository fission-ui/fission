use super::{
    animation_gallery_example::AnimationGalleryExample, chart_gallery_example::ChartGalleryExample,
    counter_example::CounterExample, embed_3d_example::Embed3dExample,
    icons_gallery_example::IconsGalleryExample, inbox_example::InboxExample,
    mobile_smoke_example::MobileSmokeExample, motion_memory_example::MotionMemoryExample,
    text_lab_example::TextLabExample, todo_design_system_example::TodoDesignSystemExample,
    web_smoke_example::WebSmokeExample, widget_gallery_example::WidgetGalleryExample,
};
use crate::catalog::ExampleDefinition;
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PreviewRouter {
    pub(crate) example: ExampleDefinition,
}

impl From<PreviewRouter> for Widget {
    fn from(component: PreviewRouter) -> Self {
        match component.example.slug {
            "animation-gallery" => AnimationGalleryExample.into(),
            "chart-gallery" => ChartGalleryExample.into(),
            "counter" => CounterExample.into(),
            "embed-3d" => Embed3dExample.into(),
            "icons_gallery" => IconsGalleryExample.into(),
            "inbox" => InboxExample.into(),
            "mobile-smoke" => MobileSmokeExample.into(),
            "motion-memory-repro" => MotionMemoryExample.into(),
            "text-lab" => TextLabExample.into(),
            "todo-design-system" => TodoDesignSystemExample.into(),
            "web-smoke" => WebSmokeExample.into(),
            "widget-gallery" => WidgetGalleryExample.into(),
            _ => CounterExample.into(),
        }
    }
}
