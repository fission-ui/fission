use crate::content::GoalMeta;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, SiteFooter, SiteHeader, TutorialGroup};
use fission::prelude::*;

fission_component! {
    pub TutorialsIndexPage<DocsState> {
        props { tutorials: Vec<GoalMeta> }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let groups = tutorial_groups(&page.tutorials);
    } =>
    Container(
        Column(gap: tokens.spacing.xl, children: [
            SiteHeader(),
            Container(
                SemanticsRegion(
                identifier: Some("tutorials-main".to_string()),
                child: Column(gap: tokens.spacing.xl, children: [
                    Column(gap: tokens.spacing.m, children: [
                        SemanticsRegion(
                            copy("tutorials.title")
                                .size(tokens.typography.display_md_size)
                                .weight(tokens.typography.font_weight_bold),
                            identifier: Some("site-heading-1:tutorials".to_string()),
                        ),
                        copy("tutorials.description")
                            .size(tokens.typography.body_large_size)
                            .color(tokens.colors.text_secondary),
                    ]),
                    SemanticsRegion(
                        identifier: Some("tutorials-list".to_string()),
                        label: Some(cx.view.env().tr("tutorials.region")),
                        child: Column(gap: tokens.spacing.l, children: [
                            for (title_key, description_key, tutorials) in groups {
                                TutorialGroup(
                                    title_key,
                                    description_key,
                                    tutorials,
                                    key: title_key.to_string(),
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

fn tutorial_groups(tutorials: &[GoalMeta]) -> Vec<(&'static str, &'static str, Vec<GoalMeta>)> {
    let ranges = [
        (
            1..=16,
            "tutorials.group.projects",
            "tutorials.group.projects_description",
        ),
        (
            17..=18,
            "tutorials.group.mobile_setup",
            "tutorials.group.mobile_setup_description",
        ),
        (
            19..=63,
            "tutorials.group.android",
            "tutorials.group.android_description",
        ),
        (
            64..=108,
            "tutorials.group.ios",
            "tutorials.group.ios_description",
        ),
        (
            109..=118,
            "tutorials.group.shared_mobile",
            "tutorials.group.shared_mobile_description",
        ),
    ];

    ranges
        .into_iter()
        .map(|(range, title_key, description_key)| {
            (
                title_key,
                description_key,
                tutorials
                    .iter()
                    .filter(|tutorial| range.contains(&tutorial.order))
                    .cloned()
                    .collect(),
            )
        })
        .collect()
}
