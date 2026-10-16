use super::{
    animation_gallery_example::AnimationGalleryExample, chart_gallery_example::ChartGalleryExample,
    icons_gallery_example::IconsGalleryExample, inbox_example::InboxExample,
    text_lab_example::TextLabExample, todo_design_system_example::TodoDesignSystemExample,
    widget_gallery_example::WidgetGalleryExample,
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
            "icons_gallery" => IconsGalleryExample.into(),
            "inbox" => InboxExample.into(),
            "text-lab" => TextLabExample.into(),
            "todo-design-system" => TodoDesignSystemExample.into(),
            "widget-gallery" => WidgetGalleryExample.into(),
            _ => InboxExample.into(),
        }
    }
}
