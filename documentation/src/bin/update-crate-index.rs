#[path = "../registry.rs"]
#[allow(dead_code)]
mod registry;

use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Utc};
use flate2::read::GzDecoder;
use registry::PLATFORMS;
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use tar::Archive;

const OFFICIAL_CRATES: &[&str] = &[
    "fission",
    "fission-3d",
    "fission-charts",
    "fission-core",
    "fission-design-system-codegen",
    "fission-diagnostics",
    "fission-i18n",
    "fission-icons",
    "fission-ir",
    "fission-layout",
    "fission-macros",
    "fission-render",
    "fission-render-vello",
    "fission-semantics",
    "fission-shell",
    "fission-shell-desktop",
    "fission-shell-mobile",
    "fission-shell-server",
    "fission-shell-site",
    "fission-shell-terminal",
    "fission-shell-web",
    "fission-shell-winit",
    "fission-test",
    "fission-test-driver",
    "fission-text-engine",
    "fission-theme",
    "fission-widgets",
];

#[derive(Debug, Deserialize)]
struct SearchResponse {
    crates: Vec<SearchCrate>,
    meta: SearchMeta,
}

#[derive(Debug, Deserialize)]
struct SearchMeta {
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchCrate {
    id: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    crate_data: CrateData,
    versions: Vec<VersionData>,
    keywords: Vec<KeywordData>,
    categories: Vec<CategoryData>,
}

#[derive(Debug, Deserialize)]
struct CrateData {
    id: String,
    description: Option<String>,
    downloads: u64,
    updated_at: String,
    repository: Option<String>,
    documentation: Option<String>,
    max_version: String,
}

#[derive(Debug, Deserialize)]
struct VersionData {
    num: String,
    yanked: bool,
    license: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KeywordData {
    keyword: String,
}

#[derive(Debug, Deserialize)]
struct CategoryData {
    slug: String,
}

#[derive(Debug)]
struct PackagedCrate {
    manifest: toml::Value,
    readme_markdown: String,
}

fn main() -> Result<()> {
    let database = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("documentation/data/fission-crates.sqlite3"));
    if let Some(parent) = database.parent() {
        fs::create_dir_all(parent)?;
    }

    let database_exists = database.exists();
    let connection = Connection::open(&database)?;
    migrate(&connection)?;
    let client = Client::builder()
        .user_agent("fission.rs crate indexer (https://fission.rs/crates)")
        .build()?;
    let lookback_hours = env::var("FISSION_CRATE_INDEX_LOOKBACK_HOURS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(if database_exists { 48 } else { 24 * 365 * 20 });
    let cutoff = Utc::now() - Duration::hours(lookback_hours);

    for candidate in discover_candidates(&client, cutoff)? {
        if let Err(error) = ingest_crate(&client, &connection, &candidate) {
            eprintln!("skipping {}: {error:#}", candidate.id);
        }
    }

    connection.execute_batch("PRAGMA optimize;")?;
    println!("updated {}", database.display());
    Ok(())
}

fn migrate(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS crates (
            name TEXT PRIMARY KEY,
            version TEXT NOT NULL,
            description TEXT NOT NULL,
            downloads INTEGER NOT NULL,
            updated_at TEXT NOT NULL,
            repository TEXT,
            documentation TEXT,
            license TEXT,
            platforms TEXT NOT NULL,
            keywords TEXT NOT NULL,
            categories TEXT NOT NULL,
            versions TEXT NOT NULL,
            readme_markdown TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn discover_candidates(client: &Client, cutoff: chrono::DateTime<Utc>) -> Result<Vec<SearchCrate>> {
    let mut page = 1;
    let mut candidates = Vec::new();
    loop {
        let response = client
            .get("https://crates.io/api/v1/crates")
            .query(&[
                ("keyword", "fission"),
                ("sort", "recent-updates"),
                ("per_page", "100"),
                ("page", &page.to_string()),
            ])
            .send()?
            .error_for_status()?
            .json::<SearchResponse>()?;
        let mut reached_cutoff = false;
        for candidate in response.crates {
            let updated = chrono::DateTime::parse_from_rfc3339(&candidate.updated_at)
                .with_context(|| format!("parse update time for {}", candidate.id))?
                .with_timezone(&Utc);
            if updated < cutoff {
                reached_cutoff = true;
            } else {
                candidates.push(candidate);
            }
        }
        if reached_cutoff || response.meta.next_page.is_none() {
            break;
        }
        page += 1;
    }
    Ok(candidates)
}

fn ingest_crate(client: &Client, connection: &Connection, candidate: &SearchCrate) -> Result<()> {
    let response = client
        .get(format!("https://crates.io/api/v1/crates/{}", candidate.id))
        .send()?
        .error_for_status()?
        .json::<CrateResponse>()?;
    let version = response
        .versions
        .iter()
        .find(|version| version.num == response.crate_data.max_version && !version.yanked)
        .or_else(|| response.versions.iter().find(|version| !version.yanked))
        .ok_or_else(|| anyhow!("no non-yanked release"))?;
    let packaged = download_package(client, &candidate.id, &version.num)?;

    if !OFFICIAL_CRATES.contains(&candidate.id.as_str())
        && !has_official_dependency(&packaged.manifest)
    {
        return Err(anyhow!("no direct dependency on an official Fission crate"));
    }

    let platforms = declared_platforms(&packaged.manifest)?;
    let rendered_readme =
        comrak::markdown_to_html(&packaged.readme_markdown, &comrak::Options::default());
    let readme_markdown = html2md::parse_html(&ammonia::clean(&rendered_readme));
    let keywords = response
        .keywords
        .into_iter()
        .map(|keyword| keyword.keyword)
        .collect::<Vec<_>>();
    let categories = response
        .categories
        .into_iter()
        .map(|category| category.slug)
        .collect::<Vec<_>>();
    let versions = response
        .versions
        .iter()
        .filter(|version| !version.yanked)
        .map(|version| version.num.clone())
        .collect::<Vec<_>>();

    connection.execute(
        "INSERT INTO crates (
            name, version, description, downloads, updated_at, repository, documentation,
            license, platforms, keywords, categories, versions, readme_markdown
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(name) DO UPDATE SET
            version=excluded.version, description=excluded.description, downloads=excluded.downloads,
            updated_at=excluded.updated_at, repository=excluded.repository,
            documentation=excluded.documentation, license=excluded.license,
            platforms=excluded.platforms, keywords=excluded.keywords, categories=excluded.categories,
            versions=excluded.versions, readme_markdown=excluded.readme_markdown",
        params![
            response.crate_data.id,
            version.num,
            response.crate_data.description.unwrap_or_default(),
            response.crate_data.downloads as i64,
            response.crate_data.updated_at,
            response.crate_data.repository,
            response.crate_data.documentation,
            version.license,
            serde_json::to_string(&platforms)?,
            serde_json::to_string(&keywords)?,
            serde_json::to_string(&categories)?,
            serde_json::to_string(&versions)?,
            readme_markdown,
        ],
    )?;
    println!("indexed {} {}", candidate.id, version.num);
    Ok(())
}

fn download_package(client: &Client, name: &str, version: &str) -> Result<PackagedCrate> {
    let bytes = client
        .get(format!(
            "https://crates.io/api/v1/crates/{name}/{version}/download"
        ))
        .send()?
        .error_for_status()?
        .bytes()?;
    let mut archive = Archive::new(GzDecoder::new(bytes.as_ref()));
    let prefix = format!("{name}-{version}/");
    let mut manifest_source = None;
    let mut files = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        if path == format!("{prefix}Cargo.toml") {
            manifest_source = Some(String::from_utf8(contents.clone())?);
        }
        files.push((path, contents));
    }
    let manifest_source = manifest_source.ok_or_else(|| anyhow!("package has no Cargo.toml"))?;
    let manifest = toml::from_str::<toml::Value>(&manifest_source)?;
    let readme_path = manifest
        .get("package")
        .and_then(|package| package.get("readme"))
        .and_then(toml::Value::as_str)
        .unwrap_or("README.md");
    let packaged_readme = format!("{prefix}{readme_path}");
    let readme_markdown = files
        .into_iter()
        .find(|(path, _)| path == &packaged_readme)
        .and_then(|(_, bytes)| String::from_utf8(bytes).ok())
        .unwrap_or_default();
    Ok(PackagedCrate {
        manifest,
        readme_markdown,
    })
}

fn has_official_dependency(manifest: &toml::Value) -> bool {
    dependency_tables(manifest).any(|table| {
        table.iter().any(|(alias, value)| {
            let package = value
                .as_table()
                .and_then(|dependency| dependency.get("package"))
                .and_then(toml::Value::as_str)
                .unwrap_or(alias);
            OFFICIAL_CRATES.contains(&package)
        })
    })
}

fn dependency_tables(
    manifest: &toml::Value,
) -> impl Iterator<Item = &toml::map::Map<String, toml::Value>> {
    let mut tables = Vec::new();
    if let Some(root) = manifest.as_table() {
        for key in ["dependencies", "build-dependencies"] {
            if let Some(table) = root.get(key).and_then(toml::Value::as_table) {
                tables.push(table);
            }
        }
        if let Some(targets) = root.get("target").and_then(toml::Value::as_table) {
            for target in targets.values().filter_map(toml::Value::as_table) {
                for key in ["dependencies", "build-dependencies"] {
                    if let Some(table) = target.get(key).and_then(toml::Value::as_table) {
                        tables.push(table);
                    }
                }
            }
        }
    }
    tables.into_iter()
}

fn declared_platforms(manifest: &toml::Value) -> Result<Vec<String>> {
    let values = manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("fission"))
        .and_then(|fission| fission.get("platforms"))
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let allowed = PLATFORMS.iter().map(|(_, id)| *id).collect::<BTreeSet<_>>();
    let mut platforms = Vec::new();
    for value in values {
        let platform = value
            .as_str()
            .ok_or_else(|| anyhow!("package.metadata.fission.platforms must contain strings"))?;
        if !allowed.contains(platform) {
            return Err(anyhow!("unknown Fission platform `{platform}`"));
        }
        if !platforms.iter().any(|existing| existing == platform) {
            platforms.push(platform.to_string());
        }
    }
    Ok(platforms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_renamed_build_dependency() {
        let manifest: toml::Value = toml::from_str(
            r#"[package]
name = "demo"
[build-dependencies.framework]
package = "fission-design-system-codegen"
version = "1"
"#,
        )
        .unwrap();
        assert!(has_official_dependency(&manifest));
    }

    #[test]
    fn validates_declared_platforms() {
        let manifest: toml::Value = toml::from_str(
            r#"[package]
name = "demo"
[package.metadata.fission]
platforms = ["web", "ssr"]
"#,
        )
        .unwrap();
        assert_eq!(declared_platforms(&manifest).unwrap(), ["web", "ssr"]);
    }
}
