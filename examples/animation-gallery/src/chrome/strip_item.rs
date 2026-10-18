use super::*;
use crate::style;

fission_component! {
    pub(super) StripItem<AnimationGalleryState> {
        title: &'static str,
        body: &'static str,
    } |item, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.xs,
            children: [
                Text(item.title)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_primary()),
                Text(item.body)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_muted()),
            ],
        )
}
