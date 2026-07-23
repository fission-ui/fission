use super::*;

pub(super) fn play_release_config_remote_state(
    project_dir: &Path,
    locales_arg: Option<&str>,
) -> Result<ReleaseConfigRemoteState> {
    let root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play metadata lock")?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut listings = fetch_play_listings(
        &client,
        &token,
        package_name,
        &edit_id,
        Some(&locales.join(",")),
    )?;
    listings.sort_by(|a, b| a.locale.cmp(&b.locale));
    remote_state(
        DistributionProvider::PlayStore,
        package_name.to_string(),
        locales,
        json!({ "package_name": package_name, "listings": listings }),
    )
}

pub(super) fn app_store_release_config_remote_state(
    project_dir: &Path,
    locales_arg: Option<&str>,
) -> Result<ReleaseConfigRemoteState> {
    let root = read_release_provider_toml(project_dir)?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(&root, &client, &token, &app_id)?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let mut localizations = fetch_app_store_version_localizations(&client, &token, &version_id)?
        .into_iter()
        .filter(|item| locales.iter().any(|locale| locale == &item.locale))
        .collect::<Vec<_>>();
    localizations.sort_by(|a, b| a.locale.cmp(&b.locale));
    remote_state(
        DistributionProvider::AppStore,
        format!("{app_id}:{version_id}"),
        locales,
        json!({
            "app_id": app_id,
            "version_id": version_id,
            "localizations": localizations,
        }),
    )
}

pub(crate) fn remote_state(
    provider: DistributionProvider,
    subject: String,
    mut locales: Vec<String>,
    snapshot: Value,
) -> Result<ReleaseConfigRemoteState> {
    locales.sort();
    let snapshot = json!({
        "provider": provider.as_str(),
        "subject": subject.clone(),
        "locales": locales.clone(),
        "snapshot": snapshot,
    });
    Ok(ReleaseConfigRemoteState {
        provider: provider.as_str().to_string(),
        subject,
        locales,
        remote_revision: format!("sha256:{}", sha256_json(&snapshot)?),
        snapshot,
    })
}

pub(super) fn app_store_release_config_import(
    project_dir: &Path,
    locales: Option<&str>,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("release-config import mutates fission.toml/release metadata; pass --yes after reviewing the provider and locales");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(&root, &client, &token, &app_id)?;
    let mut remote = fetch_app_store_version_localizations(&client, &token, &version_id)?;
    remote.sort_by(|a, b| a.locale.cmp(&b.locale));
    if !dry_run {
        write_imported_app_store_localizations(project_dir, &root, locales, &remote)?;
        let state = remote_state(
            DistributionProvider::AppStore,
            format!("{app_id}:{version_id}"),
            remote.iter().map(|item| item.locale.clone()).collect(),
            json!({
                "app_id": app_id,
                "version_id": version_id,
                "localizations": remote.clone(),
            }),
        )?;
        write_release_config_lock(project_dir, DistributionProvider::AppStore, &state)?;
    }
    let summary = json!({
        "provider": "app-store",
        "app_id": app_id,
        "version_id": version_id,
        "imported_locales": remote.iter().map(|item| item.locale.as_str()).collect::<Vec<_>>(),
        "status": if dry_run { "dry-run" } else { "imported" }
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        let verb = if dry_run { "Would import" } else { "Imported" };
        println!("{verb} {} App Store localization(s)", remote.len());
    }
    Ok(())
}

pub(super) fn app_store_release_config_diff(project_dir: &Path, json_output: bool) -> Result<()> {
    let root = read_release_provider_toml(project_dir)?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(&root, &client, &token, &app_id)?;
    let locales = resolve_release_locales(&root, None)?;
    let local = resolve_app_store_localizations(project_dir, &root, &locales)?;
    let remote = fetch_app_store_version_localizations(&client, &token, &version_id)?;
    let diff = app_store_localization_diff(&local, &remote);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else if diff.as_array().is_some_and(Vec::is_empty) {
        println!(
            "App Store metadata is in sync for {} locale(s)",
            locales.len()
        );
    } else {
        println!("App Store metadata differences:");
        for item in diff.as_array().into_iter().flatten() {
            println!(
                "{} {}: local={:?} remote={:?}",
                item.get("locale")
                    .and_then(Value::as_str)
                    .unwrap_or("<locale>"),
                item.get("field")
                    .and_then(Value::as_str)
                    .unwrap_or("<field>"),
                item.get("local"),
                item.get("remote")
            );
        }
    }
    Ok(())
}

pub(super) fn app_store_release_config_push(
    project_dir: &Path,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<Value> {
    if !dry_run && !yes {
        bail!("release-config push mutates provider metadata; pass --yes after reviewing `release-config diff`");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = app_store_config(project_dir)?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let localizations = resolve_app_store_localizations(project_dir, &root, &locales)?;
    if dry_run {
        return Ok(json!({
            "provider": "app-store",
            "locales": locales,
            "localizations": localizations,
            "status": "dry-run"
        }));
    }
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let version_id = app_store_version_id(&root, &client, &token, &app_id)?;
    let remote = fetch_app_store_version_localizations(&client, &token, &version_id)?;
    let mut responses = Vec::new();
    for localization in &localizations {
        if let Some(existing) = remote
            .iter()
            .find(|item| item.locale == localization.locale)
        {
            let response = client
                .patch(format!(
                    "{APP_STORE_API}/v1/appStoreVersionLocalizations/{}",
                    existing
                        .id
                        .as_deref()
                        .context("remote localization missing id")?
                ))
                .bearer_auth(&token)
                .json(&app_store_localization_update_payload(
                    existing.id.as_deref().unwrap(),
                    localization,
                ))
                .send()
                .with_context(|| {
                    format!(
                        "failed to update App Store localization {}",
                        localization.locale
                    )
                })?;
            responses.push(json_response(response, "App Store localization update")?);
        } else {
            let response = client
                .post(format!("{APP_STORE_API}/v1/appStoreVersionLocalizations"))
                .bearer_auth(&token)
                .json(&app_store_localization_create_payload(
                    &version_id,
                    localization,
                ))
                .send()
                .with_context(|| {
                    format!(
                        "failed to create App Store localization {}",
                        localization.locale
                    )
                })?;
            responses.push(json_response(response, "App Store localization create")?);
        }
    }
    Ok(json!({
        "provider": "app-store",
        "app_id": app_id,
        "version_id": version_id,
        "updated_locales": localizations.iter().map(|item| item.locale.as_str()).collect::<Vec<_>>(),
        "responses": responses,
        "status": "pushed"
    }))
}

pub(super) fn play_release_config_import(
    project_dir: &Path,
    locales: Option<&str>,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("release-config import mutates fission.toml/release metadata; pass --yes after reviewing the provider and locales");
    }
    let mut root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play metadata import")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut remote = fetch_play_listings(&client, &token, package_name, &edit_id, locales)?;
    remote.sort_by(|a, b| a.locale.cmp(&b.locale));
    if !dry_run {
        write_imported_play_listings(project_dir, &mut root, &remote)?;
        let state = remote_state(
            DistributionProvider::PlayStore,
            package_name.to_string(),
            remote
                .iter()
                .map(|listing| listing.locale.clone())
                .collect(),
            json!({
                "package_name": package_name,
                "listings": remote.clone(),
            }),
        )?;
        write_release_config_lock(project_dir, DistributionProvider::PlayStore, &state)?;
    }
    let summary = json!({
        "provider": "play-store",
        "package_name": package_name,
        "imported_locales": remote.iter().map(|listing| listing.locale.as_str()).collect::<Vec<_>>(),
        "status": if dry_run { "dry-run" } else { "imported" }
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        let verb = if dry_run { "Would import" } else { "Imported" };
        println!(
            "{verb} {} Google Play listing locale(s) into fission.toml/release metadata",
            remote.len()
        );
    }
    Ok(())
}

pub(super) fn play_release_config_diff(project_dir: &Path, json_output: bool) -> Result<()> {
    let root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play metadata diff")?;
    let locales = resolve_release_locales(&root, None)?;
    let local = resolve_play_listings(project_dir, &root, &locales)?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let remote = fetch_play_listings(
        &client,
        &token,
        package_name,
        &edit_id,
        Some(&locales.join(",")),
    )?;
    let diff = play_listing_diff(&local, &remote);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else if diff.as_array().is_some_and(Vec::is_empty) {
        println!(
            "Google Play listing metadata is in sync for {} locale(s)",
            locales.len()
        );
    } else {
        println!("Google Play listing metadata differences:");
        for item in diff.as_array().into_iter().flatten() {
            println!(
                "{} {}: local={:?} remote={:?}",
                item.get("locale")
                    .and_then(Value::as_str)
                    .unwrap_or("<locale>"),
                item.get("field")
                    .and_then(Value::as_str)
                    .unwrap_or("<field>"),
                item.get("local"),
                item.get("remote")
            );
        }
    }
    Ok(())
}

pub(super) fn play_release_config_push(
    project_dir: &Path,
    locales_arg: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<Value> {
    if !dry_run && !yes {
        bail!("release-config push mutates provider metadata; pass --yes after reviewing `release-config diff`");
    }
    let root = read_release_provider_toml(project_dir)?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.play_store.clone())
        .unwrap_or_default();
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play metadata push")?;
    let locales = resolve_release_locales(&root, locales_arg)?;
    let listings = resolve_play_listings(project_dir, &root, &locales)?;
    if dry_run {
        return Ok(json!({
            "provider": "play-store",
            "package_name": package_name,
            "locales": locales,
            "listings": listings,
            "status": "dry-run"
        }));
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut responses = Vec::new();
    for listing in &listings {
        let url = format!(
            "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings/{}",
            listing.locale
        );
        let response = client
            .put(url)
            .bearer_auth(&token)
            .json(&json!({
                "title": listing.title,
                "shortDescription": listing.short_description,
                "fullDescription": listing.full_description,
                "video": listing.video,
            }))
            .send()
            .with_context(|| format!("failed to update Google Play listing {}", listing.locale))?;
        responses.push(json_response(response, "Google Play listing update")?);
    }
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;
    Ok(json!({
        "provider": "play-store",
        "package_name": package_name,
        "updated_locales": listings.iter().map(|listing| listing.locale.as_str()).collect::<Vec<_>>(),
        "responses": responses,
        "status": "pushed"
    }))
}

pub(super) fn fetch_app_store_version_localizations(
    client: &Client,
    token: &str,
    version_id: &str,
) -> Result<Vec<AppStoreLocalization>> {
    let response = client
        .get(format!(
            "{APP_STORE_API}/v1/appStoreVersions/{version_id}/appStoreVersionLocalizations?limit=200"
        ))
        .bearer_auth(token)
        .send()
        .context("failed to list App Store version localizations")?;
    let value = json_response(response, "App Store localizations list")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(app_store_localization_from_value)
        .collect()
}

pub(super) fn app_store_localization_from_value(value: &Value) -> Result<AppStoreLocalization> {
    let attrs = value.get("attributes").unwrap_or(&Value::Null);
    Ok(AppStoreLocalization {
        id: value.get("id").and_then(Value::as_str).map(str::to_string),
        locale: attrs
            .get("locale")
            .and_then(Value::as_str)
            .context("App Store localization missing locale")?
            .to_string(),
        description: attrs
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        keywords: attrs
            .get("keywords")
            .and_then(Value::as_str)
            .map(str::to_string),
        marketing_url: attrs
            .get("marketingUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
        promotional_text: attrs
            .get("promotionalText")
            .and_then(Value::as_str)
            .map(str::to_string),
        support_url: attrs
            .get("supportUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
        whats_new: attrs
            .get("whatsNew")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub(super) fn resolve_app_store_localizations(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales: &[String],
) -> Result<Vec<AppStoreLocalization>> {
    let metadata = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| read_release_metadata(project_dir, metadata))
        .transpose()?
        .unwrap_or_default();
    locales
        .iter()
        .map(|locale| resolve_app_store_localization(project_dir, root, &metadata, locale))
        .collect()
}

pub(super) fn resolve_app_store_localization(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    metadata: &ReleaseMetadataToml,
    locale: &str,
) -> Result<AppStoreLocalization> {
    let listing = root
        .release
        .as_ref()
        .and_then(|release| release.store_listing.get("app_store"))
        .and_then(|listings| listings.get(locale))
        .cloned()
        .unwrap_or_default();
    let meta = metadata.app_store.get(locale).cloned().unwrap_or_default();
    let description = meta.description.with_context(|| {
        format!("active release metadata [app_store.{locale}].description is required")
    })?;
    let whats_new = active_release(root)
        .and_then(|release| release.release_notes.as_deref())
        .map(|notes| project_dir.join(notes).join(format!("{locale}.md")))
        .filter(|path| path.exists())
        .map(fs::read_to_string)
        .transpose()?;
    Ok(AppStoreLocalization {
        id: None,
        locale: locale.to_string(),
        description,
        keywords: (!listing.keywords.is_empty()).then(|| listing.keywords.join(",")),
        marketing_url: listing.marketing_url,
        promotional_text: meta.promotional_text,
        support_url: listing.support_url,
        whats_new: whats_new
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}

pub(super) fn app_store_version_id(
    root: &ReleaseProviderToml,
    client: &Client,
    token: &str,
    app_id: &str,
) -> Result<String> {
    let version = active_release(root)
        .and_then(|release| release.version.as_deref())
        .or_else(|| {
            root.release
                .as_ref()?
                .active_release
                .as_deref()
                .and_then(|id| id.split('+').next())
        })
        .context("active [[releases]].version is required for App Store metadata sync")?;
    let cfg = root
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.app_store.as_ref())
        .cloned()
        .unwrap_or_default();
    let platform = app_store_platform_api_value(&cfg)?;
    let response = client
        .get(format!(
            "{APP_STORE_API}/v1/apps/{app_id}/appStoreVersions?filter[versionString]={version}&filter[platform]={platform}&limit=1"
        ))
        .bearer_auth(token)
        .send()
        .context("failed to list App Store versions")?;
    let value = json_response(response, "App Store versions list")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("App Store version {version} was not found for app {app_id}"))
}

pub(super) fn app_store_localization_update_payload(
    id: &str,
    localization: &AppStoreLocalization,
) -> Value {
    json!({
        "data": {
            "type": "appStoreVersionLocalizations",
            "id": id,
            "attributes": app_store_localization_attributes(localization)
        }
    })
}

pub(super) fn app_store_localization_create_payload(
    version_id: &str,
    localization: &AppStoreLocalization,
) -> Value {
    json!({
        "data": {
            "type": "appStoreVersionLocalizations",
            "attributes": app_store_localization_attributes(localization),
            "relationships": {
                "appStoreVersion": {
                    "data": {"type": "appStoreVersions", "id": version_id}
                }
            }
        }
    })
}

pub(super) fn app_store_localization_attributes(localization: &AppStoreLocalization) -> Value {
    let mut attrs = serde_json::Map::new();
    attrs.insert(
        "locale".to_string(),
        Value::String(localization.locale.clone()),
    );
    attrs.insert(
        "description".to_string(),
        Value::String(localization.description.clone()),
    );
    if let Some(value) = &localization.keywords {
        attrs.insert("keywords".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.marketing_url {
        attrs.insert("marketingUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.promotional_text {
        attrs.insert("promotionalText".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.support_url {
        attrs.insert("supportUrl".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &localization.whats_new {
        attrs.insert("whatsNew".to_string(), Value::String(value.clone()));
    }
    Value::Object(attrs)
}

pub(super) fn write_imported_app_store_localizations(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales_arg: Option<&str>,
    remote: &[AppStoreLocalization],
) -> Result<()> {
    let selected = locales_arg.map(parse_locale_list).transpose()?;
    let selected = selected.as_ref();
    let fission_path = project_dir.join("fission.toml");
    let metadata_path = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| project_dir.join(metadata))
        .context("active release metadata path is required for App Store metadata import")?;
    let mut metadata_doc: toml::Value = if metadata_path.exists() {
        toml::from_str(&fs::read_to_string(&metadata_path)?)?
    } else {
        toml::Value::Table(Default::default())
    };
    let mut fission_doc =
        parse_toml_edit_document(&fs::read_to_string(&fission_path)?, &fission_path)?;
    for item in remote {
        if selected.is_some_and(|selected| !selected.contains(&item.locale)) {
            continue;
        }
        set_toml_path(
            &mut metadata_doc,
            &format!("app_store.{}.description", item.locale),
            toml::Value::String(item.description.clone()),
        )?;
        if let Some(value) = &item.promotional_text {
            set_toml_path(
                &mut metadata_doc,
                &format!("app_store.{}.promotional_text", item.locale),
                toml::Value::String(value.clone()),
            )?;
        }
        if let Some(value) = &item.support_url {
            set_toml_edit_path(
                &mut fission_doc,
                &format!(
                    "release.store_listing.app_store.{}.support_url",
                    item.locale
                ),
                toml_edit::value(value.clone()),
            )?;
        }
        if let Some(value) = &item.marketing_url {
            set_toml_edit_path(
                &mut fission_doc,
                &format!(
                    "release.store_listing.app_store.{}.marketing_url",
                    item.locale
                ),
                toml_edit::value(value.clone()),
            )?;
        }
        if let Some(value) = &item.keywords {
            set_toml_edit_path(
                &mut fission_doc,
                &format!("release.store_listing.app_store.{}.keywords", item.locale),
                toml_edit_string_array(
                    value
                        .split(',')
                        .map(|item| item.trim().to_string())
                        .collect::<Vec<_>>(),
                ),
            )?;
        }
    }
    if let Some(parent) = metadata_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &metadata_path,
        toml::to_string_pretty(&metadata_doc)? + "\n",
    )?;
    write_toml_edit_document(&fission_path, &fission_doc)?;
    Ok(())
}

pub(super) fn app_store_localization_diff(
    local: &[AppStoreLocalization],
    remote: &[AppStoreLocalization],
) -> Value {
    let mut remote_by_locale = remote
        .iter()
        .map(|item| (item.locale.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut diffs = Vec::new();
    for local_item in local {
        match remote_by_locale.remove(local_item.locale.as_str()) {
            Some(remote_item) => {
                push_field_diff(&mut diffs, &local_item.locale, "description", &local_item.description, &remote_item.description);
                push_option_diff(&mut diffs, &local_item.locale, "keywords", &local_item.keywords, &remote_item.keywords);
                push_option_diff(&mut diffs, &local_item.locale, "marketing_url", &local_item.marketing_url, &remote_item.marketing_url);
                push_option_diff(&mut diffs, &local_item.locale, "promotional_text", &local_item.promotional_text, &remote_item.promotional_text);
                push_option_diff(&mut diffs, &local_item.locale, "support_url", &local_item.support_url, &remote_item.support_url);
                push_option_diff(&mut diffs, &local_item.locale, "whats_new", &local_item.whats_new, &remote_item.whats_new);
            }
            None => diffs.push(json!({"locale": local_item.locale, "field": "localization", "local": "present", "remote": "missing"})),
        }
    }
    Value::Array(diffs)
}

pub(super) fn push_option_diff(
    diffs: &mut Vec<Value>,
    locale: &str,
    field: &str,
    local: &Option<String>,
    remote: &Option<String>,
) {
    if local != remote {
        diffs.push(json!({"locale": locale, "field": field, "local": local, "remote": remote}));
    }
}

pub(super) fn fetch_play_listings(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    locales: Option<&str>,
) -> Result<Vec<PlayListing>> {
    let locale_list = locales.map(parse_locale_list).transpose()?;
    if let Some(locales) = locale_list {
        return locales
            .into_iter()
            .map(|locale| fetch_play_listing(client, token, package_name, edit_id, &locale))
            .collect();
    }
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to list Google Play listings")?;
    let value = json_response(response, "Google Play listings list")?;
    value
        .get("listings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(play_listing_from_value)
        .collect()
}

pub(super) fn fetch_play_listing(
    client: &Client,
    token: &str,
    package_name: &str,
    edit_id: &str,
    locale: &str,
) -> Result<PlayListing> {
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/listings/{locale}"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .with_context(|| format!("failed to get Google Play listing {locale}"))?;
    play_listing_from_value(&json_response(response, "Google Play listing get")?)
}

pub(super) fn play_listing_from_value(value: &Value) -> Result<PlayListing> {
    let locale = value
        .get("language")
        .or_else(|| value.get("locale"))
        .and_then(Value::as_str)
        .context("Google Play listing response did not contain language")?
        .to_string();
    Ok(PlayListing {
        locale,
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        short_description: value
            .get("shortDescription")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        full_description: value
            .get("fullDescription")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        video: value
            .get("video")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub(super) fn resolve_release_locales(
    root: &ReleaseProviderToml,
    locales_arg: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(locales) = locales_arg {
        return parse_locale_list(locales);
    }
    let active = active_release(root);
    if let Some(release) = active
        .as_ref()
        .filter(|release| !release.locales.is_empty())
    {
        return Ok(release.locales.clone());
    }
    if let Some(release) = root
        .release
        .as_ref()
        .filter(|release| !release.default_locales.is_empty())
    {
        return Ok(release.default_locales.clone());
    }
    let listing_locales = root
        .release
        .as_ref()
        .and_then(|release| release.store_listing.get("play_store"))
        .map(|listings| listings.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if listing_locales.is_empty() {
        bail!("no release locales configured; set release.default_locales, [[releases]].locales, or pass --locales")
    }
    Ok(listing_locales)
}

pub(super) fn resolve_play_listings(
    project_dir: &Path,
    root: &ReleaseProviderToml,
    locales: &[String],
) -> Result<Vec<PlayListing>> {
    let metadata = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| read_release_metadata(project_dir, metadata))
        .transpose()?
        .unwrap_or_default();
    locales
        .iter()
        .map(|locale| resolve_play_listing(root, &metadata, locale))
        .collect()
}

pub(super) fn resolve_play_listing(
    root: &ReleaseProviderToml,
    metadata: &ReleaseMetadataToml,
    locale: &str,
) -> Result<PlayListing> {
    let listing = root
        .release
        .as_ref()
        .and_then(|release| release.store_listing.get("play_store"))
        .and_then(|listings| listings.get(locale))
        .cloned()
        .unwrap_or_default();
    let meta = metadata.play_store.get(locale).cloned().unwrap_or_default();
    let title = listing.title.or(listing.name).with_context(|| {
        format!("release.store_listing.play_store.{locale}.title or .name is required")
    })?;
    let short_description = listing
        .short_description
        .or(listing.subtitle)
        .with_context(|| {
            format!("release.store_listing.play_store.{locale}.short_description is required")
        })?;
    let full_description = meta
        .full_description
        .or(meta.description)
        .with_context(|| {
            format!("active release metadata [play_store.{locale}].full_description is required")
        })?;
    Ok(PlayListing {
        locale: locale.to_string(),
        title,
        short_description,
        full_description,
        video: listing.video.or(listing.video_url),
    })
}

pub(super) fn write_imported_play_listings(
    project_dir: &Path,
    root: &mut ReleaseProviderToml,
    listings: &[PlayListing],
) -> Result<()> {
    let fission_path = project_dir.join("fission.toml");
    let data = fs::read_to_string(&fission_path)
        .with_context(|| format!("failed to read {}", fission_path.display()))?;
    let mut doc = parse_toml_edit_document(&data, &fission_path)?;
    for listing in listings {
        set_toml_edit_path(
            &mut doc,
            &format!("release.store_listing.play_store.{}.title", listing.locale),
            toml_edit::value(listing.title.clone()),
        )?;
        set_toml_edit_path(
            &mut doc,
            &format!(
                "release.store_listing.play_store.{}.short_description",
                listing.locale
            ),
            toml_edit::value(listing.short_description.clone()),
        )?;
        if let Some(video) = &listing.video {
            set_toml_edit_path(
                &mut doc,
                &format!("release.store_listing.play_store.{}.video", listing.locale),
                toml_edit::value(video.clone()),
            )?;
        }
    }
    write_toml_edit_document(&fission_path, &doc)?;

    let metadata_path = active_release(root)
        .and_then(|release| release.metadata.as_deref())
        .map(|metadata| project_dir.join(metadata));
    if let Some(metadata_path) = metadata_path {
        let mut metadata_doc: toml::Value = if metadata_path.exists() {
            toml::from_str(&fs::read_to_string(&metadata_path)?)?
        } else {
            toml::Value::Table(Default::default())
        };
        for listing in listings {
            set_toml_path(
                &mut metadata_doc,
                &format!("play_store.{}.full_description", listing.locale),
                toml::Value::String(listing.full_description.clone()),
            )?;
        }
        if let Some(parent) = metadata_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &metadata_path,
            toml::to_string_pretty(&metadata_doc)? + "\n",
        )
        .with_context(|| format!("failed to write {}", metadata_path.display()))?;
    }
    Ok(())
}

pub(super) fn play_listing_diff(local: &[PlayListing], remote: &[PlayListing]) -> Value {
    let mut remote_by_locale = remote
        .iter()
        .map(|listing| (listing.locale.as_str(), listing))
        .collect::<BTreeMap<_, _>>();
    let mut diffs = Vec::new();
    for local_listing in local {
        let remote_listing = remote_by_locale.remove(local_listing.locale.as_str());
        match remote_listing {
            Some(remote_listing) => {
                push_field_diff(&mut diffs, &local_listing.locale, "title", &local_listing.title, &remote_listing.title);
                push_field_diff(&mut diffs, &local_listing.locale, "short_description", &local_listing.short_description, &remote_listing.short_description);
                push_field_diff(&mut diffs, &local_listing.locale, "full_description", &local_listing.full_description, &remote_listing.full_description);
                if local_listing.video != remote_listing.video {
                    diffs.push(json!({"locale": local_listing.locale, "field": "video", "local": local_listing.video, "remote": remote_listing.video}));
                }
            }
            None => diffs.push(json!({"locale": local_listing.locale, "field": "listing", "local": "present", "remote": "missing"})),
        }
    }
    for remote_listing in remote_by_locale.values() {
        diffs.push(json!({"locale": remote_listing.locale, "field": "listing", "local": "missing", "remote": "present"}));
    }
    Value::Array(diffs)
}

pub(super) fn push_field_diff(
    diffs: &mut Vec<Value>,
    locale: &str,
    field: &str,
    local: &str,
    remote: &str,
) {
    if local != remote {
        diffs.push(json!({"locale": locale, "field": field, "local": local, "remote": remote}));
    }
}
