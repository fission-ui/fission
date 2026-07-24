use crate::state::AnimationGalleryState;
use crate::style::INK;
use crate::ui;
use fission::prelude::*;

pub(super) struct InspectorGroup<'a> {
    pub title: &'a str,
    pub rows: &'a [&'a str],
}

impl From<InspectorGroup<'_>> for Widget {
    fn from(group: InspectorGroup<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut children: Vec<Widget> = widgets![Text::new(group.title)
            .size(tokens.typography.font_size_sm)
            .color(INK),];
        children.extend(group.rows.iter().map(|label| ui::LabelRow { label }.into()));

        Column {
            gap: Some(tokens.spacing.xs),
            children,
            ..Default::default()
        }
        .into()
    }
}
