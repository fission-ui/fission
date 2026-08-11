use super::routing::{
    first_h1, resolve_relative_markdown_links, route_path_from_file, route_path_from_slug,
    title_from_path,
};
use super::{resolve_project_path, ProjectSitePageElement, SidebarManifest, SiteBuildOptions};
use crate::document::{extract_page_links, ContentRoute, SidebarLink};
use crate::front_matter::split_front_matter;
use crate::site::{
    normalize_site_path, ContentTransform, SitePageElement, SitePageElementFilter,
    SitePageElementPlacement,
};
use crate::tabs::expand_mdx_tabs;
use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn load_content_routes(
    options: &SiteBuildOptions,
    transform: Option<&ContentTransform>,
) -> Result<Vec<ContentRoute>> {
    let mut routes = Vec::new();
    for config in &options.content_routes {
        if !config.source.exists() {
            bail!(
                "site content directory {} does not exist; create it or update fission.toml",
                config.source.display()
            );
        }
        let sidebar = load_sidebar(config.sidebar.as_deref())?;
        let mut files = Vec::new();
        collect_markdown_files(&config.source, &mut files)?;
        for file in files {
            let source = fs::read_to_string(&file)
                .with_context(|| format!("failed to read content file {}", file.display()))?;
            let (front, mut body) = split_front_matter(&source);
            if let Some(transform) = transform {
                body = transform(&body, &options.project_dir, &file)?;
            }
            body = expand_mdx_tabs(&body)?;
            body = strip_mdx_control_markers(&body);
            body = resolve_relative_markdown_links(&body, &config.path, &config.source, &file);
            let title = front
                .title
                .or_else(|| first_h1(&body))
                .unwrap_or_else(|| title_from_path(&file));
            let route_path = front
                .slug
                .map(|slug| route_path_from_slug(&config.path, &slug))
                .unwrap_or_else(|| route_path_from_file(&config.path, &config.source, &file));
            routes.push(ContentRoute {
                path: normalize_site_path(&route_path),
                title,
                description: front.description,
                locale: front.locale,
                headings: extract_page_links(&body),
                sidebar: sidebar.clone(),
                tags: front.tags,
                categories: front.categories,
                show_adjacent_posts: front.show_adjacent_posts.unwrap_or(true),
                body,
                source_path: file,
                rendered: None,
            });
        }
    }
    add_generated_blog_routes(options, &mut routes)?;
    if routes.is_empty() && site_has_content_routes(options) {
        bail!("configured site content routes contain no .md or .mdx files");
    }
    Ok(routes)
}

fn add_generated_blog_routes(
    options: &SiteBuildOptions,
    routes: &mut Vec<ContentRoute>,
) -> Result<()> {
    for config in &options.content_routes {
        let prefix = normalize_site_path(&config.path);
        if prefix != "/blog/" {
            continue;
        }
        let posts = routes
            .iter()
            .filter(|route| {
                route.path.starts_with(&prefix)
                    && route.path != prefix
                    && !is_blog_taxonomy_path(&route.path)
            })
            .cloned()
            .collect::<Vec<_>>();
        if posts.is_empty() {
            continue;
        }
        let sidebar = load_sidebar(config.sidebar.as_deref())?;
        if !routes.iter().any(|route| route.path == prefix) {
            let body = blog_landing_markdown(&posts);
            routes.push(ContentRoute {
                path: prefix.clone(),
                title: "Blog".to_string(),
                description: Some(
                    "Technical posts, release notes, and product updates from Fission.".to_string(),
                ),
                locale: None,
                headings: extract_page_links(&body),
                sidebar: sidebar.clone(),
                tags: Vec::new(),
                categories: Vec::new(),
                show_adjacent_posts: false,
                body,
                source_path: PathBuf::from("<generated-blog-index>"),
                rendered: None,
            });
        }
        add_generated_blog_taxonomy_routes(routes, &posts, &sidebar);
    }
    Ok(())
}

fn add_generated_blog_taxonomy_routes(
    routes: &mut Vec<ContentRoute>,
    posts: &[ContentRoute],
    sidebar: &[SidebarLink],
) {
    let categories = unique_taxonomy_values(posts, BlogTaxonomyKind::Category);
    for category in categories {
        let path = blog_taxonomy_route(BlogTaxonomyKind::Category, &category);
        if routes.iter().any(|route| route.path == path) {
            continue;
        }
        let body = format!("# {category}\n\nPosts filed under the {category} category.\n");
        routes.push(ContentRoute {
            path,
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

    let tags = unique_taxonomy_values(posts, BlogTaxonomyKind::Tag);
    for tag in tags {
        let path = blog_taxonomy_route(BlogTaxonomyKind::Tag, &tag);
        if routes.iter().any(|route| route.path == path) {
            continue;
        }
        let body = format!("# #{tag}\n\nPosts tagged #{tag}.\n");
        routes.push(ContentRoute {
            path,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

fn blog_taxonomy_route(kind: BlogTaxonomyKind, value: &str) -> String {
    let segment = match kind {
        BlogTaxonomyKind::Category => "categories",
        BlogTaxonomyKind::Tag => "tags",
    };
    normalize_site_path(&format!("/blog/{segment}/{}", taxonomy_slug(value)))
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

fn is_blog_taxonomy_path(path: &str) -> bool {
    path.starts_with("/blog/categories/") || path.starts_with("/blog/tags/")
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

    let mut body = String::from(
        "# Blog\n\nTechnical posts, release notes, and product updates from the Fission team.\n\n",
    );
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

fn strip_mdx_control_markers(markdown: &str) -> String {
    markdown
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != "{/* truncate */}" && trimmed != "<!-- truncate -->"
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_sidebar(path: Option<&Path>) -> Result<Vec<SidebarLink>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        bail!(
            "configured static site sidebar {} does not exist",
            path.display()
        );
    }
    let data =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: SidebarManifest =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(manifest
        .items
        .into_iter()
        .map(|item| SidebarLink {
            title: item.title,
            href: normalize_site_path(&item.href),
            level: item.level,
            group: item.group,
        })
        .collect())
}

pub(super) fn load_project_page_elements(
    project_dir: &Path,
    elements: Vec<ProjectSitePageElement>,
) -> Result<Vec<SitePageElement>> {
    elements
        .into_iter()
        .map(|element| {
            let placement = SitePageElementPlacement::parse(&element.placement)?;
            let html = match (element.html, element.file) {
                (Some(html), None) => html,
                (None, Some(path)) => {
                    let path = resolve_project_path(project_dir, PathBuf::from(path));
                    fs::read_to_string(&path).with_context(|| {
                        format!("failed to read static site page element {}", path.display())
                    })?
                }
                (Some(_), Some(_)) => {
                    bail!("static site page element cannot set both `html` and `file`")
                }
                (None, None) => {
                    bail!("static site page element requires either `html` or `file`")
                }
            };
            let mut out = SitePageElement::new(placement, html);
            for route in element.routes {
                out = out.filter(SitePageElementFilter::exact(route));
            }
            for prefix in element.route_prefixes {
                out = out.filter(SitePageElementFilter::prefix(prefix));
            }
            Ok(out)
        })
        .collect()
}

fn site_has_content_routes(options: &SiteBuildOptions) -> bool {
    !options.content_routes.is_empty()
}

fn collect_markdown_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_markdown_files(&path, out)?;
        } else if is_markdown_file(&path) {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

fn is_markdown_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("md") | Some("mdx")
    )
}
