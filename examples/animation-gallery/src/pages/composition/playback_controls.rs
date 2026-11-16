use crate::state::{reset_timeline, toggle_play, AnimationGalleryState, ResetTimeline, TogglePlay};
use crate::style;
use crate::ui::*;
use crate::widgets::common::{PolicyControl, TimelineControl};
use fission::prelude::*;

fission_component! {
    pub(super) PlaybackControls<AnimationGalleryState> |_controls, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    SectionTitle("Playback".to_string()),
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
                    PolicyControl(),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface_sunken())
}
