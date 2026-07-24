use super::property_preview::PropertyPreview;
use super::PropertyCase;
use crate::state::{reset_timeline, toggle_play, AnimationGalleryState, ResetTimeline, TogglePlay};
use crate::style::{BORDER, SURFACE};
use crate::ui;
use crate::widgets::common::TimelineControl;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub(super) struct PropertyWorkspace<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub property: &'a PropertyCase,
}

impl From<PropertyWorkspace<'_>> for Widget {
    fn from(workspace: PropertyWorkspace<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        ui::SmallButton {
                            ctx: workspace.ctx,
                            label: if workspace.state.playing {
                                "Pause"
                            } else {
                                "Play"
                            },
                            action: TogglePlay,
                            reducer: toggle_play,
                        },
                        ui::SmallButton {
                            ctx: workspace.ctx,
                            label: "Reset",
                            action: ResetTimeline,
                            reducer: reset_timeline,
                        },
                        TimelineControl {
                            ctx: workspace.ctx,
                            state: workspace.state,
                        },
                    ],
                    ..Default::default()
                },
                PropertyPreview {
                    property: workspace.property,
                    state: workspace.state,
                },
                ui::CodeBlock {
                    source: workspace.property.track_source,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
