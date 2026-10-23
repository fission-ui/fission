use super::*;
use crate::style;

fission_component! {
    pub(super) CompactBrand<AnimationGalleryState> |_brand, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Row(
                        gap: tokens.spacing.m,
                        children: [
                            Container(
                                Text("F")
                                    .size(typography.font_size_xl)
                                    .color(style::on_primary()),
                            )
                            .width(COMPACT_BRAND_MARK_SIZE)
                            .height(COMPACT_BRAND_MARK_SIZE)
                            .padding_all(tokens.spacing.m)
                            .border_radius(tokens.radii.large)
                            .bg(style::primary()),
                            Column(
                                gap: tokens.spacing.xs,
                                children: [
                                    Text("Fission")
                                        .size(typography.font_size_xl)
                                        .color(style::text_primary()),
                                    Text("Animation Gallery")
                                        .size(typography.font_size_lg)
                                        .color(style::primary()),
                                ],
                            ),
                        ],
                    ),
                    Text("Calm, explicit, deterministic motion for Fission apps.")
                        .size(typography.font_size_sm)
                        .color(style::text_muted()),
                ],
            ),
        )
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
