use super::*;
use crate::style;

fission_component! {
    pub(super) NavPanel<AnimationGalleryState> |_panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.m,
                children: [
                    Text("Fission")
                        .size(tokens.typography.font_size_lg)
                        .color(style::text_primary()),
                    for group in routes::NAV_GROUPS.iter() {
                        NavGroupView(group, key: group.title)
                    },
                ],
            ),
        )
        .width(NAV_PANEL_WIDTH)
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
