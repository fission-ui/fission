use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalMeta {
    pub title: String,
    pub description: String,
    pub goal: String,
    pub path: String,
    pub order: u32,
    pub difficulty: String,
    pub duration: String,
    pub app: String,
    pub prerequisites: Vec<String>,
    pub outcomes: Vec<String>,
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalStep {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub checkpoint: Option<String>,
    pub files: BTreeMap<String, String>,
}

impl GoalStep {
    pub fn preview_url(&self, app: &str) -> String {
        format!("/tutorials/{app}/{}/?fission_renderer=canvas2d", self.id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalDocument {
    pub meta: GoalMeta,
    pub introduction: String,
    pub steps: Vec<GoalStep>,
}

pub fn load_goal(path: impl AsRef<Path>) -> Result<GoalDocument> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read goal document {}", path.display()))?;
    parse_goal(&source).with_context(|| format!("invalid goal document {}", path.display()))
}

pub fn parse_goal(source: &str) -> Result<GoalDocument> {
    let (frontmatter, body) = split_frontmatter(source)?;
    let meta = parse_frontmatter(frontmatter)?;
    let (introduction, steps) = parse_steps(body)?;
    if steps.is_empty() {
        bail!("a goal document must contain at least one fission:step");
    }
    Ok(GoalDocument {
        meta,
        introduction,
        steps,
    })
}

fn split_frontmatter(source: &str) -> Result<(&str, &str)> {
    let source = source
        .strip_prefix("---\n")
        .context("goal frontmatter must start with `---`")?;
    let end = source
        .find("\n---\n")
        .context("goal frontmatter is missing its closing `---`")?;
    Ok((&source[..end], &source[end + 5..]))
}

fn parse_frontmatter(source: &str) -> Result<GoalMeta> {
    let mut scalars = BTreeMap::<String, String>::new();
    let mut lists = BTreeMap::<String, Vec<String>>::new();
    let mut active_list: Option<String> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            let key = active_list.as_ref().with_context(|| {
                format!(
                    "frontmatter line {} has a list item without a list field",
                    index + 1
                )
            })?;
            lists
                .entry(key.clone())
                .or_default()
                .push(unquote(item.trim()));
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .with_context(|| format!("frontmatter line {} must be `name: value`", index + 1))?;
        let key = key.trim().to_string();
        let value = value.trim();
        match key.as_str() {
            "layout" | "title" | "description" | "goal" | "path" | "order" | "difficulty"
            | "duration" | "app" => {
                if value.is_empty() {
                    bail!("frontmatter field `{key}` requires a value");
                }
                scalars.insert(key, unquote(value));
                active_list = None;
            }
            "prerequisites" | "outcomes" | "aliases" => {
                if !value.is_empty() && value != "[]" {
                    bail!("frontmatter list `{key}` must use indented `- item` entries");
                }
                lists.entry(key.clone()).or_default();
                active_list = Some(key);
            }
            unknown => bail!("unknown goal frontmatter field `{unknown}`"),
        }
    }

    let required = |name: &str| -> Result<String> {
        scalars
            .get(name)
            .cloned()
            .with_context(|| format!("missing required frontmatter field `{name}`"))
    };
    let layout = required("layout")?;
    if layout != "goal" {
        bail!("goal documents require `layout: goal`");
    }
    let order = required("order")?
        .parse()
        .context("`order` must be an unsigned integer")?;
    let path = required("path")?;
    if !path.starts_with('/') || !path.ends_with('/') || path.contains("..") {
        bail!("`path` must be an absolute, pretty route ending in `/`");
    }
    let app = required("app")?;
    if app.is_empty()
        || !app
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        bail!("`app` must be a lowercase URL slug");
    }
    let aliases = lists.remove("aliases").unwrap_or_default();
    if aliases
        .iter()
        .any(|alias| !alias.starts_with('/') || !alias.ends_with('/') || alias.contains(".."))
    {
        bail!("every alias must be an absolute, pretty route ending in `/`");
    }
    Ok(GoalMeta {
        title: required("title")?,
        description: required("description")?,
        goal: required("goal")?,
        path,
        order,
        difficulty: required("difficulty")?,
        duration: required("duration")?,
        app,
        prerequisites: lists.remove("prerequisites").unwrap_or_default(),
        outcomes: lists.remove("outcomes").unwrap_or_default(),
        aliases,
    })
}

fn parse_steps(body: &str) -> Result<(String, Vec<GoalStep>)> {
    const STEP_OPEN: &str = "<!-- fission:step";
    const STEP_CLOSE: &str = "<!-- /fission:step -->";
    let first = body.find(STEP_OPEN).unwrap_or(body.len());
    let introduction = body[..first].trim().to_string();
    let mut remaining = &body[first..];
    let mut steps = Vec::new();

    while !remaining.trim().is_empty() {
        let start = remaining
            .find(STEP_OPEN)
            .context("content outside a fission:step is not allowed after the first step")?;
        if !remaining[..start].trim().is_empty() {
            bail!("content outside a fission:step is not allowed after the first step");
        }
        let marker_end = remaining[start..]
            .find("-->")
            .context("unterminated fission:step marker")?
            + start;
        let marker = &remaining[start + STEP_OPEN.len()..marker_end];
        let id = marker_attr(marker, "id")?;
        let after_marker = &remaining[marker_end + 3..];
        let close = after_marker
            .find(STEP_CLOSE)
            .with_context(|| format!("step `{id}` is missing {STEP_CLOSE}"))?;
        let raw_step = &after_marker[..close];
        let step = parse_step(id, raw_step)?;
        if steps
            .iter()
            .any(|existing: &GoalStep| existing.id == step.id)
        {
            bail!("duplicate step id `{}`", step.id);
        }
        steps.push(step);
        remaining = &after_marker[close + STEP_CLOSE.len()..];
    }
    Ok((introduction, steps))
}

fn parse_step(id: String, source: &str) -> Result<GoalStep> {
    let title = source
        .lines()
        .find_map(|line| line.trim().strip_prefix("## "))
        .map(str::to_string)
        .with_context(|| format!("step `{id}` needs a level-two heading"))?;
    let files = parse_files(&id, source)?;
    if files.is_empty() {
        bail!("step `{id}` must contain at least one fission:file");
    }
    let checkpoint = extract_marker_block(
        source,
        "<!-- fission:checkpoint -->",
        "<!-- /fission:checkpoint -->",
    )?
    .map(|value| value.trim().to_string());
    let visible = strip_control_markers(source);
    let mut removed_title = false;
    let markdown = visible
        .lines()
        .filter(|line| {
            if !removed_title && line.trim_start().starts_with("## ") {
                removed_title = true;
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    Ok(GoalStep {
        id,
        title,
        markdown,
        checkpoint,
        files,
    })
}

fn parse_files(step_id: &str, source: &str) -> Result<BTreeMap<String, String>> {
    const OPEN: &str = "<!-- fission:file";
    let mut rest = source;
    let mut files = BTreeMap::new();
    while let Some(start) = rest.find(OPEN) {
        rest = &rest[start..];
        let marker_end = rest
            .find("-->")
            .with_context(|| format!("unterminated fission:file in step `{step_id}`"))?;
        let path = marker_attr(&rest[OPEN.len()..marker_end], "path")?;
        let after = rest[marker_end + 3..].trim_start();
        let fence = after.strip_prefix("```").with_context(|| {
            format!("fission:file `{path}` must be followed by a fenced code block")
        })?;
        let newline = fence
            .find('\n')
            .with_context(|| format!("fission:file `{path}` has an invalid code fence"))?;
        let code = &fence[newline + 1..];
        let end = code
            .find("\n```")
            .with_context(|| format!("fission:file `{path}` has an unterminated code fence"))?;
        if path.starts_with('/') || path.split('/').any(|part| part == ".." || part.is_empty()) {
            bail!("fission:file path `{path}` must be a safe relative path");
        }
        if files
            .insert(path.clone(), code[..end].to_string())
            .is_some()
        {
            bail!("step `{step_id}` contains duplicate file `{path}`");
        }
        rest = &code[end + 4..];
    }
    Ok(files)
}

fn extract_marker_block<'a>(source: &'a str, open: &str, close: &str) -> Result<Option<&'a str>> {
    let Some(start) = source.find(open) else {
        return Ok(None);
    };
    let after = &source[start + open.len()..];
    let end = after.find(close).context("unterminated marker block")?;
    Ok(Some(&after[..end]))
}

fn strip_control_markers(source: &str) -> String {
    let mut visible = source.to_string();
    while let Some(start) = visible.find("<!-- fission:checkpoint -->") {
        let after = start + "<!-- fission:checkpoint -->".len();
        let Some(relative_end) = visible[after..].find("<!-- /fission:checkpoint -->") else {
            break;
        };
        let end = after + relative_end + "<!-- /fission:checkpoint -->".len();
        visible.replace_range(start..end, "");
    }
    visible
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.starts_with("<!-- fission:") && !line.starts_with("<!-- /fission:")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn marker_attr(marker: &str, name: &str) -> Result<String> {
    let needle = format!("{name}=\"");
    let start = marker
        .find(&needle)
        .with_context(|| format!("marker requires `{name}=\"...\"`"))?
        + needle.len();
    let end = marker[start..]
        .find('"')
        .context("unterminated marker attribute")?
        + start;
    let value = &marker[start..end];
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        bail!("marker attribute `{name}` contains unsupported characters");
    }
    Ok(value.to_string())
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_goal_steps_and_complete_files() {
        let document =
            parse_goal(include_str!("../content/goals/interactive-counter.mdx")).unwrap();
        assert_eq!(document.meta.app, "interactive-counter");
        assert_eq!(document.steps.len(), 2);
        assert!(!document.steps[0].markdown.starts_with("## "));
        assert!(document.steps[1].files["src/lib.rs"].contains("on_press: reduce"));
    }

    #[test]
    fn rejects_unknown_frontmatter() {
        let source = include_str!("../content/goals/interactive-counter.mdx")
            .replace("order: 1", "order: 1\nmagic: true");
        assert!(parse_goal(&source)
            .unwrap_err()
            .to_string()
            .contains("unknown goal frontmatter"));
    }
}
