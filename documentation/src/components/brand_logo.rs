use super::home_widgets::SemanticRow;
use super::state::DocsState;
use fission::op::{AlignItems, FlexWrap, JustifyContent};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct BrandLogo {
    size: f32,
    centered: bool,
    route: &'static str,
}

impl BrandLogo {
    pub(super) fn new(size: f32) -> Self {
        Self {
            size,
            centered: false,
            route: "/",
        }
    }

    pub(super) fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    pub(super) fn route(mut self, route: &'static str) -> Self {
        self.route = route;
        self
    }
}

impl From<BrandLogo> for Widget {
    fn from(brand: BrandLogo) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            format!("site-route:{}", brand.route),
            vec![
                Image::asset("/img/fission-mark.svg")
                    .size(brand.size, brand.size)
                    .into(),
                Text::new("ission")
                    .size(brand.size * 0.72)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
            ],
            Some(0.0),
            FlexWrap::NoWrap,
            AlignItems::Center,
            if brand.centered {
                JustifyContent::Center
            } else {
                JustifyContent::Start
            },
        )
        .into()
    }
}
