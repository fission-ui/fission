use super::SiteBuildOptions;
use crate::document::ContentRoute;
use crate::site::{normalize_site_path, FissionSite, SitePageElementPlacement};
use anyhow::{bail, Result};
use serde_json::json;
use std::path::{Path, PathBuf};

pub(super) fn detect_duplicate_routes(routes: &[ContentRoute]) -> Result<()> {
    for pair in routes.windows(2) {
        if pair[0].path == pair[1].path {
            bail!("duplicate static site route `{}`", pair[0].path);
        }
    }
    Ok(())
}

pub(super) fn output_path_for_route(output_dir: &Path, route_path: &str) -> PathBuf {
    let trimmed = route_path.trim_matches('/');
    if trimmed.is_empty() {
        output_dir.join("index.html")
    } else {
        output_dir.join(trimmed).join("index.html")
    }
}

pub(super) fn route_path_from_file(prefix: &str, content_dir: &Path, file: &Path) -> String {
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

pub(super) fn route_path_from_slug(prefix: &str, slug: &str) -> String {
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

pub(super) fn resolve_relative_markdown_links(
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

pub(super) fn stylesheet_href_for_route(route_path: &str) -> String {
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

pub(super) fn search_script_href_for_route(route_path: &str, search_path: &str) -> String {
    let target = format!(
        "/{}/search.js",
        search_path.trim_matches('/').trim_end_matches('/')
    );
    relative_href_for_route(route_path, &target)
}

pub(super) fn page_asset_href_for_route(route_path: &str, href: &str) -> String {
    if href.starts_with('/') {
        relative_href_for_route(route_path, href)
    } else {
        href.to_string()
    }
}

fn relative_href_for_route(current_route_path: &str, target: &str) -> String {
    let suffix_start = target
        .find('#')
        .or_else(|| target.find('?'))
        .unwrap_or(target.len());
    let (path, suffix) = target.split_at(suffix_start);
    let depth = current_route_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count();
    let prefix = "../".repeat(depth);
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        if prefix.is_empty() {
            format!("./{suffix}")
        } else {
            format!("{prefix}{suffix}")
        }
    } else {
        format!("{prefix}{trimmed}{suffix}")
    }
}

pub(super) fn canonical_url_for_route(
    options: &SiteBuildOptions,
    route_path: &str,
) -> Option<String> {
    let base = options.base_url.as_ref()?;
    let path = normalize_site_path(route_path);
    if path == "/" {
        Some(format!("{base}/"))
    } else {
        Some(format!("{base}{path}"))
    }
}

pub(super) fn structured_data_for_route(
    options: &SiteBuildOptions,
    title: &str,
    description: Option<&str>,
    route_path: &str,
) -> Vec<String> {
    let Some(url) = canonical_url_for_route(options, route_path) else {
        return Vec::new();
    };
    let mut data = Vec::new();
    if normalize_site_path(route_path) == "/" {
        data.push(
            json!({
                "@context": "https://schema.org",
                "@type": "WebSite",
                "name": options.site_title,
                "url": url,
            })
            .to_string(),
        );
    }
    data.push(
        json!({
            "@context": "https://schema.org",
            "@type": "WebPage",
            "name": title,
            "url": url,
            "description": description.or(options.site_description.as_deref()),
            "isPartOf": {
                "@type": "WebSite",
                "name": options.site_title,
                "url": options.base_url,
            },
        })
        .to_string(),
    );
    data
}

pub(super) fn page_elements_for_route(
    options: &SiteBuildOptions,
    site: &FissionSite,
    route_path: &str,
    placement: SitePageElementPlacement,
) -> Vec<String> {
    options
        .page_elements
        .iter()
        .chain(site.document.page_elements().iter())
        .filter(|element| element.placement == placement && element.applies_to(route_path))
        .map(|element| element.html.clone())
        .collect()
}

pub(super) fn first_h1(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
}

pub(super) fn title_from_path(path: &Path) -> String {
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
