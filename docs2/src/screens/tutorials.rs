use crate::content::GoalMeta;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, tutorial_image, DocsHeader, SiteFooter, TutorialCard};
use fission::prelude::*;

fission_component! {
    pub TutorialsIndexPage<DocsState> {
        props { tutorials: Vec<GoalMeta> }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Column(gap: tokens.spacing.xl, children: [
            DocsHeader::new(false),
            Container(
                SemanticsRegion(
                identifier: Some("tutorials-main".to_string()),
                child: Column(gap: tokens.spacing.xl, children: [
                    Column(gap: tokens.spacing.m, children: [
                        copy("tutorials.title")
                            .size(tokens.typography.display_md_size)
                            .weight(tokens.typography.font_weight_bold),
                        copy("tutorials.description")
                            .size(tokens.typography.body_large_size)
                            .color(tokens.colors.text_secondary),
                    ]),
                    SemanticsRegion(
                        identifier: Some("tutorials-list".to_string()),
                        label: Some(cx.view.env().tr("tutorials.region")),
                        child: Column(gap: tokens.spacing.l, children: [
                            for tutorial in &page.tutorials {
                                TutorialCard(
                                    tutorial.title.clone(),
                                    tutorial.description.clone(),
                                    tutorial.path.clone(),
                                    tutorial.difficulty.clone(),
                                    tutorial.duration.clone(),
                                    tutorial_image(tutorial.order),
                                    false,
                                    key: tutorial.path.clone(),
                                )
                            },
                        ]),
                    ),
                    SemanticsRegion(
                        Link::to(cx.view.env().tr("tutorials.reference_link"), navigation::REFERENCE_OVERVIEW),
                        identifier: Some("tutorials-reference-link".to_string()),
                    ),
                ]),
                ),
            )
            .width_length(Length::percent(100.0))
            .padding_all(tokens.spacing.xl),
            SiteFooter(),
        ]),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}
