use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalCard<DocsState> {
        props {
            icon: &'static str,
            title_key: &'static str,
            body_key: &'static str,
            image: &'static str,
            href: &'static str,
        }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-goal-card".to_string()),
        child: Row(gap: tokens.spacing.s, children: [
            Column(gap: tokens.spacing.m, children: [
                Icon::svg(card.icon).size(tokens.spacing.xl),
                copy(card.title_key).weight(tokens.typography.font_weight_bold),
                copy(card.body_key).color(tokens.colors.text_secondary),
                Spacer(flex_grow: 1.0),
                Link::to(cx.view.env().tr("home.goal.open"), card.href),
            ]),
            Image::asset(card.image).size(110.0, 150.0).fit(ir_op::ImageFit::Cover).semantic_label(""),
        ]),
    )
}
