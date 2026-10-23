use super::{
    animation_gallery_example::AnimationGalleryExample, chart_gallery_example::ChartGalleryExample,
    icons_gallery_example::IconsGalleryExample, inbox_example::InboxExample,
    text_lab_example::TextLabExample, todo_design_system_example::TodoDesignSystemExample,
    widget_gallery_example::WidgetGalleryExample,
};
use crate::catalog::ExampleDefinition;
use crate::state::ShowcaseState;
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) PreviewRouter<ShowcaseState> {
        example: ExampleDefinition,
    }
    |component, _state, _cx| {} =>
        match component.example.slug {
            "animation-gallery" => AnimationGalleryExample(),
            "chart-gallery" => ChartGalleryExample(),
            "icons_gallery" => IconsGalleryExample(),
            "inbox" => InboxExample(),
            "text-lab" => TextLabExample(),
            "todo-design-system" => TodoDesignSystemExample(),
            "widget-gallery" => WidgetGalleryExample(),
            _ => InboxExample(),
        }
}
