use super::*;
use crate::style;

fission_component! {
    pub(super) StoryLabel<AnimationGalleryState> {
        step: &'static str,
        title: &'static str,
        caption: &'static str,
    } |label, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            gap: tokens.spacing.s,
            children: [
                Container(
                    Text(label.step)
                        .size(tokens.typography.font_size_xs)
                        .color(style::on_primary()),
                )
                .width(STORY_STEP_SIZE)
                .height(STORY_STEP_SIZE)
                .padding_all(tokens.spacing.s)
                .border_radius(tokens.radii.full)
                .bg(style::primary()),
                Column(
                    gap: tokens.spacing.xs,
                    children: [
                        Text(label.title)
                            .size(tokens.typography.font_size_sm)
                            .color(style::primary()),
                        Text(label.caption)
                            .size(tokens.typography.font_size_xs)
                            .color(style::text_muted()),
                    ],
                ),
            ],
        )
}
