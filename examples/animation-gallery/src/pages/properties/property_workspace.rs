use super::property_preview::PropertyPreview;
use super::PropertyCase;
use crate::state::{reset_timeline, toggle_play, AnimationGalleryState, ResetTimeline, TogglePlay};
use crate::style;
use crate::ui::*;
use crate::widgets::common::TimelineControl;
use fission::prelude::*;

fission_component! {
    pub(super) PropertyWorkspace<AnimationGalleryState> { property: PropertyCase }
    |workspace, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Row(
                        gap: tokens.spacing.s,
                        children: [
                            SmallButton(
                                (if state.playing { "Pause" } else { "Play" }).to_string(),
                                cx.bind(TogglePlay, reduce_with!(toggle_play)),
                            ),
                            SmallButton(
                                "Reset".to_string(),
                                cx.bind(ResetTimeline, reduce_with!(reset_timeline)),
                            ),
                            TimelineControl(),
                        ],
                    ),
                    PropertyPreview(workspace.property.clone()),
                    CodeBlock(workspace.property.track_source.to_string()),
                ],
            ),
        )
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
