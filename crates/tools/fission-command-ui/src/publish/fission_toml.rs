use super::{PublishBoard, UiCheck};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use toml_edit::{value as toml_value, Array, DocumentMut, Item, Table, Value};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FissionTomlEditorState {
    pub field_path: String,
    pub value: String,
    pub selected_preset: usize,
    pub status_message: String,
}

impl FissionTomlEditorState {
    pub(super) fn load(project_dir: &Path, focus_field: Option<String>) -> Self {
        let field_path = focus_field
            .filter(|field| !field.trim().is_empty())
            .unwrap_or_else(|| "app.homepage".to_string());
        let value = read_fission_toml_field(project_dir, &field_path).unwrap_or_default();
        let selected_preset = field_specs()
            .iter()
            .position(|spec| spec.path == field_path)
            .unwrap_or(0);
        Self {
            field_path,
            value,
            selected_preset,
            status_message: "Choose a field, enter the value, then apply.".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfigValueKind {
    String,
    Integer,
    Bool,
    StringList,
    TomlLiteral,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ConfigFieldSpec {
    pub path: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub placeholder: &'static str,
    pub kind: ConfigValueKind,
}

pub(super) fn field_specs() -> &'static [ConfigFieldSpec] {
    FIELD_SPECS
}

pub(super) fn config_field_for_check(check: &UiCheck, board: PublishBoard) -> Option<String> {
    let id = check.id.as_str();
    if let Some(field) = id.strip_prefix("release_config.app.") {
        return Some(match field {
            "id" => "app.app_id".to_string(),
            other => format!("app.{other}"),
        });
    }
    match id {
        "release_config.active_release" => Some("release.active_release".to_string()),
        "release_config.metadata_root" => Some("release.metadata_root".to_string()),
        "release_config.content_output_dir" => Some("release.content_output_dir".to_string()),
        "release_config.default_locales" => Some("release.default_locales".to_string()),
        "release_content.screenshots_configured" => Some("release.screenshots.raw_dir".to_string()),
        "release_content.screenshots.raw_dir_exists" => {
            Some("release.screenshots.raw_dir".to_string())
        }
        "release_content.screenshots.rendered_dir_exists" => {
            Some("release.screenshots.rendered_dir".to_string())
        }
        "release_content.play_store.feature_graphic" => {
            Some("release.assets.play_store.feature_graphic".to_string())
        }
        "release_content.play_store.screenshot_sets_dir"
        | "release_content.play_store.required_assets" => {
            Some("release.assets.play_store.screenshot_sets_dir".to_string())
        }
        "release_content.play_store.preview_video_dir" => {
            Some("release.assets.play_store.preview_video_dir".to_string())
        }
        "release_content.app_store.screenshot_sets_dir"
        | "release_content.app_store.required_assets" => {
            Some("release.assets.app_store.screenshot_sets_dir".to_string())
        }
        "release_content.app_store.app_previews_dir" => {
            Some("release.assets.app_store.app_previews_dir".to_string())
        }
        "release_content.microsoft_store.screenshot_sets_dir"
        | "release_content.microsoft_store.required_assets" => {
            Some("release.assets.microsoft_store.screenshot_sets_dir".to_string())
        }
        "release_content.microsoft_store.trailers_dir" => {
            Some("release.assets.microsoft_store.trailers_dir".to_string())
        }
        "release_content.microsoft_store.logo_dir"
        | "release_content.microsoft_store.required_logos" => {
            Some("release.assets.microsoft_store.logo_dir".to_string())
        }
        _ if id.starts_with("release.package.android.") => Some(match id.rsplit('.').next() {
            Some("package_name_matches_app_id") => "package.android.package_name".to_string(),
            _ => "package.android.package_name".to_string(),
        }),
        _ if id.starts_with("release.package.ios.") => Some("package.ios.bundle_id".to_string()),
        _ if id.starts_with("release.package.windows.") => match board {
            PublishBoard::Windows => Some("package.windows.identity_name".to_string()),
            _ => None,
        },
        _ => config_field_from_remediation(check),
    }
}

fn config_field_from_remediation(check: &UiCheck) -> Option<String> {
    let mut text = check.summary.clone();
    if let Some(details) = &check.details {
        text.push(' ');
        text.push_str(details);
    }
    for remediation in &check.remediation {
        text.push(' ');
        text.push_str(remediation);
    }
    for token in text.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';' || ch == '`') {
        let candidate = token
            .trim_matches(|ch: char| matches!(ch, '.' | ':' | '[' | ']' | '(' | ')' | '\'' | '"'));
        if candidate.contains('.') && candidate.chars().all(valid_field_path_char) {
            if matches!(candidate, "fission.toml" | "release.env") {
                continue;
            }
            return Some(candidate.to_string());
        }
    }
    None
}

fn valid_field_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}

pub(super) fn read_fission_toml_field(project_dir: &Path, field: &str) -> Option<String> {
    let data = fs::read_to_string(project_dir.join("fission.toml")).ok()?;
    let doc = data.parse::<DocumentMut>().ok()?;
    let item = toml_item_at_path(&doc, field).ok().flatten()?;
    Some(item_to_input(item))
}

pub(super) fn apply_fission_toml_field(
    project_dir: &Path,
    field: &str,
    input: &str,
) -> Result<String> {
    validate_field_path(field)?;
    reject_secretish_field(field, input)?;
    let kind = field_kind(field);
    let item = value_for_input(kind, input)?;
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut doc = data
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    set_toml_item_at_path(&mut doc, field, item)?;
    fs::write(&path, doc.to_string())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(format!("Updated {field} in {}", path.display()))
}

pub(super) fn field_kind(field: &str) -> ConfigValueKind {
    field_specs()
        .iter()
        .find(|spec| spec.path == field)
        .map(|spec| spec.kind)
        .unwrap_or(ConfigValueKind::TomlLiteral)
}

fn toml_item_at_path<'a>(doc: &'a DocumentMut, field: &str) -> Result<Option<&'a Item>> {
    let parts = field_segments(field)?;
    let mut table = doc.as_table();
    for part in &parts[..parts.len() - 1] {
        let Some(item) = table.get(part.as_str()) else {
            return Ok(None);
        };
        let Some(next) = item.as_table() else {
            return Ok(None);
        };
        table = next;
    }
    Ok(table.get(parts[parts.len() - 1].as_str()))
}

fn set_toml_item_at_path(doc: &mut DocumentMut, field: &str, item: Item) -> Result<()> {
    let parts = field_segments(field)?;
    let mut table = doc.as_table_mut();
    for part in &parts[..parts.len() - 1] {
        let entry = table
            .entry(part.as_str())
            .or_insert_with(|| Item::Table(Table::new()));
        if !entry.is_table() {
            *entry = Item::Table(Table::new());
        }
        table = entry
            .as_table_mut()
            .context("failed to create TOML table")?;
    }
    table[parts[parts.len() - 1].as_str()] = item;
    Ok(())
}

fn field_segments(field: &str) -> Result<Vec<String>> {
    validate_field_path(field)?;
    Ok(field.split('.').map(str::to_string).collect())
}

fn validate_field_path(field: &str) -> Result<()> {
    let field = field.trim();
    if field.is_empty() || field.starts_with('.') || field.ends_with('.') || field.contains("..") {
        bail!("field path must be a dotted fission.toml path, for example app.homepage");
    }
    if !field.chars().all(valid_field_path_char) {
        bail!("field path can only contain ASCII letters, numbers, _, -, and .");
    }
    Ok(())
}

fn value_for_input(kind: ConfigValueKind, input: &str) -> Result<Item> {
    let trimmed = input.trim();
    match kind {
        ConfigValueKind::String => Ok(toml_value(trimmed.to_string())),
        ConfigValueKind::Integer => Ok(toml_value(
            trimmed.parse::<i64>().context("expected an integer")?,
        )),
        ConfigValueKind::Bool => match trimmed {
            "true" => Ok(toml_value(true)),
            "false" => Ok(toml_value(false)),
            _ => bail!("expected true or false"),
        },
        ConfigValueKind::StringList => {
            let mut array = Array::default();
            for item in trimmed
                .trim_matches(|ch| ch == '[' || ch == ']')
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
            {
                array.push(item.trim_matches('"').to_string());
            }
            Ok(Item::Value(Value::Array(array)))
        }
        ConfigValueKind::TomlLiteral => parse_toml_literal(trimmed),
    }
}

fn parse_toml_literal(input: &str) -> Result<Item> {
    if input.is_empty() {
        return Ok(toml_value(""));
    }
    let literal = if looks_like_toml_literal(input) {
        input.to_string()
    } else {
        format!("{input:?}")
    };
    let doc = format!("value = {literal}")
        .parse::<DocumentMut>()
        .context("value is not valid TOML")?;
    Ok(doc
        .get("value")
        .cloned()
        .context("failed to parse TOML value")?)
}

fn looks_like_toml_literal(input: &str) -> bool {
    input.starts_with('"')
        || input.starts_with('[')
        || input.starts_with('{')
        || matches!(input, "true" | "false")
        || input.parse::<i64>().is_ok()
        || input.parse::<f64>().is_ok()
}

fn item_to_input(item: &Item) -> String {
    item.as_value()
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| item.to_string().trim().to_string())
}

fn reject_secretish_field(field: &str, value: &str) -> Result<()> {
    let lower_field = field.to_ascii_lowercase();
    let lower_value = value.to_ascii_lowercase();
    let secret_field = [
        "password",
        "secret",
        "token",
        "private_key",
        "keystore",
        "certificate",
        "client_secret",
        "api_key_path",
    ]
    .iter()
    .any(|needle| lower_field.contains(needle));
    let secret_value = ["-----begin", ".jks", ".p12", ".pfx", ".p8"]
        .iter()
        .any(|needle| lower_value.contains(needle));
    if secret_field || secret_value {
        bail!("Fission will not write secrets or machine-specific secret file paths to fission.toml. Use release.env, environment variables, or CI secrets instead.");
    }
    Ok(())
}

static FIELD_SPECS: &[ConfigFieldSpec] = &[
    ConfigFieldSpec {
        path: "app.name",
        label: "App name",
        group: "App",
        placeholder: "Cacydil",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "app.app_id",
        label: "App/package id",
        group: "App",
        placeholder: "pl.cacydil.store",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "app.version",
        label: "Version",
        group: "App",
        placeholder: "1.0.0",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "app.build",
        label: "Build number",
        group: "App",
        placeholder: "2",
        kind: ConfigValueKind::Integer,
    },
    ConfigFieldSpec {
        path: "app.publisher",
        label: "Publisher",
        group: "App",
        placeholder: "Cacydil",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "app.homepage",
        label: "Homepage URL",
        group: "App",
        placeholder: "https://example.com",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "app.support_url",
        label: "Support URL",
        group: "App",
        placeholder: "https://example.com/support",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "app.privacy_url",
        label: "Privacy URL",
        group: "App",
        placeholder: "https://example.com/privacy",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "app.license",
        label: "License",
        group: "App",
        placeholder: "Proprietary",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "targets",
        label: "Targets",
        group: "Project",
        placeholder: "android, ios, windows",
        kind: ConfigValueKind::StringList,
    },
    ConfigFieldSpec {
        path: "package.android.package_name",
        label: "Android package",
        group: "Android",
        placeholder: "pl.cacydil.store",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "package.android.version_name",
        label: "Android version name",
        group: "Android",
        placeholder: "1.0.0",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "package.android.version_code",
        label: "Android version code",
        group: "Android",
        placeholder: "2",
        kind: ConfigValueKind::Integer,
    },
    ConfigFieldSpec {
        path: "package.android.min_sdk",
        label: "Android min SDK",
        group: "Android",
        placeholder: "23",
        kind: ConfigValueKind::Integer,
    },
    ConfigFieldSpec {
        path: "package.android.target_sdk",
        label: "Android target SDK",
        group: "Android",
        placeholder: "35",
        kind: ConfigValueKind::Integer,
    },
    ConfigFieldSpec {
        path: "distribution.play_store.package_name",
        label: "Play package",
        group: "Play Store",
        placeholder: "pl.cacydil.store",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "distribution.play_store.default_track",
        label: "Play default track",
        group: "Play Store",
        placeholder: "internal",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "package.ios.bundle_id",
        label: "iOS bundle id",
        group: "iOS",
        placeholder: "pl.cacydil.store",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "package.ios.display_name",
        label: "iOS display name",
        group: "iOS",
        placeholder: "Cacydil",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "distribution.app_store.bundle_id",
        label: "App Store bundle id",
        group: "App Store",
        placeholder: "pl.cacydil.store",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "distribution.app_store.sku",
        label: "App Store SKU",
        group: "App Store",
        placeholder: "cacydil-ios",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "package.windows.identity_name",
        label: "Windows identity",
        group: "Windows",
        placeholder: "Cacydil.Store",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "package.windows.publisher",
        label: "Windows publisher",
        group: "Windows",
        placeholder: "CN=Publisher",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "distribution.microsoft_store.product_id",
        label: "Partner product id",
        group: "Microsoft Store",
        placeholder: "9N...",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "distribution.s3.bucket",
        label: "S3 bucket",
        group: "S3",
        placeholder: "my-release-bucket",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "distribution.s3.prefix",
        label: "S3 prefix",
        group: "S3",
        placeholder: "releases/production/",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "distribution.s3.overwrite",
        label: "S3 overwrite",
        group: "S3",
        placeholder: "false",
        kind: ConfigValueKind::Bool,
    },
    ConfigFieldSpec {
        path: "release.active_release",
        label: "Active release",
        group: "Release",
        placeholder: "1.0.0+2",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.metadata_root",
        label: "Metadata root",
        group: "Release",
        placeholder: "release-content/metadata",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.content_output_dir",
        label: "Content output",
        group: "Release",
        placeholder: "target/fission/release-content",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.default_locales",
        label: "Default locales",
        group: "Release",
        placeholder: "pl-PL, en-US",
        kind: ConfigValueKind::StringList,
    },
    ConfigFieldSpec {
        path: "release.screenshots.raw_dir",
        label: "Raw screenshots",
        group: "Release content",
        placeholder: "release-content/screenshots/raw",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.screenshots.rendered_dir",
        label: "Rendered screenshots",
        group: "Release content",
        placeholder: "release-content/screenshots/rendered",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.assets.play_store.feature_graphic",
        label: "Play feature graphic",
        group: "Release assets",
        placeholder: "release-content/play/feature-graphic.png",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.assets.play_store.screenshot_sets_dir",
        label: "Play screenshots",
        group: "Release assets",
        placeholder: "release-content/play/screenshots",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.assets.play_store.preview_video_dir",
        label: "Play videos",
        group: "Release assets",
        placeholder: "release-content/play/videos",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.assets.app_store.screenshot_sets_dir",
        label: "App Store screenshots",
        group: "Release assets",
        placeholder: "release-content/app-store/screenshots",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.assets.app_store.app_previews_dir",
        label: "App Store previews",
        group: "Release assets",
        placeholder: "release-content/app-store/previews",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.assets.microsoft_store.screenshot_sets_dir",
        label: "MS screenshots",
        group: "Release assets",
        placeholder: "release-content/microsoft/screenshots",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "release.assets.microsoft_store.logo_dir",
        label: "MS logos",
        group: "Release assets",
        placeholder: "release-content/microsoft/logos",
        kind: ConfigValueKind::String,
    },
    ConfigFieldSpec {
        path: "site.generate_sitemap",
        label: "Generate sitemap",
        group: "Static site",
        placeholder: "true",
        kind: ConfigValueKind::Bool,
    },
    ConfigFieldSpec {
        path: "site.generate_robots",
        label: "Generate robots.txt",
        group: "Static site",
        placeholder: "true",
        kind: ConfigValueKind::Bool,
    },
];
