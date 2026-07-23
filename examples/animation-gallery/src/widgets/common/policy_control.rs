use crate::state::{select_policy, AnimationGalleryState, MotionPolicy, SelectPolicy};
use crate::style::MUTED;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub struct PolicyControl<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PolicyControl<'_>> for Widget {
    fn from(control: PolicyControl<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(tokens.spacing.s),
            children: widgets![
                Text::new("Motion Policy")
                    .size(tokens.typography.font_size_sm)
                    .color(MUTED),
                ui::ChoiceButton {
                    ctx: control.ctx,
                    label: "Full",
                    active: control.state.policy == MotionPolicy::Full,
                    action: SelectPolicy(MotionPolicy::Full),
                    reducer: select_policy,
                },
                ui::ChoiceButton {
                    ctx: control.ctx,
                    label: "Reduced",
                    active: control.state.policy == MotionPolicy::Reduced,
                    action: SelectPolicy(MotionPolicy::Reduced),
                    reducer: select_policy,
                },
                ui::ChoiceButton {
                    ctx: control.ctx,
                    label: "Disabled",
                    active: control.state.policy == MotionPolicy::Disabled,
                    action: SelectPolicy(MotionPolicy::Disabled),
                    reducer: select_policy,
                },
            ],
        }
        .into()
    }
}
