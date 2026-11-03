use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub ProductLinkCard<DocsState> {
        props {
            title_key: &'static str,
            summary_key: &'static str,
            href: &'static str,
        }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some(format!("product-card:{}", card.href)),
        child: Column(gap: tokens.spacing.s, children: [
            copy(card.title_key).weight(tokens.typography.font_weight_bold),
            copy(card.summary_key).color(tokens.colors.text_secondary).wrap(true),
            Link::to(cx.view.env().tr("product.open"), card.href),
        ]),
    )
}
