use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use vello::kurbo::{BezPath, Rect, RoundedRect};

lazy_static! {
    static ref SVG_CACHE: Mutex<HashMap<u64, Arc<SvgCacheEntry>>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
pub(crate) struct SvgCacheEntry {
    pub(crate) view_box: Option<(f64, f64, f64, f64)>,
    pub(crate) shapes: Vec<SvgShape>,
    pub(crate) error: Option<String>,
}

#[derive(Debug)]
pub(crate) enum SvgShape {
    Path(BezPath),
    Rect(RoundedRect),
}

fn svg_cache_key(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn quoted_attribute<'a>(tag: &'a str, name: &str) -> Result<Option<&'a str>, String> {
    let mut search_from = 0;
    let start = loop {
        let Some(relative_start) = tag[search_from..].find(name) else {
            return Ok(None);
        };
        let start = search_from + relative_start;
        let before_is_boundary = start == 0
            || tag[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        let after_name = start + name.len();
        let after_is_boundary = after_name == tag.len()
            || tag[after_name..]
                .chars()
                .next()
                .is_some_and(|character| character == '=' || character.is_whitespace());
        if before_is_boundary && after_is_boundary {
            break start;
        }
        search_from = after_name;
    };

    let after_name = tag[start + name.len()..].trim_start();
    let Some(after_equals) = after_name.strip_prefix('=') else {
        return Err(format!("{name} attribute is missing '='"));
    };
    let value = after_equals.trim_start();
    let Some(quote) = value
        .chars()
        .next()
        .filter(|quote| matches!(quote, '\'' | '"'))
    else {
        return Err(format!("{name} attribute must use a quoted value"));
    };
    let value = &value[quote.len_utf8()..];
    let Some(end) = value.find(quote) else {
        return Err(format!("{name} attribute has an unterminated quoted value"));
    };
    Ok(Some(&value[..end]))
}

fn numeric_attribute(tag: &str, name: &str, default: f64) -> Result<f64, String> {
    let Some(value) = quoted_attribute(tag, name)? else {
        return Ok(default);
    };
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{name} attribute must be a finite number"))
}

fn unsupported_transform(tag: &str, element: &str) -> Option<String> {
    match quoted_attribute(tag, "transform") {
        Ok(Some(_)) => Some(format!(
            "transform attributes on <{element}> are not supported by the Vello SVG renderer"
        )),
        Ok(None) => None,
        Err(error) => Some(format!(
            "malformed <{element}> transform attribute: {error}"
        )),
    }
}

pub(crate) fn parse_svg_entry(content: &str) -> SvgCacheEntry {
    let mut view_box = None;
    let mut shapes = Vec::new();
    let mut error = None;

    for tag in content.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or("");
        let tag_name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/');

        if tag_name == "path" {
            if let Some(transform_error) = unsupported_transform(tag, "path") {
                error.get_or_insert(transform_error);
                continue;
            }
            match quoted_attribute(tag, "d") {
                Ok(Some(data)) => {
                    let mut d = data.to_string();
                    // Preserve the existing treatment of common transparent icon
                    // bounding paths, whose visual bounds must not become paint.
                    d = d.replace("M0 0h24v24H0z", "");
                    d = d.replace("M0 0h24v24H0V0z", "");
                    d = d.replace("M0,0h24v24H0V0z", "");
                    if !d.trim().is_empty() {
                        match BezPath::from_svg(&d) {
                            Ok(bez_path) => shapes.push(SvgShape::Path(bez_path)),
                            Err(parse_error) => {
                                error.get_or_insert_with(|| {
                                    format!("path element contains invalid d data: {parse_error}")
                                });
                            }
                        }
                    }
                }
                Ok(None) => {
                    error.get_or_insert_with(|| {
                        "path element is missing a quoted d attribute".to_string()
                    });
                }
                Err(attribute_error) => {
                    error.get_or_insert_with(|| {
                        format!("path element has a malformed d attribute: {attribute_error}")
                    });
                }
            }
        } else if tag_name == "rect" {
            if tag.contains("fill=\"none\"") || tag.contains("fill='none'") {
                continue;
            }
            if let Some(transform_error) = unsupported_transform(tag, "rect") {
                error.get_or_insert(transform_error);
                continue;
            }
            match (
                numeric_attribute(tag, "x", 0.0),
                numeric_attribute(tag, "y", 0.0),
                numeric_attribute(tag, "width", 0.0),
                numeric_attribute(tag, "height", 0.0),
            ) {
                (Ok(x), Ok(y), Ok(width), Ok(height)) if width > 0.0 && height > 0.0 => {
                    shapes.push(SvgShape::Rect(RoundedRect::from_rect(
                        Rect::new(x, y, x + width, y + height),
                        0.0,
                    )));
                }
                (Ok(_), Ok(_), Ok(width), Ok(height)) if width < 0.0 || height < 0.0 => {
                    error.get_or_insert_with(|| {
                        "rect width and height attributes must not be negative".to_string()
                    });
                }
                (Ok(_), Ok(_), Ok(_), Ok(_)) => {}
                values => {
                    let attribute_error = [
                        values.0.err(),
                        values.1.err(),
                        values.2.err(),
                        values.3.err(),
                    ]
                    .into_iter()
                    .flatten()
                    .next()
                    .expect("at least one rectangle attribute failed");
                    error.get_or_insert_with(|| {
                        format!("rect element has a malformed attribute: {attribute_error}")
                    });
                }
            }
        } else if tag_name == "polygon" {
            if let Some(transform_error) = unsupported_transform(tag, "polygon") {
                error.get_or_insert(transform_error);
                continue;
            }
            match quoted_attribute(tag, "points") {
                Ok(Some(points)) => {
                    let numbers = points
                        .split(|c: char| c.is_whitespace() || c == ',')
                        .filter(|s| !s.is_empty())
                        .map(str::parse::<f64>)
                        .collect::<Result<Vec<_>, _>>();
                    match numbers {
                        Ok(numbers)
                            if numbers.len() >= 4
                                && numbers.len() % 2 == 0
                                && numbers.iter().all(|value| value.is_finite()) =>
                        {
                            let mut bez = BezPath::new();
                            bez.move_to((numbers[0], numbers[1]));
                            for point in numbers[2..].chunks_exact(2) {
                                bez.line_to((point[0], point[1]));
                            }
                            bez.close_path();
                            shapes.push(SvgShape::Path(bez));
                        }
                        _ => {
                            error.get_or_insert_with(|| {
                                "polygon points must be finite numeric coordinate pairs".to_string()
                            });
                        }
                    }
                }
                Ok(None) => {
                    error.get_or_insert_with(|| {
                        "polygon element is missing a quoted points attribute".to_string()
                    });
                }
                Err(attribute_error) => {
                    error.get_or_insert_with(|| {
                        format!(
                            "polygon element has a malformed points attribute: {attribute_error}"
                        )
                    });
                }
            }
        } else if tag_name == "svg" {
            if let Some(transform_error) = unsupported_transform(tag, "svg") {
                error.get_or_insert(transform_error);
            }
            match quoted_attribute(tag, "viewBox") {
                Ok(Some(value)) => {
                    let numbers = value
                        .split(|character: char| character.is_whitespace() || character == ',')
                        .filter(|value| !value.is_empty())
                        .map(str::parse::<f64>)
                        .collect::<Result<Vec<_>, _>>();
                    match numbers {
                        Ok(numbers)
                            if numbers.len() == 4
                                && numbers.iter().all(|value| value.is_finite())
                                && numbers[2] >= 0.0
                                && numbers[3] >= 0.0 =>
                        {
                            view_box = Some((numbers[0], numbers[1], numbers[2], numbers[3]));
                        }
                        _ => {
                            error.get_or_insert_with(|| {
                                "viewBox must contain four finite numeric values".to_string()
                            });
                        }
                    }
                }
                Ok(None) => {}
                Err(attribute_error) => {
                    error.get_or_insert_with(|| {
                        format!("svg element has a malformed viewBox: {attribute_error}")
                    });
                }
            }
        } else if tag_name == "g" {
            if let Some(transform_error) = unsupported_transform(tag, "g") {
                error.get_or_insert(transform_error);
            }
        } else if matches!(tag_name, "" | "title" | "desc")
            || tag_name.starts_with('/')
            || tag_name.starts_with('!')
            || tag_name.starts_with('?')
        {
            continue;
        } else {
            error.get_or_insert_with(|| format!("unsupported SVG element <{tag_name}>"));
        }
    }

    SvgCacheEntry {
        view_box,
        shapes,
        error,
    }
}

pub(crate) fn svg_cache_entry(content: &str) -> Arc<SvgCacheEntry> {
    let key = svg_cache_key(content);
    if let Some(entry) = SVG_CACHE.lock().unwrap().get(&key) {
        return Arc::clone(entry);
    }

    let parsed = Arc::new(parse_svg_entry(content));
    let mut cache = SVG_CACHE.lock().unwrap();
    cache.entry(key).or_insert_with(|| Arc::clone(&parsed));
    parsed
}
