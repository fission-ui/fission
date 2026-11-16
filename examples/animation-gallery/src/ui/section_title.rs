use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

fission_component! {
    pub SectionTitle<AnimationGalleryState> { title: String } |title, _state, cx| {} =>
        Text(title.title)
            .size(cx.view.env().theme.tokens.typography.font_size_sm)
            .color(style::text_primary())
}
