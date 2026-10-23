use super::{current_values::CurrentValues, inspector_group::InspectorGroup, GalleryCase};
use crate::state::AnimationGalleryState;
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub InspectorPanel<AnimationGalleryState> { case: GalleryCase }
    |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    SectionTitle("Inspector".to_string()),
                    InspectorGroup("Slots".to_string(), panel.case.slots.iter().map(|value| (*value).to_string()).collect()),
                    InspectorGroup("Tracks".to_string(), panel.case.tracks.iter().map(|value| (*value).to_string()).collect()),
                    InspectorGroup("MotionExpr".to_string(), panel.case.exprs.iter().map(|value| (*value).to_string()).collect()),
                    CurrentValues(),
                    PageNote("Diagnostics".to_string(), panel.case.diagnostic.to_string()),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
