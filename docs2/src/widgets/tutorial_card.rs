use crate::state::DocsState;
use fission::prelude::*;

pub(crate) fn tutorial_image(order: u32) -> &'static str {
    match order {
        10 => "/img/examples/chart-gallery.png",
        13 => "/img/examples/inbox-compact.png",
        9 => "/img/examples/inbox.png",
        6 => "/img/examples/inbox-compose.png",
        12 => "/img/examples/editor.png",
        _ => "/img/examples/widget-gallery.png",
    }
}

fission_component! {
    pub TutorialCard<DocsState> {
        props {
            title: String,
            description: String,
            href: String,
            difficulty: String,
            duration: String,
            image: &'static str,
            compact: bool,
        }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let heading_identifier = format!(
            "site-heading-3:{}",
            card.href.trim_matches('/').replace('/', "-")
        );
    } =>
    SemanticsRegion(
        identifier: Some("home-tutorial-card".to_string()),
        label: Some(card.title.clone()),
        child: Column(gap: tokens.spacing.m, children: [
            Image::asset(card.image)
                .size(320.0, 112.0)
                .fit(ir_op::ImageFit::Cover)
                .semantic_label(""),
            SemanticsRegion(
                Text(card.title.clone())
                    .color(tokens.colors.primary)
                    .weight(tokens.typography.font_weight_bold),
                identifier: Some(heading_identifier),
            ),
            Text(card.description.clone()).color(tokens.colors.text_secondary),
            SemanticsRegion(
                identifier: Some(if card.compact {
                    "home-tutorial-meta".to_string()
                } else {
                    "tutorial-card-meta".to_string()
                }),
                child: Row(gap: tokens.spacing.s, children: [
                    Badge(text: card.difficulty.clone()),
                    Badge(text: card.duration.clone()),
                ]),
            ),
            Text(cx.view.env().tr("home.learn.start"))
                .color(tokens.colors.primary)
                .weight(tokens.typography.font_weight_bold),
        ]),
    )
    .hyperlink(Hyperlink::new(card.href.clone()))
}
