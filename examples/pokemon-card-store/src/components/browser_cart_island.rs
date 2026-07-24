use crate::app::StoreState;
use crate::components::layout::CART_ISLAND_WIDTH;
use crate::components::palette::{AMBER, SURFACE_RAISED, TEXT_BODY};
use fission::prelude::*;

pub struct BrowserCartIsland;

impl From<BrowserCartIsland> for Widget {
    fn from(_: BrowserCartIsland) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        SemanticsRegion::new(
            Container::new(Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Text::new("Island booting")
                        .size(typography.font_size_sm)
                        .line_height(
                            typography.font_size_sm * typography.line_height_snug
                        )
                        .weight(typography.font_weight_bold)
                        .color(AMBER)
                        .semantics_identifier("island-status:cart-drawer"),
                    Text::new("The focused Fission island replaces this fallback with its own widget tree after its WASM artifact loads.")
                        .size(typography.font_size_base)
                        .line_height(
                            typography.font_size_base
                                * typography.line_height_normal
                        )
                        .color(TEXT_BODY),
                ],
                ..Default::default()
            })
            .width_length(Length::min(vec![
                Length::percent(100.0),
                Length::points(CART_ISLAND_WIDTH),
            ]))
            .padding_all(tokens.spacing.m)
            .border(AMBER.with_alpha(130), 1.0)
            .border_radius(tokens.radii.xxl)
            .bg(SURFACE_RAISED),
        )
        .identifier("cart-drawer")
        .id(WidgetId::explicit("cart-drawer"))
        .into()
    }
}
