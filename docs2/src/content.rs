use anyhow::{bail, Context, Result};
use fission::site::expand_mdx_tabs;
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
    pub run_command: String,
    pub app: String,
    pub outcomes: Vec<String>,
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalStep {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub preview: bool,
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
    pub project_setup: Option<String>,
    pub steps: Vec<GoalStep>,
}

pub fn load_goal(path: impl AsRef<Path>) -> Result<GoalDocument> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read goal document {}", path.display()))?;
    parse_goal(&source).with_context(|| format!("invalid goal document {}", path.display()))
}

pub fn load_goals(directory: impl AsRef<Path>) -> Result<Vec<GoalDocument>> {
    let directory = directory.as_ref();
    let mut pending = vec![directory.to_path_buf()];
    let mut documents = Vec::new();

    while let Some(current) = pending.pop() {
        let entries = fs::read_dir(&current).with_context(|| {
            format!(
                "failed to read goal content directory {}",
                current.display()
            )
        })?;
        let mut paths = entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();

        for path in paths.into_iter().rev() {
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("mdx") {
                documents.push(load_goal(path)?);
            }
        }
    }

    documents.sort_by(|left, right| {
        left.meta
            .order
            .cmp(&right.meta.order)
            .then_with(|| left.meta.path.cmp(&right.meta.path))
    });
    if documents.is_empty() {
        bail!(
            "goal content directory {} contains no .mdx tutorials",
            directory.display()
        );
    }
    validate_goal_collection(&documents)?;
    Ok(documents)
}

fn validate_goal_collection(documents: &[GoalDocument]) -> Result<()> {
    let mut routes = BTreeMap::<String, String>::new();
    let mut apps = BTreeMap::<String, String>::new();

    for document in documents {
        let title = &document.meta.title;
        if let Some(previous) = apps.insert(document.meta.app.clone(), title.clone()) {
            bail!(
                "tutorial app slug `{}` is shared by `{previous}` and `{title}`",
                document.meta.app
            );
        }

        for route in std::iter::once(&document.meta.path).chain(document.meta.aliases.iter()) {
            if let Some(previous) = routes.insert(route.clone(), title.clone()) {
                bail!("tutorial route `{route}` is shared by `{previous}` and `{title}`");
            }
        }
    }

    Ok(())
}

pub fn parse_goal(source: &str) -> Result<GoalDocument> {
    let (frontmatter, body) = split_frontmatter(source)?;
    let meta = parse_frontmatter(frontmatter)?;
    let (introduction, steps) = parse_steps(body)?;
    if steps.is_empty() {
        bail!("a goal document must contain at least one fission:step");
    }
    if steps[0].files.is_empty() {
        bail!("the first fission:step must contain the initial fission:file sources");
    }
    let (introduction, project_setup) = extract_project_setup(introduction);
    Ok(GoalDocument {
        meta,
        introduction,
        project_setup,
        steps,
    })
}

fn extract_project_setup(introduction: String) -> (String, Option<String>) {
    fn is_setup_heading(line: &str) -> bool {
        matches!(
            line.trim().to_ascii_lowercase().as_str(),
            "## create the project and add the target"
                | "## files created by fission"
                | "## project layout"
                | "## create the android project"
                | "## create an ios project"
                | "## create an android project"
                | "## create the web project and enable storage"
                | "## create the server-rendered project"
                | "## create the web project"
                | "## add the target to your existing project"
                | "## shared project and new target"
                | "## create the android app and declare biometrics"
                | "## create the android app"
                | "## create the ios app and declare the capability"
                | "## create the shared mobile project"
        )
    }

    let lines: Vec<&str> = introduction.lines().collect();
    let mut visible = Vec::new();
    let mut setup_sections = Vec::new();
    let mut index = 0;
    let mut in_fence = false;

    while index < lines.len() {
        let line = lines[index];
        if !in_fence && is_setup_heading(line) {
            let start = index;
            index += 1;
            let mut section_in_fence = false;
            while index < lines.len() {
                let current = lines[index];
                if current.trim_start().starts_with("```") {
                    section_in_fence = !section_in_fence;
                } else if !section_in_fence && current.starts_with('#') {
                    break;
                }
                index += 1;
            }
            setup_sections.push(lines[start..index].join("\n").trim().to_string());
            continue;
        }

        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        visible.push(line);
        index += 1;
    }

    let visible = visible.join("\n").trim().to_string();
    let setup = setup_sections
        .into_iter()
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    (visible, (!setup.is_empty()).then_some(setup))
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
            | "duration" | "run_command" | "app" => {
                if value.is_empty() {
                    bail!("frontmatter field `{key}` requires a value");
                }
                scalars.insert(key, unquote(value));
                active_list = None;
            }
            "outcomes" | "aliases" => {
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
        run_command: required("run_command")?,
        app,
        outcomes: lists.remove("outcomes").unwrap_or_default(),
        aliases,
    })
}

fn parse_steps(body: &str) -> Result<(String, Vec<GoalStep>)> {
    const STEP_OPEN: &str = "<!-- fission:step";
    const STEP_CLOSE: &str = "<!-- /fission:step -->";
    let first = body.find(STEP_OPEN).unwrap_or(body.len());
    let introduction = expand_mdx_tabs(body[..first].trim())?;
    let mut remaining = &body[first..];
    let mut steps: Vec<GoalStep> = Vec::new();

    while !remaining.trim().is_empty() {
        let Some(start) = remaining.find(STEP_OPEN) else {
            if let Some(last) = steps.last_mut() {
                let trailing = remaining.trim();
                if !trailing.is_empty() {
                    last.markdown.push_str("\n\n");
                    last.markdown.push_str(trailing);
                }
            }
            break;
        };
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
    let checkpoint = extract_marker_block(
        source,
        "<!-- fission:checkpoint -->",
        "<!-- /fission:checkpoint -->",
    )?
    .map(|value| value.trim().to_string());
    let preview_count = source.matches("<!-- fission:preview -->").count();
    if preview_count > 1 {
        bail!("step `{id}` can contain at most one fission:preview marker");
    }
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
    let markdown = expand_mdx_tabs(&markdown)?;
    Ok(GoalStep {
        id,
        title,
        markdown,
        preview: preview_count == 1,
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
        let newline = after
            .find('\n')
            .with_context(|| format!("fission:file `{path}` has an invalid code fence"))?;
        let opening = &after[..newline];
        let delimiter = opening
            .chars()
            .next()
            .filter(|ch| matches!(ch, '`' | '~'))
            .with_context(|| format!("fission:file `{path}` needs a Markdown code fence"))?;
        let width = opening.chars().take_while(|ch| *ch == delimiter).count();
        if width < 3 {
            bail!("fission:file `{path}` needs a fence of at least three characters");
        }
        let code = &after[newline + 1..];
        // A Markdown source file may itself contain triple-backtick examples.
        // Its outer four-backtick (or tilde) fence must determine extraction.
        let mut end = None;
        let mut offset = 0;
        for line in code.split_inclusive('\n') {
            let trimmed = line.trim_start_matches(' ').trim_end();
            let indentation = line.len() - line.trim_start_matches(' ').len();
            if indentation <= 3
                && trimmed.chars().count() >= width
                && trimmed.chars().all(|ch| ch == delimiter)
            {
                end = Some((offset, offset + line.len()));
                break;
            }
            offset += line.len();
        }
        let (end, next) =
            end.with_context(|| format!("fission:file `{path}` has an unterminated code fence"))?;
        if path.starts_with('/') || path.split('/').any(|part| part == ".." || part.is_empty()) {
            bail!("fission:file path `{path}` must be a safe relative path");
        }
        if files
            .insert(
                path.clone(),
                code[..end]
                    .strip_suffix('\n')
                    .unwrap_or(&code[..end])
                    .to_string(),
            )
            .is_some()
        {
            bail!("step `{step_id}` contains duplicate file `{path}`");
        }
        rest = &code[next..];
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
    fn extracts_markdown_files_without_truncating_nested_code_examples() {
        let source = "<!-- fission:file path=\"content/guide.mdx\" -->\n````markdown\n# Guide\n\n```rust\nfn main() {}\n```\n\nRead the result.\n````\n<!-- fission:file path=\"settings.toml\" -->\n~~~toml\nenabled = true\n~~~\n";
        let files = parse_files("nested-markdown", source).unwrap();
        assert_eq!(
            files["content/guide.mdx"],
            "# Guide\n\n```rust\nfn main() {}\n```\n\nRead the result."
        );
        assert_eq!(files["settings.toml"], "enabled = true");
    }

    #[test]
    fn closing_fence_must_match_the_opening_delimiter_and_width() {
        for source in [
            "<!-- fission:file path=\"page.md\" -->\n````markdown\ntext\n```\n",
            "<!-- fission:file path=\"page.md\" -->\n~~~markdown\ntext\n```\n",
        ] {
            assert!(parse_files("bad-fence", source)
                .unwrap_err()
                .to_string()
                .contains("unterminated"));
        }
    }

    #[test]
    fn parses_goal_steps_and_complete_files() {
        let document = parse_goal(include_str!(
            "../content/goals/single-page-web-app-rust.mdx"
        ))
        .unwrap();
        assert_eq!(document.meta.app, "single-page-web-app-rust");
        assert_eq!(document.steps.len(), 3);
        assert!(document.steps.iter().all(|step| step.preview));
        assert!(!document.steps[0].markdown.starts_with("## "));
        assert!(document.steps[1].files["src/pages/board.rs"].contains("key: task.id"));
        let mut cumulative_files = BTreeMap::new();
        for step in &document.steps {
            cumulative_files.extend(step.files.clone());
            assert!(cumulative_files.contains_key("src/lib.rs"));
        }
        assert!(document.steps[2]
            .markdown
            .contains("Run, test and build the finished project"));
    }

    #[test]
    fn rejects_unknown_frontmatter() {
        let source = include_str!("../content/goals/single-page-web-app-rust.mdx")
            .replace("order: 16", "order: 16\nmagic: true");
        assert!(parse_goal(&source)
            .unwrap_err()
            .to_string()
            .contains("unknown goal frontmatter"));
    }

    #[test]
    fn loads_all_goal_documents_in_stable_order() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("content/goals");
        let goals = load_goals(root).unwrap();

        assert_eq!(goals.len(), 118);
        assert_eq!(goals[0].meta.order, 1);
        assert_eq!(
            goals[0].meta.path,
            "/tutorials/build-a-rust-website-with-dropdown-navigation/"
        );
        assert_eq!(goals[117].meta.order, 118);
        for route in [crate::navigation::LOGIN_PAGE, crate::navigation::LIVE_TEST] {
            assert!(
                goals.iter().any(|goal| goal.meta.path == route),
                "navigation must use the tutorial's canonical route: {route}"
            );
        }
    }

    #[test]
    fn sprint_tutorial_does_not_shadow_the_router_reference() {
        let tutorial = parse_goal(include_str!(
            "../content/goals/single-page-web-app-rust.mdx"
        ))
        .unwrap();

        assert_ne!(tutorial.meta.path, crate::navigation::ROUTER);
        assert!(tutorial
            .meta
            .aliases
            .iter()
            .all(|alias| alias != crate::navigation::ROUTER));
    }
}
