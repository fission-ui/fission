use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalCard<DocsState> {
        props {
            icon: &'static str,
            title_key: &'static str,
            body_key: &'static str,
            action_key: &'static str,
            image: &'static str,
            href: String,
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
        identifier: Some("home-goal-card".to_string()),
        label: Some(cx.view.env().tr(card.title_key)),
        child: Row(gap: tokens.spacing.s, children: [
            Column(gap: tokens.spacing.m, children: [
                Icon::svg(card.icon).size(tokens.spacing.xl),
                SemanticsRegion(
                    copy(card.title_key)
                        .color(tokens.colors.primary)
                        .weight(tokens.typography.font_weight_bold),
                    identifier: Some(heading_identifier),
                ),
                copy(card.body_key).color(tokens.colors.text_secondary),
                Spacer(flex_grow: 1.0),
                copy(card.action_key)
                    .color(tokens.colors.primary)
                    .weight(tokens.typography.font_weight_bold),
            ]),
            Image::asset(card.image).size(110.0, 150.0).fit(ir_op::ImageFit::Cover).semantic_label(""),
        ]),
    )
    .hyperlink(Hyperlink::new(card.href.clone()))
}
