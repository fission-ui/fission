use crate::state::{navigate_to, AnimationGalleryState, NavigateTo};
use crate::style;
use crate::ui::*;
use crate::widgets;
use fission::prelude::*;

const GLYPH_HEIGHT: f32 = 64.0;

fission_component! {
    pub(super) OverviewCard<AnimationGalleryState> {
        summary: widgets::common::WidgetSummary,
    } |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            on_press: cx.bind(
                NavigateTo(card.summary.path.to_string()),
                reduce_with!(navigate_to),
            ),
            child: Container(
                Column(
                    gap: tokens.spacing.s,
                    children: [
                        Container(
                            Text(card.summary.glyph)
                                .size(tokens.typography.body_large_size)
                                .color(style::primary()),
                        )
                        .height(GLYPH_HEIGHT)
                        .padding_all(tokens.spacing.m)
                        .border_radius(tokens.radii.xl)
                        .bg(card.summary.tint.color())
                        .border(style::border_strong(), 1.0),
                        Text(card.summary.title)
                            .size(tokens.typography.body_large_size)
                            .color(style::text_primary()),
                        Text(card.summary.subtitle)
                            .size(tokens.typography.font_size_sm)
                            .color(style::text_muted()),
                        ColorDots(vec![
                            style::success(),
                            style::secondary(),
                            style::primary(),
                            style::info(),
                        ]),
                    ],
                ),
            )
            .padding_all(tokens.spacing.s)
            .width_length(Length::percent(100.0))
            .border(style::border(), 1.0)
            .border_radius(tokens.radii.xl)
            .bg(style::surface()),
        )
}
