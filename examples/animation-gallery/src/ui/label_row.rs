use crate::layout::LABEL_DOT_SIZE;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

fission_component! {
    pub LabelRow<AnimationGalleryState> { label: String } |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            gap: tokens.spacing.s,
            children: [
                Container(Text(" "))
                    .width(LABEL_DOT_SIZE)
                    .height(LABEL_DOT_SIZE)
                    .border_radius(LABEL_DOT_SIZE / 2.0)
                    .bg(style::success()),
                Text(row.label)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_muted()),
            ],
        )
}
