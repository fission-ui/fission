//! Validation and Rust expressions for configured font assets.

use super::{f32_lit, rust_string};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

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

fn font_weight(item: &Value) -> Result<u16> {
    let weight = item.get("weight").and_then(Value::as_u64).unwrap_or(400);
    u16::try_from(weight)
        .ok()
        .filter(|weight| (1..=1000).contains(weight))
        .ok_or_else(|| anyhow!("font weight must be in the range 1..=1000, got {weight}"))
}

fn font_style_expr(krate: &str, item: &Value) -> Result<String> {
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

fn font_axes(item: &Value) -> Result<Vec<(String, f32)>> {
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

fn validate_axis_tag(tag: &str) -> Result<()> {
    if tag.len() != 4 || !tag.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        bail!("font variation axis tags must contain four printable ASCII bytes: '{tag}'");
    }
    Ok(())
}

fn required_string<'a>(item: &'a Value, field: &str, kind: &str) -> Result<&'a str> {
    item.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("{kind} requires non-empty '{field}'"))
}
