use crate::blog::{BlogCategoryLink, BlogPost, BlogPostSummary};
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{BlogPostCard, SiteFooter, SiteHeader};
use fission::prelude::*;

pub const BLOG_POSTS_PER_PAGE: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogArchive {
    pub title: String,
    pub description: String,
    pub base_path: String,
    pub page: usize,
    pub page_count: usize,
    pub posts: Vec<BlogPostSummary>,
    pub categories: Vec<BlogCategoryLink>,
    pub show_categories: bool,
}

fission_component! {
    pub BlogArchivePage<DocsState> {
        props { archive: BlogArchive }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let previous = page.archive.page.checked_sub(1).filter(|number| *number > 0);
        let next = (page.archive.page < page.archive.page_count).then_some(page.archive.page + 1);
        let page_label = format!("Page {} of {}", page.archive.page, page.archive.page_count);
    } =>
    Container(
        Column(gap: tokens.spacing.xl, children: [
            SiteHeader(),
            SemanticsRegion(
                identifier: Some("blog-archive-main".to_string()),
                child: Column(gap: tokens.spacing.xl, children: [
                    SemanticsRegion(
                        identifier: Some("blog-breadcrumb".to_string()),
                        child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                            Link::to(cx.view.env().tr("site.home"), navigation::HOME),
                            Icon::svg(material::navigation::chevron_right::regular()).size(tokens.spacing.s),
                            Text("Blog".to_string()),
                        ]),
                    ),
                    Column(gap: tokens.spacing.m, children: [
                        SemanticsRegion(
                            Text(page.archive.title.clone()).weight(tokens.typography.font_weight_bold),
                            identifier: Some("site-heading-1:blog-archive".to_string()),
                        ),
                        Text(page.archive.description.clone()).color(tokens.colors.text_secondary),
                    ]),
                    if page.archive.show_categories && !page.archive.categories.is_empty() {
                        SemanticsRegion(
                            identifier: Some("blog-categories".to_string()),
                            label: Some("Browse blog categories".to_string()),
                            child: Row(gap: tokens.spacing.s, wrap: ir_op::FlexWrap::Wrap, children: [
                                for category in &page.archive.categories {
                                    SemanticsRegion(
                                        Link::to(format!("{} ({})", category.title, category.count), category.path.clone()),
                                        identifier: Some("blog-category-link".to_string()),
                                        key: category.path.clone(),
                                    )
                                }
                            ]),
                        )
                    },
                    SemanticsRegion(
                        identifier: Some("blog-post-list".to_string()),
                        label: Some(page.archive.title.clone()),
                        child: Grid(
                            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                                GridTrack::Points(320.0),
                                GridTrack::Fr(1.0),
                            ))],
                            rows: vec![GridTrack::Auto],
                            column_gap: tokens.spacing.m,
                            row_gap: tokens.spacing.m,
                            children: [
                                for post in &page.archive.posts {
                                    BlogPostCard(post.clone(), key: post.path.clone())
                                }
                            ],
                        ),
                    ),
                    if page.archive.page_count > 1 {
                        SemanticsRegion(
                            identifier: Some("blog-pagination".to_string()),
                            label: Some("Blog pagination".to_string()),
                            child: Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Center, children: [
                                if let Some(previous_page) = previous {
                                    Link::to("← Newer posts", archive_page_path(&page.archive.base_path, previous_page))
                                },
                                Spacer(flex_grow: 1.0),
                                Text(page_label),
                                Spacer(flex_grow: 1.0),
                                if let Some(next_page) = next {
                                    Link::to("Older posts →", archive_page_path(&page.archive.base_path, next_page))
                                },
                            ]),
                        )
                    },
                ]),
            ),
            SiteFooter(),
        ]),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}

fission_component! {
    pub BlogArticlePage<DocsState> {
        props {
            article: BlogPost,
            older: Option<BlogPostSummary>,
            newer: Option<BlogPostSummary>,
        }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let authors = page.article.authors.iter().map(|author| match author.as_str() {
            "fission" => "Fission Team",
            "courtney" | "courtney-robinson" => "Courtney Robinson",
            other => other,
        }).collect::<Vec<_>>().join(", ");
        let is_website_tutorial = page.article.slug == "build-a-rust-website-with-dropdown-navigation";
    } =>
    Container(
        Column(gap: tokens.spacing.xl, children: [
            SiteHeader(),
            SemanticsRegion(
                identifier: Some("blog-article-main".to_string()),
                child: Column(gap: tokens.spacing.xl, children: [
                    SemanticsRegion(
                        identifier: Some("blog-breadcrumb".to_string()),
                        child: Row(gap: tokens.spacing.s, wrap: ir_op::FlexWrap::Wrap, align_items: ir_op::AlignItems::Center, children: [
                            Link::to(cx.view.env().tr("site.home"), navigation::HOME),
                            Icon::svg(material::navigation::chevron_right::regular()).size(tokens.spacing.s),
                            Link::to("Blog", navigation::BLOG),
                        ]),
                    ),
                    SemanticsRegion(
                        identifier: Some("blog-article-header".to_string()),
                        child: Column(gap: tokens.spacing.m, children: [
                            SemanticsRegion(
                                Text(page.article.title.clone()).weight(tokens.typography.font_weight_bold),
                                identifier: Some("site-heading-1:blog-article".to_string()),
                            ),
                            Text(page.article.description.clone()).color(tokens.colors.text_secondary),
                            Row(gap: tokens.spacing.s, wrap: ir_op::FlexWrap::Wrap, align_items: ir_op::AlignItems::Center, children: [
                                Text(page.article.published.clone()).color(tokens.colors.text_secondary),
                                if !authors.is_empty() {
                                    Text(format!("By {authors}")).color(tokens.colors.text_secondary)
                                },
                                for category in &page.article.categories {
                                    Badge(text: category.clone(), key: category.clone())
                                }
                            ]),
                        ]),
                    ),
                    if is_website_tutorial {
                        SemanticsRegion(
                            identifier: Some("blog-tutorial-callout".to_string()),
                            child: Row(gap: tokens.spacing.m, wrap: ir_op::FlexWrap::Wrap, align_items: ir_op::AlignItems::Center, children: [
                                Text("Want to build this website step by step?".to_string()),
                                Link::to("Follow the current Fission tutorial →", navigation::WEBSITE_NAVIGATION),
                            ]),
                        )
                    },
                    SemanticsRegion(
                        identifier: Some("blog-article-body".to_string()),
                        child: Some(MarkdownContent::new(page.article.markdown.clone()).into()),
                    ),
                    SemanticsRegion(
                        identifier: Some("blog-post-adjacent".to_string()),
                        child: Row(gap: tokens.spacing.xl, wrap: ir_op::FlexWrap::Wrap, align_items: ir_op::AlignItems::Start, children: [
                            if let Some(older) = &page.older {
                                Column(gap: tokens.spacing.xs, children: [
                                    Text("Older article".to_string()).color(tokens.colors.text_secondary),
                                    Link::to(older.title.clone(), older.path.clone()),
                                ])
                            },
                            Spacer(flex_grow: 1.0),
                            if let Some(newer) = &page.newer {
                                Column(gap: tokens.spacing.xs, children: [
                                    Text("Newer article".to_string()).color(tokens.colors.text_secondary),
                                    Link::to(newer.title.clone(), newer.path.clone()),
                                ])
                            },
                        ]),
                    ),
                ]),
            ),
            SiteFooter(),
        ]),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}

fn archive_page_path(base_path: &str, page: usize) -> String {
    if page <= 1 {
        base_path.to_string()
    } else {
        format!("{}page/{page}/", base_path)
    }
}
