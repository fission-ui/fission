use super::*;
use crate::style;

fission_component! {
    pub(super) NavGroupView<AnimationGalleryState> { group: &'static NavGroup }
    |group, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.xs,
            children: [
                Text(group.group.title)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_muted()),
                for item in group.group.items.iter() {
                    NavButton(item, key: item.path)
                },
            ],
        )
}
