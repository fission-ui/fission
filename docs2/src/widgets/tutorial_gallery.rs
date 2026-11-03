use crate::content::GoalMeta;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, tutorial_image, TutorialCard};
use fission::prelude::*;

fission_component! {
    pub TutorialGallery<DocsState> {
        props { tutorials: Vec<GoalMeta> }
    }
    view(gallery, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let featured_orders = [10, 13, 9];
        let featured_tutorials = featured_orders.iter()
            .filter_map(|order| gallery.tutorials.iter().find(|tutorial| tutorial.order == *order))
            .map(|tutorial| {
            let description_key = match tutorial.order {
                10 => "home.learn.admin_body",
                13 => "home.learn.mobile_body",
                9 => "home.learn.site_body",
                _ => "home.learn.summary",
            };
            (
                tutorial,
                tutorial_image(tutorial.order),
                cx.view.env().tr(description_key),
            )
        }).collect::<Vec<_>>();
    } =>
    SemanticsRegion(
        identifier: Some("home-tutorials".to_string()),
        label: Some(cx.view.env().tr("home.learn.title")),
        child: Column(
            gap: tokens.spacing.l,
            children: [
                Row(children: [
                    Column(gap: tokens.spacing.s, children: [
                        SemanticsRegion(
                            copy("home.learn.title")
                                .size(tokens.typography.heading1_size)
                                .weight(tokens.typography.font_weight_bold),
                            identifier: Some("site-heading-2:tutorial-highlights".to_string()),
                        ),
                        copy("home.learn.summary").color(tokens.colors.text_secondary),
                    ]),
                    Spacer(flex_grow: 1.0),
                    Link::to(cx.view.env().tr("home.learn.all"), navigation::TUTORIALS),
                ]),
                Row(
                    gap: tokens.spacing.l,
                    align_items: ir_op::AlignItems::Stretch,
                    wrap: ir_op::FlexWrap::Wrap,
                    children: [
                        for (tutorial, image, description) in featured_tutorials.iter().cloned() {
                            TutorialCard(
                                tutorial.title.clone(),
                                description,
                                tutorial.path.clone(),
                                tutorial.difficulty.clone(),
                                tutorial.duration.clone(),
                                image,
                                true,
                                key: tutorial.path.clone(),
                            )
                        }
                    ],
                ),
            ],
        ),
    )
}
