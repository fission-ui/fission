use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum Mode {
    Light,
    Dark,
}
impl Mode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub(super) fn color_name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Token {
    pub(super) value: String,
    pub(super) kind: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct TokenStore {
    tokens: BTreeMap<String, Token>,
}

impl TokenStore {
    pub(super) fn from_value(value: &Value) -> Result<Self> {
        let mut tokens = BTreeMap::new();
        flatten_tokens(value, String::new(), &mut tokens)?;
        Ok(Self { tokens })
    }

    pub(super) fn contains(&self, path: &str) -> bool {
        self.tokens.contains_key(path)
    }
    pub(super) fn paths(&self) -> impl Iterator<Item = &str> {
        self.tokens.keys().map(String::as_str)
    }
    pub(super) fn get_raw(&self, path: &str) -> Option<&Token> {
        self.tokens.get(path)
    }

    pub(super) fn resolve(&self, path: &str) -> Result<String> {
        self.resolve_inner(path, &mut BTreeSet::new())
    }

    pub(super) fn resolve_inner(&self, path: &str, seen: &mut BTreeSet<String>) -> Result<String> {
        if !seen.insert(path.to_string()) {
            bail!("cyclic token reference involving {path}");
        }
        let token = self
            .tokens
            .get(path)
            .ok_or_else(|| anyhow!("unknown token reference {{{path}}}"))?;
        let value = token.value.trim();
        if let Some(inner) = value.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            let resolved = self.resolve_inner(inner, seen)?;
            seen.remove(path);
            return Ok(resolved);
        }
        let mut out = String::new();
        let mut rest = value;
        while let Some(start) = rest.find('{') {
            let (before, after_start) = rest.split_at(start);
            out.push_str(before);
            let after_start = &after_start[1..];
            let Some(end) = after_start.find('}') else {
                bail!("unclosed token reference in {value}");
            };
            let inner = &after_start[..end];
            out.push_str(&self.resolve_inner(inner, seen)?);
            rest = &after_start[end + 1..];
        }
        out.push_str(rest);
        seen.remove(path);
        Ok(out)
    }
}

pub(super) fn flatten_tokens(
    value: &Value,
    prefix: String,
    out: &mut BTreeMap<String, Token>,
) -> Result<()> {
    let Some(obj) = value.as_object() else {
        return Ok(());
    };
    for (key, child) in obj {
        if key.starts_with('$') {
            continue;
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some(child_obj) = child.as_object() {
            if let Some(raw_value) = token_object_value(child_obj) {
                let token_value = match raw_value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    other => other.to_string(),
                };
                let kind = child_obj
                    .get("type")
                    .or_else(|| child_obj.get("$type"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                out.insert(
                    path,
                    Token {
                        value: token_value,
                        kind,
                    },
                );
            } else {
                flatten_tokens(child, path, out)?;
            }
        }
    }
    Ok(())
}

pub(super) fn token_object_value(obj: &serde_json::Map<String, Value>) -> Option<&Value> {
    let value_field = obj.get("value").or_else(|| obj.get("$value"))?;
    if obj.contains_key("type") || obj.contains_key("$type") {
        return Some(value_field);
    }
    if value_field
        .as_object()
        .is_some_and(looks_like_nested_token_object)
    {
        return None;
    }
    Some(value_field)
}

pub(super) fn looks_like_nested_token_object(obj: &serde_json::Map<String, Value>) -> bool {
    obj.contains_key("value")
        || obj.contains_key("$value")
        || obj.contains_key("type")
        || obj.contains_key("$type")
}

#[derive(Debug, Clone)]
pub(super) struct ShadowLayer {
    color: (u8, u8, u8, u8),
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    spread_radius: f32,
    inset: bool,
}

impl ShadowLayer {
    pub(super) const fn is_inset(&self) -> bool {
        self.inset
    }
}

pub(super) fn parse_shadow_layers(value: &str) -> Vec<ShadowLayer> {
    if value.trim() == "none" {
        return Vec::new();
    }
    split_css_layers(value)
        .into_iter()
        .filter_map(|layer| parse_shadow_layer(layer.trim()).ok())
        .collect()
}

pub(super) fn parse_shadow_layer(layer: &str) -> Result<ShadowLayer> {
    let inset = layer.contains("inset");
    let color_start = layer
        .find("rgba(")
        .or_else(|| layer.find("rgb("))
        .ok_or_else(|| anyhow!("shadow has no rgb/rgba color: {layer}"))?;
    let color_end = layer[color_start..]
        .find(')')
        .ok_or_else(|| anyhow!("unterminated rgb/rgba in shadow: {layer}"))?
        + color_start;
    let color_raw = &layer[color_start..=color_end];
    let color = parse_rgb_color(color_raw)?;
    let nums = layer[..color_start]
        .replace("inset", "")
        .split_whitespace()
        .filter_map(|part| parse_dimension(part).ok())
        .collect::<Vec<_>>();
    Ok(ShadowLayer {
        color,
        offset_x: *nums.get(0).unwrap_or(&0.0),
        offset_y: *nums.get(1).unwrap_or(&0.0),
        blur_radius: *nums.get(2).unwrap_or(&0.0),
        spread_radius: *nums.get(3).unwrap_or(&0.0),
        inset,
    })
}

pub(super) fn split_css_layers(value: &str) -> Vec<&str> {
    let mut layers = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                layers.push(&value[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    layers.push(&value[start..]);
    layers
}

pub(super) fn parse_hex_color(value: &str) -> Result<(u8, u8, u8, u8)> {
    let hex = value
        .trim()
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("not a hex color: {value}"))?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)?;
            Ok((r, g, b, 255))
        }
        6 | 8 => {
            let r = u8::from_str_radix(&hex[0..2], 16)?;
            let g = u8::from_str_radix(&hex[2..4], 16)?;
            let b = u8::from_str_radix(&hex[4..6], 16)?;
            let a = if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16)?
            } else {
                255
            };
            Ok((r, g, b, a))
        }
        _ => bail!("invalid hex color length: {value}"),
    }
}

pub(super) fn parse_color(value: &str) -> Result<(u8, u8, u8, u8)> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return Ok((0, 0, 0, 0));
    }
    if value.starts_with('#') {
        return parse_hex_color(value);
    }
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        return parse_rgb_color(value);
    }
    bail!("unsupported color value: {value}")
}

pub(super) fn parse_rgb_color(value: &str) -> Result<(u8, u8, u8, u8)> {
    let inner = value
        .trim()
        .trim_start_matches("rgba(")
        .trim_start_matches("rgb(")
        .trim_end_matches(')');
    let parts = inner.split(',').map(str::trim).collect::<Vec<_>>();
    let r = parts
        .get(0)
        .ok_or_else(|| anyhow!("missing red in {value}"))?
        .parse::<u8>()?;
    let g = parts
        .get(1)
        .ok_or_else(|| anyhow!("missing green in {value}"))?
        .parse::<u8>()?;
    let b = parts
        .get(2)
        .ok_or_else(|| anyhow!("missing blue in {value}"))?
        .parse::<u8>()?;
    let a = if let Some(alpha) = parts.get(3) {
        let alpha = alpha.parse::<f32>()?.clamp(0.0, 1.0);
        (alpha * 255.0).round() as u8
    } else {
        255
    };
    Ok((r, g, b, a))
}

pub(super) fn parse_border(value: &str) -> Option<(f32, &str)> {
    let mut width = None;
    let mut color_start = None;
    for part in value.split_whitespace() {
        if width.is_none() {
            if let Ok(px) = parse_dimension(part) {
                width = Some(px);
                continue;
            }
        }
        if part.starts_with('#') || part.starts_with("rgb") || part == "transparent" {
            color_start = value.find(part);
            break;
        }
    }
    match (width, color_start) {
        (Some(width), Some(start)) => Some((width, value[start..].trim())),
        _ => None,
    }
}

pub(super) fn parse_padding(value: &str) -> Result<[f32; 4]> {
    let parts = value
        .split_whitespace()
        .map(parse_dimension)
        .collect::<Result<Vec<_>>>()?;
    let (top, right, bottom, left) = match parts.as_slice() {
        [all] => (*all, *all, *all, *all),
        [vertical, horizontal] => (*vertical, *horizontal, *vertical, *horizontal),
        [top, horizontal, bottom] => (*top, *horizontal, *bottom, *horizontal),
        [top, right, bottom, left, ..] => (*top, *right, *bottom, *left),
        _ => (0.0, 0.0, 0.0, 0.0),
    };
    Ok([left, right, top, bottom])
}

pub(super) fn gradient_stops(value: &str) -> Vec<&str> {
    let inner = value
        .split_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .unwrap_or(value);
    split_css_layers(inner)
        .into_iter()
        .map(str::trim)
        .filter(|part| part.starts_with('#') || part.starts_with("rgb") || *part == "transparent")
        .map(|part| part.split_whitespace().next().unwrap_or(part))
        .collect()
}

pub(super) fn parse_dimension(value: &str) -> Result<f32> {
    let trimmed = value.trim().trim_matches('"');
    if let Some(px) = trimmed.strip_suffix("px") {
        return Ok(px.trim().parse()?);
    }
    if let Some(em) = trimmed.strip_suffix("em") {
        return Ok(em.trim().parse()?);
    }
    Ok(trimmed.parse()?)
}

pub(super) fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value.get(name)
}

pub(super) fn component_size_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "sm" => "Sm",
        "md" => "Md",
        "lg" => "Lg",
        "xl" => "Xl",
        _ => return None,
    };
    Some(format!("{krate}::ComponentSize::{variant}"))
}

pub(super) fn button_hierarchy_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "primary" => "Primary",
        "secondary_color" => "SecondaryColor",
        "secondary_gray" => "SecondaryGray",
        "tertiary_color" => "TertiaryColor",
        "tertiary_gray" => "TertiaryGray",
        "link_color" => "LinkColor",
        "link_gray" => "LinkGray",
        "destructive" => "Destructive",
        _ => return None,
    };
    Some(format!("{krate}::ButtonHierarchy::{variant}"))
}

pub(super) fn badge_tone_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "brand" => "Brand",
        "gray" => "Gray",
        "success" => "Success",
        "warning" => "Warning",
        "error" => "Error",
        "blue" => "Blue",
        "orange" => "Orange",
        _ => return None,
    };
    Some(format!("{krate}::BadgeTone::{variant}"))
}

pub(super) fn card_pattern_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "plain" => "Plain",
        "raised" => "Raised",
        "tinted" => "Tinted",
        "elevated" => "Elevated",
        _ => return None,
    };
    Some(format!("{krate}::CardPattern::{variant}"))
}

pub(super) fn feature_icon_tone_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "brand" => "Brand",
        "gray" => "Gray",
        "blue" => "Blue",
        "orange" => "Orange",
        _ => return None,
    };
    Some(format!("{krate}::FeatureIconTone::{variant}"))
}

pub(super) fn parse_duration_ms(value: &str) -> Result<u64> {
    let trimmed = value.trim();
    if let Some(ms) = trimmed.strip_suffix("ms") {
        return Ok(ms.trim().parse()?);
    }
    if let Some(s) = trimmed.strip_suffix('s') {
        return Ok((s.trim().parse::<f32>()? * 1000.0).round() as u64);
    }
    Ok(trimmed.parse()?)
}

pub(super) fn color_expr(krate: &str, r: u8, g: u8, b: u8, a: u8) -> String {
    format!("{krate}::Color {{ r: {r}, g: {g}, b: {b}, a: {a} }}")
}

pub(super) fn box_shadow_expr(krate: &str, layer: &ShadowLayer) -> String {
    format!(
        "{krate}::BoxShadow {{ color: {}, offset: ({}, {}), blur_radius: {}, spread_radius: {}, inset: {} }}",
        color_expr(
            krate,
            layer.color.0,
            layer.color.1,
            layer.color.2,
            layer.color.3
        ),
        f32_lit(layer.offset_x),
        f32_lit(layer.offset_y),
        f32_lit(layer.blur_radius),
        f32_lit(layer.spread_radius),
        layer.inset
    )
}

pub(super) fn shadow_layer_expr(krate: &str, layer: &ShadowLayer) -> String {
    format!(
        "{krate}::ShadowLayer {{ color: {}, offset: ({}, {}), blur_radius: {}, spread_radius: {}, inset: {} }}",
        color_expr(krate, layer.color.0, layer.color.1, layer.color.2, layer.color.3),
        f32_lit(layer.offset_x),
        f32_lit(layer.offset_y),
        f32_lit(layer.blur_radius),
        f32_lit(layer.spread_radius),
        layer.inset
    )
}

pub(super) fn easing_expr(krate: &str, value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("cubic-bezier(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let nums = inner
            .split(',')
            .filter_map(|n| n.trim().parse::<f32>().ok())
            .collect::<Vec<_>>();
        if nums.len() == 4 {
            return format!(
                "{krate}::EasingCurve::CubicBezier({}, {}, {}, {})",
                f32_lit(nums[0]),
                f32_lit(nums[1]),
                f32_lit(nums[2]),
                f32_lit(nums[3])
            );
        }
    }
    match value {
        "linear" => format!("{krate}::EasingCurve::Linear"),
        "ease" => format!("{krate}::EasingCurve::Ease"),
        _ => format!("{krate}::EasingCurve::Named({})", rust_string(value)),
    }
}

pub(super) fn asset_expr(krate: &str, item: &Value) -> String {
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| item.get("family").and_then(Value::as_str))
        .unwrap_or("");
    let path = item.get("path").and_then(Value::as_str).unwrap_or("");
    let format = item.get("format").and_then(Value::as_str).unwrap_or("");
    format!(
        "{krate}::DesignAsset {{ id: {}, path: {}, format: {} }}",
        rust_string(id),
        rust_string(path),
        rust_string(format)
    )
}

pub(super) fn font_asset_expr(krate: &str, item: &Value) -> Result<String> {
    let family = required_string(item, "family", "font asset")?;
    let path = required_string(item, "path", "font asset")?;
    let format = item
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("truetype");
    let weight = font_weight(item)?;
    let style = font_style_expr(krate, item)?;
    let axes = font_axes(item)?
        .into_iter()
        .map(|(tag, value)| {
            format!(
                "{krate}::FontVariationAxis {{ tag: {:?}, value: {} }}",
                tag.as_bytes(),
                f32_lit(value)
            )
        })
        .collect::<Vec<_>>();
    Ok(format!(
        "{krate}::DesignFontAsset {{ family: {}, weight: {weight}, style: {style}, path: {}, format: {}, axes: vec![{}] }}",
        rust_string(family),
        rust_string(path),
        rust_string(format),
        axes.join(",")
    ))
}

pub(super) fn packaged_font_expr(krate: &str, root: &Path, item: &Value) -> Result<String> {
    let family = required_string(item, "family", "font asset")?;
    let weight = font_weight(item)?;
    let style = font_style_expr(krate, item)?;
    let format = item
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("truetype");
    let path = resolve_font_path(root, item)?;
    let axes = font_axes(item)?
        .into_iter()
        .map(|(tag, value)| {
            format!(
                "{krate}::FontVariationAxis {{ tag: {:?}, value: {} }}",
                tag.as_bytes(),
                f32_lit(value)
            )
        })
        .collect::<Vec<_>>();
    Ok(format!(
        "{krate}::PackagedFont {{ family: {:?}, weight: {weight}, style: {style}, format: {:?}, data: include_bytes!({:?}), axes: &[{}] }}",
        family,
        format,
        path.display().to_string(),
        axes.join(",")
    ))
}

pub(super) fn resolve_font_path(root: &Path, item: &Value) -> Result<PathBuf> {
    let relative = required_string(item, "path", "font asset")?;
    let path = root.join(relative);
    path.canonicalize()
        .with_context(|| format!("font asset does not exist: {}", path.display()))
}

pub(super) fn font_weight(item: &Value) -> Result<u16> {
    let weight = item.get("weight").and_then(Value::as_u64).unwrap_or(400);
    u16::try_from(weight)
        .ok()
        .filter(|weight| (1..=1000).contains(weight))
        .ok_or_else(|| anyhow!("font weight must be in the range 1..=1000, got {weight}"))
}

pub(super) fn font_style_expr(krate: &str, item: &Value) -> Result<String> {
    let style = item
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or("normal");
    let variant = match style {
        "normal" => "Normal",
        "italic" => "Italic",
        "oblique" => "Oblique",
        other => bail!("unsupported font style '{other}'"),
    };
    Ok(format!("{krate}::PackagedFontStyle::{variant}"))
}

pub(super) fn font_axes(item: &Value) -> Result<Vec<(String, f32)>> {
    let Some(axes) = item.get("axes") else {
        return Ok(Vec::new());
    };
    let mut parsed = Vec::new();
    match axes {
        Value::Object(values) => {
            for (tag, value) in values {
                validate_axis_tag(tag)?;
                let value = value
                    .as_f64()
                    .ok_or_else(|| anyhow!("font axis '{tag}' must be numeric"))?;
                parsed.push((tag.clone(), value as f32));
            }
        }
        Value::Array(values) => {
            for axis in values {
                let tag = required_string(axis, "tag", "font axis")?;
                validate_axis_tag(tag)?;
                let value = axis
                    .get("value")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| anyhow!("font axis '{tag}' must have a numeric value"))?;
                parsed.push((tag.to_string(), value as f32));
            }
        }
        _ => bail!("font axes must be an object or array"),
    }
    Ok(parsed)
}

pub(super) fn validate_axis_tag(tag: &str) -> Result<()> {
    if tag.len() != 4 || !tag.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        bail!("font variation axis tags must contain four printable ASCII bytes: '{tag}'");
    }
    Ok(())
}

pub(super) fn required_string<'a>(item: &'a Value, field: &str, kind: &str) -> Result<&'a str> {
    item.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("{kind} requires non-empty '{field}'"))
}

pub(super) fn rust_string(value: &str) -> String {
    format!("{:?}.to_string()", value)
}

pub(super) fn f32_lit(value: f32) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        let mut out = value.to_string();
        if !out.contains('.') && !out.contains('e') && !out.contains('E') {
            out.push_str(".0");
        }
        out
    }
}
