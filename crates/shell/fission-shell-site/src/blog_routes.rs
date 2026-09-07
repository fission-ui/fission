use crate::build::{load_sidebar, SiteBuildOptions};
use crate::document::{extract_page_links, ContentRoute, SidebarLink, BLOG_PAGE_SIZE};
use crate::site::normalize_site_path;
use anyhow::Result;
use std::path::PathBuf;

pub(crate) fn add_generated_blog_routes(
    options: &SiteBuildOptions,
    routes: &mut Vec<ContentRoute>,
) -> Result<()> {
    for config in &options.content_routes {
        if config.template.as_deref() != Some("fission::site::blog") {
            continue;
        }
        let prefix = normalize_site_path(&config.path);
        let posts = routes
            .iter()
            .filter(|route| {
                route.path.starts_with(&prefix)
                    && route.path != prefix
                    && !is_blog_taxonomy_path(&route.path, &prefix)
            })
            .cloned()
            .collect::<Vec<_>>();
        if posts.is_empty() {
            continue;
        }
        let sidebar = load_sidebar(config.sidebar.as_deref())?;
        add_blog_index(routes, &posts, &sidebar, &prefix, &options.site_title);
        add_blog_pages(routes, &posts, &sidebar, &prefix, &options.site_title);
        add_blog_taxonomy_routes(routes, &posts, &sidebar, &prefix);
    }
    Ok(())
}

fn add_blog_index(
    routes: &mut Vec<ContentRoute>,
    posts: &[ContentRoute],
    sidebar: &[SidebarLink],
    prefix: &str,
    site_title: &str,
) {
    if routes.iter().any(|route| route.path == prefix) {
        return;
    }
    let body = blog_landing_markdown(posts);
    routes.push(ContentRoute {
        path: prefix.to_string(),
        mount_path: prefix.to_string(),
        template: Some("fission::site::blog".to_string()),
        title: "Blog".to_string(),
        description: Some(format!("Posts and updates from {site_title}.")),
        locale: None,
        headings: extract_page_links(&body),
        sidebar: sidebar.to_vec(),
        tags: Vec::new(),
        categories: Vec::new(),
        show_adjacent_posts: false,
        body,
        source_path: PathBuf::from("<generated-blog-index>"),
        rendered: None,
    });
}

fn add_blog_pages(
    routes: &mut Vec<ContentRoute>,
    posts: &[ContentRoute],
    sidebar: &[SidebarLink],
    prefix: &str,
    site_title: &str,
) {
    for page in 2..=posts.len().div_ceil(BLOG_PAGE_SIZE) {
        let path = format!("{prefix}page/{page}/");
        if routes.iter().any(|route| route.path == path) {
            continue;
        }
        let body = blog_landing_markdown(posts);
        routes.push(ContentRoute {
            path,
            mount_path: prefix.to_string(),
            template: Some("fission::site::blog".to_string()),
            title: format!("Blog — page {page}"),
            description: Some(format!("Posts and updates from {site_title}, page {page}.")),
            locale: None,
            headings: extract_page_links(&body),
            sidebar: sidebar.to_vec(),
            tags: Vec::new(),
            categories: Vec::new(),
            show_adjacent_posts: false,
            body,
            source_path: PathBuf::from(format!("<generated-blog-page-{page}>")),
            rendered: None,
        });
    }
}

fn add_blog_taxonomy_routes(
    routes: &mut Vec<ContentRoute>,
    posts: &[ContentRoute],
    sidebar: &[SidebarLink],
    prefix: &str,
) {
    for category in unique_taxonomy_values(posts, BlogTaxonomyKind::Category) {
        let path = blog_taxonomy_route(prefix, BlogTaxonomyKind::Category, &category);
        if routes.iter().any(|route| route.path == path) {
            continue;
        }
        let body = format!("# {category}\n\nPosts filed under the {category} category.\n");
        routes.push(ContentRoute {
            path,
            mount_path: prefix.to_string(),
            template: Some("fission::site::blog".to_string()),
            title: format!("{category} posts"),
            description: Some(format!("Posts filed under the {category} category.")),
            locale: None,
            headings: extract_page_links(&body),
            sidebar: sidebar.to_vec(),
            tags: Vec::new(),
            categories: vec![category.clone()],
            show_adjacent_posts: false,
            body,
            source_path: PathBuf::from(format!(
                "<generated-blog-category-{}>",
                taxonomy_slug(&category)
            )),
            rendered: None,
        });
    }

    for tag in unique_taxonomy_values(posts, BlogTaxonomyKind::Tag) {
        let path = blog_taxonomy_route(prefix, BlogTaxonomyKind::Tag, &tag);
        if routes.iter().any(|route| route.path == path) {
            continue;
        }
        let body = format!("# #{tag}\n\nPosts tagged #{tag}.\n");
        routes.push(ContentRoute {
            path,
            mount_path: prefix.to_string(),
            template: Some("fission::site::blog".to_string()),
            title: format!("#{tag} posts"),
            description: Some(format!("Posts tagged #{tag}.")),
            locale: None,
            headings: extract_page_links(&body),
            sidebar: sidebar.to_vec(),
            tags: vec![tag.clone()],
            categories: Vec::new(),
            show_adjacent_posts: false,
            body,
            source_path: PathBuf::from(format!("<generated-blog-tag-{}>", taxonomy_slug(&tag))),
            rendered: None,
        });
    }
}

#[derive(Clone, Copy)]
enum BlogTaxonomyKind {
    Category,
    Tag,
}

fn unique_taxonomy_values(posts: &[ContentRoute], kind: BlogTaxonomyKind) -> Vec<String> {
    let mut values = posts
        .iter()
        .flat_map(|route| match kind {
            BlogTaxonomyKind::Category => route.categories.iter(),
            BlogTaxonomyKind::Tag => route.tags.iter(),
        })
        .cloned()
        .collect::<Vec<_>>();
    values.sort_by_key(|value| value.to_ascii_lowercase());
    values.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    values
}

fn blog_taxonomy_route(prefix: &str, kind: BlogTaxonomyKind, value: &str) -> String {
    let segment = match kind {
        BlogTaxonomyKind::Category => "categories",
        BlogTaxonomyKind::Tag => "tags",
    };
    normalize_site_path(&format!("{prefix}{segment}/{}", taxonomy_slug(value)))
}

fn taxonomy_slug(value: &str) -> String {
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
    let slug = out.trim_matches('-');
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug.to_string()
    }
}

fn is_blog_taxonomy_path(path: &str, prefix: &str) -> bool {
    path.starts_with(&format!("{prefix}categories/")) || path.starts_with(&format!("{prefix}tags/"))
}

fn blog_landing_markdown(posts: &[ContentRoute]) -> String {
    let mut posts = posts.to_vec();
    posts.sort_by(|a, b| {
        b.source_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(&b.path)
            .cmp(
                a.source_path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or(&a.path),
            )
    });

    let mut tags = posts
        .iter()
        .flat_map(|route| route.tags.iter().cloned())
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    let mut categories = posts
        .iter()
        .flat_map(|route| route.categories.iter().cloned())
        .collect::<Vec<_>>();
    categories.sort();
    categories.dedup();

    let mut body = String::from("# Latest posts\n\n");
    if !categories.is_empty() {
        body.push_str("## Categories\n\n");
        body.push_str(&categories.join(", "));
        body.push_str("\n\n");
    }
    if !tags.is_empty() {
        body.push_str("## Tags\n\n");
        body.push_str(&tags.join(", "));
        body.push_str("\n\n");
    }
    body.push_str("## Latest posts\n\n");
    for route in posts {
        body.push_str(&format!("- [{}]({})", route.title, route.path));
        if let Some(description) = &route.description {
            body.push_str(&format!(" — {description}"));
        }
        body.push('\n');
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_slugs_are_stable() {
        assert_eq!(taxonomy_slug("Release notes"), "release-notes");
        assert_eq!(taxonomy_slug("  "), "untitled");
    }

    #[test]
    fn taxonomy_routes_follow_the_configured_blog_mount() {
        assert_eq!(
            blog_taxonomy_route("/journal/", BlogTaxonomyKind::Tag, "Release notes"),
            "/journal/tags/release-notes/"
        );
    }
}
