use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogPost {
    pub title: String,
    pub description: String,
    pub slug: String,
    pub path: String,
    pub published: String,
    pub authors: Vec<String>,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub markdown: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogPostSummary {
    pub title: String,
    pub description: String,
    pub path: String,
    pub published: String,
    pub authors: Vec<String>,
    pub categories: Vec<String>,
}

impl BlogPost {
    pub fn summary(&self) -> BlogPostSummary {
        BlogPostSummary {
            title: self.title.clone(),
            description: self.description.clone(),
            path: self.path.clone(),
            published: self.published.clone(),
            authors: self.authors.clone(),
            categories: self.categories.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogCategoryLink {
    pub title: String,
    pub path: String,
    pub count: usize,
}

pub fn load_blog_posts(directory: impl AsRef<Path>) -> Result<Vec<BlogPost>> {
    let directory = directory.as_ref();
    let entries = fs::read_dir(directory).with_context(|| {
        format!(
            "failed to read blog content directory {}",
            directory.display()
        )
    })?;
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();

    let mut posts = Vec::new();
    for path in paths {
        if path.extension().and_then(|value| value.to_str()) != Some("mdx")
            || path.file_stem().and_then(|value| value.to_str()) == Some("index")
        {
            continue;
        }
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read blog post {}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .context("blog post filename is not valid UTF-8")?;
        posts.push(parse_blog_post(&source, filename)?);
    }

    posts.sort_by(|left, right| {
        right
            .published
            .cmp(&left.published)
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut routes = BTreeMap::new();
    for post in &posts {
        if let Some(previous) = routes.insert(post.path.clone(), post.title.clone()) {
            bail!(
                "blog route `{}` is shared by `{previous}` and `{}`",
                post.path,
                post.title
            );
        }
    }
    if posts.is_empty() {
        bail!(
            "blog content directory {} contains no posts",
            directory.display()
        );
    }
    Ok(posts)
}

fn parse_blog_post(source: &str, filename: &str) -> Result<BlogPost> {
    let source = source
        .strip_prefix("---\n")
        .context("blog post frontmatter must start with `---`")?;
    let end = source
        .find("\n---\n")
        .context("blog post frontmatter is missing its closing `---`")?;
    let (frontmatter, body) = source.split_at(end);
    let body = &body[5..];
    let mut scalars = BTreeMap::<String, String>::new();
    let mut lists = BTreeMap::<String, Vec<String>>::new();
    let mut active_list: Option<String> = None;

    for raw_line in frontmatter.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            if let Some(key) = &active_list {
                lists
                    .entry(key.clone())
                    .or_default()
                    .push(unquote(item.trim()));
            }
            continue;
        }
        let Some((key, value)) = line.trim().split_once(':') else {
            active_list = None;
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "authors" | "categories" | "tags" if value.is_empty() || value == "[]" => {
                lists.entry(key.to_string()).or_default();
                active_list = Some(key.to_string());
            }
            "title" | "description" | "slug" if !value.is_empty() => {
                scalars.insert(key.to_string(), unquote(value));
                active_list = None;
            }
            _ => active_list = None,
        }
    }

    let title = scalars
        .remove("title")
        .context("blog post frontmatter is missing `title`")?;
    let (published, filename_slug) = split_filename(filename)?;
    let slug = scalars.remove("slug").unwrap_or(filename_slug);
    if slug.is_empty()
        || !slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        bail!("blog post slug `{slug}` must contain lowercase letters, digits and hyphens only");
    }
    let path = format!("/blog/{slug}/");
    let mut markdown = body.trim().to_string();
    if let Some(first) = markdown.lines().next() {
        if first.trim() == format!("# {title}") {
            markdown = markdown[first.len()..].trim_start().to_string();
        }
    }
    let markdown = linkify_bare_urls(&markdown);
    let description = scalars
        .remove("description")
        .unwrap_or_else(|| excerpt(&markdown));
    let categories = lists
        .remove("categories")
        .unwrap_or_default()
        .into_iter()
        .map(|category| normalize_category(&category))
        .collect();

    Ok(BlogPost {
        title,
        description,
        slug,
        path,
        published,
        authors: lists.remove("authors").unwrap_or_default(),
        categories,
        tags: lists.remove("tags").unwrap_or_default(),
        markdown,
    })
}

/// The previous Docusaurus blog automatically linked bare URLs. Preserve that
/// behavior for migrated posts while leaving fenced and inline code untouched.
fn linkify_bare_urls(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut fence: Option<(u8, usize)> = None;

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            output.push_str(line);
            output.push('\n');
            if is_closing_fence(trimmed, marker) {
                fence = None;
            }
            continue;
        }
        if let Some(marker) = opening_fence(trimmed) {
            fence = Some(marker);
            output.push_str(line);
            output.push('\n');
            continue;
        }
        output.push_str(&linkify_markdown_line(line));
        output.push('\n');
    }

    if !markdown.ends_with('\n') {
        output.pop();
    }
    output
}

fn opening_fence(line: &str) -> Option<(u8, usize)> {
    let marker = *line.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = line.bytes().take_while(|byte| *byte == marker).count();
    (length >= 3).then_some((marker, length))
}

fn is_closing_fence(line: &str, marker: (u8, usize)) -> bool {
    let length = line.bytes().take_while(|byte| *byte == marker.0).count();
    length >= marker.1 && line[length..].trim().is_empty()
}

fn linkify_markdown_line(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut cursor = 0;
    let mut code_delimiter: Option<usize> = None;
    let bytes = line.as_bytes();

    while cursor < bytes.len() {
        if bytes[cursor] == b'`' {
            let delimiter_len = bytes[cursor..]
                .iter()
                .take_while(|byte| **byte == b'`')
                .count();
            if code_delimiter == Some(delimiter_len) {
                code_delimiter = None;
            } else if code_delimiter.is_none() {
                code_delimiter = Some(delimiter_len);
            }
            output.push_str(&line[cursor..cursor + delimiter_len]);
            cursor += delimiter_len;
            continue;
        }

        if code_delimiter.is_none()
            && (line[cursor..].starts_with("https://") || line[cursor..].starts_with("http://"))
        {
            let is_existing_destination = output.ends_with("](") || output.ends_with('<');
            if !is_existing_destination {
                let mut end = cursor;
                while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
                    end += 1;
                }
                let mut url_end = end;
                while url_end > cursor
                    && matches!(bytes[url_end - 1], b'.' | b',' | b';' | b':' | b'!' | b'?')
                {
                    url_end -= 1;
                }
                while url_end > cursor && bytes[url_end - 1] == b')' {
                    let candidate = &line[cursor..url_end];
                    let opens = candidate.bytes().filter(|byte| *byte == b'(').count();
                    let closes = candidate.bytes().filter(|byte| *byte == b')').count();
                    if closes <= opens {
                        break;
                    }
                    url_end -= 1;
                }
                if url_end > cursor {
                    let url = &line[cursor..url_end];
                    let label = url
                        .strip_prefix("https://")
                        .or_else(|| url.strip_prefix("http://"))
                        .unwrap_or(url);
                    output.push('[');
                    output.push_str(label);
                    output.push_str("](");
                    output.push_str(url);
                    output.push(')');
                    output.push_str(&line[url_end..end]);
                    cursor = end;
                    continue;
                }
            }
        }

        let character = line[cursor..].chars().next().expect("cursor is in bounds");
        output.push(character);
        cursor += character.len_utf8();
    }

    output
}

fn split_filename(filename: &str) -> Result<(String, String)> {
    let stem = filename
        .strip_suffix(".mdx")
        .context("blog post filename must end with `.mdx`")?;
    let bytes = stem.as_bytes();
    if bytes.len() < 12
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'-'
        || !bytes[..10].iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7) && *byte == b'-'
                || !matches!(index, 4 | 7) && byte.is_ascii_digit()
        })
    {
        bail!("blog post filename `{filename}` must start with YYYY-MM-DD-");
    }
    Ok((stem[..10].to_string(), stem[11..].to_string()))
}

fn excerpt(markdown: &str) -> String {
    let paragraph = markdown
        .split("\n\n")
        .map(str::trim)
        .find(|part| !part.is_empty() && !part.starts_with('#') && !part.starts_with("```"))
        .unwrap_or("Read this Fission engineering article for release details and implementation guidance.");
    let plain = markdown_plain_text(paragraph);
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if plain.chars().count() <= 160 {
        return plain;
    }
    let mut excerpt = plain.chars().take(157).collect::<String>();
    if let Some(boundary) = excerpt.rfind(char::is_whitespace) {
        excerpt.truncate(boundary);
    }
    excerpt.push('…');
    excerpt
}

fn markdown_plain_text(markdown: &str) -> String {
    let mut text = markdown.to_string();
    while let Some(close) = text.find("](") {
        let Some(open) = text[..close].rfind('[') else {
            break;
        };
        let Some(end) = text[close + 2..].find(')') else {
            break;
        };
        let label = text[open + 1..close].to_string();
        text.replace_range(open..close + 3 + end, &label);
    }
    text.chars()
        .filter(|character| !matches!(character, '#' | '*' | '_' | '`' | '>' | '~'))
        .collect()
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_string()
}

pub fn category_route(category: &str) -> String {
    format!(
        "/blog/categories/{}/",
        category.to_ascii_lowercase().replace(' ', "-")
    )
}

fn normalize_category(category: &str) -> String {
    match category.to_ascii_lowercase().as_str() {
        "release" | "releases" => "Releases".to_string(),
        "project" => "Project".to_string(),
        "migration" => "Migration".to_string(),
        "tutorial" | "tutorials" => "Tutorials".to_string(),
        _ => category.to_string(),
    }
}

pub fn blog_categories(posts: &[BlogPost]) -> Vec<BlogCategoryLink> {
    let mut counts = BTreeMap::<String, usize>::new();
    for post in posts {
        for category in &post.categories {
            if category.eq_ignore_ascii_case("tutorials") {
                continue;
            }
            *counts.entry(category.clone()).or_default() += 1;
        }
    }
    let mut categories = counts
        .into_iter()
        .map(|(title, count)| BlogCategoryLink {
            path: category_route(&title),
            title,
            count,
        })
        .collect::<Vec<_>>();
    categories.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.title.cmp(&right.title))
    });
    categories
}
