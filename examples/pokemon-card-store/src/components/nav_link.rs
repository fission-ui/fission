use crate::app::StoreState;
use fission::prelude::*;
fission_component! { pub NavLink<StoreState> { label: &'static str, href: &'static str } |link, _state, cx| { let t=&cx.view.env().theme.tokens; } =>
    Text(link.label).size(t.typography.font_size_base).line_height(t.typography.font_size_base*t.typography.line_height_snug).weight(t.typography.font_weight_bold).color(t.colors.text_link).semantics_identifier(format!("site-route:{}", link.href))
}
