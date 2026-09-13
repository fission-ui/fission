use super::*;
use crate::style;

pub(super) struct StripItem<'a> {
    pub(super) title: &'a str,
    pub(super) body: &'a str,
}

impl From<StripItem<'_>> for Widget {
    fn from(item: StripItem<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Column {
            gap: Some(tokens.spacing.xs),
            children: vec![
                Text::new(item.title)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_primary())
                    .into(),
                Text::new(item.body)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_muted())
                    .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
