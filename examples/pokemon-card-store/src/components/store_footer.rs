use crate::app::StoreState;
use fission::prelude::*;
fission_component! { pub StoreFooter<StoreState> {} |_f, _state, cx| { let t=&cx.view.env().theme.tokens; } =>
    Text("Demo storefront: server rendering, session state, route-local workers, and focused WASM islands.").size(t.typography.font_size_sm).line_height(t.typography.font_size_sm*t.typography.line_height_normal).color(t.colors.text_muted)
}
