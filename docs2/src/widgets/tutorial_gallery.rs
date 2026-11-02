use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, TutorialCard};
use fission::prelude::*;

fission_component! {
    pub TutorialGallery<DocsState> {}
    view(_gallery, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-tutorials".to_string()),
        label: Some(cx.view.env().tr("home.learn.title")),
        child: Column(
            gap: tokens.spacing.l,
            children: [
                Row(children: [
                    Column(gap: tokens.spacing.s, children: [
                        copy("home.learn.eyebrow")
                            .color(tokens.colors.text_secondary)
                            .weight(tokens.typography.font_weight_semibold),
                        copy("home.learn.title")
                            .size(tokens.typography.heading1_size)
                            .weight(tokens.typography.font_weight_bold),
                        copy("home.learn.summary").color(tokens.colors.text_secondary),
                    ]),
                    Spacer(flex_grow: 1.0),
                    Link::to(cx.view.env().tr("home.learn.all"), navigation::COUNTER_TUTORIAL),
                ]),
                Row(gap: tokens.spacing.l, align_items: ir_op::AlignItems::Stretch, children: [
                    TutorialCard(
                        "/img/examples/inbox.png",
                        "home.learn.counter",
                        "home.learn.counter_body",
                        navigation::COUNTER_TUTORIAL,
                    ),
                    TutorialCard(
                        "/img/examples/editor.png",
                        "home.learn.admin",
                        "home.learn.admin_body",
                        navigation::SHOWCASE,
                    ),
                    TutorialCard(
                        "/img/examples/inbox-compact.png",
                        "home.learn.site",
                        "home.learn.site_body",
                        navigation::PACKAGING_GUIDE,
                    ),
                ]),
            ],
        ),
    )
}
