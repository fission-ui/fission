use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct ChartEntry {
    slug: String,
    title: String,
    family: String,
    family_slug: String,
    description: String,
    data_shape: String,
    use_when: String,
    tags: Vec<String>,
    image: String,
}

pub(crate) fn expand_documentation_mdx(body: &str, project_dir: &Path) -> Result<String> {
    let mut output = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") || trimmed.starts_with("export ") {
            continue;
        }
        if trimmed.starts_with("<ChartCatalogGrid") {
            output.push_str(&render_chart_catalog_component(trimmed, project_dir)?);
            output.push('\n');
            continue;
        }
        if trimmed == "<ChartFamilySummary />" {
            output.push_str(&render_chart_family_summary(project_dir)?);
            output.push('\n');
            continue;
        }
        if trimmed == "<Tabs>" || trimmed == "</Tabs>" || trimmed == "</TabItem>" {
            continue;
        }
        if trimmed.starts_with("<TabItem") {
            output.push_str("\n### ");
            output.push_str(&escape_markdown(
                &attr_value(trimmed, "label").unwrap_or("Option"),
            ));
            output.push_str("\n\n");
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    Ok(output)
}

fn render_chart_catalog_component(line: &str, project_dir: &Path) -> Result<String> {
    let all = load_chart_entries(project_dir)?;
    let slugs = parse_slugs(line);
    let charts = if slugs.is_empty() {
        all
    } else {
        slugs
            .into_iter()
            .filter_map(|slug| all.iter().find(|chart| chart.slug == slug).cloned())
            .collect()
    };
    if line.contains("compact") {
        Ok(render_compact_chart_grid(&charts))
    } else {
        Ok(render_full_chart_grid(&charts))
    }
}

fn render_chart_family_summary(project_dir: &Path) -> Result<String> {
    let charts = load_chart_entries(project_dir)?;
    let mut families = BTreeMap::<String, (String, usize)>::new();
    for chart in charts {
        let entry = families
            .entry(chart.family.clone())
            .or_insert_with(|| (chart.family_slug.clone(), 0));
        entry.1 += 1;
    }

    let mut html = String::from("<div class=\"chart-summary-grid\">\n");
    for (family, (family_slug, count)) in families {
        html.push_str(&format!(
            "<a class=\"chart-summary-card\" href=\"/reference/charts/{}/overview/\"><p>{} variants</p><h3>{}</h3><span>Open the family reference</span></a>\n",
            escape_attr(&family_slug),
            count,
            escape_html(&family),
        ));
    }
    html.push_str("</div>");
    Ok(html)
}

fn render_compact_chart_grid(charts: &[ChartEntry]) -> String {
    let mut html = String::from("<div class=\"chart-compact-grid\">\n");
    for chart in charts {
        html.push_str(&format!(
            "<a class=\"chart-compact-card\" href=\"{}\"><img src=\"{}\" alt=\"{} chart screenshot\" loading=\"lazy\" /><div><p>{}</p><h3>{}</h3></div></a>\n",
            chart_href(chart),
            escape_attr(&chart.image),
            escape_attr(&chart.title),
            escape_html(&chart.family),
            escape_html(&chart.title),
        ));
    }
    html.push_str("</div>");
    html
}

fn render_full_chart_grid(charts: &[ChartEntry]) -> String {
    let mut by_family = BTreeMap::<String, Vec<&ChartEntry>>::new();
    for chart in charts {
        by_family
            .entry(chart.family.clone())
            .or_default()
            .push(chart);
    }

    let mut html = String::from("<div class=\"chart-catalog-groups\">\n");
    for (family, family_charts) in by_family {
        html.push_str(&format!(
            "<section class=\"chart-family-group\"><div class=\"chart-family-header\"><p>Chart family</p><h2>{}</h2></div><div class=\"chart-catalog-grid\">\n",
            escape_html(&family),
        ));
        for chart in family_charts {
            html.push_str(&format!(
                "<a class=\"chart-card\" href=\"{}\"><img src=\"{}\" alt=\"{} chart screenshot\" loading=\"lazy\" /><div class=\"chart-card-body\"><h3>{}</h3><p>{}</p><dl><div><dt>Data</dt><dd>{}</dd></div><div><dt>Use when</dt><dd>{}</dd></div></dl>{}</div></a>\n",
                chart_href(chart),
                escape_attr(&chart.image),
                escape_attr(&chart.title),
                escape_html(&chart.title),
                escape_html(&chart.description),
                escape_html(&chart.data_shape),
                escape_html(&chart.use_when),
                render_tags(&chart.tags),
            ));
        }
        html.push_str("</div></section>\n");
    }
    html.push_str("</div>");
    html
}

fn load_chart_entries(project_dir: &Path) -> Result<Vec<ChartEntry>> {
    let root = project_dir.join("content/reference/charts");
    let mut files = Vec::new();
    collect_markdown_files(&root, &mut files)?;
    files.sort();

    let mut charts = Vec::new();
    for path in files {
        if path.file_stem().and_then(|value| value.to_str()) == Some("overview") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read chart reference {}", path.display()))?;
        let (front_matter, body) = split_front_matter(&raw);
        let Some(slug) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let Some(family_slug) = relative
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
        else {
            continue;
        };
        let title = front_matter
            .get("title")
            .cloned()
            .unwrap_or_else(|| title_from_slug(slug));
        charts.push(ChartEntry {
            slug: slug.to_string(),
            title,
            family: title_from_slug(family_slug),
            family_slug: family_slug.to_string(),
            description: first_chart_description(&body),
            data_shape: extract_between(&body, "data shape readable: `", "`")
                .unwrap_or_else(|| "Typed chart data, axes, and series.".to_string()),
            use_when: extract_use_when(&body),
            tags: extract_tags(&body),
            image: extract_image(&body).unwrap_or_else(|| format!("/img/charts/{slug}.png")),
        });
    }
    Ok(charts)
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_files(&path, out)?;
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("md" | "mdx")
        ) {
            out.push(path);
        }
    }
    Ok(())
}

fn split_front_matter(raw: &str) -> (BTreeMap<String, String>, String) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (BTreeMap::new(), raw.to_string());
    };
    let Some(end) = rest.find("\n---") else {
        return (BTreeMap::new(), raw.to_string());
    };
    let front = &rest[..end];
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']).to_string();
    let mut values = BTreeMap::new();
    for line in front.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        values.insert(
            key.trim().to_string(),
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string(),
        );
    }
    (values, body)
}

fn parse_slugs(line: &str) -> Vec<String> {
    let mut slugs = Vec::new();
    let mut parts = line.split('\'');
    while parts.next().is_some() {
        let Some(slug) = parts.next() else {
            break;
        };
        if !slug.trim().is_empty() {
            slugs.push(slug.to_string());
        }
    }
    slugs
}

fn first_chart_description(body: &str) -> String {
    body.split("\n\n")
        .map(str::trim)
        .find(|block| {
            !block.is_empty()
                && !block.starts_with('#')
                && !block.starts_with("![")
                && !block.starts_with("Tags:")
        })
        .map(|block| {
            block.replace(
                " The screenshot is captured from the native Fission chart gallery.",
                "",
            )
        })
        .unwrap_or_else(|| "A typed Fission chart reference entry.".to_string())
}

fn extract_use_when(body: &str) -> String {
    let description = first_chart_description(body);
    if let Some(start) = description.find("Use it ") {
        let use_when = &description[start..];
        return use_when
            .split_once('.')
            .map(|(sentence, _)| sentence.to_string())
            .unwrap_or_else(|| use_when.to_string());
    }
    "Use it when this visual form makes the user's question faster to answer than a table.".into()
}

fn extract_tags(body: &str) -> Vec<String> {
    let Some(tags_line) = body
        .lines()
        .find(|line| line.trim_start().starts_with("Tags:"))
    else {
        return Vec::new();
    };
    tags_line
        .split('`')
        .nth(1)
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn extract_image(body: &str) -> Option<String> {
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with("![") {
            continue;
        }
        let Some((_, rest)) = line.split_once("](") else {
            continue;
        };
        let Some((href, _)) = rest.split_once(')') else {
            continue;
        };
        return Some(href.to_string());
    }
    None
}

fn extract_between(body: &str, start: &str, end: &str) -> Option<String> {
    let (_, rest) = body.split_once(start)?;
    let (value, _) = rest.split_once(end)?;
    Some(value.to_string())
}

fn attr_value<'a>(line: &'a str, attr: &str) -> Option<&'a str> {
    let pattern = format!("{attr}=\"");
    let (_, rest) = line.split_once(&pattern)?;
    let (value, _) = rest.split_once('"')?;
    Some(value)
}

fn render_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let mut html = String::from("<div class=\"chart-tags\">");
    for tag in tags {
        html.push_str(&format!("<span>{}</span>", escape_html(tag)));
    }
    html.push_str("</div>");
    html
}

fn chart_href(chart: &ChartEntry) -> String {
    format!(
        "/reference/charts/{}/{}/",
        escape_attr(&chart.family_slug),
        escape_attr(&chart.slug)
    )
}

fn title_from_slug(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape_markdown(value: &str) -> String {
    value.replace(['#', '*', '`'], "")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value).replace('"', "&quot;")
}
