use crate::layout::LABEL_COLUMN_WIDTH;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

fission_component! {
    pub LabelValue<AnimationGalleryState> { label: String, value: String }
    |value, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            gap: tokens.spacing.s,
            children: [
                Container(
                    Text(value.label)
                        .size(tokens.typography.font_size_xs)
                        .color(style::text_muted()),
                ).width(LABEL_COLUMN_WIDTH),
                Text(value.value)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_primary()),
            ],
        )
}
