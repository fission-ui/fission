use crate::state::{navigate_to, AnimationGalleryState, NavigateTo};
use crate::style::{color, BLUE, BORDER, CYAN, INK, MUTED, SURFACE, TEAL, VIOLET};
use crate::ui;
use crate::widgets;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

const GLYPH_HEIGHT: f32 = 64.0;

pub(super) struct OverviewCard<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub summary: widgets::common::WidgetSummary,
}

impl From<OverviewCard<'_>> for Widget {
    fn from(card: OverviewCard<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Button {
            variant: ButtonVariant::Ghost,
            on_press: Some(card.ctx.bind(
                NavigateTo(card.summary.path.to_string()),
                reduce_with!(navigate_to),
            )),
            child: Some(
                Container::new(Column {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        Container::new(
                            Text::new(card.summary.glyph)
                                .size(typography.body_large_size)
                                .color(BLUE),
                        )
                        .height(GLYPH_HEIGHT)
                        .padding_all(tokens.spacing.m)
                        .border_radius(tokens.radii.xl)
                        .bg(card.summary.tint)
                        .border(color(190, 204, 244, 255), 1.0),
                        Text::new(card.summary.title)
                            .size(typography.body_large_size)
                            .color(INK),
                        Text::new(card.summary.subtitle)
                            .size(typography.font_size_sm)
                            .color(MUTED),
                        ui::ColorDots {
                            colors: &[TEAL, VIOLET, BLUE, CYAN],
                        },
                    ],
                    ..Default::default()
                })
                .padding_all(tokens.spacing.s)
                .width_length(Length::percent(100.0))
                .border(BORDER, 1.0)
                .border_radius(tokens.radii.xl)
                .bg(SURFACE)
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}
