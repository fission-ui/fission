use crate::state::{scrub_timeline, AnimationGalleryState, ScrubTimeline};
use crate::style;
use crate::ui::*;
use fission::prelude::*;

const TIMELINE_END_MS: u16 = 300;
const TIMELINE_MIDPOINT_MS: u16 = 150;

fission_component! {
    pub TimelineControl<AnimationGalleryState> {}
    |_control, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.s,
            children: [
                Text(format!("{}ms", state.scrub_ms)).size(tokens.typography.font_size_sm).color(style::text_muted()),
                Slider(
                    value: state.scrub_ms as f32,
                    min: 0.0_f32,
                    max: TIMELINE_END_MS as f32,
                    on_change: cx.bind(ScrubTimeline(state.scrub_ms), reduce_with!(scrub_timeline)),
                ),
                SmallButton("0".to_string(), cx.bind(ScrubTimeline(0), reduce_with!(scrub_timeline))),
                SmallButton("150".to_string(), cx.bind(ScrubTimeline(TIMELINE_MIDPOINT_MS), reduce_with!(scrub_timeline))),
                SmallButton("300".to_string(), cx.bind(ScrubTimeline(TIMELINE_END_MS), reduce_with!(scrub_timeline))),
            ],
        )
}
