use super::PropertyCase;
use crate::state::AnimationGalleryState;
use crate::style;
use crate::ui::*;
use crate::widgets::common::CurrentValues;
use fission::prelude::*;

fission_component! {
    pub(super) PropertyInfoPanel<AnimationGalleryState> { property: PropertyCase }
    |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    SectionTitle("Property Info".to_string()),
                    LabelValue("Name".to_string(), panel.property.property_name.to_string()),
                    LabelValue("Type".to_string(), panel.property.value_type.to_string()),
                    LabelValue("Phase".to_string(), panel.property.phase.to_string()),
                    LabelValue("Layout".to_string(), panel.property.layout.to_string()),
                    LabelValue("Paint".to_string(), panel.property.paint.to_string()),
                    LabelValue("Reduced".to_string(), panel.property.reduced.to_string()),
                    CurrentValues(),
                    PageNote("Notes".to_string(), panel.property.notes.to_string()),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
