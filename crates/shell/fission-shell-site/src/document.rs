use fission_core::op::{AlignItems, Fill, JustifyContent};
use fission_core::ui::{Column, Container, Image, Row, Text};
use fission_core::{AppState, BuildCtx, Node, View, Widget};
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
    pub source_path: PathBuf,
    pub rendered: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteNavLink {
    pub title: String,
    pub href: String,
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
impl AppState for SitePageState {}

pub(crate) struct DocumentationPage<'a> {
    pub site_title: &'a str,
    pub site_logo: Option<&'a str>,
    pub site_nav: &'a [SiteNavLink],
    pub theme_switching: bool,
    pub search_enabled: bool,
    pub route: &'a ContentRoute,
    pub all_routes: &'a [ContentRoute],
}

impl<'a> Widget<SitePageState> for DocumentationPage<'a> {
    fn build(&self, ctx: &mut BuildCtx<SitePageState>, view: &View<SitePageState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Column {
                children: vec![self.header(tokens), self.document_grid(ctx, view)],
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        )
        .min_height(tokens.spacing.xxxxl * 9.0)
        .bg_fill(Fill::Solid(tokens.colors.background))
        .into_node()
    }
}

impl DocumentationPage<'_> {
    fn header(&self, tokens: &Tokens) -> Node {
        let mut children = vec![sidebar_toggle(tokens), self.brand(tokens)];
        if !self.site_nav.is_empty() {
            children.push(
                Row {
                    children: self
                        .site_nav
                        .iter()
                        .map(|link| nav_link(&link.title, &link.href, tokens))
                        .collect(),
                    gap: Some(tokens.spacing.l),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::End,
                    semantics: Some(site_semantics("site-doc-nav")),
                    ..Default::default()
                }
                .into_node(),
            );
        }
        if self.theme_switching {
            children.push(theme_toggle(tokens));
        }
        if self.search_enabled {
            children.push(search_trigger(tokens));
        }
        Container::new(
            Row {
                children,
                gap: Some(tokens.spacing.m),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                semantics: Some(site_semantics("site-doc-header")),
                ..Default::default()
            }
            .into_node(),
        )
        .padding([
            tokens.spacing.xxxxl,
            tokens.spacing.xxxxl,
            tokens.spacing.l,
            tokens.spacing.l,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface))
        .border(tokens.colors.border, 1.0)
        .into_node()
    }

    fn brand(&self, tokens: &Tokens) -> Node {
        let mut children = Vec::new();
        if let Some(logo) = self.site_logo {
            children.push(
                Image::asset(logo.to_string())
                    .size(tokens.spacing.xl, tokens.spacing.xl)
                    .into_node(),
            );
        }
        children.push(
            Text::new(self.site_title)
                .size(tokens.typography.font_size_lg)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .semantics_identifier("site-route:/")
                .into_node(),
        );
        Row {
            children,
            gap: Some(tokens.spacing.s),
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into_node()
    }

    fn document_grid(&self, ctx: &mut BuildCtx<SitePageState>, view: &View<SitePageState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Row {
            children: vec![
                self.sidebar(tokens),
                self.article(ctx, view),
                self.toc(tokens),
            ],
            semantics: Some(site_semantics("site-doc-layout")),
            gap: Some(tokens.spacing.xl),
            align_items: AlignItems::Stretch,
            ..Default::default()
        }
        .into_node()
    }

    fn sidebar(&self, tokens: &Tokens) -> Node {
        let mut children = Vec::new();
        if self.route.sidebar.is_empty() {
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
            children: vec![Container::new(
                Column {
                    children,
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into_node(),
            )
            .padding_all(tokens.spacing.l)
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(tokens.colors.border, 1.0)
            .width(tokens.spacing.xxxxl * 3.0)
            .min_height(tokens.spacing.xxxxl * 9.0)
            .flex_shrink(0.0)
            .into_node()],
            semantics: Some(site_semantics("site-doc-sidebar")),
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into_node()
    }

    fn sidebar_item(
        &self,
        title: &str,
        href: &str,
        level: usize,
        group: bool,
        index: usize,
        tokens: &Tokens,
    ) -> Node {
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
                .semantics_identifier(format!("site-route:{href}"))
                .into_node(),
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
            children: vec![item.into_node()],
            semantics: Some(site_semantics(format!(
                "site-sidebar-item:{level}:{active}:{group}:{index}"
            ))),
            ..Default::default()
        }
        .into_node()
    }

    fn article(&self, ctx: &mut BuildCtx<SitePageState>, view: &View<SitePageState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut children = Vec::new();
        if let Some(breadcrumbs) = self.breadcrumbs(tokens) {
            children.push(breadcrumbs);
        }
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
                    .into_node(),
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
                        .into_node(),
                );
            }
        }
        let markdown = MarkdownViewer {
            markdown: self.route.body.clone(),
            show_scrollbar: false,
        };
        children.push(markdown.build(ctx, view));

        Column {
            children: vec![Container::new(
                Column {
                    children,
                    gap: Some(tokens.spacing.l),
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
            )
            .padding([0.0, 0.0, tokens.spacing.xxl, tokens.spacing.xxl])
            .bg_fill(Fill::Solid(tokens.colors.background))
            .flex_grow(1.0)
            .into_node()],
            semantics: Some(site_semantics("site-doc-main")),
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node()
    }

    fn breadcrumbs(&self, tokens: &Tokens) -> Option<Node> {
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
                .into_node(),
        );
        for (title, href) in items {
            children.push(
                Text::new(">")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_muted)
                    .into_node(),
            );
            children.push(
                Text::new(title)
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_link)
                    .semantics_identifier(format!("site-route:{href}"))
                    .into_node(),
            );
        }
        Some(
            Row {
                children,
                gap: Some(tokens.spacing.s),
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into_node(),
        )
    }

    fn toc(&self, tokens: &Tokens) -> Node {
        let mut children = Vec::new();
        for heading in &self.route.headings {
            children.push(
                Text::new(heading.title.clone())
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_primary)
                    .semantics_identifier(format!("site-heading:{}", heading.anchor))
                    .into_node(),
            );
        }
        Column {
            children: vec![Container::new(
                Column {
                    children,
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into_node(),
            )
            .padding_all(tokens.spacing.l)
            .width(tokens.spacing.xxxxl * 2.75)
            .flex_shrink(0.0)
            .into_node()],
            semantics: Some(site_semantics("site-doc-toc")),
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into_node()
    }
}

fn site_semantics(identifier: impl Into<String>) -> Semantics {
    Semantics {
        role: Role::Generic,
        identifier: Some(identifier.into()),
        ..Semantics::default()
    }
}

fn nav_link(label: &str, href: &str, tokens: &Tokens) -> Node {
    Text::new(label.to_string())
        .size(tokens.typography.label_large_size)
        .weight(tokens.typography.font_weight_semibold)
        .color(tokens.colors.text_link)
        .semantics_identifier(format!("site-route:{href}"))
        .into_node()
}

fn theme_toggle(tokens: &Tokens) -> Node {
    Text::new("Theme")
        .size(tokens.typography.label_large_size)
        .weight(tokens.typography.font_weight_semibold)
        .color(tokens.colors.text_link)
        .semantics_identifier("site-theme-toggle")
        .into_node()
}

fn search_trigger(tokens: &Tokens) -> Node {
    Row {
        children: vec![
            Text::new("Search")
                .size(tokens.typography.label_large_size)
                .weight(tokens.typography.font_weight_semibold)
                .color(tokens.colors.text_link)
                .into_node(),
            Text::new("Cmd K")
                .size(tokens.typography.font_size_xs)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.text_muted)
                .into_node(),
        ],
        gap: Some(tokens.spacing.s),
        align_items: AlignItems::Center,
        semantics: Some(site_semantics("site-search-trigger")),
        ..Default::default()
    }
    .into_node()
}

fn sidebar_toggle(tokens: &Tokens) -> Node {
    Row {
        children: vec![Text::new("Menu")
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_link)
            .into_node()],
        gap: Some(tokens.spacing.xs),
        align_items: AlignItems::Center,
        semantics: Some(site_semantics("site-sidebar-toggle")),
        ..Default::default()
    }
    .into_node()
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
