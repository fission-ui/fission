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
    ("Server-rendered", "ssr"),
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
    /// Machine-readable maturity declared by `package.metadata.fission.api-status`.
    pub api_status: String,
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
    let has_api_status = connection
        .prepare("PRAGMA table_info(crates)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|column| column == "api_status");
    let api_status = if has_api_status {
        "api_status"
    } else {
        "'stable' AS api_status"
    };
    let query = format!(
        "SELECT name, version, description, downloads, updated_at, repository, documentation, \
         license, {api_status}, platforms, keywords, categories, versions, readme_markdown \
         FROM crates ORDER BY updated_at DESC, name ASC"
    );
    let mut statement = connection.prepare(&query)?;
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
        api_status: row.get(8)?,
        platforms: json_column(row, 9),
        keywords: json_column(row, 10),
        categories: json_column(row, 11),
        versions: json_column(row, 12),
        readme_markdown: row.get(13)?,
    })
}

fn json_column(row: &Row<'_>, index: usize) -> Vec<String> {
    row.get::<_, String>(index)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}
