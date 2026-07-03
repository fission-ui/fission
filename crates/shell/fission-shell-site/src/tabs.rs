use anyhow::{bail, Result};
use serde_json::json;

#[derive(Debug, Default)]
struct TabsGroup {
    tabs: Vec<TabPanel>,
}

#[derive(Debug)]
struct TabPanel {
    label: String,
    value: String,
    content: String,
}

#[derive(Debug)]
struct OpenTab {
    label: String,
    value: String,
    content: String,
}

/// Lowers Docusaurus-style MDX `<Tabs>/<TabItem>` blocks into Markdown marker
/// fences. Markdown inside each panel is still rendered by Fission; the site
/// enhancement script only groups the already-rendered nodes into interactive tabs.
pub(crate) fn expand_mdx_tabs(markdown: &str) -> Result<String> {
    let mut out = String::new();
    let mut group: Option<TabsGroup> = None;
    let mut tab: Option<OpenTab> = None;
    let mut fence: Option<&'static str> = None;
    let mut group_index = 0usize;

    for line in markdown.lines() {
        let trimmed = line.trim();

        if let Some(marker) = fence {
            append_line(&mut out, group.as_mut(), tab.as_mut(), line);
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }

        if let Some(marker) = code_fence_marker(trimmed) {
            if group.is_some() && tab.is_none() {
                bail!("content inside <Tabs> must be wrapped in <TabItem>: {trimmed}");
            }
            append_line(&mut out, group.as_mut(), tab.as_mut(), line);
            fence = Some(marker);
            continue;
        }

        if group.is_none() {
            if is_tabs_import(trimmed) {
                continue;
            }
            if is_tabs_start(trimmed) {
                group = Some(TabsGroup::default());
                group_index += 1;
                continue;
            }
            if is_tabs_end(trimmed) || is_tab_item_start(trimmed) || is_tab_item_end(trimmed) {
                bail!("encountered {trimmed} outside a <Tabs> block");
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if let Some(open_tab) = tab.as_mut() {
            if is_tab_item_end(trimmed) {
                let finished = tab.take().expect("tab is open");
                group.as_mut().expect("group is open").tabs.push(TabPanel {
                    label: finished.label,
                    value: finished.value,
                    content: finished.content,
                });
                continue;
            }
            if is_tabs_end(trimmed) {
                bail!("encountered </Tabs> before closing the current <TabItem>");
            }
            if is_tab_item_start(trimmed) {
                bail!("encountered nested <TabItem>; close the previous tab first");
            }
            open_tab.content.push_str(line);
            open_tab.content.push('\n');
            continue;
        }

        if is_tabs_end(trimmed) {
            let finished = group.take().expect("group is open");
            if finished.tabs.is_empty() {
                bail!("<Tabs> block must contain at least one <TabItem>");
            }
            out.push_str(&render_tabs_group(&finished, group_index));
            continue;
        }

        if is_tab_item_start(trimmed) {
            let index = group.as_ref().expect("group is open").tabs.len();
            let label = attr_value(trimmed, "label")
                .or_else(|| attr_value(trimmed, "value"))
                .unwrap_or_else(|| format!("Option {}", index + 1));
            let value = attr_value(trimmed, "value").unwrap_or_else(|| slugify(&label, index));
            tab = Some(OpenTab {
                label,
                value,
                content: String::new(),
            });
            continue;
        }

        if !trimmed.is_empty() {
            bail!("content inside <Tabs> must be wrapped in <TabItem>: {trimmed}");
        }
    }

    if tab.is_some() {
        bail!("unclosed <TabItem> in <Tabs> block");
    }
    if group.is_some() {
        bail!("unclosed <Tabs> block");
    }

    Ok(out)
}

fn append_line(
    out: &mut String,
    group: Option<&mut TabsGroup>,
    tab: Option<&mut OpenTab>,
    line: &str,
) {
    if group.is_some() {
        if let Some(open_tab) = tab {
            open_tab.content.push_str(line);
            open_tab.content.push('\n');
        }
    } else {
        out.push_str(line);
        out.push('\n');
    }
}

fn render_tabs_group(group: &TabsGroup, group_index: usize) -> String {
    let id = format!("fission-tabs-{group_index}");
    let tabs = group
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            json!({
                "index": index,
                "label": tab.label,
                "value": tab.value,
            })
        })
        .collect::<Vec<_>>();
    let mut out = String::new();
    out.push_str("```fission-tabs-start\n");
    out.push_str(
        &json!({
            "id": id,
            "tabs": tabs,
        })
        .to_string(),
    );
    out.push_str("\n```\n\n");

    for (index, tab) in group.tabs.iter().enumerate() {
        out.push_str("```fission-tab-start\n");
        out.push_str(
            &json!({
                "id": id,
                "index": index,
                "label": tab.label,
                "value": tab.value,
            })
            .to_string(),
        );
        out.push_str("\n```\n\n");
        out.push_str(tab.content.trim_matches('\n'));
        out.push_str("\n\n");
    }

    out.push_str("```fission-tabs-end\n");
    out.push_str(&json!({ "id": id }).to_string());
    out.push_str("\n```\n");
    out
}

fn is_tabs_import(trimmed: &str) -> bool {
    trimmed.starts_with("import Tabs from ") || trimmed.starts_with("import TabItem from ")
}

fn is_tabs_start(trimmed: &str) -> bool {
    is_start_tag(trimmed, "Tabs")
}

fn is_tabs_end(trimmed: &str) -> bool {
    trimmed == "</Tabs>"
}

fn is_tab_item_start(trimmed: &str) -> bool {
    is_start_tag(trimmed, "TabItem")
}

fn is_tab_item_end(trimmed: &str) -> bool {
    trimmed == "</TabItem>"
}

fn is_start_tag(trimmed: &str, name: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix('<') else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(name) else {
        return false;
    };
    matches!(rest.chars().next(), Some('>' | '/' | ' ' | '\t'))
}

fn code_fence_marker(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn attr_value(line: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=");
    let start = line.find(&needle)? + needle.len();
    let value = line[start..].trim_start();
    let quote = value.chars().next()?;
    if quote == '"' || quote == '\'' {
        let value = &value[quote.len_utf8()..];
        let end = value.find(quote)?;
        return Some(value[..end].to_string());
    }
    let end = value
        .find(|ch: char| ch.is_whitespace() || ch == '>')
        .unwrap_or(value.len());
    let value = value[..end].trim_end_matches('/');
    (!value.is_empty()).then(|| value.to_string())
}

fn slugify(label: &str, index: usize) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in label.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        format!("tab-{index}")
    } else {
        slug.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_mdx_tabs_without_touching_code_fences() {
        let markdown = r#"import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

# Compare

<Tabs>
<TabItem value="react" label="React">

```tsx
<TabItem label="Not parsed inside code" />
```

React body.

</TabItem>
<TabItem value="fission" label="Fission">

Fission body.

</TabItem>
</Tabs>
"#;

        let expanded = expand_mdx_tabs(markdown).unwrap();

        assert!(!expanded.contains("@theme/Tabs"));
        assert!(expanded.contains("```fission-tabs-start"));
        assert!(expanded.contains("\"label\":\"React\""));
        assert!(expanded.contains("\"value\":\"fission\""));
        assert!(expanded.contains("```tsx\n<TabItem label=\"Not parsed inside code\" />\n```"));
        assert!(expanded.contains("React body."));
        assert!(expanded.contains("Fission body."));
        assert!(expanded.contains("```fission-tabs-end"));
    }

    #[test]
    fn rejects_unclosed_tabs() {
        let error = expand_mdx_tabs("<Tabs>\n<TabItem label=\"A\">\nBody\n</Tabs>")
            .expect_err("tabs should fail");
        assert!(error.to_string().contains("before closing"));
    }
}
