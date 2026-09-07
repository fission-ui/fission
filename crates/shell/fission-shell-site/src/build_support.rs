use crate::site::normalize_site_path;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn stylesheet_href_for_route(route_path: &str) -> String {
    let depth = route_depth(route_path);
    if depth == 0 {
        "site.css".to_string()
    } else {
        format!("{}site.css", "../".repeat(depth))
    }
}

pub(crate) fn search_script_href_for_route(route_path: &str, search_path: &str) -> String {
    let target = format!(
        "/{}/search.js",
        search_path.trim_matches('/').trim_end_matches('/')
    );
    relative_href_for_route(route_path, &target)
}

pub(crate) fn page_asset_href_for_route(route_path: &str, href: &str) -> String {
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
    let prefix = "../".repeat(route_depth(current_route_path));
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

fn route_depth(route_path: &str) -> usize {
    route_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count()
}

pub(crate) fn validate_generated_internal_links(output_dir: &Path) -> Result<()> {
    let mut html_files = Vec::new();
    collect_generated_html_files(output_dir, &mut html_files)?;
    let mut missing = Vec::new();
    for html_file in html_files {
        let html = fs::read_to_string(&html_file)
            .with_context(|| format!("failed to read generated HTML {}", html_file.display()))?;
        for target in extract_html_attr_values(&html, "href")
            .into_iter()
            .chain(extract_html_attr_values(&html, "src"))
        {
            if generated_link_target_exists(output_dir, &html_file, &target) {
                continue;
            }
            missing.push(format!("{} -> {}", html_file.display(), target));
            if missing.len() >= 10 {
                break;
            }
        }
        if missing.len() >= 10 {
            break;
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        bail!(
            "static site generated links that do not resolve:\n{}",
            missing.join("\n")
        )
    }
}

fn collect_generated_html_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_generated_html_files(&path, out)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("html") {
            out.push(path);
        }
    }
    Ok(())
}

fn extract_html_attr_values(html: &str, attr: &str) -> Vec<String> {
    let needle = format!("{attr}=\"");
    let mut values = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(&needle) {
        let after_start = &rest[start + needle.len()..];
        let Some(end) = after_start.find('"') else {
            break;
        };
        values.push(unescape_basic_attr(&after_start[..end]));
        rest = &after_start[end + 1..];
    }
    values
}

fn generated_link_target_exists(output_dir: &Path, source: &Path, target: &str) -> bool {
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || target.starts_with("data:")
    {
        return true;
    }
    let target = target.split(['#', '?']).next().unwrap_or(target).trim();
    if target.is_empty() {
        return true;
    }
    let path = if target.starts_with('/') {
        output_dir.join(target.trim_start_matches('/'))
    } else {
        source.parent().unwrap_or(output_dir).join(target)
    };
    generated_target_path_exists(path)
}

fn generated_target_path_exists(path: PathBuf) -> bool {
    path.is_file()
        || (path.is_dir() && path.join("index.html").is_file())
        || (path.extension().is_none() && path.join("index.html").is_file())
}

fn unescape_basic_attr(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

pub(crate) fn first_h1(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn title_from_path(path: &Path) -> String {
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

pub(crate) fn resolve_project_path(project_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

pub(crate) fn normalize_site_link_href(value: &str) -> String {
    let value = value.trim();
    if is_absolute_href(value) || value.starts_with('#') {
        value.to_string()
    } else {
        normalize_site_path(value)
    }
}

pub(crate) fn normalize_site_asset_href(value: &str) -> String {
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

pub(crate) fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn escape_attr(value: &str) -> String {
    escape_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
