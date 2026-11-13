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

pub(crate) fn expand_documentation_mdx(
    body: &str,
    project_dir: &Path,
    _source_file: &Path,
) -> Result<String> {
    let mut output = String::new();
    let mut fence: Option<(u8, usize)> = None;
    for line in body.lines() {
        let trimmed = line.trim();
        let marker = trimmed.as_bytes().first().copied();
        let marker_length = marker.map_or(0, |marker| {
            trimmed.bytes().take_while(|byte| *byte == marker).count()
        });
        if let Some((open, length)) = fence {
            output.push_str(line);
            output.push('\n');
            if marker == Some(open)
                && marker_length >= length
                && trimmed[marker_length..].trim().is_empty()
            {
                fence = None;
            }
            continue;
        }
        if matches!(marker, Some(b'`' | b'~')) && marker_length >= 3 {
            fence = marker.map(|marker| (marker, marker_length));
            output.push_str(line);
            output.push('\n');
            continue;
        }
        // Only consume this legacy component's import. Shell commands, Java /
        // Swift imports, prose and unrelated MDX imports are not ours to rewrite.
        if trimmed.starts_with("import ")
            && trimmed.contains("@site/src/components/charts/ChartCatalogGrid")
        {
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
            .map(|slug| {
                all.iter()
                    .find(|chart| chart.slug == slug)
                    .cloned()
                    .with_context(|| format!("chart catalogue references missing chart `{slug}`"))
            })
            .collect::<Result<Vec<_>>>()?
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

    let mut markdown = String::from("## Chart families\n\n");
    for (family, (family_slug, count)) in families {
        markdown.push_str(&format!(
            "- [{}](/reference/charts/{}/overview/) - {} variants\n",
            escape_markdown(&family),
            family_slug,
            count,
        ));
    }
    Ok(markdown)
}

fn render_compact_chart_grid(charts: &[ChartEntry]) -> String {
    let mut markdown = String::new();
    for chart in charts {
        markdown.push_str(&format!(
            "### {}\n\n[Open {}]({})\n\nFamily: {}\n\n![{} chart screenshot]({})\n\n",
            escape_markdown(&chart.title),
            escape_markdown(&chart.title),
            chart_href(chart),
            escape_markdown(&chart.family),
            escape_markdown(&chart.title),
            chart.image,
        ));
    }
    markdown
}

fn render_full_chart_grid(charts: &[ChartEntry]) -> String {
    let mut by_family = BTreeMap::<String, Vec<&ChartEntry>>::new();
    for chart in charts {
        by_family
            .entry(chart.family.clone())
            .or_default()
            .push(chart);
    }

    let mut markdown = String::new();
    for (family, family_charts) in by_family {
        markdown.push_str(&format!("## {}\n\n", escape_markdown(&family)));
        for chart in family_charts {
            markdown.push_str(&format!(
                "### {}\n\n[Open {}]({})\n\n![{} chart screenshot]({})\n\n{}\n\nData: {}\n\nUse when: {}\n\n{}\n\n",
                escape_markdown(&chart.title),
                escape_markdown(&chart.title),
                chart_href(chart),
                escape_markdown(&chart.title),
                chart.image,
                chart.description,
                chart.data_shape,
                chart.use_when,
                render_tags(&chart.tags),
            ));
        }
    }
    markdown
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

fn render_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    format!("Tags: `{}`", tags.join(", "))
}

fn chart_href(chart: &ChartEntry) -> String {
    format!("/reference/charts/{}/{}/", chart.family_slug, chart.slug)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_chart_overview_keeps_its_catalogue_images_and_family_links() {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = project.join("content/legacy/docs/charts/overview.mdx");
        let input = fs::read_to_string(&source).unwrap();
        let expanded = expand_documentation_mdx(&input, project, &source).unwrap();
        assert!(!expanded.contains("<ChartCatalogGrid"));
        assert!(!expanded.contains("<ChartFamilySummary"));
        assert!(!expanded.contains("@site/src/components/charts/ChartCatalogGrid"));
        for slug in [
            "line-stacked-area",
            "bar-horizontal",
            "scatter-bubble",
            "calendar-heatmap",
            "polar-bar",
            "bar3d-basic",
        ] {
            assert!(
                expanded.contains(&format!("/img/charts/{slug}.png")),
                "missing {slug} image"
            );
        }
        assert!(expanded.contains("/reference/charts/line/overview/"));
        assert!(expanded.contains("/reference/charts/3d-and-gl/overview/"));
    }

    #[test]
    fn code_fences_and_export_instructions_are_preserved_verbatim() {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        let input = "export this certificate before continuing.\n\n```sh\nexport TEAM_ID=example\n<ChartFamilySummary />\n```\n\n~~~java\nimport java.io.InputStream;\n~~~\n";
        assert_eq!(
            expand_documentation_mdx(input, project, Path::new("test.mdx")).unwrap(),
            input
        );
    }

    #[test]
    fn missing_catalogue_entries_fail_the_documentation_build() {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        let error = expand_documentation_mdx(
            "<ChartCatalogGrid slugs={['not-a-real-chart']} />",
            project,
            Path::new("test.mdx"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("not-a-real-chart"));
    }
}
