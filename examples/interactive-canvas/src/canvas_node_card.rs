use fission::prelude::*;

pub(crate) struct CanvasNodeCard {
    pub title: String,
    pub detail: String,
}

impl From<CanvasNodeCard> for Widget {
    fn from(card: CanvasNodeCard) -> Self {
        let (_, view) = fission::build::current::<crate::state::CanvasExampleState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(card.title)
                    .size(tokens.typography.heading_size)
                    .weight(700)
                    .color(tokens.colors.text_primary),
                Text::new(card.detail)
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .bg(tokens.colors.surface_raised)
        .border(tokens.colors.border, 1.0)
        .border_radius(14.0)
        .into()
    }
}
