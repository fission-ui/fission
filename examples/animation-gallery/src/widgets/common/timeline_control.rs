use crate::state::{scrub_timeline, AnimationGalleryState, ScrubTimeline};
use crate::style::MUTED;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

const TIMELINE_END_MS: u16 = 300;
const TIMELINE_MIDPOINT_MS: u16 = 150;

pub struct TimelineControl<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<TimelineControl<'_>> for Widget {
    fn from(control: TimelineControl<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(tokens.spacing.s),
            children: widgets![
                Text::new(format!("{}ms", control.state.scrub_ms))
                    .size(tokens.typography.font_size_sm)
                    .color(MUTED),
                Slider {
                    value: control.state.scrub_ms as f32,
                    min: 0.0,
                    max: TIMELINE_END_MS as f32,
                    on_change: Some(control.ctx.bind(
                        ScrubTimeline(control.state.scrub_ms),
                        reduce_with!(scrub_timeline),
                    )),
                    ..Default::default()
                },
                ui::SmallButton {
                    ctx: control.ctx,
                    label: "0",
                    action: ScrubTimeline(0),
                    reducer: scrub_timeline,
                },
                ui::SmallButton {
                    ctx: control.ctx,
                    label: "150",
                    action: ScrubTimeline(TIMELINE_MIDPOINT_MS),
                    reducer: scrub_timeline,
                },
                ui::SmallButton {
                    ctx: control.ctx,
                    label: "300",
                    action: ScrubTimeline(TIMELINE_END_MS),
                    reducer: scrub_timeline,
                },
            ],
        }
        .into()
    }
}
