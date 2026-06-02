use fission_core::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission_core::ui::{Column, Container, Image, Row, Text};
use fission_core::{GlobalState, ViewHandle, Widget};
use fission_ir::{Role, Semantics};
use fission_theme::Tokens;
use fission_widgets::MarkdownViewer;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct ContentRoute {
    pub path: String,
    pub title: String,
    pub description: Option<String>,
    pub body: String,
    pub headings: Vec<HeadingLink>,
    pub sidebar: Vec<SidebarLink>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub show_adjacent_posts: bool,
    pub source_path: PathBuf,
    pub rendered: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteNavLink {
    pub title: String,
    pub href: String,
    pub children: Vec<SiteNavLink>,
}

impl SiteNavLink {
    pub fn new(title: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            href: href.into(),
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: impl IntoIterator<Item = SiteNavLink>) -> Self {
        self.children = children.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeadingLink {
    pub title: String,
    pub anchor: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SidebarLink {
    pub title: String,
    pub href: String,
    pub level: usize,
    pub group: bool,
}

#[derive(Debug, Default)]
pub(crate) struct SitePageState;
impl GlobalState for SitePageState {}

pub(crate) struct DocumentationPage<'a> {
    pub site_title: &'a str,
    pub site_logo: Option<&'a str>,
    pub site_nav: &'a [SiteNavLink],
    pub theme_switching: bool,
    pub search_enabled: bool,
    pub route: &'a ContentRoute,
    pub all_routes: &'a [ContentRoute],
}

impl From<DocumentationPage<'_>> for Widget {
    fn from(page: DocumentationPage<'_>) -> Widget {
        let (_, view) = fission_core::build::current::<SitePageState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![page.header(tokens), page.document_grid(view)],
            flex_grow: 1.0,
            ..Default::default()
        })
        .min_height(tokens.spacing.xxxxl * 9.0)
        .bg_fill(Fill::Solid(tokens.colors.background))
        .into()
    }
}

impl DocumentationPage<'_> {
    fn header(&self, tokens: &Tokens) -> Widget {
        let mut children = vec![sidebar_toggle(tokens), self.brand(tokens)];
        if !self.site_nav.is_empty() {
            children.push(
                Row {
                    children: self
                        .site_nav
                        .iter()
                        .enumerate()
                        .map(|(index, link)| nav_item(link, 0, index, tokens))
                        .collect(),
                    gap: Some(tokens.spacing.l),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::End,
                    semantics: Some(site_semantics("site-doc-nav")),
                    ..Default::default()
                }
                .into(),
            );
        }
        if self.theme_switching {
            children.push(theme_toggle(tokens));
        }
        if self.search_enabled {
            children.push(search_trigger(tokens));
        }
        Container::new(Row {
            children,
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            semantics: Some(site_semantics("site-doc-header")),
            ..Default::default()
        })
        .padding([
            tokens.spacing.xxxxl,
            tokens.spacing.xxxxl,
            tokens.spacing.l,
            tokens.spacing.l,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface))
        .border(tokens.colors.border, 1.0)
        .into()
    }

    fn brand(&self, tokens: &Tokens) -> Widget {
        let mut children = Vec::new();
        if let Some(logo) = self.site_logo {
            children.push(
                Image::asset(logo.to_string())
                    .size(tokens.spacing.xl, tokens.spacing.xl)
                    .into(),
            );
        }
        children.push(
            Text::new(self.site_title)
                .size(tokens.typography.font_size_lg)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .semantics_identifier("site-route:/")
                .into(),
        );
        Row {
            children,
            gap: Some(tokens.spacing.s),
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into()
    }

    fn document_grid(&self, view: ViewHandle<SitePageState>) -> Widget {
        let tokens = &view.env().theme.tokens;
        if is_blog_route(&self.route.path) {
            return Row {
                children: vec![
                    self.sidebar(tokens),
                    self.article(view),
                    self.blog_rail(tokens),
                ],
                semantics: Some(site_semantics("site-blog-layout")),
                gap: Some(tokens.spacing.xl),
                align_items: AlignItems::Stretch,
                ..Default::default()
            }
            .into();
        }
        Row {
            children: vec![self.sidebar(tokens), self.article(view), self.toc(tokens)],
            semantics: Some(site_semantics("site-doc-layout")),
            gap: Some(tokens.spacing.xl),
            align_items: AlignItems::Stretch,
            ..Default::default()
        }
        .into()
    }

    fn sidebar(&self, tokens: &Tokens) -> Widget {
        let mut children = Vec::new();
        if is_blog_route(&self.route.path) {
            children.extend(self.blog_sidebar_items(tokens));
        } else if self.route.sidebar.is_empty() {
            let section_prefix = section_prefix(&self.route.path);
            for (index, route) in self
                .all_routes
                .iter()
                .filter(|route| route.path.starts_with(section_prefix))
                .enumerate()
            {
                children.push(self.sidebar_item(
                    &route.title,
                    &route.path,
                    0,
                    false,
                    index,
                    tokens,
                ));
            }
        } else {
            for (index, item) in self.route.sidebar.iter().enumerate() {
                children.push(self.sidebar_item(
                    &item.title,
                    &item.href,
                    item.level,
                    item.group,
                    index,
                    tokens,
                ));
            }
        }
        Column {
            children: vec![Container::new(Column {
                children,
                gap: Some(tokens.spacing.s),
                ..Default::default()
            })
            .padding_all(tokens.spacing.l)
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(tokens.colors.border, 1.0)
            .width(tokens.spacing.xxxxl * 3.0)
            .min_height(tokens.spacing.xxxxl * 9.0)
            .flex_shrink(0.0)
            .into()],
            semantics: Some(site_semantics("site-doc-sidebar")),
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }

    fn blog_sidebar_items(&self, tokens: &Tokens) -> Vec<Widget> {
        let mut children = Vec::new();
        children.push(self.sidebar_item("Blog", "/blog/", 0, true, 0, tokens));

        for (index, route) in ordered_blog_routes(self.all_routes).iter().enumerate() {
            children.push(self.sidebar_item(
                &route.title,
                &route.path,
                1,
                false,
                index + 1,
                tokens,
            ));
        }
        children
    }

    fn sidebar_item(
        &self,
        title: &str,
        href: &str,
        level: usize,
        group: bool,
        index: usize,
        tokens: &Tokens,
    ) -> Widget {
        let active = normalize_link_path(href) == self.route.path;
        let color = if active {
            tokens.colors.primary
        } else {
            tokens.colors.text_primary
        };
        let mut item = Container::new(
            Text::new(title.to_string())
                .size(if group {
                    tokens.typography.font_size_sm
                } else {
                    tokens.typography.font_size_base
                })
                .weight(if active || group {
                    tokens.typography.font_weight_bold
                } else {
                    tokens.typography.font_weight_medium
                })
                .color(color)
                .semantics_identifier(format!("site-route:{href}")),
        )
        .padding([
            (level as f32 * tokens.spacing.m) + tokens.spacing.s,
            tokens.spacing.s,
            tokens.spacing.xs,
            tokens.spacing.xs,
        ])
        .border_radius(tokens.radii.medium);
        if active {
            item = item.bg_fill(Fill::Solid(tokens.colors.surface_raised));
        }
        Column {
            children: vec![item.into()],
            semantics: Some(site_semantics(format!(
                "site-sidebar-item:{level}:{active}:{group}:{index}"
            ))),
            ..Default::default()
        }
        .into()
    }

    fn article(&self, view: ViewHandle<SitePageState>) -> Widget {
        let tokens = &view.env().theme.tokens;
        let mut children = Vec::new();
        if let Some(breadcrumbs) = self.breadcrumbs(tokens) {
            children.push(breadcrumbs);
        }
        if is_blog_index_route(&self.route.path) {
            children.push(self.blog_index_article(tokens));
        } else if is_blog_taxonomy_route(&self.route.path) {
            children.push(self.blog_taxonomy_article(tokens));
        } else if is_blog_post_route(&self.route.path) {
            children.push(self.blog_post_article(tokens));
        } else {
            if !body_first_heading_matches_title(&self.route.body, &self.route.title) {
                children.push(
                    Text::new(self.route.title.clone())
                        .size(tokens.typography.heading1_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .weight(tokens.typography.font_weight_bold)
                        .line_height(
                            tokens.typography.heading1_size * tokens.typography.line_height_heading,
                        )
                        .color(tokens.colors.heading)
                        .into(),
                );
                if let Some(description) = &self.route.description {
                    children.push(
                        Text::new(description.clone())
                            .size(tokens.typography.body_large_size)
                            .line_height(
                                tokens.typography.body_large_size
                                    * tokens.typography.line_height_relaxed,
                            )
                            .color(tokens.colors.text_secondary)
                            .into(),
                    );
                }
            }
            let markdown = MarkdownViewer {
                markdown: self.route.body.clone(),
                show_scrollbar: false,
            };
            children.push(markdown.into());
        }

        Column {
            children: vec![Container::new(Column {
                children,
                gap: Some(tokens.spacing.l),
                flex_grow: 1.0,
                ..Default::default()
            })
            .padding([0.0, 0.0, tokens.spacing.xxl, tokens.spacing.xxl])
            .bg_fill(Fill::Solid(tokens.colors.background))
            .flex_grow(1.0)
            .into()],
            semantics: Some(site_semantics("site-doc-main")),
            flex_grow: 1.0,
            ..Default::default()
        }
        .into()
    }

    fn blog_index_article(&self, tokens: &Tokens) -> Widget {
        let posts = ordered_blog_routes(self.all_routes);
        let featured = posts.first().copied();
        let mut post_cards = Vec::new();
        for route in posts.iter().skip(usize::from(featured.is_some())) {
            post_cards.push(blog_post_card(route, false, tokens));
        }

        let mut children = vec![
            Container::new(Column {
                children: vec![
                    Text::new("Blog")
                        .size(tokens.typography.heading1_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .weight(tokens.typography.font_weight_bold)
                        .line_height(
                            tokens.typography.heading1_size
                                * tokens.typography.line_height_heading,
                        )
                        .color(tokens.colors.heading)
                        .semantics_identifier("site-blog-index-title")
                        .into(),
                    Text::new("Technical essays, release notes, and product updates for building with Fission.")
                        .size(tokens.typography.body_large_size)
                        .line_height(
                            tokens.typography.body_large_size
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .into(),
                ],
                gap: Some(tokens.spacing.m),
                semantics: Some(site_semantics("site-blog-index-hero")),
                ..Default::default()
            })
            .into(),
        ];

        if let Some(route) = featured {
            children.push(
                Container::new(Column {
                    children: vec![
                        Text::new("Featured")
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.primary)
                            .into(),
                        blog_post_card(route, true, tokens),
                    ],
                    gap: Some(tokens.spacing.m),
                    ..Default::default()
                })
                .into(),
            );
        }

        if !post_cards.is_empty() {
            children.push(
                Container::new(Column {
                    children: vec![
                        Text::new("Latest posts")
                            .size(tokens.typography.heading2_size)
                            .family(tokens.typography.font_family_serif.clone())
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Column {
                            children: post_cards,
                            gap: Some(tokens.spacing.m),
                            semantics: Some(site_semantics("site-blog-post-list")),
                            ..Default::default()
                        }
                        .into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    ..Default::default()
                })
                .into(),
            );
        }

        Column {
            children,
            gap: Some(tokens.spacing.xl),
            semantics: Some(site_semantics("site-blog-index")),
            ..Default::default()
        }
        .into()
    }

    fn blog_taxonomy_article(&self, tokens: &Tokens) -> Widget {
        let Some((kind, value)) = blog_taxonomy_from_route(self.route) else {
            return self.blog_index_article(tokens);
        };
        let posts = ordered_blog_routes(self.all_routes)
            .into_iter()
            .filter(|route| blog_route_has_taxonomy(route, kind, &value))
            .collect::<Vec<_>>();
        let post_cards = posts
            .iter()
            .map(|route| blog_post_card(route, false, tokens))
            .collect::<Vec<_>>();
        let title = match kind {
            BlogTaxonomyKind::Category => format!("Category: {value}"),
            BlogTaxonomyKind::Tag => format!("Tag: #{value}"),
        };
        let summary = match (posts.len(), kind) {
            (1, BlogTaxonomyKind::Category) => {
                format!("1 post is filed under the {value} category.")
            }
            (count, BlogTaxonomyKind::Category) => {
                format!("{count} posts are filed under the {value} category.")
            }
            (1, BlogTaxonomyKind::Tag) => format!("1 post uses the #{value} tag."),
            (count, BlogTaxonomyKind::Tag) => format!("{count} posts use the #{value} tag."),
        };

        let mut children = vec![Container::new(Column {
            children: vec![
                Text::new(title)
                    .size(tokens.typography.heading1_size)
                    .family(tokens.typography.font_family_serif.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .line_height(
                        tokens.typography.heading1_size * tokens.typography.line_height_heading,
                    )
                    .color(tokens.colors.heading)
                    .semantics_identifier("site-blog-taxonomy-title")
                    .into(),
                Text::new(summary)
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .into(),
                Text::new("Back to all posts")
                    .size(tokens.typography.label_large_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_link)
                    .semantics_identifier("site-route:/blog/")
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            semantics: Some(site_semantics("site-blog-taxonomy-hero")),
            ..Default::default()
        })
        .into()];

        if post_cards.is_empty() {
            children.push(
                Text::new("No posts match this selection yet.")
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );
        } else {
            children.push(
                Column {
                    children: post_cards,
                    gap: Some(tokens.spacing.m),
                    semantics: Some(site_semantics("site-blog-post-list")),
                    ..Default::default()
                }
                .into(),
            );
        }

        Column {
            children,
            gap: Some(tokens.spacing.xl),
            semantics: Some(site_semantics("site-blog-taxonomy-archive")),
            ..Default::default()
        }
        .into()
    }

    fn blog_post_article(&self, tokens: &Tokens) -> Widget {
        let mut children = Vec::new();
        children.push(
            Container::new(Column {
                children: vec![
                    Text::new(
                        blog_date_label(self.route).unwrap_or_else(|| "Fission blog".to_string()),
                    )
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                    Text::new(self.route.title.clone())
                        .size(tokens.typography.heading2_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .weight(tokens.typography.font_weight_bold)
                        .line_height(
                            tokens.typography.heading2_size * tokens.typography.line_height_heading,
                        )
                        .color(tokens.colors.heading)
                        .semantics_identifier("site-blog-post-title")
                        .into(),
                ],
                gap: Some(tokens.spacing.m),
                semantics: Some(site_semantics("site-blog-post-hero")),
                ..Default::default()
            })
            .into(),
        );
        if let Some(metadata) = self.blog_metadata(tokens) {
            children.push(metadata);
        }
        children.push(
            Container::new(MarkdownViewer {
                markdown: strip_first_heading(&self.route.body, &self.route.title),
                show_scrollbar: false,
            })
            .into(),
        );
        if self.route.show_adjacent_posts {
            if let Some(pager) = self.blog_adjacent_posts(tokens) {
                children.push(pager);
            }
        }
        children.push(
            Text::new("Back to blog")
                .size(tokens.typography.label_large_size)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.text_link)
                .semantics_identifier("site-route:/blog/")
                .into(),
        );
        Column {
            children,
            gap: Some(tokens.spacing.l),
            semantics: Some(site_semantics("site-blog-post")),
            ..Default::default()
        }
        .into()
    }

    fn blog_metadata(&self, tokens: &Tokens) -> Option<Widget> {
        let mut chips = Vec::new();
        for category in &self.route.categories {
            chips.push(taxonomy_chip("Category", category, tokens));
        }
        for tag in &self.route.tags {
            chips.push(taxonomy_chip("Tag", tag, tokens));
        }
        if chips.is_empty() {
            return None;
        }
        Some(
            Row {
                children: chips,
                gap: Some(tokens.spacing.s),
                align_items: AlignItems::Center,
                semantics: Some(site_semantics("site-blog-taxonomy")),
                ..Default::default()
            }
            .into(),
        )
    }

    fn blog_adjacent_posts(&self, tokens: &Tokens) -> Option<Widget> {
        let posts = ordered_blog_routes(self.all_routes);
        let index = posts
            .iter()
            .position(|route| route.path == self.route.path)?;
        let newer = index.checked_sub(1).map(|idx| posts[idx]);
        let older = posts.get(index + 1).copied();
        if newer.is_none() && older.is_none() {
            return None;
        }

        let mut children = Vec::new();
        if let Some(route) = older {
            children.push(adjacent_post_link("Older post", route, tokens));
        }
        if let Some(route) = newer {
            children.push(adjacent_post_link("Newer post", route, tokens));
        }

        Some(
            Container::new(Row {
                children,
                gap: Some(tokens.spacing.m),
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::SpaceBetween,
                semantics: Some(site_semantics("site-blog-adjacent-posts")),
                ..Default::default()
            })
            .padding([tokens.spacing.l, 0.0, tokens.spacing.l, 0.0])
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.large)
            .into(),
        )
    }

    fn blog_rail(&self, tokens: &Tokens) -> Widget {
        let posts = ordered_blog_routes(self.all_routes);
        let category_groups = blog_taxonomy_values(self.all_routes, BlogTaxonomyKind::Category);
        let tag_groups = blog_taxonomy_values(self.all_routes, BlogTaxonomyKind::Tag);
        let mut sections = Vec::new();

        sections.push(blog_rail_section(
            "Latest",
            posts
                .iter()
                .take(5)
                .map(|route| blog_rail_link(&route.title, &route.path, tokens))
                .collect(),
            tokens,
        ));

        if !category_groups.is_empty() {
            sections.push(blog_rail_section(
                "Categories",
                category_groups
                    .iter()
                    .map(|(label, count)| {
                        blog_count_chip(BlogTaxonomyKind::Category, label, *count, tokens)
                    })
                    .collect(),
                tokens,
            ));
        }

        if !tag_groups.is_empty() {
            sections.push(blog_rail_section(
                "Tags",
                tag_groups
                    .iter()
                    .map(|(label, count)| {
                        blog_count_chip(BlogTaxonomyKind::Tag, label, *count, tokens)
                    })
                    .collect(),
                tokens,
            ));
        }

        Column {
            children: vec![Container::new(Column {
                children: sections,
                gap: Some(tokens.spacing.l),
                ..Default::default()
            })
            .padding_all(tokens.spacing.l)
            .width(tokens.spacing.xxxxl * 3.0)
            .flex_shrink(0.0)
            .into()],
            semantics: Some(site_semantics("site-blog-rail")),
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }

    fn breadcrumbs(&self, tokens: &Tokens) -> Option<Widget> {
        let items = breadcrumb_items(&self.route.sidebar, &self.route.path);
        if items.is_empty() {
            return None;
        }
        let mut children = Vec::new();
        children.push(
            Text::new("Home")
                .size(tokens.typography.font_size_sm)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary)
                .semantics_identifier("site-route:/")
                .into(),
        );
        for (title, href) in items {
            children.push(
                Text::new("›")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_muted)
                    .into(),
            );
            children.push(
                Text::new(title)
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_link)
                    .semantics_identifier(format!("site-route:{href}"))
                    .into(),
            );
        }
        Some(
            Row {
                children,
                gap: Some(tokens.spacing.s),
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into(),
        )
    }

    fn toc(&self, tokens: &Tokens) -> Widget {
        let mut children = Vec::new();
        for heading in &self.route.headings {
            children.push(
                Text::new(heading.title.clone())
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_primary)
                    .semantics_identifier(format!("site-heading:{}", heading.anchor))
                    .into(),
            );
        }
        Column {
            children: vec![Container::new(Column {
                children,
                gap: Some(tokens.spacing.s),
                ..Default::default()
            })
            .padding_all(tokens.spacing.l)
            .width(tokens.spacing.xxxxl * 2.75)
            .flex_shrink(0.0)
            .into()],
            semantics: Some(site_semantics("site-doc-toc")),
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }
}

fn site_semantics(identifier: impl Into<String>) -> Semantics {
    Semantics {
        role: Role::Generic,
        identifier: Some(identifier.into()),
        ..Semantics::default()
    }
}

fn nav_link(label: &str, href: &str, tokens: &Tokens) -> Widget {
    Text::new(label.to_string())
        .size(tokens.typography.label_large_size)
        .weight(tokens.typography.font_weight_semibold)
        .color(tokens.colors.text_link)
        .semantics_identifier(format!("site-route:{href}"))
        .into()
}

fn nav_item(link: &SiteNavLink, depth: usize, index: usize, tokens: &Tokens) -> Widget {
    let has_children = !link.children.is_empty();
    let mut label_children = vec![nav_link(&link.title, &link.href, tokens)];
    if has_children {
        label_children.push(
            Text::new(if depth == 0 { "▾" } else { "▸" })
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.text_muted)
                .into(),
        );
    }

    let mut children = vec![Row {
        children: label_children,
        gap: Some(tokens.spacing.xs),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Start,
        semantics: Some(site_semantics(format!(
            "site-nav-label:{depth}:{has_children}:{index}"
        ))),
        ..Default::default()
    }
    .into()];

    if has_children {
        children.push(nav_menu(&link.children, depth + 1, tokens));
    }

    Column {
        children,
        semantics: Some(site_semantics(format!(
            "site-nav-item:{depth}:{has_children}:{index}"
        ))),
        ..Default::default()
    }
    .into()
}

fn taxonomy_chip(kind: &str, value: &str, tokens: &Tokens) -> Widget {
    let label = match kind {
        "Category" => value.to_string(),
        "Tag" => format!("#{value}"),
        _ => format!("{kind}: {value}"),
    };
    let mut text = Text::new(label)
        .size(tokens.typography.font_size_sm)
        .weight(tokens.typography.font_weight_semibold)
        .color(tokens.colors.text_secondary);
    if let Some(href) = blog_taxonomy_href(kind, value) {
        text = text
            .color(tokens.colors.text_link)
            .semantics_identifier(format!("site-route:{href}"));
    }
    Container::new(text)
        .padding([
            tokens.spacing.s,
            tokens.spacing.s,
            tokens.spacing.xs,
            tokens.spacing.xs,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.full)
        .into()
}

fn adjacent_post_link(label: &str, route: &ContentRoute, tokens: &Tokens) -> Widget {
    Container::new(Column {
        children: vec![
            Text::new(label)
                .size(tokens.typography.font_size_sm)
                .color(tokens.colors.text_muted)
                .into(),
            Text::new(route.title.clone())
                .size(tokens.typography.font_size_base)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.text_link)
                .semantics_identifier(format!("site-route:{}", route.path))
                .into(),
        ],
        gap: Some(tokens.spacing.xs),
        ..Default::default()
    })
    .padding_all(tokens.spacing.m)
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.medium)
    .flex_grow(1.0)
    .into()
}

fn blog_post_card(route: &ContentRoute, featured: bool, tokens: &Tokens) -> Widget {
    let mut meta = Vec::new();
    if let Some(date) = blog_date_label(route) {
        meta.push(
            Text::new(date)
                .size(tokens.typography.font_size_sm)
                .family(tokens.typography.font_family_mono.clone())
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary)
                .into(),
        );
    }
    meta.push(
        Text::new(reading_time_label(&route.body))
            .size(tokens.typography.font_size_sm)
            .color(tokens.colors.text_muted)
            .into(),
    );

    let mut children = vec![
        Row {
            children: meta,
            gap: Some(tokens.spacing.m),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into(),
        Text::new(route.title.clone())
            .size(if featured {
                tokens.typography.heading_size
            } else {
                tokens.typography.font_size_lg
            })
            .family(tokens.typography.font_family_serif.clone())
            .weight(tokens.typography.font_weight_bold)
            .line_height(if featured {
                tokens.typography.heading_size * tokens.typography.line_height_heading
            } else {
                tokens.typography.font_size_lg * tokens.typography.line_height_heading
            })
            .color(tokens.colors.heading)
            .semantics_identifier(format!("site-route:{}", route.path))
            .into(),
        Text::new(blog_excerpt(route))
            .size(tokens.typography.body_large_size)
            .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
            .color(tokens.colors.text_secondary)
            .into(),
    ];

    let chips = blog_route_chips(route, tokens);
    if !chips.is_empty() {
        children.push(
            Row {
                children: chips,
                gap: Some(tokens.spacing.s),
                wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into(),
        );
    }

    children.push(
        Text::new("Read post")
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_bold)
            .color(tokens.colors.text_link)
            .semantics_identifier(format!("site-route:{}", route.path))
            .into(),
    );

    Container::new(Column {
        children,
        gap: Some(if featured {
            tokens.spacing.l
        } else {
            tokens.spacing.m
        }),
        semantics: Some(site_semantics(if featured {
            "site-blog-featured-card"
        } else {
            "site-blog-card"
        })),
        ..Default::default()
    })
    .padding_all(if featured {
        tokens.spacing.xl
    } else {
        tokens.spacing.l
    })
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xl)
    .into()
}

fn blog_route_chips(route: &ContentRoute, tokens: &Tokens) -> Vec<Widget> {
    route
        .categories
        .iter()
        .map(|category| taxonomy_chip("Category", category, tokens))
        .chain(
            route
                .tags
                .iter()
                .map(|tag| taxonomy_chip("Tag", tag, tokens)),
        )
        .collect()
}

fn blog_rail_section(title: &str, children: Vec<Widget>, tokens: &Tokens) -> Widget {
    Container::new(Column {
        children: std::iter::once(
            Text::new(title.to_string())
                .size(tokens.typography.font_size_sm)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
        )
        .chain(children)
        .collect(),
        gap: Some(tokens.spacing.s),
        semantics: Some(site_semantics("site-blog-rail-section")),
        ..Default::default()
    })
    .into()
}

fn blog_rail_link(label: &str, href: &str, tokens: &Tokens) -> Widget {
    Text::new(label.to_string())
        .size(tokens.typography.font_size_sm)
        .weight(tokens.typography.font_weight_semibold)
        .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_snug)
        .color(tokens.colors.text_link)
        .semantics_identifier(format!("site-route:{href}"))
        .into()
}

fn blog_count_chip(kind: BlogTaxonomyKind, label: &str, count: usize, tokens: &Tokens) -> Widget {
    let href = blog_taxonomy_route(kind, label);
    Container::new(
        Text::new(format!("{label} ({count})"))
            .size(tokens.typography.font_size_sm)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_link)
            .semantics_identifier(format!("site-route:{href}")),
    )
    .padding([
        tokens.spacing.s,
        tokens.spacing.s,
        tokens.spacing.xs,
        tokens.spacing.xs,
    ])
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.full)
    .into()
}

fn nav_menu(items: &[SiteNavLink], depth: usize, tokens: &Tokens) -> Widget {
    Column {
        children: items
            .iter()
            .enumerate()
            .map(|(index, item)| nav_item(item, depth, index, tokens))
            .collect(),
        gap: Some(tokens.spacing.xs),
        semantics: Some(site_semantics(format!(
            "site-nav-menu:{depth}:{}",
            items.len()
        ))),
        ..Default::default()
    }
    .into()
}

fn theme_toggle(tokens: &Tokens) -> Widget {
    Text::new("Theme")
        .size(tokens.typography.label_large_size)
        .weight(tokens.typography.font_weight_semibold)
        .color(tokens.colors.text_link)
        .semantics_identifier("site-theme-toggle")
        .into()
}

fn search_trigger(tokens: &Tokens) -> Widget {
    Row {
        children: vec![
            Text::new("Search")
                .size(tokens.typography.label_large_size)
                .weight(tokens.typography.font_weight_semibold)
                .color(tokens.colors.text_link)
                .into(),
            Text::new("Cmd K")
                .size(tokens.typography.font_size_xs)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.text_muted)
                .into(),
        ],
        gap: Some(tokens.spacing.s),
        align_items: AlignItems::Center,
        semantics: Some(site_semantics("site-search-trigger")),
        ..Default::default()
    }
    .into()
}

fn sidebar_toggle(tokens: &Tokens) -> Widget {
    Row {
        children: vec![Text::new("Menu")
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_link)
            .into()],
        gap: Some(tokens.spacing.xs),
        align_items: AlignItems::Center,
        semantics: Some(site_semantics("site-sidebar-toggle")),
        ..Default::default()
    }
    .into()
}

pub(crate) fn extract_page_links(markdown: &str) -> Vec<HeadingLink> {
    let section_links = markdown_heading_links(markdown, false);
    if section_links.is_empty() {
        markdown_heading_links(markdown, true)
    } else {
        section_links
    }
}

fn markdown_heading_links(markdown: &str, include_h1: bool) -> Vec<HeadingLink> {
    markdown
        .lines()
        .filter_map(|line| markdown_heading(line).filter(|(level, _)| include_h1 || *level > 1))
        .map(|(_, title)| HeadingLink {
            title: title.to_string(),
            anchor: slug(title),
        })
        .collect()
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let title = trimmed.get(hashes..)?.trim();
    if title.is_empty() {
        return None;
    }
    Some((hashes, title))
}

fn body_first_heading_matches_title(body: &str, title: &str) -> bool {
    body.lines()
        .find_map(markdown_heading)
        .is_some_and(|(_, heading)| comparable_title(heading) == comparable_title(title))
}

fn comparable_title(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn section_prefix(path: &str) -> &str {
    let trimmed = path.trim_start_matches('/');
    let Some((first, _)) = trimmed.split_once('/') else {
        return path;
    };
    match first {
        "docs" => "/docs/",
        "reference" => "/reference/",
        "blog" => "/blog/",
        _ => "/",
    }
}

fn is_blog_route(path: &str) -> bool {
    path == "/blog/" || path.starts_with("/blog/")
}

fn is_blog_post_route(path: &str) -> bool {
    is_blog_route(path) && path != "/blog/" && !is_blog_taxonomy_route(path)
}

fn is_blog_index_route(path: &str) -> bool {
    path == "/blog/"
}

fn is_blog_taxonomy_route(path: &str) -> bool {
    path.starts_with("/blog/categories/") || path.starts_with("/blog/tags/")
}

fn ordered_blog_routes<'a>(routes: &'a [ContentRoute]) -> Vec<&'a ContentRoute> {
    let mut posts = routes
        .iter()
        .filter(|route| is_blog_post_route(&route.path) && route.rendered.is_none())
        .collect::<Vec<_>>();
    posts.sort_by(|a, b| blog_sort_key(b).cmp(&blog_sort_key(a)));
    posts
}

fn blog_sort_key(route: &ContentRoute) -> String {
    route
        .source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&route.path)
        .to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlogTaxonomyKind {
    Category,
    Tag,
}

fn blog_taxonomy_from_route(route: &ContentRoute) -> Option<(BlogTaxonomyKind, String)> {
    if route.path.starts_with("/blog/categories/") {
        return route
            .categories
            .first()
            .cloned()
            .or_else(|| taxonomy_value_from_path(&route.path, "/blog/categories/"))
            .map(|value| (BlogTaxonomyKind::Category, value));
    }
    if route.path.starts_with("/blog/tags/") {
        return route
            .tags
            .first()
            .cloned()
            .or_else(|| taxonomy_value_from_path(&route.path, "/blog/tags/"))
            .map(|value| (BlogTaxonomyKind::Tag, value));
    }
    None
}

fn taxonomy_value_from_path(path: &str, prefix: &str) -> Option<String> {
    let value = path.trim_start_matches(prefix).trim_end_matches('/');
    if value.is_empty() {
        None
    } else {
        Some(value.replace('-', " "))
    }
}

fn blog_route_has_taxonomy(route: &ContentRoute, kind: BlogTaxonomyKind, value: &str) -> bool {
    let target = slug(value);
    let source = match kind {
        BlogTaxonomyKind::Category => &route.categories,
        BlogTaxonomyKind::Tag => &route.tags,
    };
    source.iter().any(|item| slug(item) == target)
}

fn blog_taxonomy_href(kind: &str, value: &str) -> Option<String> {
    match kind {
        "Category" => Some(blog_taxonomy_route(BlogTaxonomyKind::Category, value)),
        "Tag" => Some(blog_taxonomy_route(BlogTaxonomyKind::Tag, value)),
        _ => None,
    }
}

fn blog_taxonomy_route(kind: BlogTaxonomyKind, value: &str) -> String {
    let segment = match kind {
        BlogTaxonomyKind::Category => "categories",
        BlogTaxonomyKind::Tag => "tags",
    };
    format!("/blog/{segment}/{}/", slug(value))
}

fn blog_taxonomy_values(routes: &[ContentRoute], kind: BlogTaxonomyKind) -> Vec<(String, usize)> {
    let mut values: Vec<(String, usize)> = Vec::new();
    for route in ordered_blog_routes(routes) {
        let source = match kind {
            BlogTaxonomyKind::Category => &route.categories,
            BlogTaxonomyKind::Tag => &route.tags,
        };
        for value in source {
            if let Some((_, count)) = values.iter_mut().find(|(label, _)| label == value) {
                *count += 1;
            } else {
                values.push((value.clone(), 1));
            }
        }
    }
    values.sort_by(|a, b| a.0.cmp(&b.0));
    values
}

fn blog_date_label(route: &ContentRoute) -> Option<String> {
    let stem = route.source_path.file_stem()?.to_str()?;
    let date = stem.get(0..10)?;
    if !date_matches_yyyy_mm_dd(date) {
        return None;
    }
    let year = date.get(0..4)?;
    let month = date.get(5..7)?.parse::<usize>().ok()?;
    let day = date.get(8..10)?.trim_start_matches('0');
    let month_name = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ]
    .get(month.checked_sub(1)?)?;
    Some(format!("{month_name} {day}, {year}"))
}

fn date_matches_yyyy_mm_dd(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| index == 4 || index == 7 || ch.is_ascii_digit())
}

fn reading_time_label(markdown: &str) -> String {
    let words = markdown
        .split_whitespace()
        .filter(|word| !word.starts_with('#') && *word != "{/*" && *word != "*/}")
        .count();
    let minutes = words.div_ceil(225).max(1);
    format!("{minutes} min read")
}

fn blog_excerpt(route: &ContentRoute) -> String {
    if let Some(description) = &route.description {
        if !description.trim().is_empty() {
            return description.clone();
        }
    }
    route
        .body
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("```")
                && *line != "{/* truncate */}"
                && *line != "<!-- truncate -->"
        })
        .next()
        .unwrap_or("Read the latest Fission update.")
        .chars()
        .take(220)
        .collect()
}

fn strip_first_heading(markdown: &str, title: &str) -> String {
    let mut stripped = false;
    markdown
        .lines()
        .filter(|line| {
            if stripped {
                return true;
            }
            if markdown_heading(line)
                .is_some_and(|(_, heading)| comparable_title(heading) == comparable_title(title))
            {
                stripped = true;
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_link_path(path: &str) -> String {
    let mut out = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if out.len() > 1 && !out.ends_with('/') {
        out.push('/');
    }
    out
}

fn active_sidebar_index(items: &[SidebarLink], current_path: &str) -> Option<usize> {
    let current = normalize_link_path(current_path);
    items
        .iter()
        .position(|item| normalize_link_path(&item.href) == current)
        .or_else(|| {
            items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    let href = normalize_link_path(&item.href);
                    let prefix = href.trim_end_matches('/');
                    (prefix.len() > 1 && current.starts_with(prefix)).then_some(index)
                })
                .last()
        })
}

fn ancestor_at_level(items: &[SidebarLink], index: usize, level: usize) -> Option<usize> {
    items
        .iter()
        .take(index + 1)
        .enumerate()
        .rev()
        .find_map(|(candidate, item)| (item.level == level).then_some(candidate))
}

fn breadcrumb_items(items: &[SidebarLink], current_path: &str) -> Vec<(String, String)> {
    let Some(active) = active_sidebar_index(items, current_path) else {
        return Vec::new();
    };
    let active_level = items[active].level;
    let mut out = Vec::new();
    for level in 1..=active_level {
        if let Some(index) = ancestor_at_level(items, active, level) {
            out.push((items[index].title.clone(), items[index].href.clone()));
        }
    }
    if out
        .last()
        .is_none_or(|(_, href)| normalize_link_path(href) != normalize_link_path(current_path))
    {
        out.push((items[active].title.clone(), items[active].href.clone()));
    }
    out
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_dash = false;
        } else if !previous_dash && !out.is_empty() {
            out.push('-');
            previous_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_links_prefer_sections() {
        let links = extract_page_links("# Getting Started\n\n## Install Rust\n### Add target");
        assert_eq!(links[0].title, "Install Rust");
        assert_eq!(links[0].anchor, "install-rust");
        assert_eq!(links[1].title, "Add target");
    }
}
