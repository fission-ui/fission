use anyhow::{Context, Result};
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const PLATFORMS: &[(&str, &str)] = &[
    ("Android", "android"),
    ("iOS", "ios"),
    ("Linux", "linux"),
    ("macOS", "macos"),
    ("Windows", "windows"),
    ("Web", "web"),
    ("Terminal", "terminal"),
    ("Static site", "static-site"),
    ("SSR", "ssr"),
];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RegistryCrate {
    pub name: String,
    pub version: String,
    pub description: String,
    pub downloads: u64,
    pub updated_at: String,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub license: Option<String>,
    pub platforms: Vec<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub versions: Vec<String>,
    pub readme_markdown: String,
}

impl RegistryCrate {
    pub fn is_prerelease(&self) -> bool {
        self.version.contains('-')
    }
}

pub fn load_registry(path: &Path) -> Result<Vec<RegistryCrate>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let connection = Connection::open(path)
        .with_context(|| format!("open crate registry at {}", path.display()))?;
    let mut statement = connection.prepare(
        "SELECT name, version, description, downloads, updated_at, repository, documentation, \
         license, platforms, keywords, categories, versions, readme_markdown \
         FROM crates ORDER BY updated_at DESC, name ASC",
    )?;
    let crates = statement
        .query_map([], row_to_crate)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(crates)
}

fn row_to_crate(row: &Row<'_>) -> rusqlite::Result<RegistryCrate> {
    Ok(RegistryCrate {
        name: row.get(0)?,
        version: row.get(1)?,
        description: row.get(2)?,
        downloads: row.get::<_, i64>(3)?.max(0) as u64,
        updated_at: row.get(4)?,
        repository: row.get(5)?,
        documentation: row.get(6)?,
        license: row.get(7)?,
        platforms: json_column(row, 8),
        keywords: json_column(row, 9),
        categories: json_column(row, 10),
        versions: json_column(row, 11),
        readme_markdown: row.get(12)?,
    })
}

fn json_column(row: &Row<'_>, index: usize) -> Vec<String> {
    row.get::<_, String>(index)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}
