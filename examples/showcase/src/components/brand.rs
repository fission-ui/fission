use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::op::{AlignItems, Fill};
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Debug)] pub(super) Brand<ShowcaseState> {}
    |_component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let navigate = with_reducer!(cx.build, Navigate("/".into()), on_navigate);
    } =>
        Pressable(Row(
            children: [
                Icon::svg(include_str!(
                    "../../../../documentation/static/img/fission-mark.svg"
                ))
                .size(tokens.sizing.icon_lg),
                Text::new(TextContent::Key("showcase.app.title".into()))
                    .size(tokens.typography.font_size_base)
                    .weight(tokens.typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
            ],
            gap: tokens.spacing.s,
            align_items: AlignItems::Center,
        ))
        .id(WidgetId::explicit("showcase.nav.brand"))
        .on_press(navigate)
        .role(PressableRole::Link)
        .label(cx.view.env().tr("showcase.app.title"))
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
}
