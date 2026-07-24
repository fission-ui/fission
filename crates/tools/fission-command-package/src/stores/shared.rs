use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AppStorePlatform {
    Ios,
    Macos,
}

impl AppStorePlatform {
    pub(super) fn api_value(self) -> &'static str {
        match self {
            Self::Ios => "IOS",
            Self::Macos => "MAC_OS",
        }
    }

    pub(super) fn altool_type(self) -> &'static str {
        match self {
            Self::Ios => "ios",
            Self::Macos => "macos",
        }
    }

    pub(super) fn artifact_extension(self) -> &'static str {
        match self {
            Self::Ios => "ipa",
            Self::Macos => "pkg",
        }
    }

    pub(super) fn target(self) -> &'static str {
        match self {
            Self::Ios => "ios",
            Self::Macos => "macos",
        }
    }
}

pub(super) fn app_store_platform(cfg: &AppStoreConfig) -> Result<AppStorePlatform> {
    parse_app_store_platform(cfg.platform.as_deref().unwrap_or("ios"))
}

pub(super) fn app_store_platform_for_manifest(
    cfg: &AppStoreConfig,
    manifest: &ArtifactManifest,
) -> Result<AppStorePlatform> {
    let inferred = match (manifest.target.as_str(), manifest.format.as_str()) {
        ("ios", "ipa") => AppStorePlatform::Ios,
        ("macos", "pkg") => AppStorePlatform::Macos,
        (target, format) => bail!(
            "App Store upload requires an iOS .ipa or macOS .pkg artifact, got target `{target}` format `{format}`"
        ),
    };
    if cfg.platform.is_none() {
        return Ok(inferred);
    }
    let configured = app_store_platform(cfg)?;
    if configured != inferred {
        bail!(
            "distribution.app_store.platform `{}` does not match artifact target `{}`",
            configured.target(),
            manifest.target
        );
    }
    Ok(configured)
}

fn parse_app_store_platform(value: &str) -> Result<AppStorePlatform> {
    match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "ios" => Ok(AppStorePlatform::Ios),
        "macos" | "mac-os" => Ok(AppStorePlatform::Macos),
        other => bail!("distribution.app_store.platform must be `ios` or `macos`, got `{other}`"),
    }
}

pub(super) fn create_play_edit(client: &Client, token: &str, package_name: &str) -> Result<String> {
    let url = format!("{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits");
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(&json!({}))
        .send()
        .context("failed to create Google Play edit")?;
    let value = json_response(response, "Google Play edit insert")?;
    value
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("Google Play edit insert response did not contain id")
}

pub(super) fn upload_play_artifact(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    path: &Path,
    artifact_kind: &str,
) -> Result<String> {
    let endpoint = match artifact_kind {
        "aab" => "bundles",
        "apk" => "apks",
        other => bail!("Google Play upload expected .aab or .apk, got .{other}"),
    };
    let url = format!(
        "{PLAY_UPLOAD_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/{endpoint}?uploadType=media"
    );
    let len = fs::metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len();
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let response = client
        .post(url)
        .bearer_auth(token)
        .header("Content-Type", "application/octet-stream")
        .body(Body::sized(file, len))
        .send()
        .with_context(|| format!("failed to upload {} to Google Play", path.display()))?;
    let value = json_response(response, "Google Play artifact upload")?;
    let version = value
        .get("versionCode")
        .and_then(|value| {
            value
                .as_i64()
                .map(|value| value.to_string())
                .or_else(|| value.as_str().map(str::to_string))
        })
        .context("Google Play upload response did not contain versionCode")?;
    Ok(version)
}

pub(super) fn play_internal_sharing_upload_url(
    package_name: &str,
    artifact_kind: &str,
) -> Result<String> {
    let endpoint = match artifact_kind {
        "aab" => "bundle",
        "apk" => "apk",
        other => bail!("Google Play internal sharing upload expected .aab or .apk, got .{other}"),
    };
    Ok(format!(
        "{PLAY_UPLOAD_API}/androidpublisher/v3/applications/internalappsharing/{package_name}/artifacts/{endpoint}?uploadType=media"
    ))
}

pub(super) fn upload_play_internal_sharing_artifact(
    client: &Client,
    token: &str,
    package_name: &str,
    path: &Path,
    artifact_kind: &str,
) -> Result<Value> {
    let url = play_internal_sharing_upload_url(package_name, artifact_kind)?;
    let len = fs::metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len();
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let response = client
        .post(url)
        .bearer_auth(token)
        .header("Content-Type", "application/octet-stream")
        .body(Body::sized(file, len))
        .send()
        .with_context(|| {
            format!(
                "failed to upload {} to Google Play internal app sharing",
                path.display()
            )
        })?;
    json_response(response, "Google Play internal app sharing upload")
}

pub(super) fn update_play_track(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    track: &str,
    release_status: &str,
    version_code: &str,
    release_notes: &[Value],
) -> Result<()> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/tracks/{track}"
    );
    let mut release = json!({
        "status": release_status,
        "versionCodes": [version_code]
    });
    if !release_notes.is_empty() {
        release["releaseNotes"] = Value::Array(release_notes.to_vec());
    }
    let body = json!({ "releases": [release] });
    let response = client
        .put(url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .context("failed to update Google Play track")?;
    json_response(response, "Google Play track update")?;
    Ok(())
}

pub(super) fn play_release_notes(project_dir: &Path, cli_locales: &[String]) -> Result<Vec<Value>> {
    let path = project_dir.join("fission.toml");
    let Ok(data) = fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let manifest: ReleaseNotesManifest =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    let Some(entry) = active_release_notes_entry(&manifest) else {
        return Ok(Vec::new());
    };
    let Some(notes_path) = entry
        .release_notes
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(Vec::new());
    };
    let locales = if cli_locales.is_empty() {
        if !entry.locales.is_empty() {
            entry.locales.clone()
        } else {
            manifest
                .release
                .as_ref()
                .map(|release| release.default_locales.clone())
                .unwrap_or_default()
        }
    } else {
        cli_locales.to_vec()
    };
    let path = project_dir.join(notes_path);
    if locales.is_empty() || !path.exists() {
        return Ok(Vec::new());
    }
    let mut notes = Vec::new();
    for locale in locales {
        if let Some(text) = read_release_note_text(&path, &locale)? {
            notes.push(json!({
                "language": locale,
                "text": text,
            }));
        }
    }
    Ok(notes)
}

pub(super) fn active_release_notes_entry(
    manifest: &ReleaseNotesManifest,
) -> Option<&ReleaseNotesEntry> {
    if let Some(active) = manifest
        .release
        .as_ref()
        .and_then(|release| release.active_release.as_deref())
    {
        if let Some(entry) = manifest
            .releases
            .iter()
            .find(|entry| entry.id.as_deref() == Some(active))
        {
            return Some(entry);
        }
    }
    manifest.releases.first()
}

pub(super) fn read_release_note_text(path: &Path, locale: &str) -> Result<Option<String>> {
    let file = if path.is_dir() {
        ["md", "txt", ""]
            .into_iter()
            .map(|ext| {
                if ext.is_empty() {
                    path.join(locale)
                } else {
                    path.join(format!("{locale}.{ext}"))
                }
            })
            .find(|candidate| candidate.exists())
    } else {
        Some(path.to_path_buf())
    };
    let Some(file) = file else {
        return Ok(None);
    };
    let text = fs::read_to_string(&file)
        .with_context(|| format!("failed to read release notes {}", file.display()))?;
    let text = text.trim().to_string();
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

pub(super) fn ensure_play_version_code_unused(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    selected_track: &str,
    version_code: &str,
) -> Result<()> {
    let mut tracks = vec![selected_track.to_string()];
    for track in ["internal", "closed", "open", "production"] {
        if !tracks.iter().any(|existing| existing == track) {
            tracks.push(track.to_string());
        }
    }

    let mut used_on = Vec::new();
    for track in tracks {
        let Some(value) = read_play_track(client, token, package_name, edit_id, &track)? else {
            continue;
        };
        if play_track_contains_version_code(&value, version_code) {
            used_on.push(track);
        }
    }
    for (endpoint, label) in [
        ("bundles", "app bundle inventory"),
        ("apks", "APK inventory"),
    ] {
        if let Some(value) =
            read_play_artifact_inventory(client, token, package_name, edit_id, endpoint, label)?
        {
            if play_artifact_inventory_contains_version_code(&value, endpoint, version_code) {
                used_on.push(label.to_string());
            }
        }
    }
    if !used_on.is_empty() {
        bail!(
            "Google Play version code {version_code} has already been used in Play state: {}. Increment [package.android].version_code or [app].build, rebuild the Android artifact, then publish again.",
            used_on.join(", ")
        );
    }
    Ok(())
}

pub(super) fn read_play_track(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    track: &str,
) -> Result<Option<Value>> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/tracks/{track}"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .with_context(|| format!("failed to read Google Play track {track}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    json_response(response, &format!("Google Play track {track} get")).map(Some)
}

pub(super) fn read_play_artifact_inventory(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    endpoint: &str,
    label: &str,
) -> Result<Option<Value>> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/{endpoint}"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .with_context(|| format!("failed to read Google Play {label}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    json_response(response, &format!("Google Play {label} get")).map(Some)
}

pub(super) fn play_track_contains_version_code(value: &Value, version_code: &str) -> bool {
    value
        .get("releases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|release| {
            release
                .get("versionCodes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .any(|value| {
            value
                .as_str()
                .map(|candidate| candidate == version_code)
                .unwrap_or_else(|| {
                    value
                        .as_i64()
                        .map(|candidate| candidate.to_string() == version_code)
                        .unwrap_or(false)
                })
        })
}

pub(super) fn play_artifact_inventory_contains_version_code(
    value: &Value,
    endpoint: &str,
    version_code: &str,
) -> bool {
    let key = match endpoint {
        "bundles" => "bundles",
        "apks" => "apks",
        _ => return false,
    };
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|artifact| {
            artifact
                .get("versionCode")
                .and_then(|value| {
                    value
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| value.as_i64().map(|value| value.to_string()))
                })
                .is_some_and(|candidate| candidate == version_code)
        })
}

pub(super) fn configured_android_version_code(project_dir: &Path) -> Result<Option<String>> {
    let path = project_dir.join("fission.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    let value: toml::Value = text
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value
        .get("package")
        .and_then(|package| package.get("android"))
        .and_then(|android| android.get("version_code"))
        .and_then(toml::Value::as_integer)
        .map(|value| value.to_string())
        .or_else(|| {
            value
                .get("app")
                .and_then(|app| app.get("build"))
                .and_then(toml::Value::as_integer)
                .map(|value| value.to_string())
        })
        .or_else(|| active_release_build(&value).map(|value| value.to_string())))
}

pub(super) fn configured_ios_build_number(project_dir: &Path) -> Result<Option<String>> {
    let path = project_dir.join("fission.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    let value: toml::Value = text
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value
        .get("package")
        .and_then(|package| package.get("ios"))
        .and_then(|ios| ios.get("build_number"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_integer().map(|value| value.to_string()))
        })
        .or_else(|| {
            value
                .get("app")
                .and_then(|app| app.get("build"))
                .and_then(toml::Value::as_integer)
                .map(|value| value.to_string())
        })
        .or_else(|| active_release_build(&value).map(|value| value.to_string())))
}

pub(super) fn configured_macos_build_number(project_dir: &Path) -> Result<Option<String>> {
    let path = project_dir.join("fission.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    let value: toml::Value = text
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value
        .get("package")
        .and_then(|package| package.get("macos"))
        .and_then(|macos| macos.get("build_number"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_integer().map(|value| value.to_string()))
        })
        .or_else(|| {
            value
                .get("app")
                .and_then(|app| app.get("build"))
                .and_then(toml::Value::as_integer)
                .map(|value| value.to_string())
        })
        .or_else(|| active_release_build(&value).map(|value| value.to_string())))
}

pub(super) fn active_release_build(value: &toml::Value) -> Option<i64> {
    let active = value
        .get("release")
        .and_then(|release| release.get("active_release"))
        .and_then(toml::Value::as_str)?;
    value
        .get("releases")
        .and_then(toml::Value::as_array)?
        .iter()
        .find(|release| {
            release
                .get("id")
                .and_then(toml::Value::as_str)
                .is_some_and(|id| id == active)
        })
        .and_then(|release| release.get("build"))
        .and_then(toml::Value::as_integer)
}

pub(super) fn validate_play_edit(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
) -> Result<()> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}:validate"
    );
    let response = client
        .post(url)
        .bearer_auth(token)
        .header(CONTENT_LENGTH, "0")
        .body(Vec::new())
        .send()
        .context("failed to validate Google Play edit")?;
    json_response(response, "Google Play edit validate")?;
    Ok(())
}

pub(super) fn commit_play_edit(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
) -> Result<()> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}:commit"
    );
    let response = client
        .post(url)
        .bearer_auth(token)
        .header(CONTENT_LENGTH, "0")
        .body(Vec::new())
        .send()
        .context("failed to commit Google Play edit")?;
    json_response(response, "Google Play edit commit")?;
    Ok(())
}

pub(super) fn google_play_access_token(cfg: &PlayStoreConfig, client: &Client) -> Result<String> {
    let access_token_env = play_access_token_env(cfg);
    if let Some(token) = env_value(&access_token_env) {
        return Ok(token);
    }
    let service_account_json_env = play_service_account_json_env(cfg);
    let service_account_json_base64_env = play_service_account_json_base64_env(cfg);
    let google_application_credentials_env = play_google_application_credentials_env(cfg);
    let service_account_base64 = base64_env_secret_file(
        &service_account_json_base64_env,
        "play-service-account.json",
    )?;
    let secret_source = env_value(&service_account_json_env)
        .or_else(|| {
            service_account_base64
                .as_ref()
                .map(|file| file.path.display().to_string())
        })
        .or_else(|| env_value(&google_application_credentials_env));
    let Some(source) = secret_source else {
        bail!(
            "Google Play credentials are missing; set {service_account_json_env}, {service_account_json_base64_env}, {access_token_env}, or {google_application_credentials_env}"
        )
    };
    if looks_like_bearer_token(&source) {
        return Ok(source);
    }
    let service_account = load_google_service_account(&source)?;
    service_account_access_token(&service_account, client)
}

pub(super) fn service_account_access_token(
    account: &GoogleServiceAccount,
    client: &Client,
) -> Result<String> {
    let token_uri = account.token_uri.as_deref().unwrap_or(GOOGLE_TOKEN_URI);
    let iat = now_unix_seconds();
    let claims = GoogleJwtClaims {
        iss: &account.client_email,
        scope: GOOGLE_PLAY_SCOPE,
        aud: token_uri,
        iat,
        exp: iat + 3600,
    };
    let key = EncodingKey::from_rsa_pem(account.private_key.as_bytes())
        .context("failed to parse Google service account private_key as RSA PEM")?;
    let jwt = encode(&Header::new(Algorithm::RS256), &claims, &key)
        .context("failed to sign Google service account JWT")?;
    let response = client
        .post(token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", jwt.as_str()),
        ])
        .send()
        .context("failed to exchange Google service account JWT")?;
    let token: OAuthTokenResponse = response
        .error_for_status()
        .context("Google OAuth token exchange failed")?
        .json()
        .context("failed to parse Google OAuth token response")?;
    let _ = (&token.token_type, token.expires_in);
    Ok(token.access_token)
}

pub(super) fn app_store_access_token(cfg: &AppStoreConfig) -> Result<String> {
    let access_token_env = app_store_access_token_env(cfg);
    if let Some(token) = env_value(&access_token_env) {
        return Ok(token);
    }
    let issuer_id_env = app_store_issuer_id_env(cfg);
    let key_id_env = app_store_key_id_env(cfg);
    let issuer_id = env_value(&issuer_id_env)
        .or(cfg.issuer_id.clone())
        .with_context(|| {
            format!("distribution.app_store.issuer_id or {issuer_id_env} is required")
        })?;
    let key_id = env_value(&key_id_env)
        .or(cfg.key_id.clone())
        .with_context(|| format!("distribution.app_store.key_id or {key_id_env} is required"))?;
    let (key_source, _temp_key) = app_store_api_key_source_with_temp(cfg)?;
    if looks_like_bearer_token(&key_source) {
        return Ok(key_source);
    }
    let key_text = if key_source.contains("-----BEGIN PRIVATE KEY-----") {
        key_source
    } else {
        fs::read_to_string(&key_source).with_context(|| {
            format!("failed to read App Store Connect API key from {key_source}")
        })?
    };
    let now = now_unix_seconds();
    let claims = AppStoreJwtClaims {
        iss: &issuer_id,
        aud: "appstoreconnect-v1",
        iat: now,
        exp: now + 20 * 60,
    };
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id);
    encode(
        &header,
        &claims,
        &EncodingKey::from_ec_pem(key_text.as_bytes())
            .context("failed to parse App Store Connect .p8 key as EC private key")?,
    )
    .context("failed to sign App Store Connect JWT")
}

pub(super) struct AppStoreApiKeyFile {
    pub(super) path: PathBuf,
    _temp_file: Option<TemporarySecretFile>,
}

pub(super) fn app_store_api_key_file(
    key_id: &str,
    cfg: &AppStoreConfig,
) -> Result<AppStoreApiKeyFile> {
    if let Some(path) = env_value(&app_store_api_key_path_env(cfg)) {
        return Ok(AppStoreApiKeyFile {
            path: PathBuf::from(path),
            _temp_file: None,
        });
    }

    let key_name = format!("AuthKey_{key_id}.p8");
    let temp_file = if let Some(file) =
        base64_env_secret_file(&app_store_api_key_base64_env(cfg), &key_name)?
    {
        file
    } else {
        let key_text = app_store_api_key_text(cfg)?;
        temporary_secret_file("app-store-key", &key_name, key_text.as_bytes())?
    };
    let path = temp_file.path.clone();
    Ok(AppStoreApiKeyFile {
        path,
        _temp_file: Some(temp_file),
    })
}

pub(super) fn app_store_api_key_text(cfg: &AppStoreConfig) -> Result<String> {
    let (source, _temp_key) = app_store_api_key_source_with_temp(cfg)?;
    if source.contains("-----BEGIN PRIVATE KEY-----") {
        Ok(source)
    } else {
        fs::read_to_string(&source)
            .with_context(|| format!("failed to read App Store Connect API key from {source}"))
    }
}

fn app_store_api_key_source_with_temp(
    cfg: &AppStoreConfig,
) -> Result<(String, Option<TemporarySecretFile>)> {
    let api_key_env = app_store_api_key_env(cfg);
    let api_key_base64_env = app_store_api_key_base64_env(cfg);
    let api_key_path_env = app_store_api_key_path_env(cfg);
    if let Some(value) = env_value(&api_key_env) {
        return Ok((value, None));
    }
    if let Some(file) = base64_env_secret_file(&api_key_base64_env, "AuthKey.p8")? {
        return Ok((file.path.display().to_string(), Some(file)));
    }
    if let Some(path) = env_value(&api_key_path_env) {
        return Ok((path, None));
    }
    bail!("{api_key_env}, {api_key_base64_env}, or {api_key_path_env} is required")
}

pub(super) fn app_store_app_id(
    cfg: &AppStoreConfig,
    client: &Client,
    token: &str,
) -> Result<String> {
    if let Some(app_id) = cfg
        .app_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(app_id.to_string());
    }
    let bundle_id = cfg.bundle_id.as_deref().context(
        "distribution.app_store.app_id or bundle_id is required for App Store Connect status",
    )?;
    let url = format!("{APP_STORE_API}/v1/apps?filter[bundleId]={bundle_id}&limit=1");
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to resolve App Store Connect app id from bundle id")?;
    let value = json_response(response, "App Store app lookup")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| {
            format!("App Store Connect did not return an app for bundle id {bundle_id}")
        })
}

pub(super) fn ensure_app_store_build_number_unused(
    client: &Client,
    token: &str,
    app_id: &str,
    build_number: &str,
) -> Result<()> {
    let url = format!(
        "{APP_STORE_API}/v1/builds?filter[app]={}&filter[version]={}&limit=1&fields[builds]=version,processingState,uploadedDate",
        encode_query_component(app_id),
        encode_query_component(build_number)
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to query App Store Connect existing build numbers")?;
    let value = json_response(response, "App Store Connect build-number preflight")?;
    if app_store_builds_contain_build_number(&value, build_number) {
        bail!(
            "App Store Connect build number {build_number} already exists for app {app_id}. Increment the target build number or [app].build, rebuild the App Store artifact, then publish again."
        );
    }
    Ok(())
}

pub(super) fn app_store_builds_contain_build_number(value: &Value, build_number: &str) -> bool {
    value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| {
            item.get("attributes")
                .and_then(|attrs| attrs.get("version"))
                .and_then(Value::as_str)
                .is_some_and(|candidate| candidate == build_number)
        })
}

pub(super) fn encode_query_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub(super) fn microsoft_store_access_token(
    cfg: &MicrosoftStoreConfig,
    client: &Client,
) -> Result<String> {
    let token_env = microsoft_store_token_env(cfg);
    if let Some(token) = env_value(&token_env) {
        return Ok(token);
    }
    let tenant_id_env = microsoft_store_tenant_id_env(cfg);
    let client_id_env = microsoft_store_client_id_env(cfg);
    let client_secret_env = microsoft_store_client_secret_env(cfg);
    let tenant_id = microsoft_store_tenant_id(cfg).context(
        format!(
            "distribution.microsoft_store.tenant_id, {tenant_id_env}, or PARTNER_CENTER_TENANT_ID is required"
        ),
    )?;
    let client_id = microsoft_store_client_id(cfg).context(
        format!(
            "distribution.microsoft_store.client_id, {client_id_env}, or PARTNER_CENTER_CLIENT_ID is required"
        ),
    )?;
    let client_secret = microsoft_store_client_secret(cfg).context(format!(
        "{client_secret_env} or PARTNER_CENTER_CLIENT_SECRET is required"
    ))?;
    let url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");
    let response = client
        .post(url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("scope", MICROSOFT_STORE_SCOPE),
        ])
        .send()
        .context("failed to request Microsoft Store access token")?;
    let token: OAuthTokenResponse = response
        .error_for_status()
        .context("Microsoft Store access token request failed")?
        .json()
        .context("failed to parse Microsoft Store access token response")?;
    Ok(token.access_token)
}

pub(super) fn run_msstore(args: &[String], operation: &str) -> Result<(String, String)> {
    let output = Command::new("msstore")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "failed to run msstore; install Microsoft Store Developer CLI before {operation}"
            )
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!("{operation} failed with {}: {}", output.status, detail);
    }
    Ok((stdout, stderr))
}

pub(super) fn load_google_service_account(source: &str) -> Result<GoogleServiceAccount> {
    let text = if source.trim_start().starts_with('{') {
        source.to_string()
    } else {
        fs::read_to_string(source)
            .with_context(|| format!("failed to read Google service account JSON from {source}"))?
    };
    serde_json::from_str(&text).context("failed to parse Google service account JSON")
}

pub(super) fn json_response(response: Response, operation: &str) -> Result<Value> {
    let status = response.status();
    let text = response
        .text()
        .with_context(|| format!("failed to read {operation} response"))?;
    if !status.is_success() {
        bail!("{operation} failed with {status}: {text}");
    }
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {operation} JSON response: {text}"))
    }
}

pub(super) fn microsoft_store_success(value: &Value, operation: &str) -> Result<()> {
    if value
        .get("isSuccess")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        Ok(())
    } else {
        bail!("{operation} returned an unsuccessful response: {value}")
    }
}

pub(super) fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("cargo-fission-release/0.1")
        .build()
        .context("failed to build release HTTP client")
}

pub(super) fn play_store_config(config: &PublishManifest) -> PlayStoreConfig {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default()
}

pub(super) fn app_store_config(config: &PublishManifest) -> AppStoreConfig {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.app_store.clone())
        .unwrap_or_default()
}

pub(super) fn microsoft_store_config(config: &PublishManifest) -> MicrosoftStoreConfig {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.microsoft_store.clone())
        .unwrap_or_default()
}

pub(super) fn primary_artifact_with_extensions(
    manifest: &ArtifactManifest,
    exts: &[&str],
) -> Result<PathBuf> {
    manifest
        .artifacts
        .iter()
        .map(|file| PathBuf::from(&file.path))
        .find(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| {
                    exts.iter()
                        .any(|candidate| ext.eq_ignore_ascii_case(candidate))
                })
        })
        .with_context(|| {
            format!(
                "artifact manifest does not contain one of: {}",
                exts.join(", ")
            )
        })
}

pub(super) fn primary_artifact_extension(manifest: &ArtifactManifest) -> Option<&str> {
    manifest
        .artifacts
        .iter()
        .map(|file| Path::new(&file.path))
        .find_map(|path| path.extension().and_then(|value| value.to_str()))
}

pub(super) fn has_artifact_with_extension(manifest: &ArtifactManifest, exts: &[&str]) -> bool {
    manifest.artifacts.iter().any(|file| {
        Path::new(&file.path)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| {
                exts.iter()
                    .any(|candidate| ext.eq_ignore_ascii_case(candidate))
            })
    })
}

pub(super) fn microsoft_store_package_type(
    cfg: &MicrosoftStoreConfig,
    manifest: &ArtifactManifest,
) -> String {
    cfg.package_type
        .as_deref()
        .or_else(|| primary_artifact_extension(manifest))
        .unwrap_or("exe")
        .to_ascii_lowercase()
}

pub(super) fn is_microsoft_store_msix_type(package_type: &str) -> bool {
    MICROSOFT_STORE_MSIX_TYPES
        .iter()
        .any(|candidate| package_type.eq_ignore_ascii_case(candidate))
}

pub(super) fn microsoft_store_msstore_project(
    options: &DistributeOptions,
    cfg: &MicrosoftStoreConfig,
) -> PathBuf {
    cfg.msstore_project
        .as_deref()
        .map(|path| {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                options.project_dir.join(path)
            }
        })
        .unwrap_or_else(|| options.project_dir.clone())
}

pub(super) fn msstore_publish_args(
    project_path: &Path,
    artifact: &Path,
    product_id: &str,
    flight_id: Option<&str>,
    rollout: Option<u8>,
    should_submit: bool,
) -> Vec<String> {
    let mut args = vec![
        "publish".to_string(),
        project_path.display().to_string(),
        "-i".to_string(),
        artifact.display().to_string(),
        "-id".to_string(),
        product_id.to_string(),
    ];
    if !should_submit {
        args.push("-nc".to_string());
    }
    if let Some(flight_id) = flight_id.filter(|value| !value.trim().is_empty()) {
        args.push("-f".to_string());
        args.push(flight_id.to_string());
    }
    if let Some(rollout) = rollout {
        args.push("-prp".to_string());
        args.push(rollout.to_string());
    }
    args
}

pub(super) fn microsoft_store_should_submit(
    options: &DistributeOptions,
    cfg: &MicrosoftStoreConfig,
) -> bool {
    cfg.submit.unwrap_or(false) || options.track.as_deref() == Some("public") && options.yes
}

pub(super) fn microsoft_store_flight_id(
    track: Option<&str>,
    cfg: &MicrosoftStoreConfig,
) -> Result<Option<String>> {
    match track.map(str::trim).filter(|value| !value.is_empty()) {
        Some("public") | None => Ok(None),
        Some("private") => cfg.flight_id.clone().map(Some).context(
            "distribution.microsoft_store.flight_id is required when --track private is used for MSIX publishing",
        ),
        Some(flight_id) => Ok(Some(flight_id.to_string())),
    }
}

pub(super) fn microsoft_store_rollout_percentage(cfg: &MicrosoftStoreConfig) -> Result<Option<u8>> {
    if let Some(rollout) = cfg.package_rollout_percentage {
        if rollout > 100 {
            bail!(
                "distribution.microsoft_store.package_rollout_percentage must be between 0 and 100"
            );
        }
    }
    Ok(cfg.package_rollout_percentage)
}

pub(super) fn microsoft_store_seller_id(cfg: &MicrosoftStoreConfig) -> Option<String> {
    env_value(&microsoft_store_seller_id_env(cfg))
        .or_else(|| env_value("PARTNER_CENTER_SELLER_ID"))
        .or_else(|| cfg.seller_id.clone())
}

pub(super) fn microsoft_store_tenant_id(cfg: &MicrosoftStoreConfig) -> Option<String> {
    env_value(&microsoft_store_tenant_id_env(cfg))
        .or_else(|| env_value("PARTNER_CENTER_TENANT_ID"))
        .or_else(|| cfg.tenant_id.clone())
}

pub(super) fn microsoft_store_client_id(cfg: &MicrosoftStoreConfig) -> Option<String> {
    env_value(&microsoft_store_client_id_env(cfg))
        .or_else(|| env_value("PARTNER_CENTER_CLIENT_ID"))
        .or_else(|| cfg.client_id.clone())
}

pub(super) fn microsoft_store_client_secret(cfg: &MicrosoftStoreConfig) -> Option<String> {
    env_value(&microsoft_store_client_secret_env(cfg))
        .or_else(|| env_value("PARTNER_CENTER_CLIENT_SECRET"))
}

pub(super) fn command_line(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| shell_word(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn shell_word(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '\\'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

pub(super) fn artifact_format_check(
    id: &str,
    manifest: &ArtifactManifest,
    accepted: &[&str],
    remediation: &str,
) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if accepted.iter().any(|format| manifest.format == *format) {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        format!("artifact format is one of {}", accepted.join(", ")),
        Some(format!("manifest format: {}", manifest.format)),
        vec![remediation],
    )
}

pub(super) fn secret_check_env_names(
    id: &str,
    env_names: &[String],
    remediation: &str,
) -> ReadinessCheck {
    let env_name = env_names.iter().find(|name| env::var_os(name).is_some());
    check(
        id,
        CheckSeverity::Error,
        if env_name.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "provider credentials are available",
        env_name.map(|name| format!("environment variable {name}")),
        vec![remediation],
    )
}

pub(super) fn play_credentials_env_available(cfg: &PlayStoreConfig) -> bool {
    play_credential_env_names(cfg)
        .iter()
        .any(|name| env::var_os(name).is_some())
}

pub(super) fn app_store_credentials_available_for_cfg(cfg: &AppStoreConfig) -> bool {
    if env::var_os(app_store_access_token_env(cfg)).is_some() {
        return true;
    }
    let key_material = app_store_api_key_env_names(cfg)
        .iter()
        .any(|name| env::var_os(name).is_some());
    let issuer = env::var_os(app_store_issuer_id_env(cfg)).is_some()
        || cfg
            .issuer_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    let key_id = env::var_os(app_store_key_id_env(cfg)).is_some()
        || cfg
            .key_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    key_material && issuer && key_id
}

pub(super) fn play_credential_env_names(cfg: &PlayStoreConfig) -> Vec<String> {
    vec![
        play_access_token_env(cfg),
        play_service_account_json_env(cfg),
        play_service_account_json_base64_env(cfg),
        play_google_application_credentials_env(cfg),
    ]
}

pub(super) fn app_store_api_key_env_names(cfg: &AppStoreConfig) -> Vec<String> {
    vec![
        app_store_api_key_env(cfg),
        app_store_api_key_base64_env(cfg),
        app_store_api_key_path_env(cfg),
    ]
}

fn play_access_token_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(cfg.access_token_env.as_deref(), "PLAY_STORE_ACCESS_TOKEN")
}

fn play_service_account_json_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(
        cfg.service_account_json_env.as_deref(),
        "PLAY_STORE_SERVICE_ACCOUNT_JSON",
    )
}

fn play_service_account_json_base64_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(
        cfg.service_account_json_base64_env.as_deref(),
        "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64",
    )
}

fn play_google_application_credentials_env(cfg: &PlayStoreConfig) -> String {
    configured_env_name(
        cfg.google_application_credentials_env.as_deref(),
        "GOOGLE_APPLICATION_CREDENTIALS",
    )
}

fn app_store_access_token_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(
        cfg.access_token_env.as_deref(),
        "APP_STORE_CONNECT_ACCESS_TOKEN",
    )
}

pub(super) fn app_store_issuer_id_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(cfg.issuer_id_env.as_deref(), "APP_STORE_CONNECT_ISSUER_ID")
}

pub(super) fn app_store_key_id_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(cfg.key_id_env.as_deref(), "APP_STORE_CONNECT_KEY_ID")
}

fn app_store_api_key_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(cfg.api_key_env.as_deref(), "APP_STORE_CONNECT_API_KEY")
}

fn app_store_api_key_base64_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(
        cfg.api_key_base64_env.as_deref(),
        "APP_STORE_CONNECT_API_KEY_BASE64",
    )
}

fn app_store_api_key_path_env(cfg: &AppStoreConfig) -> String {
    configured_env_name(
        cfg.api_key_path_env.as_deref(),
        "APP_STORE_CONNECT_API_KEY_PATH",
    )
}

pub(super) fn microsoft_store_token_env(cfg: &MicrosoftStoreConfig) -> String {
    configured_env_name(cfg.token_env.as_deref(), "MICROSOFT_STORE_TOKEN")
}

pub(super) fn microsoft_store_seller_id_env(cfg: &MicrosoftStoreConfig) -> String {
    configured_env_name(cfg.seller_id_env.as_deref(), "MICROSOFT_STORE_SELLER_ID")
}

pub(super) fn microsoft_store_tenant_id_env(cfg: &MicrosoftStoreConfig) -> String {
    configured_env_name(cfg.tenant_id_env.as_deref(), "AZURE_TENANT_ID")
}

pub(super) fn microsoft_store_client_id_env(cfg: &MicrosoftStoreConfig) -> String {
    configured_env_name(cfg.client_id_env.as_deref(), "AZURE_CLIENT_ID")
}

pub(super) fn microsoft_store_client_secret_env(cfg: &MicrosoftStoreConfig) -> String {
    configured_env_name(
        cfg.client_secret_env.as_deref(),
        "MICROSOFT_STORE_CLIENT_SECRET",
    )
}

fn configured_env_name(configured: Option<&str>, default: &str) -> String {
    configured
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
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

pub(super) fn store_receipt(
    options: &DistributeOptions,
    provider: &str,
    artifact_path: &Path,
    status: &str,
    deployment_id: Option<String>,
    canonical_url: Option<String>,
    manual_follow_up: Vec<String>,
) -> DistributionReceipt {
    DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id,
        canonical_url,
        preview_url: None,
        custom_domain: None,
        status: status.to_string(),
        stdout: None,
        stderr: None,
        manual_follow_up,
    }
}

pub(super) fn package_name_from_project(manifest: &ArtifactManifest) -> Option<&str> {
    (!manifest.project.app_id.trim().is_empty()).then_some(manifest.project.app_id.as_str())
}

pub(super) fn looks_like_bearer_token(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.starts_with('{') && !Path::new(trimmed).exists() && trimmed.matches('.').count() >= 2
}

pub(super) fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

pub(super) struct TemporarySecretFile {
    pub(super) path: PathBuf,
    temp_dir: PathBuf,
}

impl Drop for TemporarySecretFile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

pub(super) fn base64_env_secret_file(
    name: &str,
    file_name: &str,
) -> Result<Option<TemporarySecretFile>> {
    let Some(value) = env_value(name) else {
        return Ok(None);
    };
    let bytes = BASE64_STANDARD
        .decode(value.trim())
        .with_context(|| format!("failed to decode base64 environment variable {name}"))?;
    temporary_secret_file(name, file_name, &bytes).map(Some)
}

fn temporary_secret_file(
    kind: &str,
    file_name: &str,
    contents: &[u8],
) -> Result<TemporarySecretFile> {
    let temp_dir = env::temp_dir().join(format!(
        "fission-secret-{}-{}-{}",
        sanitize_temp_component(kind),
        std::process::id(),
        temp_nanos()
    ));
    fs::create_dir_all(&temp_dir).with_context(|| {
        format!(
            "failed to create temporary secret directory {}",
            temp_dir.display()
        )
    })?;
    set_private_dir_permissions(&temp_dir)?;
    let path = temp_dir.join(sanitize_temp_component(file_name));
    fs::write(&path, contents)
        .with_context(|| format!("failed to write temporary secret file {}", path.display()))?;
    set_private_file_permissions(&path)?;
    Ok(TemporarySecretFile { path, temp_dir })
}

fn temp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn sanitize_temp_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

pub(super) fn env_value_ref(name: &str) -> Option<&'static str> {
    if env::var_os(name).is_some() {
        Some("set")
    } else {
        None
    }
}
