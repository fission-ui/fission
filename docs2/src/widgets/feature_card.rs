use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub FeatureCard<DocsState> {
        props {
            icon: &'static str,
            title_key: &'static str,
            body_key: &'static str,
            link_key: &'static str,
            href: &'static str,
        }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let heading_identifier = format!(
            "site-heading-3:{}",
            card.title_key.replace('.', "-")
        );
    } =>
    SemanticsRegion(
        identifier: Some("home-feature-card".to_string()),
        child: Column(gap: tokens.spacing.m, children: [
            Icon::svg(card.icon).size(tokens.spacing.xl),
            SemanticsRegion(
                copy(card.title_key).weight(tokens.typography.font_weight_bold),
                identifier: Some(heading_identifier),
            ),
            copy(card.body_key).color(tokens.colors.text_secondary),
            Link::to(
                cx.view.env().tr(card.link_key),
                card.href,
            ),
        ]),
    )
}
