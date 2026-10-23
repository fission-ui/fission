use crate::state::AnimationGalleryState;
use fission::prelude::*;

fission_component! {
    pub SmallButton<AnimationGalleryState> { label: String, action: ActionEnvelope }
    |button, _state, cx| {} =>
        Button(
            variant: ButtonVariant::Outline,
            child: Text(button.label.clone())
                .size(cx.view.env().theme.tokens.typography.font_size_xs),
            on_press: button.action.clone(),
        ).semantics_identifier(format!(
            "animation-gallery.action.{}",
            button.label.to_ascii_lowercase().replace(' ', "-"),
        ))
}
