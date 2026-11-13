use crate::content::GoalMeta;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, SiteFooter, SiteHeader, TutorialListItem};
use fission::prelude::*;

pub const TUTORIALS_PER_PAGE: usize = 8;

#[derive(Clone, Debug)]
pub struct TutorialCategory {
    pub slug: String,
    pub title_key: &'static str,
    pub description_key: &'static str,
    pub tutorials: Vec<GoalMeta>,
}

impl TutorialCategory {
    pub fn route(&self, page: usize) -> String {
        if page <= 1 {
            format!("{}{}/", navigation::TUTORIALS, self.slug)
        } else {
            format!("{}{}/page/{page}/", navigation::TUTORIALS, self.slug)
        }
    }

    pub fn page_count(&self) -> usize {
        self.tutorials.len().div_ceil(TUTORIALS_PER_PAGE)
    }
}

pub fn tutorial_categories(tutorials: &[GoalMeta]) -> Vec<TutorialCategory> {
    let definitions = [
        (
            "projects",
            "tutorials.group.projects",
            "tutorials.group.projects_description",
        ),
        (
            "mobile-setup",
            "tutorials.group.mobile_setup",
            "tutorials.group.mobile_setup_description",
        ),
        (
            "android",
            "tutorials.group.android",
            "tutorials.group.android_description",
        ),
        (
            "ios",
            "tutorials.group.ios",
            "tutorials.group.ios_description",
        ),
        (
            "shared-mobile",
            "tutorials.group.shared_mobile",
            "tutorials.group.shared_mobile_description",
        ),
    ];

    definitions
        .into_iter()
        .map(|(slug, title_key, description_key)| TutorialCategory {
            slug: slug.to_string(),
            title_key,
            description_key,
            tutorials: tutorials
                .iter()
                .filter(|tutorial| match slug {
                    "projects" => tutorial.order <= 16,
                    "mobile-setup" => tutorial.app.starts_with("set-up-rust-"),
                    "android" => tutorial.app.starts_with("android-"),
                    "ios" => tutorial.app.starts_with("ios-"),
                    "shared-mobile" => tutorial.app.starts_with("mobile-"),
                    _ => false,
                })
                .cloned()
                .collect(),
        })
        .filter(|category| !category.tutorials.is_empty())
        .collect()
}

fission_component! {
    pub TutorialsIndexPage<DocsState> {
        props { categories: Vec<TutorialCategory> }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
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
                                    .weight(tokens.typography.font_weight_bold),
                                identifier: Some("site-heading-1:tutorials".to_string()),
                            ),
                            copy("tutorials.description")
                                .color(tokens.colors.text_secondary),
                        ]),
                        SemanticsRegion(
                            identifier: Some("tutorials-list".to_string()),
                            label: Some(cx.view.env().tr("tutorials.region")),
                            child: Grid(
                                columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                                    GridTrack::Points(300.0),
                                    GridTrack::Fr(1.0),
                                ))],
                                rows: vec![GridTrack::Auto],
                                column_gap: tokens.spacing.m,
                                row_gap: tokens.spacing.m,
                                children: [
                                    for category in &page.categories {
                                        SemanticsRegion(
                                            identifier: Some("tutorial-category-card".to_string()),
                                            label: Some(cx.view.env().tr(category.title_key)),
                                            child: Column(gap: tokens.spacing.m, children: [
                                                SemanticsRegion(
                                                    copy(category.title_key)
                                                        .weight(tokens.typography.font_weight_bold),
                                                    identifier: Some(format!(
                                                        "site-heading-2:tutorial-category-{}",
                                                        category.slug,
                                                    )),
                                                ),
                                                copy(category.description_key)
                                                    .color(tokens.colors.text_secondary),
                                                Row(
                                                    gap: tokens.spacing.s,
                                                    align_items: ir_op::AlignItems::Center,
                                                    children: [
                                                        Badge(text: category.tutorials.len().to_string()),
                                                        Text(cx.view.env().tr("tutorials.browse"))
                                                            .color(tokens.colors.primary)
                                                            .weight(tokens.typography.font_weight_bold),
                                                    ],
                                                ),
                                            ]),
                                            key: category.slug.clone(),
                                        )
                                        .hyperlink(Hyperlink::new(category.route(1)))
                                    },
                                ],
                            ),
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

fission_component! {
    pub TutorialCategoryPage<DocsState> {
        props {
            category: TutorialCategory,
            page: usize,
        }
    }
    view(view, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let page_count = view.category.page_count();
        let start = (view.page.saturating_sub(1)) * TUTORIALS_PER_PAGE;
        let end = (start + TUTORIALS_PER_PAGE).min(view.category.tutorials.len());
        let tutorials = &view.category.tutorials[start..end];
        let previous = view.page.checked_sub(1).filter(|page| *page > 0);
        let next = (view.page < page_count).then_some(view.page + 1);
        let page_count_label = format!(
            "{} {} {} {}",
            cx.view.env().tr("tutorials.page"),
            view.page,
            cx.view.env().tr("tutorials.page.of"),
            page_count,
        );
    } =>
    Container(
        Column(gap: tokens.spacing.xl, children: [
            SiteHeader(),
            Container(
                SemanticsRegion(
                    identifier: Some("tutorials-main".to_string()),
                    child: Column(gap: tokens.spacing.xl, children: [
                        SemanticsRegion(
                            identifier: Some("tutorial-breadcrumb".to_string()),
                            child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                                Link::to(cx.view.env().tr("site.home"), navigation::HOME),
                                Icon::svg(material::navigation::chevron_right::regular()).size(tokens.spacing.s),
                                Link::to(cx.view.env().tr("site.tutorials"), navigation::TUTORIALS),
                                Icon::svg(material::navigation::chevron_right::regular()).size(tokens.spacing.s),
                                Text(cx.view.env().tr(view.category.title_key)),
                            ]),
                        ),
                        Column(gap: tokens.spacing.m, children: [
                            SemanticsRegion(
                                copy(view.category.title_key).weight(tokens.typography.font_weight_bold),
                                identifier: Some("site-heading-1:tutorial-category".to_string()),
                            ),
                            copy(view.category.description_key).color(tokens.colors.text_secondary),
                        ]),
                        SemanticsRegion(
                            identifier: Some("tutorials-list".to_string()),
                            label: Some(cx.view.env().tr(view.category.title_key)),
                            child: Grid(
                                columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                                    GridTrack::Points(300.0),
                                    GridTrack::Fr(1.0),
                                ))],
                                rows: vec![GridTrack::Auto],
                                column_gap: tokens.spacing.m,
                                row_gap: tokens.spacing.m,
                                children: [
                                    for tutorial in tutorials {
                                        TutorialListItem(tutorial.clone(), key: tutorial.path.clone())
                                    },
                                ],
                            ),
                        ),
                        SemanticsRegion(
                            identifier: Some("tutorial-pagination".to_string()),
                            label: Some(cx.view.env().tr("tutorials.pagination")),
                            child: Row(
                                gap: tokens.spacing.m,
                                align_items: ir_op::AlignItems::Center,
                                children: [
                                    if let Some(page) = previous {
                                        Link::to(
                                            cx.view.env().tr("tutorials.previous"),
                                            view.category.route(page),
                                        )
                                    } else {
                                        Spacer(flex_grow: 1.0)
                                    },
                                    Text(page_count_label),
                                    if let Some(page) = next {
                                        Link::to(
                                            cx.view.env().tr("tutorials.next"),
                                            view.category.route(page),
                                        )
                                    } else {
                                        Spacer(flex_grow: 1.0)
                                    },
                                ],
                            ),
                        ),
                        Link::to(cx.view.env().tr("tutorials.all_categories"), navigation::TUTORIALS),
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
