use crate::layout::LABEL_DOT_SIZE;
use crate::state::AnimationGalleryState;
use crate::style::{MUTED, TEAL};
use fission::prelude::*;

pub struct LabelRow<'a> {
    pub label: &'a str,
}

impl From<LabelRow<'_>> for Widget {
    fn from(row: LabelRow<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Row {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Container::new(Text::new(" "))
                    .width(LABEL_DOT_SIZE)
                    .height(LABEL_DOT_SIZE)
                    .border_radius(LABEL_DOT_SIZE / 2.0)
                    .bg(TEAL),
                Text::new(row.label)
                    .size(tokens.typography.font_size_xs)
                    .color(MUTED),
            ],
            ..Default::default()
        }
        .into()
    }
}
