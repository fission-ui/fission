use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub TutorialCard<DocsState> {
        props {
            image: &'static str,
            title_key: &'static str,
            body_key: &'static str,
            href: &'static str,
        }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-tutorial-card".to_string()),
        child: Column(gap: tokens.spacing.m, children: [
            Image::asset(card.image)
                .height(180.0)
                .fit(ir_op::ImageFit::Cover)
                .semantic_label(cx.view.env().tr(card.title_key)),
            copy(card.title_key)
                .size(tokens.typography.heading2_size)
                .weight(tokens.typography.font_weight_bold),
            copy(card.body_key).color(tokens.colors.text_secondary),
            Link::to(cx.view.env().tr("home.learn.start"), card.href),
        ]),
    )
}
