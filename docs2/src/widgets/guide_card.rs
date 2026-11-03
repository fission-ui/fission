use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GuideCard<DocsState> {
        props {
            icon: &'static str,
            title: String,
            description: String,
            href: String,
            tags: Vec<String>,
        }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("hub-guide-card".to_string()),
        child: Column(gap: tokens.spacing.s, children: [
            Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Start, children: [
                Icon::svg(card.icon).size(tokens.spacing.xl).color(tokens.colors.primary),
                Column(gap: tokens.spacing.xs, children: [
                    Text(card.title.clone()).weight(tokens.typography.font_weight_bold),
                    Text(card.description.clone()).color(tokens.colors.text_secondary),
                    Row(gap: tokens.spacing.s, children: [
                        for tag in &card.tags {
                            SemanticsRegion(
                                Text(tag.clone()),
                                identifier: Some("hub-guide-tag".to_string()),
                                label: Some(tag.clone()),
                                key: tag.clone(),
                            )
                        },
                    ]),
                    SemanticsRegion(
                        copy("hub.guide.read").color(tokens.colors.primary).weight(tokens.typography.font_weight_bold),
                        identifier: Some("hub-guide-read".to_string()),
                    ),
                ]),
            ]),
        ]),
    )
    .hyperlink(Hyperlink::new(card.href.clone()))
}
