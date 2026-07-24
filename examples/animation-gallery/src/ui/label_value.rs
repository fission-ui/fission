use crate::layout::LABEL_COLUMN_WIDTH;
use crate::state::AnimationGalleryState;
use crate::style::{INK, MUTED};
use fission::prelude::*;

pub struct LabelValue<'a> {
    pub label: &'a str,
    pub value: &'a str,
}

impl From<LabelValue<'_>> for Widget {
    fn from(value: LabelValue<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Row {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Container::new(
                    Text::new(value.label)
                        .size(tokens.typography.font_size_xs)
                        .color(MUTED),
                )
                .width(LABEL_COLUMN_WIDTH),
                Text::new(value.value)
                    .size(tokens.typography.font_size_xs)
                    .color(INK),
            ],
            ..Default::default()
        }
        .into()
    }
}
