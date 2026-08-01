#[path = "../registry.rs"]
#[allow(dead_code)]
mod registry;

use anyhow::{anyhow, Result};
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

const DISCOVERY_KEYWORD: &str = "fission-framework";

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

#[derive(Debug, Eq, PartialEq)]
enum IngestOutcome {
    Indexed(String),
    Skipped,
}

fn main() -> Result<()> {
    let database = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("documentation/data/fission-crates.sqlite3"));
    if let Some(parent) = database.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut connection = Connection::open(&database)?;
    migrate(&connection)?;
    let client = Client::builder()
        .user_agent("fission.rs crate indexer (https://fission.rs/crates)")
        .build()?;
    let mut indexed = BTreeSet::new();
    let mut refresh_complete = true;
    for candidate in discover_candidates(&client)? {
        match ingest_crate(&client, &connection, &candidate) {
            Ok(IngestOutcome::Indexed(name)) => {
                indexed.insert(name);
            }
            Ok(IngestOutcome::Skipped) => {}
            Err(error) => {
                refresh_complete = false;
                eprintln!("failed to refresh {}: {error:#}", candidate.id);
            }
        }
    }

    match prune_stale_records(&mut connection, &indexed, refresh_complete)? {
        Some(removed) => println!("removed {removed} stale crate(s)"),
        None => eprintln!("crate refresh incomplete; stale indexed records were retained"),
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

fn discover_candidates(client: &Client) -> Result<Vec<SearchCrate>> {
    let mut page = 1;
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    loop {
        let response = client
            .get("https://crates.io/api/v1/crates")
            .query(&[
                ("keyword", DISCOVERY_KEYWORD),
                ("sort", "recent-updates"),
                ("per_page", "100"),
                ("page", &page.to_string()),
            ])
            .send()?
            .error_for_status()?
            .json::<SearchResponse>()?;
        for candidate in response.crates {
            if seen.insert(candidate.id.clone()) {
                candidates.push(candidate);
            }
        }
        if response.meta.next_page.is_none() {
            break;
        }
        page += 1;
    }
    Ok(candidates)
}

fn ingest_crate(
    client: &Client,
    connection: &Connection,
    candidate: &SearchCrate,
) -> Result<IngestOutcome> {
    let response = client
        .get(format!("https://crates.io/api/v1/crates/{}", candidate.id))
        .send()?
        .error_for_status()?
        .json::<CrateResponse>()?;
    let Some(version) = response
        .versions
        .iter()
        .find(|version| version.num == response.crate_data.max_version && !version.yanked)
        .or_else(|| response.versions.iter().find(|version| !version.yanked))
    else {
        println!("skipped {}: no non-yanked release", candidate.id);
        return Ok(IngestOutcome::Skipped);
    };
    let packaged = download_package(client, &candidate.id, &version.num)?;

    if !OFFICIAL_CRATES.contains(&candidate.id.as_str())
        && !has_official_dependency(&packaged.manifest)
    {
        println!(
            "skipped {}: no direct dependency on an official Fission crate",
            candidate.id
        );
        return Ok(IngestOutcome::Skipped);
    }

    let keywords = response
        .keywords
        .into_iter()
        .map(|keyword| keyword.keyword)
        .collect::<Vec<_>>();
    let platforms = declared_platforms(&packaged.manifest, &keywords)?;
    let rendered_readme =
        comrak::markdown_to_html(&packaged.readme_markdown, &comrak::Options::default());
    let readme_markdown = html2md::parse_html(&ammonia::clean(&rendered_readme));
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
    Ok(IngestOutcome::Indexed(candidate.id.clone()))
}

fn prune_stale_records(
    connection: &mut Connection,
    indexed: &BTreeSet<String>,
    refresh_complete: bool,
) -> Result<Option<usize>> {
    if !refresh_complete {
        return Ok(None);
    }

    let transaction = connection.transaction()?;
    let existing = {
        let mut statement = transaction.prepare("SELECT name FROM crates")?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let mut removed = 0;
    for name in existing {
        if !indexed.contains(&name) {
            removed += transaction.execute("DELETE FROM crates WHERE name = ?1", params![name])?;
        }
    }
    transaction.commit()?;
    Ok(Some(removed))
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

fn declared_platforms(manifest: &toml::Value, keywords: &[String]) -> Result<Vec<String>> {
    let values = manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("fission"))
        .and_then(|fission| fission.get("platforms"))
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut platforms = BTreeSet::new();
    if values.is_empty() {
        for keyword in keywords {
            if let Some(platform) = platform_from_keyword(keyword) {
                platforms.insert(platform.to_string());
            }
        }
    } else {
        let allowed = PLATFORMS.iter().map(|(_, id)| *id).collect::<BTreeSet<_>>();
        for value in values {
            let platform = value.as_str().ok_or_else(|| {
                anyhow!("package.metadata.fission.platforms must contain strings")
            })?;
            let normalized = platform_from_keyword(platform).unwrap_or(platform);
            if !allowed.contains(normalized) {
                return Err(anyhow!("unknown Fission platform `{platform}`"));
            }
            platforms.insert(normalized.to_string());
        }
    }

    Ok(PLATFORMS
        .iter()
        .filter_map(|(_, id)| platforms.contains(*id).then(|| (*id).to_string()))
        .collect())
}

fn platform_from_keyword(keyword: &str) -> Option<&'static str> {
    match keyword.trim().to_ascii_lowercase().as_str() {
        "android" => Some("android"),
        "ios" => Some("ios"),
        "linux" => Some("linux"),
        "macos" => Some("macos"),
        "web" => Some("web"),
        "windows" => Some("windows"),
        "static-site" => Some("static-site"),
        "server-rendered-pages" | "ssr" => Some("ssr"),
        "terminal" => Some("terminal"),
        _ => None,
    }
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
        assert_eq!(declared_platforms(&manifest, &[]).unwrap(), ["web", "ssr"]);
    }

    #[test]
    fn falls_back_to_normalized_platform_keywords() {
        let manifest: toml::Value = toml::from_str(
            r#"[package]
name = "demo"
"#,
        )
        .unwrap();
        let keywords = [
            "terminal",
            "ssr",
            "server-rendered-pages",
            "ANDROID",
            "ios",
            "linux",
            "macos",
            "web",
            "windows",
            "static-site",
            "not-a-platform",
        ]
        .map(str::to_string);

        assert_eq!(
            declared_platforms(&manifest, &keywords).unwrap(),
            [
                "android",
                "ios",
                "linux",
                "macos",
                "windows",
                "web",
                "terminal",
                "static-site",
                "ssr",
            ]
        );
    }

    #[test]
    fn declared_platforms_take_precedence_over_keywords() {
        let manifest: toml::Value = toml::from_str(
            r#"[package]
name = "demo"
[package.metadata.fission]
platforms = ["web"]
"#,
        )
        .unwrap();

        assert_eq!(
            declared_platforms(&manifest, &["android".to_string()]).unwrap(),
            ["web"]
        );
    }

    #[test]
    fn rejects_unknown_declared_platforms() {
        let manifest: toml::Value = toml::from_str(
            r#"[package]
name = "demo"
[package.metadata.fission]
platforms = ["browser"]
"#,
        )
        .unwrap();

        assert!(declared_platforms(&manifest, &[]).is_err());
    }

    #[test]
    fn normalizes_server_rendered_metadata_alias() {
        let manifest: toml::Value = toml::from_str(
            r#"[package]
name = "demo"
[package.metadata.fission]
platforms = ["server-rendered-pages"]
"#,
        )
        .unwrap();

        assert_eq!(declared_platforms(&manifest, &[]).unwrap(), ["ssr"]);
    }

    #[test]
    fn prunes_only_records_missing_from_a_complete_refresh() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        for name in ["keep", "stale"] {
            connection
                .execute(
                    "INSERT INTO crates (
                        name, version, description, downloads, updated_at, repository,
                        documentation, license, platforms, keywords, categories, versions,
                        readme_markdown
                     ) VALUES (?1, '1.0.0', '', 0, '', NULL, NULL, NULL, '[]', '[]', '[]', '[]', '')",
                    params![name],
                )
                .unwrap();
        }
        let indexed = ["keep".to_string()].into_iter().collect();

        assert_eq!(
            prune_stale_records(&mut connection, &indexed, true).unwrap(),
            Some(1)
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM crates WHERE name = 'keep'",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM crates WHERE name = 'stale'",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn retains_stale_records_after_an_incomplete_refresh() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO crates (
                    name, version, description, downloads, updated_at, repository,
                    documentation, license, platforms, keywords, categories, versions,
                    readme_markdown
                 ) VALUES ('stale', '1.0.0', '', 0, '', NULL, NULL, NULL, '[]', '[]', '[]', '[]', '')",
                [],
            )
            .unwrap();

        assert_eq!(
            prune_stale_records(&mut connection, &BTreeSet::new(), false).unwrap(),
            None
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM crates", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
