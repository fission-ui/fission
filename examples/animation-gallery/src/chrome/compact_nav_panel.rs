use super::*;
use crate::style;

const SECONDARY_ROUTES: [(&str, &str); 4] = [
    ("Opacity", "/properties/opacity"),
    ("Composition", "/composition/additive"),
    ("Policy", "/policy/full"),
    ("Diagnostics", "/diagnostics/declarations"),
];

fission_component! {
    pub(super) CompactNavPanel<AnimationGalleryState> |_panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text("Routes")
                        .size(tokens.typography.font_size_xs)
                        .color(style::text_muted()),
                    Wrap(
                        direction: FlexDirection::Row,
                        spacing: tokens.spacing.s,
                        children: [
                            NavChip("Overview", routes::OVERVIEW),
                            for item in routes::WIDGET_ROUTES.iter() {
                                NavChip(item.label, item.path, key: item.path)
                            },
                            for (label, path) in SECONDARY_ROUTES {
                                NavChip(label, path, key: path)
                            },
                        ],
                    ),
                ],
            ),
        )
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
