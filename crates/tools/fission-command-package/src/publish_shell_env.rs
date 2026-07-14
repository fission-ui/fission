use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub(super) fn upsert_env(path: &Path, key: &str, value: &str) -> Result<()> {
    let mut entries = read_env_entries(path)?;
    entries.retain(|(existing, _)| existing != key);
    entries.push((key.to_string(), value.to_string()));
    write_env_file(path, &entries)
}

fn read_env_entries(path: &Path) -> Result<Vec<(String, String)>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some((key, value)) = line.split_once('=') {
            entries.push((key.trim().to_string(), unquote_env_value(value.trim())));
        }
    }
    Ok(entries)
}

pub(super) fn write_env_file(path: &Path, entries: &[(String, String)]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        set_private_dir_permissions(parent)?;
    }
    let mut out = String::new();
    out.push_str("# Fission local release environment. Do not commit this file.\n");
    out.push_str("# Secrets here are loaded only for local publish flows.\n");
    for (key, value) in entries {
        out.push_str("export ");
        out.push_str(key);
        out.push('=');
        out.push_str(&quote_env_value(value));
        out.push('\n');
    }
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))?;
    set_private_file_permissions(path)?;
    Ok(())
}

fn quote_env_value(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
    )
}

pub(super) fn unquote_env_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        inner
            .replace("\\\"", "\"")
            .replace("\\$", "$")
            .replace("\\\\", "\\")
    } else {
        trimmed.to_string()
    }
}

#[cfg(unix)]
pub(super) fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
