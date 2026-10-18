use crate::state::AnimationGalleryState;
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub(super) MatrixCard<AnimationGalleryState> { title: &'static str, content: &'static str }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    SectionTitle(card.title.to_string()),
                    CodeBlock(card.content.to_string()),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
