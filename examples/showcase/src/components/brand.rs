use crate::i18n::message;
use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::op::{AlignItems, Fill};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct Brand;

impl From<Brand> for Widget {
    fn from(_component: Brand) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let navigate = with_reducer!(ctx, Navigate("/".into()), on_navigate);

        Pressable::new(Row {
            children: widgets![
                Icon::svg(include_str!(
                    "../../../../documentation/static/img/fission-mark.svg"
                ))
                .size(tokens.spacing.l),
                Column {
                    children: widgets![
                        Text::new("Fission")
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.text_muted),
                        Text::new(TextContent::Key("showcase.app.title".into()))
                            .size(tokens.typography.font_size_lg)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading),
                    ],
                    gap: Some(tokens.spacing.none),
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.s),
            align_items: AlignItems::Center,
            ..Default::default()
        })
        .id(WidgetId::explicit("showcase.nav.brand"))
        .on_press(navigate)
        .role(PressableRole::Link)
        .label(message(view.env(), "showcase.app.title"))
        .semantics_identifier("showcase.nav.home")
        .style(PressableStyle {
            padding: Some(Length::all(Length::points(tokens.spacing.xs))),
            corner_radius: Some(tokens.radii.medium),
            ..Default::default()
        })
        .hover(PressableStyle {
            background: Some(Fill::Solid(tokens.colors.primary_subtle)),
            ..Default::default()
        })
        .into()
    }
}
