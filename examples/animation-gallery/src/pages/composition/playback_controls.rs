use crate::state::{reset_timeline, AnimationGalleryState, ResetTimeline, TogglePlay};
use crate::style::{color, BORDER};
use crate::ui;
use crate::widgets::common::{PolicyControl, TimelineControl};
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub(super) struct PlaybackControls<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PlaybackControls<'_>> for Widget {
    fn from(controls: PlaybackControls<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ui::SectionTitle { title: "Playback" },
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: if controls.state.playing {
                                "Pause"
                            } else {
                                "Play"
                            },
                            action: TogglePlay,
                            reducer: crate::state::toggle_play,
                        },
                        ui::SmallButton {
                            ctx: controls.ctx,
                            label: "Reset",
                            action: ResetTimeline,
                            reducer: reset_timeline,
                        },
                        TimelineControl {
                            ctx: controls.ctx,
                            state: controls.state,
                        },
                    ],
                    ..Default::default()
                },
                PolicyControl {
                    ctx: controls.ctx,
                    state: controls.state,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(color(249, 251, 255, 255))
        .into()
    }
}
