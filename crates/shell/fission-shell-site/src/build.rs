use crate::document::{
    extract_page_links, ContentRoute, DocumentationPage, SidebarLink, SiteNavLink, SitePageState,
};
use crate::front_matter::split_front_matter;
use crate::html::{
    render_ir_to_html_with_styles, theme_variables_css, CssVariableMap, HtmlRenderOptions,
    StyleRegistry,
};
use crate::site::{normalize_site_path, ContentTransform, FissionSite, SiteRenderContext};
use anyhow::{bail, Context, Result};
use fission_core::ui::Column;
use fission_core::{BuildCtx, Env, LoweringContext, Node, RuntimeState, View, Widget};
use fission_layout::LayoutSize;
use fission_theme::DesignMode;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const SITE_CSS: &str = include_str!("../assets/site.css");

#[derive(Clone, Debug)]
pub struct SiteBuildOptions {
    pub project_dir: PathBuf,
    pub output_dir: PathBuf,
    pub site_title: String,
    pub site_description: Option<String>,
    pub site_logo: Option<String>,
    pub site_nav: Vec<SiteNavLink>,
    pub user_css: Vec<String>,
    pub content_routes: Vec<SiteContentRouteConfig>,
    pub asset_dirs: Vec<PathBuf>,
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
            site_nav: Vec::new(),
            user_css: Vec::new(),
            content_routes: vec![SiteContentRouteConfig {
                path: "/content".to_string(),
                source: project_dir.join("content"),
                template: None,
                sidebar: None,
            }],
            asset_dirs: Vec::new(),
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
        let site_nav = site
            .nav
            .into_iter()
            .map(|link| SiteNavLink {
                title: link.title,
                href: normalize_site_link_href(&link.href),
            })
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
        Ok(Self {
            project_dir,
            output_dir,
            site_title,
            site_description: site.description,
            site_logo,
            site_nav,
            user_css,
            content_routes,
            asset_dirs,
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
    let mut routes = load_content_routes(options, site.content_transform.as_deref())?;
    let mut styles = StyleRegistry::default();
    let custom_routes = render_custom_routes(options, site, &mut styles)?;
    routes.extend(custom_routes);
    routes.sort_by(|a, b| a.path.cmp(&b.path));
    detect_duplicate_routes(&routes)?;

    if options.clean && options.output_dir.exists() {
        fs::remove_dir_all(&options.output_dir).with_context(|| {
            format!(
                "failed to clean site output dir {}",
                options.output_dir.display()
            )
        })?;
    }
    prepare_output_dir(options)?;
    copy_asset_dirs(options)?;

    let mut rendered_routes = Vec::new();
    for route in &routes {
        let html = render_route(route, &routes, options, site, &mut styles)?;
        rendered_routes.push((route, html));
    }
    write_site_css(options, site, &styles)?;

    let mut report_routes = Vec::new();
    for (route, html) in rendered_routes {
        let output = output_path_for_route(&options.output_dir, &route.path);
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

    write_root_index_if_needed(options, &routes)?;

    Ok(SiteBuildReport {
        output_dir: options.output_dir.clone(),
        routes: report_routes,
    })
}

pub fn check_site(options: &SiteBuildOptions, site: &FissionSite) -> Result<SiteBuildReport> {
    let mut routes = load_content_routes(options, site.content_transform.as_deref())?;
    let mut styles = StyleRegistry::default();
    routes.extend(render_custom_routes(options, site, &mut styles)?);
    routes.sort_by(|a, b| a.path.cmp(&b.path));
    detect_duplicate_routes(&routes)?;

    let mut report_routes = Vec::new();
    for route in &routes {
        render_route(route, &routes, options, site, &mut styles)?;
        report_routes.push(SiteRouteReport {
            path: route.path.clone(),
            title: route.title.clone(),
            source: route.source_path.clone(),
            output: output_path_for_route(&options.output_dir, &route.path),
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
    let mut routes = load_content_routes(options, site.content_transform.as_deref())?;
    for route in &site.custom_routes {
        routes.push(ContentRoute {
            path: route.path.clone(),
            title: route.title.clone(),
            description: route.description.clone(),
            body: String::new(),
            headings: Vec::new(),
            sidebar: Vec::new(),
            source_path: PathBuf::from("<custom>"),
            rendered: None,
        });
    }
    routes.sort_by(|a, b| a.path.cmp(&b.path));
    detect_duplicate_routes(&routes)?;
    Ok(routes
        .iter()
        .map(|route| SiteRouteReport {
            path: route.path.clone(),
            title: route.title.clone(),
            source: route.source_path.clone(),
            output: output_path_for_route(&options.output_dir, &route.path),
        })
        .collect())
}

fn prepare_output_dir(options: &SiteBuildOptions) -> Result<()> {
    fs::create_dir_all(&options.output_dir).with_context(|| {
        format!(
            "failed to create site output dir {}",
            options.output_dir.display()
        )
    })
}

fn write_site_css(
    options: &SiteBuildOptions,
    site: &FissionSite,
    styles: &StyleRegistry,
) -> Result<()> {
    let mut css = String::new();
    css.push_str(SITE_CSS);
    css.push('\n');
    css.push_str(&site_theme_css(site));
    css.push('\n');
    css.push_str(&styles.to_css());
    for user_css in options.user_css.iter().chain(site.user_css.iter()) {
        css.push('\n');
        css.push_str(user_css);
        css.push('\n');
    }
    fs::write(options.output_dir.join("site.css"), css).with_context(|| {
        format!(
            "failed to write {}",
            options.output_dir.join("site.css").display()
        )
    })
}

fn site_theme_css(site: &FissionSite) -> String {
    let mut css = String::new();
    if site.theme_switching {
        let default_selector = match site.default_theme_mode.unwrap_or(DesignMode::Light) {
            DesignMode::Light => ":root,[data-theme=\"light\"]",
            DesignMode::Dark => ":root,[data-theme=\"dark\"]",
        };
        css.push_str(&theme_variables_css(default_selector, &site.theme));
        if let Some(light) = &site.light_theme {
            css.push_str(&theme_variables_css("[data-theme=\"light\"]", light));
        }
        if let Some(dark) = &site.dark_theme {
            css.push_str(&theme_variables_css("[data-theme=\"dark\"]", dark));
        }
    } else {
        css.push_str(&theme_variables_css(":root", &site.theme));
    }
    css
}

fn render_footer_node(
    options: &SiteBuildOptions,
    site: &FissionSite,
    route_path: &str,
) -> Result<Option<Node>> {
    let Some(footer) = &site.footer else {
        return Ok(None);
    };
    let ctx = SiteRenderContext {
        project_dir: &options.project_dir,
        route_path,
        theme: &site.theme,
    };
    Ok(Some(footer(&ctx)?))
}

fn append_footer(node: Node, footer: Option<Node>) -> Node {
    let Some(footer) = footer else {
        return node;
    };
    Column {
        children: vec![node, footer],
        ..Default::default()
    }
    .into_node()
}

fn copy_asset_dirs(options: &SiteBuildOptions) -> Result<()> {
    for source in &options.asset_dirs {
        if !source.exists() {
            continue;
        }
        copy_dir_contents(source, &options.output_dir)?;
    }
    Ok(())
}

fn copy_dir_contents(source: &Path, dest: &Path) -> Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir_contents(&source_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy asset {} to {}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn load_content_routes(
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
                headings: extract_page_links(&body),
                sidebar: sidebar.clone(),
                body,
                source_path: file,
                rendered: None,
            });
        }
    }
    if routes.is_empty() && site_has_content_routes(options) {
        bail!("configured site content routes contain no .md or .mdx files");
    }
    Ok(routes)
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

fn site_has_content_routes(options: &SiteBuildOptions) -> bool {
    !options.content_routes.is_empty()
}

fn render_custom_routes(
    options: &SiteBuildOptions,
    site: &FissionSite,
    styles: &mut StyleRegistry,
) -> Result<Vec<ContentRoute>> {
    let mut routes = Vec::new();
    for route in &site.custom_routes {
        let ctx = SiteRenderContext {
            project_dir: &options.project_dir,
            route_path: &route.path,
            theme: &site.theme,
        };
        let node = (route.render)(&ctx)?;
        let node = append_footer(node, render_footer_node(options, site, &route.path)?);
        let html = render_node_to_html(
            node,
            &route.title,
            route
                .description
                .clone()
                .or_else(|| options.site_description.clone()),
            &route.path,
            site,
            styles,
        )?;
        routes.push(ContentRoute {
            path: route.path.clone(),
            title: route.title.clone(),
            description: route.description.clone(),
            body: String::new(),
            headings: Vec::new(),
            sidebar: Vec::new(),
            source_path: PathBuf::from("<custom>"),
            rendered: Some(html),
        });
    }
    Ok(routes)
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

fn render_route(
    route: &ContentRoute,
    routes: &[ContentRoute],
    options: &SiteBuildOptions,
    site: &FissionSite,
    styles: &mut StyleRegistry,
) -> Result<String> {
    if let Some(rendered) = &route.rendered {
        return Ok(rendered.clone());
    }
    let runtime = RuntimeState::default();
    let mut env = Env::default();
    env.theme = site.theme.clone();
    env.viewport_size = LayoutSize::new(1280.0, 900.0);
    let state = SitePageState;
    let view = View::new(&state, &runtime, &env, None);
    let mut build_ctx = BuildCtx::<SitePageState>::new();
    let page = DocumentationPage {
        site_title: &options.site_title,
        site_logo: options.site_logo.as_deref(),
        site_nav: &options.site_nav,
        theme_switching: site.theme_switching,
        route,
        all_routes: routes,
    };
    let node = append_footer(
        page.build(&mut build_ctx, &view),
        render_footer_node(options, site, &route.path)?,
    );
    render_node_to_html(
        node,
        &format!("{} | {}", route.title, options.site_title),
        route
            .description
            .clone()
            .or_else(|| options.site_description.clone()),
        &route.path,
        site,
        styles,
    )
}

fn render_node_to_html(
    node: Node,
    title: &str,
    description: Option<String>,
    route_path: &str,
    site: &FissionSite,
    styles: &mut StyleRegistry,
) -> Result<String> {
    let runtime = RuntimeState::default();
    let mut env = Env::default();
    env.theme = site.theme.clone();
    let mut lowering = LoweringContext::new(&env, &runtime, None, None);
    let root = node.lower(&mut lowering);
    lowering.ir.set_root(root);

    let render_options = HtmlRenderOptions {
        document_title: title.to_string(),
        description,
        stylesheet_href: stylesheet_href_for_route(route_path),
        current_route_path: route_path.to_string(),
        css_variables: CssVariableMap::from_theme(&site.theme),
        default_theme_mode: site.default_theme_mode,
        theme_switching: site.theme_switching,
        ..Default::default()
    };
    Ok(render_ir_to_html_with_styles(&lowering.ir, &render_options, styles)?.html)
}

fn detect_duplicate_routes(routes: &[ContentRoute]) -> Result<()> {
    for pair in routes.windows(2) {
        if pair[0].path == pair[1].path {
            bail!("duplicate static site route `{}`", pair[0].path);
        }
    }
    Ok(())
}

fn write_root_index_if_needed(options: &SiteBuildOptions, routes: &[ContentRoute]) -> Result<()> {
    if routes.iter().any(|route| route.path == "/") || routes.is_empty() {
        return Ok(());
    }
    let first = &routes[0];
    let href = first.path.clone();
    let html = format!(
        "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n    <meta http-equiv=\"refresh\" content=\"0; url={}\">\n    <title>{}</title>\n  </head>\n  <body><a href=\"{}\">{}</a></body>\n</html>\n",
        escape_attr(&href),
        escape_text(&options.site_title),
        escape_attr(&href),
        escape_text(&first.title)
    );
    fs::write(options.output_dir.join("index.html"), html)?;
    Ok(())
}

fn output_path_for_route(output_dir: &Path, route_path: &str) -> PathBuf {
    let trimmed = route_path.trim_matches('/');
    if trimmed.is_empty() {
        output_dir.join("index.html")
    } else {
        output_dir.join(trimmed).join("index.html")
    }
}

fn route_path_from_file(prefix: &str, content_dir: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(content_dir).unwrap_or(file);
    let mut pieces = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>();
    if let Some(last) = pieces.last_mut() {
        if let Some((stem, _)) = last.rsplit_once('.') {
            *last = stem.to_string();
        }
    }
    if pieces.last().is_some_and(|value| value == "index") {
        pieces.pop();
    }
    let suffix = pieces.join("/");
    if suffix.is_empty() {
        prefix.to_string()
    } else {
        format!("{}/{suffix}", prefix.trim_end_matches('/'))
    }
}

fn route_path_from_slug(prefix: &str, slug: &str) -> String {
    let prefix = normalize_site_path(prefix);
    let slug = slug.trim_matches('/');
    if slug.is_empty() {
        return prefix;
    }
    let prefixed = format!("/{}", slug);
    if prefixed == prefix || prefixed.starts_with(prefix.trim_end_matches('/')) {
        normalize_site_path(&prefixed)
    } else {
        normalize_site_path(&format!("{}/{}", prefix.trim_end_matches('/'), slug))
    }
}

fn resolve_relative_markdown_links(
    markdown: &str,
    route_prefix: &str,
    content_dir: &Path,
    source_file: &Path,
) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(open) = rest.find("](") {
        let (before, after_open) = rest.split_at(open + 2);
        out.push_str(before);
        let Some(close) = after_open.find(')') else {
            out.push_str(after_open);
            return out;
        };
        let (target, after_target) = after_open.split_at(close);
        out.push_str(&resolve_markdown_link_target(
            target,
            route_prefix,
            content_dir,
            source_file,
        ));
        out.push(')');
        rest = &after_target[1..];
    }
    out.push_str(rest);
    out
}

fn resolve_markdown_link_target(
    target: &str,
    route_prefix: &str,
    content_dir: &Path,
    source_file: &Path,
) -> String {
    let (path, suffix) = split_link_suffix(target);
    if !(path.starts_with("./") || path.starts_with("../")) {
        return target.to_string();
    }
    let Some(parent) = source_file.parent() else {
        return target.to_string();
    };
    let raw_target = parent.join(path);
    let Some(target_file) = resolve_markdown_target_file(&raw_target) else {
        return target.to_string();
    };
    let route = normalize_site_path(&route_path_from_file(
        route_prefix,
        content_dir,
        &target_file,
    ));
    format!("{route}{suffix}")
}

fn split_link_suffix(target: &str) -> (&str, &str) {
    let end = target
        .find('#')
        .or_else(|| target.find('?'))
        .unwrap_or(target.len());
    target.split_at(end)
}

fn resolve_markdown_target_file(path: &Path) -> Option<PathBuf> {
    if path.extension().is_some() && path.exists() {
        return Some(path.to_path_buf());
    }
    for extension in ["mdx", "md"] {
        let candidate = path.with_extension(extension);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    for extension in ["mdx", "md"] {
        let candidate = path.join(format!("index.{extension}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn stylesheet_href_for_route(route_path: &str) -> String {
    let depth = route_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count();
    if depth == 0 {
        "site.css".to_string()
    } else {
        format!("{}site.css", "../".repeat(depth))
    }
}

fn first_h1(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled")
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
    out_dir: Option<String>,
    #[serde(default)]
    nav: Vec<ProjectSiteNavLink>,
    #[serde(default)]
    routes: Vec<ProjectSiteRoute>,
    #[serde(default)]
    asset_dirs: Vec<String>,
    #[serde(default)]
    css_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProjectSiteNavLink {
    title: String,
    href: String,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn route_paths_are_derived_from_content_tree() {
        let root = PathBuf::from("content/docs");
        assert_eq!(
            route_path_from_file("/docs", &root, Path::new("content/docs/index.md")),
            "/docs"
        );
        assert_eq!(
            route_path_from_file("/docs", &root, Path::new("content/docs/guides/start.md")),
            "/docs/guides/start"
        );
        assert_eq!(
            route_path_from_slug("/reference", "/widgets/button"),
            "/reference/widgets/button/"
        );
    }

    #[test]
    fn content_site_build_writes_real_html() {
        let temp = std::env::temp_dir().join(format!(
            "fission-site-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp.join("content")).unwrap();
        fs::write(
            temp.join("content/getting-started.md"),
            "---\ntitle: Getting started\ndescription: First page\n---\n# Getting started\n\nThis is rendered by Fission.",
        )
        .unwrap();
        let options = SiteBuildOptions::for_project(&temp, "Test site");
        let report = build_content_site(&options).unwrap();
        let output = temp.join("target/fission/site/content/getting-started/index.html");
        assert_eq!(report.routes.len(), 1);
        assert!(output.exists());
        let html = fs::read_to_string(output).unwrap();
        assert!(html.contains("This is rendered by"));
        assert!(html.contains("Fission."));
        assert!(!html.contains("style=\""));
        let css = fs::read_to_string(temp.join("target/fission/site/site.css")).unwrap();
        assert!(css.contains(":root"));
        assert!(css.contains(".fs_"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn relative_markdown_links_are_resolved_to_site_routes() {
        let temp = std::env::temp_dir().join(format!(
            "fission-site-link-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let content = temp.join("content/reference/charts/bar");
        fs::create_dir_all(&content).unwrap();
        fs::write(content.join("overview.mdx"), "# Bar").unwrap();
        fs::write(content.join("bar-ranked.mdx"), "# Ranked").unwrap();

        let source_file = content.join("bar-ranked.mdx");
        let resolved = resolve_relative_markdown_links(
            "[Bar family overview](./overview) and [Ranked](./bar-ranked#example)",
            "/reference",
            &temp.join("content/reference"),
            &source_file,
        );

        assert_eq!(
            resolved,
            "[Bar family overview](/reference/charts/bar/overview/) and [Ranked](/reference/charts/bar/bar-ranked/#example)"
        );
        let _ = fs::remove_dir_all(temp);
    }
}
