use super::{PolicyControl, TimelineControl};
use crate::state::{
    motion_label, open_composer, reset_timeline, select_motion, toggle_play, AnimationGalleryState,
    MotionChoice, OpenComposer, ResetTimeline, SelectMotion, TogglePlay,
};
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub ControlsPanel<AnimationGalleryState> { motions: Vec<MotionChoice> }
    |panel, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    SectionTitle("Controls".to_string()),
                    Wrap(direction: FlexDirection::Row, spacing: tokens.spacing.s, children: [
                        for motion in panel.motions.iter().copied() {
                            ChoiceButton(
                                motion_label(motion).to_string(),
                                state.motion == motion,
                                cx.bind(SelectMotion(motion), reduce_with!(select_motion)),
                                key: motion_label(motion),
                            )
                        }
                        SmallButton("Compose...".to_string(), cx.bind(OpenComposer, reduce_with!(open_composer))),
                    ]),
                    Wrap(
                        direction: FlexDirection::Row,
                        spacing: tokens.spacing.s,
                        children: [
                            SmallButton(
                            (if state.playing { "Pause" } else { "Play" }).to_string(),
                                cx.bind(TogglePlay, reduce_with!(toggle_play)),
                            ),
                            SmallButton("Reset".to_string(), cx.bind(ResetTimeline, reduce_with!(reset_timeline))),
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
        .bg(style::surface())
}
