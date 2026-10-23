use crate::state::{select_policy, AnimationGalleryState, MotionPolicy, SelectPolicy};
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub PolicyControl<AnimationGalleryState> {}
    |_control, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.s,
            children: [
                Text("Motion Policy").size(tokens.typography.font_size_sm).color(style::text_muted()),
                ChoiceButton("Full".to_string(), state.policy == MotionPolicy::Full, cx.bind(SelectPolicy(MotionPolicy::Full), reduce_with!(select_policy))),
                ChoiceButton("Reduced".to_string(), state.policy == MotionPolicy::Reduced, cx.bind(SelectPolicy(MotionPolicy::Reduced), reduce_with!(select_policy))),
                ChoiceButton("Disabled".to_string(), state.policy == MotionPolicy::Disabled, cx.bind(SelectPolicy(MotionPolicy::Disabled), reduce_with!(select_policy))),
            ],
        )
}
