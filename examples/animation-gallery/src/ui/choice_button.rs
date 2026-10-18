use crate::state::AnimationGalleryState;
use fission::prelude::*;

fission_component! {
    pub ChoiceButton<AnimationGalleryState> {
        label: String,
        active: bool,
        action: ActionEnvelope,
    }
    |button, _state, cx| {} =>
        Button(
            variant: if button.active {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Outline
            },
            child: Text(button.label.clone())
                .size(cx.view.env().theme.tokens.typography.font_size_xs),
            on_press: button.action.clone(),
        )
}
