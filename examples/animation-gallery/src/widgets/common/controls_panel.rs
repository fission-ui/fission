use super::{PolicyControl, TimelineControl};
use crate::state::{
    motion_label, open_composer, reset_timeline, select_motion, toggle_play, AnimationGalleryState,
    MotionChoice, OpenComposer, ResetTimeline, SelectMotion, TogglePlay,
};
use crate::style::{BORDER, SURFACE};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub struct ControlsPanel<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub motions: &'a [MotionChoice],
}

impl From<ControlsPanel<'_>> for Widget {
    fn from(panel: ControlsPanel<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut motion_buttons: Vec<Widget> = panel
            .motions
            .iter()
            .map(|motion| {
                ui::ChoiceButton {
                    ctx: panel.ctx,
                    label: motion_label(*motion),
                    active: panel.state.motion == *motion,
                    action: SelectMotion(*motion),
                    reducer: select_motion,
                }
                .into()
            })
            .collect();
        motion_buttons.push(
            ui::SmallButton {
                ctx: panel.ctx,
                label: "Compose...",
                action: OpenComposer,
                reducer: open_composer,
            }
            .into(),
        );

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ui::SectionTitle { title: "Controls" },
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    children: motion_buttons,
                },
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        ui::SmallButton {
                            ctx: panel.ctx,
                            label: if panel.state.playing { "Pause" } else { "Play" },
                            action: TogglePlay,
                            reducer: toggle_play,
                        },
                        ui::SmallButton {
                            ctx: panel.ctx,
                            label: "Reset",
                            action: ResetTimeline,
                            reducer: reset_timeline,
                        },
                        TimelineControl {
                            ctx: panel.ctx,
                            state: panel.state,
                        },
                    ],
                },
                PolicyControl {
                    ctx: panel.ctx,
                    state: panel.state,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
