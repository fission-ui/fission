use fission::prelude::*;

fission_component! {
    pub(crate) CanvasNodeCard<crate::state::CanvasExampleState> {
        title: String,
        detail: String,
    } |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
            gap: tokens.spacing.xs,
            children: [
                Text(card.title)
                    .size(tokens.typography.heading_size)
                    .weight(700)
                    .color(tokens.colors.text_primary),
                Text(card.detail)
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
            ],
        ))
        .padding_all(tokens.spacing.m)
        .bg(tokens.colors.surface_raised)
        .border(tokens.colors.border, 1.0)
        .border_radius(14.0)
}
