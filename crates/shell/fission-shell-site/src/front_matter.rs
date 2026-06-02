#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FrontMatter {
    pub title: Option<String>,
    pub description: Option<String>,
    pub slug: Option<String>,
    pub template: Option<String>,
    pub locale: Option<String>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub show_adjacent_posts: Option<bool>,
}

pub(crate) fn split_front_matter(source: &str) -> (FrontMatter, String) {
    let normalized = source.strip_prefix('\u{feff}').unwrap_or(source);
    if !normalized.starts_with("---\n") && !normalized.starts_with("---\r\n") {
        return (FrontMatter::default(), normalized.to_string());
    }

    let body_start = if normalized.starts_with("---\r\n") {
        5
    } else {
        4
    };
    let rest = &normalized[body_start..];
    let Some((front, body)) = find_front_matter_end(rest) else {
        return (FrontMatter::default(), normalized.to_string());
    };

    (parse_front_matter(front), body.to_string())
}

fn find_front_matter_end(rest: &str) -> Option<(&str, &str)> {
    for marker in ["\n---\n", "\r\n---\r\n", "\n---\r\n", "\r\n---\n"] {
        if let Some(index) = rest.find(marker) {
            let front = &rest[..index];
            let body = &rest[index + marker.len()..];
            return Some((front, body));
        }
    }
    None
}

fn parse_front_matter(source: &str) -> FrontMatter {
    let mut front = FrontMatter::default();
    let mut current_list_key: Option<String> = None;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(item) = line.strip_prefix("- ") {
            if let Some(key) = current_list_key.as_deref() {
                push_list_value(&mut front, key, clean_scalar(item.trim()));
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            current_list_key = None;
            continue;
        };
        let key = key.trim();
        let value = clean_scalar(value.trim());
        if value.is_empty() {
            current_list_key = Some(key.to_string());
            continue;
        }
        current_list_key = None;
        match key {
            "title" => front.title = Some(value),
            "description" => front.description = Some(value),
            "slug" => front.slug = Some(value),
            "template" => front.template = Some(value),
            "locale" | "lang" | "language" => front.locale = Some(value),
            "tags" => front.tags = parse_inline_list(&value),
            "categories" | "category" => front.categories = parse_inline_list(&value),
            "show_adjacent_posts" | "show_next_previous" | "show_prev_next" | "prev_next" => {
                front.show_adjacent_posts = parse_bool(&value)
            }
            _ => {}
        }
    }
    front
}

fn push_list_value(front: &mut FrontMatter, key: &str, value: String) {
    match key {
        "tags" => front.tags.push(value),
        "categories" | "category" => front.categories.push(value),
        _ => {}
    }
}

fn parse_inline_list(value: &str) -> Vec<String> {
    let trimmed = value.trim().trim_start_matches('[').trim_end_matches(']');
    trimmed
        .split(',')
        .map(clean_scalar)
        .filter(|value| !value.is_empty())
        .collect()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => Some(true),
        "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn clean_scalar(value: &str) -> String {
    value
        .trim_matches(|ch| ch == '"' || ch == '\'')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_front_matter_fields() {
        let (front, body) =
            split_front_matter("---\ntitle: Intro\ndescription: 'Start here'\n---\n# Intro");
        assert_eq!(front.title.as_deref(), Some("Intro"));
        assert_eq!(front.description.as_deref(), Some("Start here"));
        assert_eq!(body, "# Intro");
    }

    #[test]
    fn parses_lists_and_blog_navigation_flags() {
        let (front, _) = split_front_matter(
            "---\ntags:\n  - release\n  - platform\ncategories: [docs, site]\nprev_next: false\n---\n# Intro",
        );
        assert_eq!(front.tags, vec!["release", "platform"]);
        assert_eq!(front.categories, vec!["docs", "site"]);
        assert_eq!(front.show_adjacent_posts, Some(false));
    }
}
