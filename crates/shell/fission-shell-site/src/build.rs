use crate::document::{ContentRoute, SiteNavLink};
use crate::html::{CodeHighlightingOptions, StyleRegistry};
use crate::search::SiteSearchOptions;
use crate::site::{normalize_site_path, FissionSite, SitePageElement};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

mod content;
mod output;
mod render;
mod routing;
mod validation;

#[cfg(test)]
mod tests;

const SITE_CSS: &str = include_str!("../assets/site.css");
const SITE_ENHANCEMENT_JS: &str = include_str!("../assets/site-enhancement.js");
const SEARCH_JS: &str = include_str!("../assets/search.js");

pub fn site_base_css() -> &'static str {
    SITE_CSS
}

pub fn site_enhancement_js() -> &'static str {
    SITE_ENHANCEMENT_JS
}

#[derive(Clone, Debug)]
pub struct SiteBuildOptions {
    pub project_dir: PathBuf,
    pub output_dir: PathBuf,
    pub site_title: String,
    pub site_description: Option<String>,
    pub site_logo: Option<String>,
    pub site_favicon: Option<String>,
    pub base_url: Option<String>,
    pub default_locale: String,
    pub site_nav: Vec<SiteNavLink>,
    pub user_css: Vec<String>,
    pub page_elements: Vec<SitePageElement>,
    pub content_routes: Vec<SiteContentRouteConfig>,
    pub asset_dirs: Vec<PathBuf>,
    pub generate_sitemap: bool,
    pub generate_robots: bool,
    pub code_highlighting: CodeHighlightingOptions,
    pub search: SiteSearchOptions,
    pub clean: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteContentRouteConfig {
    pub path: String,
    pub source: PathBuf,
    pub template: Option<String>,
    pub sidebar: Option<PathBuf>,
}

impl SiteBuildOptions {
    pub fn for_project(project_dir: impl Into<PathBuf>, site_title: impl Into<String>) -> Self {
        let project_dir = project_dir.into();
        Self {
            output_dir: project_dir.join("target/fission/site"),
            project_dir: project_dir.clone(),
            site_title: site_title.into(),
            site_description: None,
            site_logo: None,
            site_favicon: None,
            base_url: None,
            default_locale: "en".to_string(),
            site_nav: Vec::new(),
            user_css: Vec::new(),
            page_elements: Vec::new(),
            content_routes: vec![SiteContentRouteConfig {
                path: "/content".to_string(),
                source: project_dir.join("content"),
                template: None,
                sidebar: None,
            }],
            asset_dirs: Vec::new(),
            generate_sitemap: false,
            generate_robots: false,
            code_highlighting: CodeHighlightingOptions::default(),
            search: SiteSearchOptions::default(),
            clean: true,
        }
    }

    pub fn from_project_dir(
        project_dir: impl Into<PathBuf>,
        fallback_title: impl Into<String>,
    ) -> Result<Self> {
        let project_dir = project_dir.into();
        let path = project_dir.join("fission.toml");
        let data = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let manifest: ProjectManifest =
            toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
        let app_name = manifest.app.as_ref().map(|app| app.name.clone());
        let site = manifest.site.unwrap_or_default();
        let site_title = site
            .title
            .or(app_name)
            .unwrap_or_else(|| fallback_title.into());
        let site_logo = site.logo.as_deref().map(normalize_site_asset_href);
        let site_favicon = site.favicon.as_deref().map(normalize_site_asset_href);
        let base_url = site
            .base_url
            .map(|url| url.trim_end_matches('/').to_string());
        let default_locale = site.default_locale.unwrap_or_else(|| "en".to_string());
        let site_nav = site
            .nav
            .into_iter()
            .map(normalize_project_site_nav_link)
            .collect();
        let user_css = site
            .css_files
            .into_iter()
            .map(|path| {
                let path = resolve_project_path(&project_dir, PathBuf::from(path));
                fs::read_to_string(&path)
                    .with_context(|| format!("failed to read site CSS {}", path.display()))
            })
            .collect::<Result<Vec<_>>>()?;
        let page_elements = content::load_project_page_elements(&project_dir, site.elements)?;
        let output_dir = resolve_project_path(
            &project_dir,
            site.out_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("target/fission/site")),
        );
        let content_routes = if site.routes.is_empty() {
            vec![SiteContentRouteConfig {
                path: "/content".to_string(),
                source: project_dir.join("content"),
                template: None,
                sidebar: None,
            }]
        } else {
            site.routes
                .into_iter()
                .filter(|route| route.kind.as_deref().unwrap_or("content") == "content")
                .map(|route| SiteContentRouteConfig {
                    path: normalize_site_path(&route.path),
                    source: resolve_project_path(&project_dir, PathBuf::from(route.source)),
                    template: route.template,
                    sidebar: route
                        .sidebar
                        .map(|path| resolve_project_path(&project_dir, PathBuf::from(path))),
                })
                .collect()
        };
        let asset_dirs = site
            .asset_dirs
            .into_iter()
            .map(|path| resolve_project_path(&project_dir, PathBuf::from(path)))
            .collect();
        let generate_sitemap = site.generate_sitemap.unwrap_or(false);
        let generate_robots = site.generate_robots.unwrap_or(false);
        let code_highlighting = site
            .code_highlighting
            .map(CodeHighlightingOptions::from)
            .unwrap_or_default();
        let search = site.search.map(SiteSearchOptions::from).unwrap_or_default();
        Ok(Self {
            project_dir,
            output_dir,
            site_title,
            site_description: site.description,
            site_logo,
            site_favicon,
            base_url,
            default_locale,
            site_nav,
            user_css,
            page_elements,
            content_routes,
            asset_dirs,
            generate_sitemap,
            generate_robots,
            code_highlighting,
            search,
            clean: true,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteRouteReport {
    pub path: String,
    pub title: String,
    pub source: PathBuf,
    pub output: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteBuildReport {
    pub output_dir: PathBuf,
    pub routes: Vec<SiteRouteReport>,
}

pub fn build_content_site(options: &SiteBuildOptions) -> Result<SiteBuildReport> {
    build_site(options, &FissionSite::new())
}

pub fn check_content_site(options: &SiteBuildOptions) -> Result<SiteBuildReport> {
    check_site(options, &FissionSite::new())
}

pub fn list_content_routes(options: &SiteBuildOptions) -> Result<Vec<SiteRouteReport>> {
    list_site_routes(options, &FissionSite::new())
}

pub fn build_site(options: &SiteBuildOptions, site: &FissionSite) -> Result<SiteBuildReport> {
    let mut routes = content::load_content_routes(options, site.content_transform.as_deref())?;
    let mut styles = StyleRegistry::default();
    let custom_routes = render::render_custom_routes(options, site, &mut styles)?;
    routes.extend(custom_routes);
    routes.sort_by(|a, b| a.path.cmp(&b.path));
    routing::detect_duplicate_routes(&routes)?;

    if options.clean && options.output_dir.exists() {
        fs::remove_dir_all(&options.output_dir).with_context(|| {
            format!(
                "failed to clean site output dir {}",
                options.output_dir.display()
            )
        })?;
    }
    output::prepare_output_dir(options)?;
    output::copy_asset_dirs(options)?;

    let mut rendered_routes = Vec::new();
    for route in &routes {
        let html = render::render_route(route, &routes, options, site, &mut styles)?;
        rendered_routes.push((route, html));
    }
    output::write_site_css(options, site, &styles)?;
    output::write_site_enhancement_js(options)?;

    let mut report_routes = Vec::new();
    for (route, html) in rendered_routes {
        let output = routing::output_path_for_route(&options.output_dir, &route.path);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output, html)
            .with_context(|| format!("failed to write {}", output.display()))?;
        report_routes.push(SiteRouteReport {
            path: route.path.clone(),
            title: route.title.clone(),
            source: route.source_path.clone(),
            output,
        });
    }

    output::write_search_assets_if_needed(options, &routes)?;
    output::write_root_index_if_needed(options, &routes)?;
    output::write_sitemap_if_needed(options, &routes)?;
    output::write_robots_if_needed(options)?;
    validation::validate_generated_internal_links(&options.output_dir)?;

    Ok(SiteBuildReport {
        output_dir: options.output_dir.clone(),
        routes: report_routes,
    })
}

pub fn check_site(options: &SiteBuildOptions, site: &FissionSite) -> Result<SiteBuildReport> {
    let mut routes = content::load_content_routes(options, site.content_transform.as_deref())?;
    let mut styles = StyleRegistry::default();
    routes.extend(render::render_custom_routes(options, site, &mut styles)?);
    routes.sort_by(|a, b| a.path.cmp(&b.path));
    routing::detect_duplicate_routes(&routes)?;

    let mut report_routes = Vec::new();
    for route in &routes {
        render::render_route(route, &routes, options, site, &mut styles)?;
        report_routes.push(SiteRouteReport {
            path: route.path.clone(),
            title: route.title.clone(),
            source: route.source_path.clone(),
            output: routing::output_path_for_route(&options.output_dir, &route.path),
        });
    }
    Ok(SiteBuildReport {
        output_dir: options.output_dir.clone(),
        routes: report_routes,
    })
}

pub fn list_site_routes(
    options: &SiteBuildOptions,
    site: &FissionSite,
) -> Result<Vec<SiteRouteReport>> {
    let mut routes = content::load_content_routes(options, site.content_transform.as_deref())?;
    for route in &site.custom_routes {
        routes.push(ContentRoute {
            path: route.path.clone(),
            title: route.title.clone(),
            description: route.description.clone(),
            locale: None,
            body: String::new(),
            headings: Vec::new(),
            sidebar: Vec::new(),
            tags: Vec::new(),
            categories: Vec::new(),
            show_adjacent_posts: false,
            source_path: PathBuf::from("<custom>"),
            rendered: None,
        });
    }
    routes.sort_by(|a, b| a.path.cmp(&b.path));
    routing::detect_duplicate_routes(&routes)?;
    Ok(routes
        .iter()
        .map(|route| SiteRouteReport {
            path: route.path.clone(),
            title: route.title.clone(),
            source: route.source_path.clone(),
            output: routing::output_path_for_route(&options.output_dir, &route.path),
        })
        .collect())
}

fn resolve_project_path(project_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

fn normalize_site_link_href(value: &str) -> String {
    let value = value.trim();
    if is_absolute_href(value) || value.starts_with('#') {
        value.to_string()
    } else {
        normalize_site_path(value)
    }
}

fn normalize_project_site_nav_link(link: ProjectSiteNavLink) -> SiteNavLink {
    SiteNavLink {
        title: link.title,
        href: normalize_site_link_href(&link.href),
        children: link
            .children
            .into_iter()
            .map(normalize_project_site_nav_link)
            .collect(),
    }
}

fn normalize_site_asset_href(value: &str) -> String {
    let value = value.trim();
    if is_absolute_href(value) || value.starts_with("data:") {
        return value.to_string();
    }
    let mut out = if value.starts_with('/') {
        value.to_string()
    } else {
        format!("/{value}")
    };
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}

fn is_absolute_href(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("mailto:")
        || value.starts_with("tel:")
}

#[derive(Debug, Deserialize, Default)]
struct ProjectManifest {
    app: Option<ProjectApp>,
    site: Option<ProjectSite>,
}

#[derive(Debug, Deserialize)]
struct ProjectApp {
    name: String,
}

#[derive(Debug, Deserialize, Default)]
struct ProjectSite {
    title: Option<String>,
    description: Option<String>,
    logo: Option<String>,
    favicon: Option<String>,
    base_url: Option<String>,
    default_locale: Option<String>,
    out_dir: Option<String>,
    #[serde(default)]
    nav: Vec<ProjectSiteNavLink>,
    #[serde(default)]
    routes: Vec<ProjectSiteRoute>,
    #[serde(default)]
    asset_dirs: Vec<String>,
    #[serde(default)]
    css_files: Vec<String>,
    #[serde(default)]
    elements: Vec<ProjectSitePageElement>,
    #[serde(default)]
    generate_sitemap: Option<bool>,
    #[serde(default)]
    generate_robots: Option<bool>,
    #[serde(default)]
    code_highlighting: Option<ProjectCodeHighlighting>,
    #[serde(default)]
    search: Option<ProjectSearch>,
}

#[derive(Debug, Deserialize, Default)]
struct ProjectCodeHighlighting {
    enabled: Option<bool>,
    stylesheet_href: Option<String>,
    script_src: Option<String>,
}

impl From<ProjectCodeHighlighting> for CodeHighlightingOptions {
    fn from(value: ProjectCodeHighlighting) -> Self {
        let defaults = CodeHighlightingOptions::default();
        Self {
            enabled: value.enabled.unwrap_or(false),
            stylesheet_href: value.stylesheet_href.unwrap_or(defaults.stylesheet_href),
            script_src: value.script_src.unwrap_or(defaults.script_src),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct ProjectSearch {
    enabled: Option<bool>,
    output_path: Option<String>,
    min_token_len: Option<usize>,
}

impl From<ProjectSearch> for SiteSearchOptions {
    fn from(value: ProjectSearch) -> Self {
        let defaults = SiteSearchOptions::default();
        Self {
            enabled: value.enabled.unwrap_or(false),
            output_path: value.output_path.unwrap_or(defaults.output_path),
            min_token_len: value.min_token_len.unwrap_or(defaults.min_token_len),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ProjectSitePageElement {
    placement: String,
    #[serde(default)]
    html: Option<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    routes: Vec<String>,
    #[serde(default)]
    route_prefixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProjectSiteNavLink {
    title: String,
    href: String,
    #[serde(default)]
    children: Vec<ProjectSiteNavLink>,
}

#[derive(Debug, Deserialize)]
struct ProjectSiteRoute {
    #[serde(default)]
    kind: Option<String>,
    path: String,
    source: String,
    #[serde(default)]
    template: Option<String>,
    #[serde(default)]
    sidebar: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SidebarManifest {
    #[serde(default)]
    items: Vec<SidebarManifestItem>,
}

#[derive(Debug, Deserialize)]
struct SidebarManifestItem {
    title: String,
    href: String,
    #[serde(default)]
    level: usize,
    #[serde(default)]
    group: bool,
}
