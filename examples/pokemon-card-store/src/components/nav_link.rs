use crate::app::StoreState;
use crate::components::palette::BLUE_TEXT;
use fission::prelude::*;

pub struct NavLink {
    pub label: &'static str,
    pub href: &'static str,
}

impl From<NavLink> for Widget {
    fn from(link: NavLink) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let typography = &view.env().theme.tokens.typography;

        Text::new(link.label)
            .size(typography.font_size_base)
            .line_height(typography.font_size_base * typography.line_height_snug)
            .weight(typography.font_weight_bold)
            .color(BLUE_TEXT)
            .semantics_identifier(format!("site-route:{}", link.href))
            .into()
    }
}
