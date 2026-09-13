use crate::app::StoreState;
use fission::prelude::*;

pub struct StoreFooter;

impl From<StoreFooter> for Widget {
    fn from(_: StoreFooter) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Text::new(
            "Demo storefront: server rendering, session state, route-local workers, and focused WASM islands.",
        )
        .size(typography.font_size_sm)
        .line_height(typography.font_size_sm * typography.line_height_normal)
        .color(tokens.colors.text_muted)
        .into()
    }
}
