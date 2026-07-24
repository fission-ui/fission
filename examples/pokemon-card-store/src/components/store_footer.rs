use crate::app::StoreState;
use crate::components::palette::TEXT_MUTED;
use fission::prelude::*;

pub struct StoreFooter;

impl From<StoreFooter> for Widget {
    fn from(_: StoreFooter) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let typography = &view.env().theme.tokens.typography;

        Text::new(
            "Demo storefront: server rendering, session state, route-local workers, and focused WASM islands.",
        )
        .size(typography.font_size_sm)
        .line_height(typography.font_size_sm * typography.line_height_normal)
        .color(TEXT_MUTED)
        .into()
    }
}
