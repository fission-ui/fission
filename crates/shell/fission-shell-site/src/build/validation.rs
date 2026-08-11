use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn validate_generated_internal_links(output_dir: &Path) -> Result<()> {
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
    if path.is_file() {
        return true;
    }
    if path.is_dir() && path.join("index.html").is_file() {
        return true;
    }
    if path.extension().is_none() && path.join("index.html").is_file() {
        return true;
    }
    false
}

fn unescape_basic_attr(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}
