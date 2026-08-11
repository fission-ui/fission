use super::routing::canonical_url_for_route;
use super::{SiteBuildOptions, SEARCH_JS, SITE_CSS, SITE_ENHANCEMENT_JS};
use crate::document::ContentRoute;
use crate::html::{theme_variables_css, StyleRegistry};
use crate::search::write_search_index;
use crate::site::FissionSite;
use anyhow::{Context, Result};
use fission_theme::DesignMode;
use std::{fs, path::Path};

pub(super) fn prepare_output_dir(options: &SiteBuildOptions) -> Result<()> {
    fs::create_dir_all(&options.output_dir).with_context(|| {
        format!(
            "failed to create site output dir {}",
            options.output_dir.display()
        )
    })
}

pub(super) fn write_site_css(
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
    for user_css in options
        .user_css
        .iter()
        .chain(site.document.user_css().iter())
    {
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

pub(super) fn write_site_enhancement_js(options: &SiteBuildOptions) -> Result<()> {
    let path = options.output_dir.join("site-enhancement.js");
    fs::write(&path, SITE_ENHANCEMENT_JS)
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn write_sitemap_if_needed(
    options: &SiteBuildOptions,
    routes: &[ContentRoute],
) -> Result<()> {
    if !options.generate_sitemap {
        return Ok(());
    }
    let Some(base_url) = options.base_url.as_ref() else {
        return Ok(());
    };
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for route in routes {
        let Some(location) = canonical_url_for_route(options, &route.path) else {
            continue;
        };
        xml.push_str("  <url><loc>");
        xml.push_str(&escape_text(&location));
        xml.push_str("</loc></url>\n");
    }
    xml.push_str("</urlset>\n");
    if routes.iter().all(|route| route.path != "/") {
        xml = xml.replace(
            "</urlset>",
            &format!(
                "  <url><loc>{}/</loc></url>\n</urlset>",
                escape_text(base_url)
            ),
        );
    }
    fs::write(options.output_dir.join("sitemap.xml"), xml).with_context(|| {
        format!(
            "failed to write {}",
            options.output_dir.join("sitemap.xml").display()
        )
    })
}

pub(super) fn write_robots_if_needed(options: &SiteBuildOptions) -> Result<()> {
    if !options.generate_robots {
        return Ok(());
    }
    let mut robots = String::from("User-agent: *\nAllow: /\n");
    if let Some(base_url) = &options.base_url {
        robots.push_str("Sitemap: ");
        robots.push_str(base_url);
        robots.push_str("/sitemap.xml\n");
    }
    fs::write(options.output_dir.join("robots.txt"), robots).with_context(|| {
        format!(
            "failed to write {}",
            options.output_dir.join("robots.txt").display()
        )
    })
}

pub(super) fn write_search_assets_if_needed(
    options: &SiteBuildOptions,
    routes: &[ContentRoute],
) -> Result<()> {
    if !options.search.enabled {
        return Ok(());
    }
    let search_dir = options
        .output_dir
        .join(options.search.output_path.trim_matches('/'));
    fs::create_dir_all(&search_dir).with_context(|| {
        format!(
            "failed to create search output dir {}",
            search_dir.display()
        )
    })?;
    fs::write(search_dir.join("search.js"), SEARCH_JS)
        .with_context(|| format!("failed to write {}", search_dir.join("search.js").display()))?;
    write_search_index(
        &search_dir,
        routes,
        &options.default_locale,
        &options.search,
    )
}

fn site_theme_css(site: &FissionSite) -> String {
    let mut css = String::new();
    if site.document.theme_switching() {
        let default_selector = match site
            .document
            .default_theme_mode()
            .unwrap_or(DesignMode::Light)
        {
            DesignMode::Light => ":root,[data-theme=\"light\"]",
            DesignMode::Dark => ":root,[data-theme=\"dark\"]",
        };
        css.push_str(&theme_variables_css(
            default_selector,
            site.document.theme(),
        ));
        if let Some(light) = site.document.light_theme() {
            css.push_str(&theme_variables_css("[data-theme=\"light\"]", light));
        }
        if let Some(dark) = site.document.dark_theme() {
            css.push_str(&theme_variables_css("[data-theme=\"dark\"]", dark));
        }
    } else {
        css.push_str(&theme_variables_css(":root", site.document.theme()));
    }
    css
}

pub(super) fn copy_asset_dirs(options: &SiteBuildOptions) -> Result<()> {
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

pub(super) fn write_root_index_if_needed(
    options: &SiteBuildOptions,
    routes: &[ContentRoute],
) -> Result<()> {
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
